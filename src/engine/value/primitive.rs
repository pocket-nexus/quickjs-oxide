use std::cell::Cell;
use std::cell::RefCell;
use std::collections::TryReserveError;
use std::fmt;

use std::hash::{Hash, Hasher};
use std::iter::FusedIterator;
use std::rc::{Rc, Weak};

use crate::engine::api::error::{Error, ErrorKind, NativeErrorMessage};
use crate::engine::value::bigint::JsBigInt;

/// ECMAScript strings are sequences of UTF-16 code units, not UTF-8 scalar
/// values. Compact Latin-1/UTF-16 leaves and bounded ropes mirror QuickJS's
/// current representation while preserving lone surrogates.
#[derive(Clone)]
pub struct JsString(Rc<StringRepr>);

/// Non-owning handle to a String value ([`JsString::downgrade`]): `upgrade`
/// recovers the same string while another handle keeps it alive.
#[derive(Clone)]
pub struct WeakJsString(Weak<StringRepr>);

impl WeakJsString {
    #[must_use]
    pub fn upgrade(&self) -> Option<JsString> {
        self.0.upgrade().map(JsString)
    }
}

impl fmt::Debug for WeakJsString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("WeakJsString")
            .field(&self.0.as_ptr())
            .finish()
    }
}

/// Why a string-kernel operation failed. It becomes a QuickJS InternalError
/// (`ErrorKind::JsInternal`) in the front end's errors, and a
/// [`RuntimeError`](crate::engine::api::core::RuntimeError) in the embedding
/// API.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JsStringError {
    TooLong,
    OutOfMemory,
}

impl fmt::Display for JsStringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLong => formatter.write_str("string too long"),
            Self::OutOfMemory => formatter.write_str("out of memory"),
        }
    }
}

impl std::error::Error for JsStringError {}

impl From<JsStringError> for Error {
    fn from(error: JsStringError) -> Self {
        match error {
            JsStringError::TooLong | JsStringError::OutOfMemory => {
                Error::new(ErrorKind::JsInternal, error.to_string())
            }
        }
    }
}

#[cfg(test)]
thread_local! {
    static FAIL_NEXT_CONCAT_RESERVATION: Cell<bool> = const { Cell::new(false) };
    static FAIL_NEXT_REPEAT_RESERVATION: Cell<bool> = const { Cell::new(false) };
    static FAIL_NEXT_PAD_RESERVATION: Cell<bool> = const { Cell::new(false) };
    static FAIL_NEXT_TRIM_RESERVATION: Cell<bool> = const { Cell::new(false) };
    static FAIL_NEXT_REPLACEMENT_RESERVATION: Cell<bool> = const { Cell::new(false) };
}

#[cfg(test)]
fn arm_reservation_failure(flag: &'static std::thread::LocalKey<Cell<bool>>, label: &str) {
    flag.with(|armed| {
        assert!(
            !armed.replace(true),
            "{label} reservation failure was already armed"
        )
    });
}

#[cfg(test)]
pub fn fail_next_repeat_reservation_for_test() {
    arm_reservation_failure(&FAIL_NEXT_REPEAT_RESERVATION, "repeat");
}

#[cfg(test)]
pub fn fail_next_pad_reservation_for_test() {
    arm_reservation_failure(&FAIL_NEXT_PAD_RESERVATION, "pad");
}

#[cfg(test)]
pub fn fail_next_trim_reservation_for_test() {
    arm_reservation_failure(&FAIL_NEXT_TRIM_RESERVATION, "trim");
}

struct StringRepr {
    content_hash: Cell<Option<u32>>,
    data: StringData,
}

impl StringRepr {
    fn allocate(data: StringData) -> Rc<Self> {
        Rc::new(Self {
            content_hash: Cell::new(None),
            data,
        })
    }

    fn data(&self) -> &StringData {
        &self.data
    }
}

enum StringData {
    Latin1(Vec<u8>),
    Utf16(Vec<u16>),
    Rope(RopeRepr),
}

struct RopeRepr {
    len: usize,
    is_wide: bool,
    depth: u8,
    /// QuickJS rewrites a shared rope to `flat + empty` after ToString while
    /// retaining its tag and cached depth. The state transition releases the
    /// old children and preserves that exact observable performance shape.
    state: RefCell<RopeState>,
}

enum RopeState {
    Tree { left: JsString, right: JsString },
    Linearized { flat: JsString },
}

struct Utf16Units {
    stack: [Option<JsString>; 61],
    stack_len: usize,
    current_flat: Option<(JsString, usize)>,
    remaining: usize,
}

/// Borrowed code-unit iterator for [`JsString::utf16_units`]. Flat strings
/// iterate their storage slice in place; ropes keep the owning traversal.
enum BorrowedUtf16Units<'a> {
    Latin1(std::slice::Iter<'a, u8>),
    Utf16(std::slice::Iter<'a, u16>),
    // Boxed: the traversal stack dwarfs the flat variants, and only ropes
    // pay the allocation.
    Rope(Box<Utf16Units>),
}

impl Iterator for BorrowedUtf16Units<'_> {
    type Item = u16;

    #[inline]
    fn next(&mut self) -> Option<u16> {
        match self {
            Self::Latin1(units) => units.next().copied().map(u16::from),
            Self::Utf16(units) => units.next().copied(),
            Self::Rope(units) => units.next(),
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::Latin1(units) => units.size_hint(),
            Self::Utf16(units) => units.size_hint(),
            Self::Rope(units) => units.size_hint(),
        }
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<u16> {
        match self {
            Self::Latin1(units) => units.nth(n).copied().map(u16::from),
            Self::Utf16(units) => units.nth(n).copied(),
            Self::Rope(units) => units.nth(n),
        }
    }
}

impl ExactSizeIterator for BorrowedUtf16Units<'_> {}

struct QuickJsUtf8Bytes {
    units: std::iter::Peekable<Utf16Units>,
    cesu8: bool,
    pending: [u8; 4],
    pending_index: usize,
    pending_len: usize,
}

pub struct JsStringBuilder {
    units: Vec<u16>,
    limit: usize,
    failed: bool,
}

impl JsStringBuilder {
    pub fn with_limit(capacity: usize, limit: usize) -> Self {
        let limit = limit.min(JsString::MAX_LEN);
        Self {
            units: Vec::with_capacity(capacity.min(limit).min(4096)),
            limit,
            failed: false,
        }
    }

    fn push_latin1(&mut self, value: &[u8]) -> Result<(), JsStringError> {
        self.ensure_additional(value.len())?;
        self.units.extend(value.iter().copied().map(u16::from));
        Ok(())
    }

    pub fn push_code_point(&mut self, value: u32) -> Result<(), JsStringError> {
        debug_assert!(value <= 0x10_ffff);
        let additional = if value <= 0xffff { 1 } else { 2 };
        self.ensure_additional(additional)?;
        if value <= 0xffff {
            self.units.push(value as u16);
        } else {
            let adjusted = value - 0x1_0000;
            self.units.push(0xd800 | ((adjusted >> 10) as u16));
            self.units.push(0xdc00 | ((adjusted & 0x3ff) as u16));
        }
        Ok(())
    }

    fn ensure_additional(&mut self, additional: usize) -> Result<(), JsStringError> {
        if self.failed {
            return Err(JsStringError::TooLong);
        }
        if JsString::checked_length_with_limit(self.units.len(), additional, self.limit).is_err() {
            self.units = Vec::new();
            self.failed = true;
            return Err(JsStringError::TooLong);
        }
        Ok(())
    }

    pub fn finish(self) -> Result<JsString, JsStringError> {
        if self.failed {
            return Err(JsStringError::TooLong);
        }
        debug_assert!(self.units.len() <= self.limit);
        Ok(JsString::from_validated_utf16(self.units))
    }
}

impl Utf16Units {
    fn new(string: &JsString) -> Self {
        let mut iterator = Self {
            // Inline-const init compiles to a plain zeroing store; the closure
            // form lowered to one non-inlined call per slot on this hot path.
            stack: [const { None }; 61],
            stack_len: 0,
            current_flat: None,
            remaining: string.len(),
        };
        iterator.push(string.clone());
        iterator
    }

    fn push(&mut self, string: JsString) {
        assert!(
            self.stack_len < self.stack.len(),
            "String rope exceeded its bounded iterator depth"
        );
        self.stack[self.stack_len] = Some(string);
        self.stack_len += 1;
    }

    fn pop(&mut self) -> Option<JsString> {
        self.stack_len = self.stack_len.checked_sub(1)?;
        self.stack[self.stack_len].take()
    }
}

impl Iterator for Utf16Units {
    type Item = u16;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some((flat, index)) = &mut self.current_flat {
                let unit = match flat.0.data() {
                    StringData::Latin1(units) => units.get(*index).copied().map(u16::from),
                    StringData::Utf16(units) => units.get(*index).copied(),
                    StringData::Rope(_) => {
                        unreachable!("UTF-16 iterator current node must be flat")
                    }
                };
                if let Some(unit) = unit {
                    *index += 1;
                    self.remaining -= 1;
                    return Some(unit);
                }
                self.current_flat = None;
            }

            let node = self.pop()?;
            match node.0.data() {
                StringData::Latin1(_) | StringData::Utf16(_) => {
                    self.current_flat = Some((node, 0));
                }
                StringData::Rope(rope) => match &*rope.state.borrow() {
                    RopeState::Tree { left, right } => {
                        self.push(right.clone());
                        self.push(left.clone());
                    }
                    RopeState::Linearized { flat } => self.push(flat.clone()),
                },
            }
        }
    }

    fn nth(&mut self, mut n: usize) -> Option<Self::Item> {
        while n > 0 {
            if let Some((flat, index)) = &mut self.current_flat {
                let skip = n.min(flat.len() - *index);
                *index += skip;
                self.remaining -= skip;
                n -= skip;
                if n == 0 {
                    break;
                }
                self.current_flat = None;
            }
            let node = self.pop()?;
            if node.len() <= n {
                n -= node.len();
                self.remaining -= node.len();
                continue;
            }
            match node.0.data() {
                StringData::Latin1(_) | StringData::Utf16(_) => self.current_flat = Some((node, 0)),
                StringData::Rope(rope) => match &*rope.state.borrow() {
                    RopeState::Tree { left, right } => {
                        self.push(right.clone());
                        self.push(left.clone());
                    }
                    RopeState::Linearized { flat } => self.push(flat.clone()),
                },
            }
        }
        self.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl ExactSizeIterator for Utf16Units {
    fn len(&self) -> usize {
        self.remaining
    }
}

impl FusedIterator for Utf16Units {}

impl QuickJsUtf8Bytes {
    fn new(string: &JsString, cesu8: bool) -> Self {
        Self {
            units: Utf16Units::new(string).peekable(),
            cesu8,
            pending: [0; 4],
            pending_index: 0,
            pending_len: 0,
        }
    }
}

impl Iterator for QuickJsUtf8Bytes {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pending_index < self.pending_len {
            let byte = self.pending[self.pending_index];
            self.pending_index += 1;
            return Some(byte);
        }

        let unit = self.units.next()?;
        let code_point = if !self.cesu8
            && (0xd800..=0xdbff).contains(&unit)
            && self
                .units
                .peek()
                .is_some_and(|next| (0xdc00..=0xdfff).contains(next))
        {
            let low = self
                .units
                .next()
                .expect("peeked low surrogate disappeared from UTF-16 iterator");
            0x1_0000 + ((u32::from(unit) - 0xd800) << 10) + (u32::from(low) - 0xdc00)
        } else {
            u32::from(unit)
        };
        self.pending_len = encode_quickjs_utf8(&mut self.pending, code_point);
        self.pending_index = 1;
        Some(self.pending[0])
    }
}

impl FusedIterator for QuickJsUtf8Bytes {}

impl NativeErrorMessage {
    pub fn to_js_string(&self) -> Result<JsString, JsStringError> {
        JsString::try_from_bytes(self.visible_bytes())
    }

    pub fn to_utf8_lossy(&self) -> String {
        self.to_js_string()
            .expect("a 255-byte native Error message cannot exceed the String length limit")
            .to_utf8_lossy()
    }
}

impl JsString {
    pub(crate) fn flat_latin1(&self) -> Option<&[u8]> {
        if let StringData::Latin1(units) = self.0.data() {
            Some(units)
        } else {
            None
        }
    }

    pub(crate) fn flat_utf16(&self) -> Option<&[u16]> {
        if let StringData::Utf16(units) = self.0.data() {
            Some(units)
        } else {
            None
        }
    }

    /// QuickJS reserves 30 bits for a string's UTF-16 code-unit length.
    pub const MAX_LEN: usize = (1 << 30) - 1;
    const ROPE_SHORT_LEN: usize = 512;
    const ROPE_SHORT2_LEN: usize = 8192;
    const ROPE_MAX_DEPTH: u8 = 60;
    const ROPE_BUCKET_LENGTHS: [usize; 44] = [
        1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144, 233, 377, 610, 987, 1597, 2584, 4181, 6765, 10946,
        17711, 28657, 46368, 75025, 121393, 196418, 317811, 514229, 832040, 1346269, 2178309,
        3524578, 5702887, 9227465, 14930352, 24157817, 39088169, 63245986, 102334155, 165580141,
        267914296, 433494437, 701408733, 1134903170,
    ];

    #[must_use]
    pub fn downgrade(&self) -> WeakJsString {
        WeakJsString(Rc::downgrade(&self.0))
    }

    #[must_use]
    pub fn same_representation(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    #[must_use]
    pub fn content_hash(&self) -> u32 {
        if let Some(hash) = self.0.content_hash.get() {
            return hash;
        }
        let hash = self.quickjs_hash(0);
        self.0.content_hash.set(Some(hash));
        hash
    }

    pub fn checked_length_with_limit(
        current: usize,
        additional: usize,
        limit: usize,
    ) -> Result<usize, JsStringError> {
        current
            .checked_add(additional)
            .filter(|length| *length <= limit)
            .ok_or(JsStringError::TooLong)
    }

    pub fn checked_length(current: usize, additional: usize) -> Result<usize, JsStringError> {
        Self::checked_length_with_limit(current, additional, Self::MAX_LEN)
    }

    /// Construct a dynamically supplied UTF-8 string while enforcing
    /// QuickJS's 30-bit UTF-16 length limit.
    ///
    /// # Errors
    /// Returns [`JsStringError::TooLong`] when the decoded value exceeds
    /// [`Self::MAX_LEN`]. Recoverable allocator failure remains a separate
    /// parity gap.
    pub fn try_from_utf8(value: &str) -> Result<Self, JsStringError> {
        if value.is_ascii() {
            return Self::try_from_latin1(value.as_bytes());
        }
        Self::try_from_utf16(value.encode_utf16())
    }

    /// Decode the byte-oriented input accepted by QuickJS `JS_NewStringLen`.
    ///
    /// This is deliberately not Rust's strict or standard lossy UTF-8
    /// decoder. Three-byte surrogate encodings are preserved as UTF-16 code
    /// units, non-BMP scalars become surrogate pairs, and malformed sequences
    /// use QuickJS's release-pinned replacement-and-skip algorithm.
    /// Embedded NUL bytes are ordinary U+0000 code units because the input
    /// length is explicit.
    ///
    /// # Errors
    /// Returns [`JsStringError::TooLong`] when the decoded value exceeds
    /// [`Self::MAX_LEN`] UTF-16 code units. Recoverable allocation failure for
    /// String storage remains a separate parity gap.
    pub fn try_from_bytes(value: &[u8]) -> Result<Self, JsStringError> {
        Self::try_from_bytes_with_limit(value, Self::MAX_LEN)
    }

    /// Construct one of the engine's trusted static table/literal strings.
    /// Dynamic host input must use [`Self::try_from_utf8`].
    pub fn from_static(value: &'static str) -> Self {
        Self::try_from_utf8(value).expect("static ECMAScript String exceeded QuickJS's length cap")
    }

    /// Construct a dynamically supplied UTF-16 string while enforcing
    /// QuickJS's 30-bit code-unit length limit.
    ///
    /// The iterator is consumed only after its lower size bound is validated;
    /// a dishonest or unbounded upper hint is never used for an enormous eager
    /// allocation.
    ///
    /// # Errors
    /// Returns [`JsStringError::TooLong`] when the value exceeds
    /// [`Self::MAX_LEN`]. Recoverable allocator failure remains a separate
    /// parity gap.
    pub fn try_from_utf16(units: impl IntoIterator<Item = u16>) -> Result<Self, JsStringError> {
        Self::try_from_utf16_with_limit(units, Self::MAX_LEN)
    }

    pub fn try_from_utf16_with_limit(
        units: impl IntoIterator<Item = u16>,
        max_len: usize,
    ) -> Result<Self, JsStringError> {
        let max_len = max_len.min(Self::MAX_LEN);
        let mut iterator = units.into_iter();
        let (lower, upper) = iterator.size_hint();
        if lower > max_len {
            return Err(JsStringError::TooLong);
        }
        let initial_capacity = upper.unwrap_or(lower).min(max_len).min(4096);
        let mut collected = Vec::with_capacity(initial_capacity);
        for unit in &mut iterator {
            if collected.len() == max_len {
                return Err(JsStringError::TooLong);
            }
            collected.push(unit);
        }
        Ok(Self::from_validated_utf16(collected))
    }

    fn try_from_bytes_with_limit(value: &[u8], max_len: usize) -> Result<Self, JsStringError> {
        let max_len = max_len.min(Self::MAX_LEN);
        let ascii_len = value.iter().take_while(|byte| **byte < 0x80).count();
        if ascii_len > max_len {
            return Err(JsStringError::TooLong);
        }
        if ascii_len == value.len() {
            return Ok(Self::from_owned_latin1(value.to_vec()));
        }

        let mut output = JsStringBuilder::with_limit(value.len(), max_len);
        output.push_latin1(&value[..ascii_len])?;
        let mut index = ascii_len;
        while index < value.len() {
            if value[index] < 0x80 {
                output.push_code_point(u32::from(value[index]))?;
                index += 1;
                continue;
            }

            match decode_quickjs_utf8(&value[index..]) {
                Some((code_point, consumed)) if code_point <= 0x10_ffff => {
                    output.push_code_point(code_point)?;
                    index += consumed;
                }
                Some(_) | None => {
                    index = skip_quickjs_invalid_utf8(value, index);
                    output.push_code_point(0xfffd)?;
                }
            }
        }
        output.finish()
    }

    fn from_validated_utf16(units: Vec<u16>) -> Self {
        debug_assert!(units.len() <= Self::MAX_LEN);
        if units.iter().all(|&unit| unit <= 0xff) {
            let latin1 = units.iter().map(|&unit| unit as u8).collect();
            Self(StringRepr::allocate(StringData::Latin1(latin1)))
        } else {
            Self(StringRepr::allocate(StringData::Utf16(units)))
        }
    }

    /// Adopt a checked narrow result buffer without collecting its contents a
    /// second time. Callers must have enforced [`Self::MAX_LEN`].
    pub fn from_owned_latin1(units: Vec<u8>) -> Self {
        debug_assert!(units.len() <= Self::MAX_LEN);
        Self(StringRepr::allocate(StringData::Latin1(units)))
    }

    /// Materialize QuickJS's `JS_AtomToString` result for one tagged integer
    /// atom. The returned narrow String deliberately owns a fresh
    /// representation on every call; routing this spelling through an atom or
    /// constant pool would make QuickJS's representation identity observable.
    #[must_use]
    pub fn from_fresh_decimal_u32(value: u32) -> Self {
        let mut buffer = fmt::NumBuffer::new();
        let digits = value.format_into(&mut buffer).as_bytes().to_vec();
        Self(StringRepr::allocate(StringData::Latin1(digits)))
    }

    /// A copy of Latin-1 code units.
    ///
    /// # Errors
    /// Returns [`JsStringError::TooLong`] past [`Self::MAX_LEN`].
    pub fn try_from_latin1(units: &[u8]) -> Result<Self, JsStringError> {
        if units.len() > Self::MAX_LEN {
            return Err(JsStringError::TooLong);
        }
        Ok(Self::from_owned_latin1(units.to_vec()))
    }

    /// A copy of UTF-16 code units, stored narrow when every unit fits in a
    /// byte.
    ///
    /// # Errors
    /// Returns [`JsStringError::TooLong`] past [`Self::MAX_LEN`].
    pub fn try_from_utf16_slice(units: &[u16]) -> Result<Self, JsStringError> {
        if units.len() > Self::MAX_LEN {
            return Err(JsStringError::TooLong);
        }
        if units.iter().all(|&unit| unit <= 0xff) {
            return Ok(Self::from_owned_latin1(
                units.iter().map(|&unit| unit as u8).collect(),
            ));
        }
        Ok(Self::from_owned_utf16(units.to_vec()))
    }

    /// Adopt a checked wide result buffer without collecting its contents a
    /// second time. Callers must have enforced [`Self::MAX_LEN`].
    pub fn from_owned_utf16(units: Vec<u16>) -> Self {
        debug_assert!(units.len() <= Self::MAX_LEN);
        Self(StringRepr::allocate(StringData::Utf16(units)))
    }

    /// Build the compact one-code-unit form used by String exotic indices and
    /// character methods, equivalent to QuickJS's `js_new_string_char`.
    #[must_use]
    pub fn from_code_unit(unit: u16) -> Self {
        match u8::try_from(unit) {
            Ok(unit) => Self(StringRepr::allocate(StringData::Latin1(vec![unit]))),
            Err(_) => Self(StringRepr::allocate(StringData::Utf16(vec![unit]))),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        match self.0.data() {
            StringData::Latin1(units) => units.len(),
            StringData::Utf16(units) => units.len(),
            StringData::Rope(rope) => rope.len,
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    pub fn utf16_units(&self) -> impl ExactSizeIterator<Item = u16> + '_ {
        // Flat strings walk their storage slice directly; only a live rope
        // pays for the owning traversal iterator and its bounded stack.
        match self.0.data() {
            StringData::Latin1(units) => BorrowedUtf16Units::Latin1(units.iter()),
            StringData::Utf16(units) => BorrowedUtf16Units::Utf16(units.iter()),
            StringData::Rope(_) => BorrowedUtf16Units::Rope(Box::new(Utf16Units::new(self))),
        }
    }

    /// Return one UTF-16 code unit without decoding or normalizing surrogate
    /// pairs. This is the equivalent of QuickJS's `string_get` fast path.
    #[must_use]
    pub fn code_unit_at(&self, index: usize) -> Option<u16> {
        if index >= self.len() {
            return None;
        }
        match self.0.data() {
            StringData::Latin1(units) => units.get(index).copied().map(u16::from),
            StringData::Utf16(units) => units.get(index).copied(),
            StringData::Rope(rope) => match &*rope.state.borrow() {
                RopeState::Linearized { flat } => flat.code_unit_at(index),
                RopeState::Tree { left, right } => {
                    if index < left.len() {
                        left.code_unit_at(index)
                    } else {
                        right.code_unit_at(index - left.len())
                    }
                }
            },
        }
    }

    /// Return the code point beginning at one UTF-16 code-unit index. A lead
    /// surrogate is combined only with an immediately following trail
    /// surrogate; every other code unit is returned unchanged.
    #[must_use]
    pub fn code_point_at(&self, index: usize) -> Option<u32> {
        let first = self.code_unit_at(index)?;
        if !(0xd800..=0xdbff).contains(&first) {
            return Some(u32::from(first));
        }
        let Some(second) = index
            .checked_add(1)
            .and_then(|next| self.code_unit_at(next))
        else {
            return Some(u32::from(first));
        };
        if !(0xdc00..=0xdfff).contains(&second) {
            return Some(u32::from(first));
        }
        Some(0x1_0000 + ((u32::from(first) - 0xd800) << 10) + (u32::from(second) - 0xdc00))
    }

    /// Whether every surrogate participates in a valid UTF-16 pair.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        self.first_unpaired_surrogate().is_none()
    }

    /// Replace each unpaired surrogate with U+FFFD. Well-formed strings retain
    /// their existing compact allocation, matching QuickJS's no-copy path.
    #[must_use]
    pub fn to_well_formed(&self) -> Self {
        let Some(first_invalid) = self.first_unpaired_surrogate() else {
            return self.clone();
        };
        let mut units = self.utf16_units().collect::<Vec<_>>();
        let mut index = first_invalid;
        while index < units.len() {
            let unit = units[index];
            if (0xd800..=0xdbff).contains(&unit)
                && units
                    .get(index + 1)
                    .is_some_and(|next| (0xdc00..=0xdfff).contains(next))
            {
                index += 2;
            } else {
                if (0xd800..=0xdfff).contains(&unit) {
                    units[index] = 0xfffd;
                }
                index += 1;
            }
        }
        Self::from_validated_utf16(units)
    }

    fn first_unpaired_surrogate(&self) -> Option<usize> {
        if !self.is_wide() {
            return None;
        }
        let mut units = self.utf16_units().enumerate().peekable();
        while let Some((index, unit)) = units.next() {
            if (0xd800..=0xdbff).contains(&unit) {
                if units
                    .peek()
                    .is_some_and(|(_, next)| (0xdc00..=0xdfff).contains(next))
                {
                    units.next();
                    continue;
                }
                return Some(index);
            }
            if (0xd800..=0xdfff).contains(&unit) {
                return Some(index);
            }
        }
        None
    }

    pub fn is_wide(&self) -> bool {
        match self.0.data() {
            StringData::Latin1(_) => false,
            StringData::Utf16(_) => true,
            StringData::Rope(rope) => rope.is_wide,
        }
    }

    fn depth(&self) -> u8 {
        match self.0.data() {
            StringData::Latin1(_) | StringData::Utf16(_) => 0,
            StringData::Rope(rope) => rope.depth,
        }
    }

    pub fn is_flat(&self) -> bool {
        matches!(self.0.data(), StringData::Latin1(_) | StringData::Utf16(_))
    }

    /// Visit the current flat leaves in logical order without linearizing a
    /// rope. QuickJS's diagnostic String printer decodes each leaf
    /// independently, so surrogate pairs do not cross a live rope boundary.
    /// A previously linearized rope exposes its replacement flat value as one
    /// leaf, matching QuickJS's in-place `flat + empty` transition.
    pub fn for_each_flat_leaf(&self, mut visitor: impl FnMut(&Self)) {
        let mut stack: [Option<Self>; 61] = [const { None }; 61];
        stack[0] = Some(self.clone());
        let mut stack_len = 1;
        while stack_len != 0 {
            stack_len -= 1;
            let node = stack[stack_len]
                .take()
                .expect("String flat-leaf traversal stack contained a hole");
            match node.0.data() {
                StringData::Latin1(_) | StringData::Utf16(_) => visitor(&node),
                StringData::Rope(rope) => match &*rope.state.borrow() {
                    RopeState::Tree { left, right } => {
                        assert!(
                            stack_len + 2 <= stack.len(),
                            "String rope exceeded its bounded flat-leaf traversal depth"
                        );
                        stack[stack_len] = Some(right.clone());
                        stack[stack_len + 1] = Some(left.clone());
                        stack_len += 2;
                    }
                    RopeState::Linearized { flat } => {
                        assert!(
                            stack_len < stack.len(),
                            "String rope exceeded its bounded flat-leaf traversal depth"
                        );
                        stack[stack_len] = Some(flat.clone());
                        stack_len += 1;
                    }
                },
            }
        }
    }

    fn quickjs_hash(&self, seed: u32) -> u32 {
        let step = |hash: u32, unit: u16| hash.wrapping_mul(263).wrapping_add(u32::from(unit));
        match self.0.data() {
            StringData::Latin1(units) => units.iter().copied().map(u16::from).fold(seed, step),
            StringData::Utf16(units) => units.iter().copied().fold(seed, step),
            StringData::Rope(_) => self.utf16_units().fold(seed, step),
        }
    }

    fn rope_children(rope: &RopeRepr) -> (Self, Self) {
        match &*rope.state.borrow() {
            RopeState::Tree { left, right } => (left.clone(), right.clone()),
            RopeState::Linearized { flat } => (flat.clone(), Self::from_static("")),
        }
    }

    fn flat_concat(left: &Self, right: &Self) -> Self {
        debug_assert!(left.is_flat() && right.is_flat());
        let len = left
            .len()
            .checked_add(right.len())
            .expect("validated flat String concatenation length overflowed");
        debug_assert!(len <= Self::MAX_LEN);
        match (left.0.data(), right.0.data()) {
            (StringData::Latin1(left), StringData::Latin1(right)) => {
                let mut units = Vec::with_capacity(len);
                units.extend_from_slice(left);
                units.extend_from_slice(right);
                Self(StringRepr::allocate(StringData::Latin1(units)))
            }
            (
                StringData::Latin1(_) | StringData::Utf16(_),
                StringData::Latin1(_) | StringData::Utf16(_),
            ) => {
                let units = left
                    .utf16_units()
                    .chain(right.utf16_units())
                    .collect::<Vec<_>>();
                debug_assert_eq!(units.len(), len);
                Self(StringRepr::allocate(StringData::Utf16(units)))
            }
            (StringData::Rope(_), _) | (_, StringData::Rope(_)) => {
                unreachable!("flat String concatenation received a rope")
            }
        }
    }

    fn new_rope_unbalanced(left: Self, right: Self) -> Result<Self, JsStringError> {
        let len = left
            .len()
            .checked_add(right.len())
            .filter(|len| *len <= Self::MAX_LEN)
            .ok_or(JsStringError::TooLong)?;
        let depth = left
            .depth()
            .max(right.depth())
            .checked_add(1)
            .expect("String rope depth invariant overflowed");
        Ok(Self(StringRepr::allocate(StringData::Rope(RopeRepr {
            len,
            is_wide: left.is_wide() || right.is_wide(),
            depth,
            state: RefCell::new(RopeState::Tree { left, right }),
        }))))
    }

    fn new_rope(left: Self, right: Self) -> Result<Self, JsStringError> {
        let rope = Self::new_rope_unbalanced(left, right)?;
        if rope.depth() > Self::ROPE_MAX_DEPTH {
            rope.rebalance()
        } else {
            Ok(rope)
        }
    }

    fn rebalance(&self) -> Result<Self, JsStringError> {
        let mut buckets: [Option<Self>; Self::ROPE_BUCKET_LENGTHS.len()] =
            [const { None }; Self::ROPE_BUCKET_LENGTHS.len()];
        // Rebalancing is entered for the one temporary depth-61 rope. A
        // depth-first traversal therefore needs at most 62 pending nodes.
        let mut pending: [Option<Self>; 62] = [const { None }; 62];
        pending[0] = Some(self.clone());
        let mut pending_len = 1;
        while pending_len != 0 {
            pending_len -= 1;
            let node = pending[pending_len]
                .take()
                .expect("String rope rebalance stack contained a hole");
            match node.0.data() {
                StringData::Rope(rope) => {
                    match &*rope.state.borrow() {
                        RopeState::Tree { left, right } => {
                            assert!(
                                pending_len + 2 <= pending.len(),
                                "String rope exceeded its bounded rebalance depth"
                            );
                            pending[pending_len] = Some(right.clone());
                            pending[pending_len + 1] = Some(left.clone());
                            pending_len += 2;
                        }
                        RopeState::Linearized { flat } => {
                            assert!(
                                pending_len < pending.len(),
                                "String rope exceeded its bounded rebalance depth"
                            );
                            pending[pending_len] = Some(flat.clone());
                            pending_len += 1;
                        }
                    }
                    continue;
                }
                StringData::Latin1(_) | StringData::Utf16(_) if node.is_empty() => continue,
                StringData::Latin1(_) | StringData::Utf16(_) => {}
            }

            let len = node.len();
            let mut index = 0;
            let mut prefix = None;
            while len >= Self::ROPE_BUCKET_LENGTHS[index + 1] {
                if let Some(bucket) = buckets[index].take() {
                    prefix = Some(match prefix {
                        None => bucket,
                        Some(prefix) => Self::new_rope_unbalanced(bucket, prefix)?,
                    });
                }
                index += 1;
            }
            let mut value = match prefix {
                Some(prefix) => Self::new_rope_unbalanced(prefix, node)?,
                None => node,
            };
            while let Some(bucket) = buckets[index].take() {
                value = Self::new_rope_unbalanced(bucket, value)?;
                index += 1;
            }
            buckets[index] = Some(value);
        }

        let mut result = None;
        for bucket in buckets.into_iter().flatten() {
            result = Some(match result {
                None => bucket,
                Some(result) => Self::new_rope_unbalanced(bucket, result)?,
            });
        }
        Ok(result.unwrap_or_else(|| Self::from_static("")))
    }

    /// Return a compact flat string, caching the result on a rope. This is the
    /// safe-Rust equivalent of QuickJS `js_linearize_string_rope`.
    #[must_use]
    pub fn linearize(&self) -> Self {
        let StringData::Rope(rope) = self.0.data() else {
            return self.clone();
        };
        if let RopeState::Linearized { flat } = &*rope.state.borrow() {
            return flat.clone();
        }
        let flattened = if rope.is_wide {
            Self(StringRepr::allocate(StringData::Utf16(
                self.utf16_units().collect::<Vec<_>>(),
            )))
        } else {
            let units = self
                .utf16_units()
                .map(|unit| {
                    u8::try_from(unit).expect("a narrow String rope contained a wide code unit")
                })
                .collect::<Vec<_>>();
            Self(StringRepr::allocate(StringData::Latin1(units)))
        };
        *rope.state.borrow_mut() = RopeState::Linearized {
            flat: flattened.clone(),
        };
        flattened
    }

    /// Copy one validated UTF-16 code-unit range with QuickJS
    /// `js_sub_string` representation rules. Ropes are linearized once, a
    /// full-range request reuses that flat handle, Latin-1 ranges copy their
    /// bytes directly, and UTF-16 ranges scan only the selected units before
    /// choosing a single Latin-1 or UTF-16 allocation.
    #[must_use]
    pub fn sub_string(&self, start: usize, end: usize) -> Self {
        let source = self.linearize();
        assert!(start <= end, "String subrange start exceeded end");
        assert!(
            end <= source.len(),
            "String subrange exceeded source length"
        );
        if start == 0 && end == source.len() {
            return source;
        }
        match source.0.data() {
            StringData::Latin1(units) => Self(StringRepr::allocate(StringData::Latin1(
                units[start..end].to_vec(),
            ))),
            StringData::Utf16(units) => {
                let selected = &units[start..end];
                if selected.iter().all(|unit| *unit <= u16::from(u8::MAX)) {
                    let narrow = selected.iter().map(|unit| *unit as u8).collect::<Vec<_>>();
                    Self(StringRepr::allocate(StringData::Latin1(narrow)))
                } else {
                    Self(StringRepr::allocate(StringData::Utf16(selected.to_vec())))
                }
            }
            StringData::Rope(_) => unreachable!("linearized String remained a rope"),
        }
    }

    /// Repeat one linearized String into the single flat allocation produced by
    /// QuickJS `js_string_repeat`. The checked product uses the caller-supplied
    /// limit so native white-box tests can exercise the length failure without
    /// attempting a near-gigabyte allocation.
    pub fn repeat_with_limit(&self, count: usize, max_len: usize) -> Result<Self, JsStringError> {
        let source = self.linearize();
        // QuickJS returns the already-converted receiver before checking the
        // product. Count conversion and validation happen outside this kernel.
        if source.is_empty() || count == 1 {
            return Ok(source);
        }
        let output_len = source
            .len()
            .checked_mul(count)
            .filter(|length| *length <= max_len.min(Self::MAX_LEN))
            .ok_or(JsStringError::TooLong)?;
        if output_len == 0 {
            // `string_buffer_end` releases the temporary zero-length buffer
            // and returns QuickJS's canonical narrow empty String.
            return Ok(Self::from_static(""));
        }

        fn grow_repetition<T: Copy>(
            source: &[T],
            output_len: usize,
        ) -> Result<Vec<T>, JsStringError> {
            // `string_buffer_init2` performs one exact fallible allocation.
            // Reserve the complete buffer up front so later doubling copies
            // cannot allocate and allocator failure remains catchable.
            #[cfg(test)]
            if FAIL_NEXT_REPEAT_RESERVATION.with(|armed| armed.replace(false)) {
                return Err(JsStringError::OutOfMemory);
            }
            let mut output = Vec::new();
            output
                .try_reserve_exact(output_len)
                .map_err(|_| JsStringError::OutOfMemory)?;
            if source.len() == 1 {
                output.resize(output_len, source[0]);
                return Ok(output);
            }
            output.extend_from_slice(source);
            while output.len() < output_len {
                let copied = output.len().min(output_len - output.len());
                output.extend_from_within(..copied);
            }
            Ok(output)
        }

        Ok(match source.0.data() {
            StringData::Latin1(units) => Self(StringRepr::allocate(StringData::Latin1(
                grow_repetition(units, output_len)?,
            ))),
            StringData::Utf16(units) => Self(StringRepr::allocate(StringData::Utf16(
                grow_repetition(units, output_len)?,
            ))),
            StringData::Rope(_) => unreachable!("linearized String remained a rope"),
        })
    }

    /// Build the one flat result of pinned QuickJS `js_string_pad` while
    /// retaining its narrow-first `StringBuffer` behavior. A wide code unit
    /// triggers one fallible widening reservation at the point it is copied;
    /// unused wide units in a truncated filler therefore do not widen the
    /// result.
    pub fn pad_with_limit(
        &self,
        target_len: usize,
        filler: Option<&Self>,
        pad_at_end: bool,
        max_len: usize,
    ) -> Result<Self, JsStringError> {
        let source = self.linearize();
        if source.len() >= target_len {
            return Ok(source);
        }
        let filler = filler.map(Self::linearize);
        if filler.as_ref().is_some_and(Self::is_empty) {
            return Ok(source);
        }
        if target_len > max_len.min(Self::MAX_LEN) {
            return Err(JsStringError::TooLong);
        }

        fn reserve_pad_buffer<T>(capacity: usize) -> Result<Vec<T>, JsStringError> {
            #[cfg(test)]
            if FAIL_NEXT_PAD_RESERVATION.with(|armed| armed.replace(false)) {
                return Err(JsStringError::OutOfMemory);
            }
            let mut output = Vec::new();
            output
                .try_reserve_exact(capacity)
                .map_err(|_| JsStringError::OutOfMemory)?;
            Ok(output)
        }

        enum PadBuffer {
            Latin1(Vec<u8>),
            Utf16(Vec<u16>),
        }

        fn push_pad_unit(
            buffer: &mut PadBuffer,
            unit: u16,
            target_len: usize,
        ) -> Result<(), JsStringError> {
            match buffer {
                PadBuffer::Latin1(units) if unit <= u16::from(u8::MAX) => {
                    units.push(unit as u8);
                }
                PadBuffer::Latin1(units) => {
                    // QuickJS first allocates a complete narrow buffer, then
                    // fallibly widens it when the first copied unit needs 16
                    // bits. Preserve both recoverable allocation points.
                    let mut wide = reserve_pad_buffer(target_len)?;
                    wide.extend(units.iter().map(|unit| u16::from(*unit)));
                    wide.push(unit);
                    *buffer = PadBuffer::Utf16(wide);
                }
                PadBuffer::Utf16(units) => units.push(unit),
            }
            Ok(())
        }

        fn append_source(
            buffer: &mut PadBuffer,
            source: &JsString,
            target_len: usize,
        ) -> Result<(), JsStringError> {
            for unit in source.utf16_units() {
                push_pad_unit(buffer, unit, target_len)?;
            }
            Ok(())
        }

        fn append_padding(
            buffer: &mut PadBuffer,
            filler: Option<&JsString>,
            padding_len: usize,
            target_len: usize,
        ) -> Result<(), JsStringError> {
            let filler_len = filler.map_or(1, JsString::len);
            for index in 0..padding_len {
                let unit = match filler {
                    Some(filler) => filler
                        .code_unit_at(index % filler_len)
                        .expect("non-empty flat pad filler lost a code unit"),
                    None => u16::from(b' '),
                };
                push_pad_unit(buffer, unit, target_len)?;
            }
            Ok(())
        }

        // string_buffer_init(ctx, b, n) always starts narrow, even if either
        // input is wide. Subsequent writes widen only for copied code units.
        let mut output = PadBuffer::Latin1(reserve_pad_buffer(target_len)?);
        let padding_len = target_len - source.len();
        if pad_at_end {
            append_source(&mut output, &source, target_len)?;
            append_padding(&mut output, filler.as_ref(), padding_len, target_len)?;
        } else {
            append_padding(&mut output, filler.as_ref(), padding_len, target_len)?;
            append_source(&mut output, &source, target_len)?;
        }

        Ok(match output {
            PadBuffer::Latin1(units) => {
                debug_assert_eq!(units.len(), target_len);
                Self(StringRepr::allocate(StringData::Latin1(units)))
            }
            PadBuffer::Utf16(units) => {
                debug_assert_eq!(units.len(), target_len);
                Self(StringRepr::allocate(StringData::Utf16(units)))
            }
        })
    }

    /// Trim the selected ends with pinned QuickJS `js_string_trim` and
    /// `js_sub_string` representation rules. Full-range and empty results do
    /// not enter or consume the partial-substring reservation path; a partial
    /// wide range narrows when every retained UTF-16 code unit fits Latin-1.
    pub fn trim_whitespace(&self, trim_start: bool, trim_end: bool) -> Result<Self, JsStringError> {
        let source = self.linearize();
        let mut start = 0;
        let mut end = source.len();
        if trim_start {
            while start < end
                && source
                    .code_unit_at(start)
                    .is_some_and(is_ecmascript_whitespace)
            {
                start += 1;
            }
        }
        if trim_end {
            while end > start
                && source
                    .code_unit_at(end - 1)
                    .is_some_and(is_ecmascript_whitespace)
            {
                end -= 1;
            }
        }
        if start == 0 && end == source.len() {
            return Ok(source);
        }
        if start == end {
            // `js_new_string8_len(..., 0)` returns the canonical narrow empty
            // atom without performing the substring allocation.
            return Ok(Self::from_static(""));
        }

        fn reserve_trim_buffer<T>(capacity: usize) -> Result<Vec<T>, JsStringError> {
            #[cfg(test)]
            if FAIL_NEXT_TRIM_RESERVATION.with(|armed| armed.replace(false)) {
                return Err(JsStringError::OutOfMemory);
            }
            let mut selected = Vec::new();
            selected
                .try_reserve_exact(capacity)
                .map_err(|_| JsStringError::OutOfMemory)?;
            Ok(selected)
        }

        let selected_len = end - start;
        match source.0.data() {
            StringData::Latin1(units) => {
                let mut selected = reserve_trim_buffer(selected_len)?;
                selected.extend_from_slice(&units[start..end]);
                Ok(Self(StringRepr::allocate(StringData::Latin1(selected))))
            }
            StringData::Utf16(units) => {
                let range = &units[start..end];
                if range.iter().all(|unit| *unit <= u16::from(u8::MAX)) {
                    let mut selected = reserve_trim_buffer(selected_len)?;
                    selected.extend(range.iter().map(|unit| *unit as u8));
                    Ok(Self(StringRepr::allocate(StringData::Latin1(selected))))
                } else {
                    let mut selected = reserve_trim_buffer(selected_len)?;
                    selected.extend_from_slice(range);
                    Ok(Self(StringRepr::allocate(StringData::Utf16(selected))))
                }
            }
            StringData::Rope(_) => unreachable!("linearized String remained a rope"),
        }
    }

    /// Concatenate with QuickJS's short-flat/rope thresholds, bounded depth,
    /// Fibonacci rebalance and 30-bit length cap.
    ///
    /// # Errors
    /// Returns [`JsStringError::TooLong`] when the result would exceed
    /// [`Self::MAX_LEN`] UTF-16 code units.
    pub fn try_concat(&self, other: &Self) -> Result<Self, JsStringError> {
        Self::checked_length(self.len(), other.len())?;

        if other.is_flat() {
            if other.is_empty() {
                return Ok(self.clone());
            }
            if self.is_empty() && self.is_flat() {
                return Ok(other.clone());
            }
            if other.len() <= Self::ROPE_SHORT_LEN {
                if self.is_flat() {
                    if self.len() <= Self::ROPE_SHORT2_LEN {
                        return Ok(Self::flat_concat(self, other));
                    }
                    return Self::new_rope(self.clone(), other.clone());
                }
                if let StringData::Rope(rope) = self.0.data() {
                    let (left, right) = Self::rope_children(rope);
                    if right.is_flat() && right.len() <= Self::ROPE_SHORT_LEN {
                        let tail = Self::flat_concat(&right, other);
                        return Self::new_rope(left, tail);
                    }
                }
            }
        } else if self.is_flat() {
            if self.is_empty() {
                return Ok(other.clone());
            }
            if let StringData::Rope(rope) = other.0.data() {
                let (left, right) = Self::rope_children(rope);
                if left.is_flat() && left.len() <= Self::ROPE_SHORT_LEN {
                    let head = Self::flat_concat(self, &left);
                    return Self::new_rope(head, right);
                }
            }
        }
        Self::new_rope(self.clone(), other.clone())
    }

    /// Append to an exclusively owned compatible flat buffer. Shared strings
    /// and ropes retain the ordinary concatenation representation rules. The
    /// same short-flat thresholds apply even to an exclusively owned buffer.
    ///
    /// # Errors
    /// Rejects lengths above `MAX_LEN` and reports reservation failure.
    pub fn concat_owned(mut self, other: &Self) -> Result<Self, JsStringError> {
        Self::checked_length(self.len(), other.len())?;
        if other.is_empty() && other.is_flat() {
            return Ok(self);
        }
        if self.is_empty() && self.is_flat() {
            return Ok(other.clone());
        }
        if self.try_concat_in_place(other)? {
            return Ok(self);
        }
        self.try_concat(other)
    }

    /// Append only if this flat buffer has one strong owner and no weak owners.
    /// Reservation precedes every mutation: a failed allocation leaves the
    /// original local binding intact. A declined append likewise changes nothing.
    pub(crate) fn try_concat_in_place(&mut self, other: &Self) -> Result<bool, JsStringError> {
        fn reserve<T>(buffer: &mut Vec<T>, additional: usize) -> Result<(), JsStringError> {
            #[cfg(test)]
            if FAIL_NEXT_CONCAT_RESERVATION.with(|armed| armed.replace(false)) {
                return Err(JsStringError::OutOfMemory);
            }
            buffer
                .try_reserve(additional)
                .map_err(|_| JsStringError::OutOfMemory)
        }
        Self::checked_length(self.len(), other.len())?;
        // QuickJS exposes rope/flat distinctions in host value printing. Keep
        // its flat-concat admission even when we own spare buffer capacity.
        if self.len() > Self::ROPE_SHORT2_LEN || other.len() > Self::ROPE_SHORT_LEN {
            return Ok(false);
        }

        if let Some(repr) = Rc::get_mut(&mut self.0) {
            // Only this exclusive mutation can change code units. Shared rope
            // linearization preserves content and therefore preserves its hash.
            repr.content_hash.set(None);
            match (&mut repr.data, other.0.data()) {
                (StringData::Latin1(left), StringData::Latin1(right)) => {
                    reserve(left, right.len())?;
                    left.extend_from_slice(right);
                    return Ok(true);
                }
                (StringData::Utf16(left), StringData::Utf16(right)) => {
                    reserve(left, right.len())?;
                    left.extend_from_slice(right);
                    return Ok(true);
                }
                (StringData::Utf16(left), StringData::Latin1(right)) => {
                    reserve(left, right.len())?;
                    left.extend(right.iter().copied().map(u16::from));
                    return Ok(true);
                }
                _ => {}
            }
        }
        Ok(false)
    }

    /// Encode the byte slice exposed by QuickJS `JS_ToCStringLen2` when its
    /// `cesu8` flag is false.
    ///
    /// Valid surrogate pairs become standard four-byte UTF-8. Unpaired
    /// surrogates are deliberately retained as three-byte WTF-8 sequences.
    /// Embedded U+0000 is returned as an ordinary zero byte; the terminating C
    /// NUL owned by QuickJS is not part of its reported length and is therefore
    /// not included in this safe Rust byte vector. As in QuickJS, converting a
    /// rope caches its flat representation on the shared String identity.
    ///
    /// # Errors
    /// Returns [`TryReserveError`] if the output buffer cannot be reserved.
    pub fn try_to_wtf8_bytes(&self) -> Result<Vec<u8>, TryReserveError> {
        self.try_to_quickjs_utf8_bytes(false)
    }

    /// Append the WTF-8 bytes exposed by `JS_ToCStringLen2(..., false)` to an
    /// existing buffer while preserving the same rope-linearization side
    /// effect as [`Self::try_to_wtf8_bytes`].
    pub fn try_append_wtf8_bytes(&self, output: &mut Vec<u8>) -> Result<(), TryReserveError> {
        self.try_append_quickjs_utf8_bytes(output, false)
    }

    /// Encode the byte slice exposed by QuickJS `JS_ToCStringLen2` when its
    /// `cesu8` flag is true.
    ///
    /// Every UTF-16 code unit is encoded independently, so a valid surrogate
    /// pair occupies two three-byte CESU-8 sequences. Embedded U+0000 is
    /// retained and no trailing C NUL is included in the returned vector. Rope
    /// conversion has the same shared flat-cache transition as the WTF-8 path.
    ///
    /// # Errors
    /// Returns [`TryReserveError`] if the output buffer cannot be reserved.
    pub fn try_to_cesu8_bytes(&self) -> Result<Vec<u8>, TryReserveError> {
        self.try_to_quickjs_utf8_bytes(true)
    }

    fn try_to_quickjs_utf8_bytes(&self, cesu8: bool) -> Result<Vec<u8>, TryReserveError> {
        let mut output = Vec::new();
        self.try_append_quickjs_utf8_bytes(&mut output, cesu8)?;
        Ok(output)
    }

    fn try_append_quickjs_utf8_bytes(
        &self,
        output: &mut Vec<u8>,
        cesu8: bool,
    ) -> Result<(), TryReserveError> {
        // JS_ToCStringLen2 first rewrites a rope to its cached flat form. The
        // root keeps its rope identity, but later diagnostics observe one
        // logical leaf instead of the former tree boundary.
        let value = self.linearize();
        let encoded_len = quickjs_utf8_length(value.utf16_units(), cesu8);
        let initial_len = output.len();
        output.try_reserve_exact(encoded_len)?;
        output.extend(QuickJsUtf8Bytes::new(&value, cesu8));
        debug_assert_eq!(output.len(), initial_len + encoded_len);
        Ok(())
    }

    pub fn wtf8_bytes(&self) -> impl Iterator<Item = u8> {
        QuickJsUtf8Bytes::new(self, false)
    }

    pub fn push_c_string_to(&self, output: &mut NativeErrorMessage) {
        output.push_c_string_bytes(self.wtf8_bytes());
    }

    /// Reproduce `JS_AtomGetStr(..., char buf[64], 64)` as consumed by a C
    /// `%s` native-error argument. Narrow all-ASCII atoms bypass the scratch
    /// buffer; every other text atom is encoded one UTF-16 code unit at a time
    /// and stops before starting a unit once 58 bytes have already been written.
    pub fn push_atom_get_str_to(&self, output: &mut NativeErrorMessage) {
        let atom = self.linearize();
        if !atom.is_wide() && atom.utf16_units().all(|unit| unit < 0x80) {
            atom.push_c_string_to(output);
            return;
        }

        const ATOM_BUFFER_SIZE: usize = 64;
        const UTF8_CHAR_LEN_MAX: usize = 6;
        let mut scratch = [0_u8; ATOM_BUFFER_SIZE];
        let mut len = 0;
        for unit in atom.utf16_units() {
            if len >= ATOM_BUFFER_SIZE - UTF8_CHAR_LEN_MAX {
                break;
            }
            let mut encoded = [0_u8; 4];
            let encoded_len = encode_quickjs_utf8(&mut encoded, u32::from(unit));
            scratch[len..len + encoded_len].copy_from_slice(&encoded[..encoded_len]);
            len += encoded_len;
        }
        output.push_c_string_bytes(scratch[..len].iter().copied());
    }

    /// Lossy conversion is suitable for terminal diagnostics. It must not be
    /// used for language-level string comparison or indexing.
    #[must_use]
    pub fn to_utf8_lossy(&self) -> String {
        match self.0.data() {
            StringData::Latin1(units) if units.is_ascii() => {
                String::from_utf8(units.clone()).expect("ASCII is UTF-8")
            }
            StringData::Latin1(units) => units.iter().map(|&unit| char::from(unit)).collect(),
            StringData::Utf16(units) => String::from_utf16_lossy(units),
            StringData::Rope(_) => char::decode_utf16(self.utf16_units())
                .map(|result| result.unwrap_or(char::REPLACEMENT_CHARACTER))
                .collect(),
        }
    }
}

pub fn decode_quickjs_utf8(value: &[u8]) -> Option<(u32, usize)> {
    let first = *value.first()?;
    if first < 0x80 {
        return Some((u32::from(first), 1));
    }

    let (continuation_count, mask, minimum): (usize, u8, u32) = match first {
        0x00..=0x7f => unreachable!("ASCII byte escaped the fast path"),
        0xc0..=0xdf => (1, 0x1f, 0x80),
        0xe0..=0xef => (2, 0x0f, 0x800),
        0xf0..=0xf7 => (3, 0x07, 0x1_0000),
        0xf8..=0xfb => (4, 0x03, 0x20_0000),
        0xfc..=0xfd => (5, 0x01, 0x400_0000),
        0x80..=0xbf | 0xfe..=0xff => return None,
    };
    if value.len() <= continuation_count {
        return None;
    }

    let mut code_point = u32::from(first & mask);
    for &byte in &value[1..=continuation_count] {
        if !(0x80..=0xbf).contains(&byte) {
            return None;
        }
        code_point = (code_point << 6) | u32::from(byte & 0x3f);
    }
    (code_point >= minimum).then_some((code_point, continuation_count + 1))
}

fn skip_quickjs_invalid_utf8(value: &[u8], mut index: usize) -> usize {
    while value
        .get(index)
        .is_some_and(|byte| (0x80..=0xbf).contains(byte))
    {
        index += 1;
    }
    if index < value.len() {
        index += 1;
        while value
            .get(index)
            .is_some_and(|byte| (0x80..=0xbf).contains(byte))
        {
            index += 1;
        }
    }
    index
}

fn quickjs_utf8_length(units: impl Iterator<Item = u16>, cesu8: bool) -> usize {
    let mut units = units.peekable();
    let mut length = 0;
    while let Some(unit) = units.next() {
        if !cesu8
            && (0xd800..=0xdbff).contains(&unit)
            && units
                .peek()
                .is_some_and(|next| (0xdc00..=0xdfff).contains(next))
        {
            units.next();
            length += 4;
        } else {
            length += match unit {
                0x0000..=0x007f => 1,
                0x0080..=0x07ff => 2,
                0x0800..=0xffff => 3,
            };
        }
    }
    length
}

fn encode_quickjs_utf8(output: &mut [u8; 4], code_point: u32) -> usize {
    match code_point {
        0x0000..=0x007f => {
            output[0] = code_point as u8;
            1
        }
        0x0080..=0x07ff => {
            output[0] = (0xc0 | (code_point >> 6)) as u8;
            output[1] = (0x80 | (code_point & 0x3f)) as u8;
            2
        }
        0x0800..=0xffff => {
            output[0] = (0xe0 | (code_point >> 12)) as u8;
            output[1] = (0x80 | ((code_point >> 6) & 0x3f)) as u8;
            output[2] = (0x80 | (code_point & 0x3f)) as u8;
            3
        }
        0x1_0000..=0x10_ffff => {
            output[0] = (0xf0 | (code_point >> 18)) as u8;
            output[1] = (0x80 | ((code_point >> 12) & 0x3f)) as u8;
            output[2] = (0x80 | ((code_point >> 6) & 0x3f)) as u8;
            output[3] = (0x80 | (code_point & 0x3f)) as u8;
            4
        }
        _ => unreachable!("QuickJS byte encoder received an invalid code point"),
    }
}

impl PartialEq for JsString {
    fn eq(&self, other: &Self) -> bool {
        if Rc::ptr_eq(&self.0, &other.0) {
            return true;
        }
        if self.len() != other.len() {
            return false;
        }
        // Flat strings need neither owned traversal handles nor a rope stack.
        // Width is a representation choice, so mixed-width equality compares
        // exact code units rather than treating Latin-1 as UTF-8.
        match (self.0.data(), other.0.data()) {
            (StringData::Latin1(left), StringData::Latin1(right)) => left == right,
            (StringData::Utf16(left), StringData::Utf16(right)) => left == right,
            (StringData::Latin1(narrow), StringData::Utf16(wide))
            | (StringData::Utf16(wide), StringData::Latin1(narrow)) => narrow
                .iter()
                .copied()
                .map(u16::from)
                .eq(wide.iter().copied()),
            _ => self.utf16_units().eq(other.utf16_units()),
        }
    }
}

impl Eq for JsString {}

impl Hash for JsString {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u32(self.content_hash());
    }
}

impl fmt::Debug for JsString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("JsString")
            .field(&self.to_utf8_lossy())
            .finish()
    }
}

impl fmt::Display for JsString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_utf8_lossy())
    }
}

/// A constant-pool value of the front end. Numbers follow `QuickJS`'s split
/// between immediate 32-bit integers and IEEE-754 doubles.
#[derive(Clone, Debug)]
pub enum PrimitiveValue {
    Int(i32),
    Float(f64),
    BigInt(JsBigInt),
}

impl PrimitiveValue {
    #[must_use]
    pub fn number(value: f64) -> Self {
        if value == f64::from(value as i32) && !(value == 0.0 && value.is_sign_negative()) {
            Self::Int(value as i32)
        } else {
            Self::Float(value)
        }
    }

    #[must_use]
    pub const fn as_number(&self) -> Option<f64> {
        match self {
            Self::Int(value) => Some(*value as f64),
            Self::Float(value) => Some(*value),
            _ => None,
        }
    }

    /// Apply ECMAScript `ToString` to the value kinds implemented by the
    /// runtime kernel.
    ///
    /// # Errors
    /// Symbol conversion throws, an extended-limit BigInt can fail the pinned
    /// QuickJS decimal-conversion allocation guard, and object conversion must
    /// be routed through a context so `ToPrimitive` can execute user code.
    pub fn to_js_string(&self) -> Result<JsString, Error> {
        let text = match self {
            Self::Int(value) => value.to_string(),
            Self::Float(value) => number_to_string(*value),
            Self::BigInt(value) => {
                if value.exceeds_allocation_limit() {
                    return Err(Error::new(
                        ErrorKind::Range,
                        "BigInt is too large to allocate",
                    ));
                }
                value.to_string()
            }
        };
        Ok(JsString::try_from_utf8(&text)?)
    }

    #[must_use]
    #[expect(clippy::float_cmp)]
    pub fn strict_equal(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::BigInt(left), Self::BigInt(right)) => left == right,
            (Self::Int(left), Self::Int(right)) => left == right,
            (left, right) => match (left.as_number(), right.as_number()) {
                (Some(left), Some(right)) => left == right,
                _ => false,
            },
        }
    }
}

/// `Number::toString(value, 10)`, as the core prints it.
#[must_use]
pub fn number_to_string(value: f64) -> String {
    oxide_core::vm::radix::to_string_radix(value, 10)
}

pub(super) const fn is_ecmascript_whitespace(unit: u16) -> bool {
    matches!(
        unit,
        0x0009..=0x000d
            | 0x0020
            | 0x00a0
            | 0x1680
            | 0x2000..=0x200a
            | 0x2028
            | 0x2029
            | 0x202f
            | 0x205f
            | 0x3000
            | 0xfeff
    )
}

impl PartialEq for PrimitiveValue {
    fn eq(&self, other: &Self) -> bool {
        self.strict_equal(other)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::collections::hash_map::DefaultHasher;

    use std::hash::{Hash, Hasher};
    use std::rc::Rc;

    use super::{
        JsString, JsStringError, PrimitiveValue, StringData, StringRepr, number_to_string,
    };
    use crate::engine::api::error::{Error, ErrorKind};

    #[test]
    fn cached_content_hash_survives_linearization_and_invalidates_on_append() {
        let mut flat = JsString::from_static("abc");
        assert_eq!(
            flat.content_hash(),
            JsString::from_static("abc").content_hash()
        );
        assert!(flat.0.content_hash.get().is_some());
        assert!(
            flat.try_concat_in_place(&JsString::from_static("def"))
                .unwrap()
        );
        assert_eq!(
            flat.content_hash(),
            JsString::from_static("abcdef").content_hash()
        );
        let left = JsString::from_owned_latin1(vec![b'a'; 1024]);
        let right = JsString::from_owned_utf16(vec![0xd800; 1024]);
        let rope = left.try_concat(&right).unwrap();
        let hash = rope.content_hash();
        let flat = rope.linearize();
        assert_eq!(flat.content_hash(), hash);
        assert_eq!(rope.content_hash(), hash);
        assert!(rope.same_representation(&rope.clone()));
    }

    #[test]
    fn utf16_nth_skips_leaf_and_rope_ranges() {
        let left = JsString::from_owned_latin1(vec![b'a'; 1024]);
        let right = JsString::from_owned_utf16(vec![0x1234; 1024]);
        let joined = left.try_concat(&right).unwrap();
        assert!(matches!(joined.0.data(), StringData::Rope(_)));
        for source in [&left, &joined] {
            let expected = source.utf16_units().collect::<Vec<_>>();
            for skip in 0..=expected.len() + 1 {
                let mut iter = source.utf16_units();
                assert_eq!(iter.nth(skip), expected.get(skip).copied());
                assert_eq!(
                    iter.collect::<Vec<_>>(),
                    expected.get(skip.saturating_add(1)..).unwrap_or(&[])
                );
            }
        }
    }

    #[test]
    fn owned_concat_preserves_aliases_width_identity_and_rope_content() {
        let empty = JsString::from_static("");
        let narrow = JsString::from_owned_latin1(b"abc".to_vec());
        assert!(Rc::ptr_eq(&empty.try_concat(&narrow).unwrap().0, &narrow.0));
        assert!(Rc::ptr_eq(&narrow.try_concat(&empty).unwrap().0, &narrow.0));
        let identity = Rc::as_ptr(&narrow.0);
        let grown = narrow.concat_owned(&JsString::from_static("d")).unwrap();
        assert_eq!(Rc::as_ptr(&grown.0), identity);
        assert_eq!(grown.to_string(), "abcd");
        let alias = grown.clone();
        let copied = grown.concat_owned(&JsString::from_static("e")).unwrap();
        assert_eq!(alias.to_string(), "abcd");
        assert_eq!(copied.to_string(), "abcde");
        assert!(!Rc::ptr_eq(&alias.0, &copied.0));
        let wide = JsString::from_owned_utf16(vec![0xd800]);
        let identity = Rc::as_ptr(&wide.0);
        let wide = wide.concat_owned(&JsString::from_static("x")).unwrap();
        assert_eq!(Rc::as_ptr(&wide.0), identity);
        assert_eq!(wide.utf16_units().collect::<Vec<_>>(), vec![0xd800, 120]);
        let wide = wide
            .concat_owned(&JsString::from_owned_utf16(vec![0xdc00]))
            .unwrap();
        assert_eq!(Rc::as_ptr(&wide.0), identity);
        let widened = JsString::from_static("a").concat_owned(&wide).unwrap();
        assert_eq!(
            widened.utf16_units().collect::<Vec<_>>(),
            vec![97, 0xd800, 120, 0xdc00]
        );
        let weak_owner = JsString::from_owned_latin1(b"weak".to_vec());
        let weak = Rc::downgrade(&weak_owner.0);
        let identity = Rc::as_ptr(&weak_owner.0);
        let weak_result = weak_owner
            .concat_owned(&JsString::from_static("!"))
            .unwrap();
        assert_ne!(Rc::as_ptr(&weak_result.0), identity);
        assert!(weak.upgrade().is_none());
        let suffix = JsString::from_static("xy");
        let mut owned = JsString::from_owned_latin1(b"start".to_vec());
        let identity = Rc::as_ptr(&owned.0);
        let mut reference = owned.clone();
        // Release the temporary alias before beginning owned append.
        reference = reference.try_concat(&suffix).unwrap();
        owned = owned.concat_owned(&suffix).unwrap();
        for _ in 0..100 {
            owned = owned.concat_owned(&suffix).unwrap();
            reference = reference.try_concat(&suffix).unwrap();
        }
        assert_eq!(Rc::as_ptr(&owned.0), identity);
        assert!(owned.is_flat());
        for _ in 0..8900 {
            owned = owned.concat_owned(&suffix).unwrap();
            reference = reference.try_concat(&suffix).unwrap();
        }
        assert_eq!(owned, reference);
        assert_eq!(content_hash(&owned), content_hash(&reference));
        assert!(!owned.is_flat());
        assert!(!reference.is_flat());
        let rope = reference.clone();
        let appended = reference.concat_owned(&suffix).unwrap();
        assert_eq!(appended, rope.try_concat(&suffix).unwrap());
    }

    #[test]
    fn owned_concat_preserves_observable_flat_rope_thresholds() {
        for (left_len, right_len, expected_in_place) in [
            (600, 600, false),
            (8192, 512, true),
            (8193, 512, false),
            (8192, 513, false),
        ] {
            let mut left = JsString::from_owned_latin1(vec![b'a'; left_len]);
            let right = JsString::from_owned_latin1(vec![b'b'; right_len]);
            let identity = Rc::as_ptr(&left.0);
            assert_eq!(left.try_concat_in_place(&right).unwrap(), expected_in_place);
            assert_eq!(Rc::as_ptr(&left.0), identity);
            if expected_in_place {
                assert!(left.is_flat());
                assert_eq!(left.len(), left_len + right_len);
            } else {
                assert_eq!(left.len(), left_len);
                let result = left.concat_owned(&right).unwrap();
                assert!(!result.is_flat());
                assert_eq!(result.len(), left_len + right_len);
            }
        }
    }

    fn content_hash(value: &JsString) -> u64 {
        let mut state = DefaultHasher::new();
        value.hash(&mut state);
        state.finish()
    }

    #[test]
    fn tagged_atom_decimal_strings_are_narrow_and_fresh() {
        for (value, expected) in [(0, "0"), (42, "42"), (2_147_483_647, "2147483647")] {
            let left = JsString::from_fresh_decimal_u32(value);
            let right = JsString::from_fresh_decimal_u32(value);
            assert_eq!(left, JsString::from_static(expected));
            assert_eq!(right, left);
            assert!(!left.same_representation(&right));
            assert!(matches!(left.0.data(), StringData::Latin1(_)));
        }
    }

    #[test]
    fn checked_string_construction_rejects_limits_and_hostile_hints_without_overpolling() {
        struct PanicOnPoll;
        impl Iterator for PanicOnPoll {
            type Item = u16;

            fn next(&mut self) -> Option<Self::Item> {
                panic!("oversized lower bound should reject before polling")
            }

            fn size_hint(&self) -> (usize, Option<usize>) {
                (usize::MAX, None)
            }
        }

        struct LateOverflow {
            polls: Rc<Cell<usize>>,
        }
        impl Iterator for LateOverflow {
            type Item = u16;

            fn next(&mut self) -> Option<Self::Item> {
                let poll = self.polls.get();
                self.polls.set(poll + 1);
                match poll {
                    0..=3 => Some(u16::from(b'a') + u16::try_from(poll).unwrap()),
                    _ => panic!("constructor polled after observing the overflowing unit"),
                }
            }

            fn size_hint(&self) -> (usize, Option<usize>) {
                (0, Some(0))
            }
        }

        assert_eq!(JsString::MAX_LEN, 0x3fff_ffff);
        assert_eq!(
            JsString::checked_length(JsString::MAX_LEN - 1, 1),
            Ok(JsString::MAX_LEN)
        );
        assert_eq!(
            JsString::checked_length(JsString::MAX_LEN, 1),
            Err(JsStringError::TooLong)
        );
        assert_eq!(
            JsString::checked_length(usize::MAX, 1),
            Err(JsStringError::TooLong)
        );
        assert_eq!(
            JsString::try_from_utf16_with_limit(PanicOnPoll, 3),
            Err(JsStringError::TooLong)
        );

        struct UpperHint<I>(I);
        impl<I: Iterator<Item = u16>> Iterator for UpperHint<I> {
            type Item = u16;

            fn next(&mut self) -> Option<Self::Item> {
                self.0.next()
            }

            fn size_hint(&self) -> (usize, Option<usize>) {
                (0, Some(usize::MAX))
            }
        }
        let accepted =
            JsString::try_from_utf16_with_limit(UpperHint([0x61, 0x100].into_iter()), 2).unwrap();
        assert_eq!(accepted.utf16_units().collect::<Vec<_>>(), [0x61, 0x100]);

        let polls = Rc::new(Cell::new(0));
        assert_eq!(
            JsString::try_from_utf16_with_limit(
                LateOverflow {
                    polls: polls.clone(),
                },
                3,
            ),
            Err(JsStringError::TooLong)
        );
        assert_eq!(polls.get(), 4);
        assert!(JsString::try_from_utf16_with_limit([], 0).is_ok());
        assert_eq!(
            JsString::try_from_utf16_with_limit([0x61], 0),
            Err(JsStringError::TooLong)
        );
        assert!(JsString::try_from_utf16_with_limit("😀".encode_utf16(), 2).is_ok());
        assert_eq!(
            JsString::try_from_utf16_with_limit("😀".encode_utf16(), 1),
            Err(JsStringError::TooLong)
        );
    }

    #[test]
    fn quickjs_byte_constructor_preserves_wtf8_and_invalid_skip_rules() {
        let cases = [
            (vec![], vec![]),
            (vec![0x00, 0x41], vec![0x0000, 0x0041]),
            (vec![0xc3, 0xa9], vec![0x00e9]),
            (vec![0xc4, 0x80], vec![0x0100]),
            (vec![0xed, 0xa0, 0x80], vec![0xd800]),
            (vec![0xed, 0xb0, 0x80], vec![0xdc00]),
            (vec![0xf0, 0x9f, 0x98, 0x80], vec![0xd83d, 0xde00]),
            (vec![0x80, 0x41], vec![0xfffd]),
            (vec![0xff, 0x41], vec![0xfffd, 0x0041]),
            (vec![0x80, 0x80, 0x41, 0x80, 0x42], vec![0xfffd, 0x0042]),
            (vec![0x80, 0xc2, 0xa2], vec![0xfffd]),
            (vec![0xe2, 0x28, 0xa1], vec![0xfffd, 0x0028, 0xfffd]),
            (vec![0xe2, 0x82], vec![0xfffd]),
            (vec![0xc0, 0x80, 0x41], vec![0xfffd, 0x0041]),
            (vec![0xf4, 0x90, 0x80, 0x80, 0x41], vec![0xfffd, 0x0041]),
            (vec![0xfe, 0x80, 0x41], vec![0xfffd, 0x0041]),
            (vec![0xf8, 0x88, 0x80, 0x80, 0x80], vec![0xfffd]),
        ];
        for (bytes, expected) in cases {
            let actual = JsString::try_from_bytes(&bytes).unwrap();
            assert_eq!(
                actual.utf16_units().collect::<Vec<_>>(),
                expected,
                "{bytes:02x?}"
            );
        }

        assert!(JsString::try_from_bytes_with_limit(&[0xf0, 0x9f, 0x98, 0x80], 2).is_ok());
        assert_eq!(
            JsString::try_from_bytes_with_limit(&[0xf0, 0x9f, 0x98, 0x80], 1),
            Err(JsStringError::TooLong)
        );
        assert!(JsString::try_from_bytes_with_limit(&[0x80, 0x41], 1).is_ok());
        assert_eq!(
            JsString::try_from_bytes_with_limit(&[0xff, 0x41], 1),
            Err(JsStringError::TooLong)
        );
        assert_eq!(
            JsString::try_from_bytes_with_limit(b"ab", 1),
            Err(JsStringError::TooLong)
        );
    }

    #[test]
    fn quickjs_byte_exports_distinguish_wtf8_from_cesu8_and_roundtrip_utf16() {
        let value = JsString::try_from_utf16([
            0x0041, 0x0000, 0x00e9, 0x0800, 0xd800, 0x0042, 0xdc00, 0xd83d, 0xde00,
        ])
        .unwrap();
        assert_eq!(
            value.try_to_wtf8_bytes().unwrap(),
            [
                0x41, 0x00, 0xc3, 0xa9, 0xe0, 0xa0, 0x80, 0xed, 0xa0, 0x80, 0x42, 0xed, 0xb0, 0x80,
                0xf0, 0x9f, 0x98, 0x80,
            ]
        );
        assert_eq!(
            value.try_to_cesu8_bytes().unwrap(),
            [
                0x41, 0x00, 0xc3, 0xa9, 0xe0, 0xa0, 0x80, 0xed, 0xa0, 0x80, 0x42, 0xed, 0xb0, 0x80,
                0xed, 0xa0, 0xbd, 0xed, 0xb8, 0x80,
            ]
        );

        let all_units = JsString::try_from_utf16(0_u16..=u16::MAX).unwrap();
        for bytes in [
            all_units.try_to_wtf8_bytes().unwrap(),
            all_units.try_to_cesu8_bytes().unwrap(),
        ] {
            assert_eq!(JsString::try_from_bytes(&bytes).unwrap(), all_units);
        }

        let high = JsString::try_from_utf16([0xd83d]).unwrap();
        let low_and_tail = JsString::try_from_utf16(
            [0xde00]
                .into_iter()
                .chain(std::iter::repeat_n(u16::from(b'b'), 512)),
        )
        .unwrap();
        let across_leaf = JsString::try_from_utf8(&"a".repeat(8193))
            .unwrap()
            .try_concat(&high)
            .unwrap()
            .try_concat(&low_and_tail)
            .unwrap();
        assert!(!across_leaf.is_flat());
        let mut leaf_lengths = Vec::new();
        across_leaf.for_each_flat_leaf(|leaf| leaf_lengths.push(leaf.len()));
        assert_eq!(leaf_lengths, [8193, 1, 513]);
        let flat = JsString::try_from_utf16(
            std::iter::repeat_n(u16::from(b'a'), 8193)
                .chain([0xd83d, 0xde00])
                .chain(std::iter::repeat_n(u16::from(b'b'), 512)),
        )
        .unwrap();
        let wtf8 = across_leaf.try_to_wtf8_bytes().unwrap();
        assert_eq!(&wtf8[8193..8197], &[0xf0, 0x9f, 0x98, 0x80]);
        assert_eq!(wtf8, flat.try_to_wtf8_bytes().unwrap());
        leaf_lengths.clear();
        across_leaf.for_each_flat_leaf(|leaf| leaf_lengths.push(leaf.len()));
        assert_eq!(leaf_lengths, [across_leaf.len()]);
        let cesu8 = across_leaf.try_to_cesu8_bytes().unwrap();
        assert_eq!(&cesu8[8193..8199], &[0xed, 0xa0, 0xbd, 0xed, 0xb8, 0x80]);
        assert_eq!(cesu8, flat.try_to_cesu8_bytes().unwrap());
    }

    #[test]
    fn string_length_counts_utf16_code_units() {
        let text = JsString::from_static("a🚀");
        assert_eq!(text.len(), 3);
        assert_eq!(
            text.utf16_units().collect::<Vec<_>>(),
            vec![0x61, 0xd83d, 0xde80]
        );
    }

    #[test]
    fn strings_preserve_lone_surrogates() {
        let text = JsString::try_from_utf16([0xd800, 0x61]).unwrap();
        assert_eq!(text.utf16_units().collect::<Vec<_>>(), vec![0xd800, 0x61]);
        assert_eq!(text.to_utf8_lossy(), "�a");
    }

    #[test]
    fn utf16_index_code_point_and_well_formed_helpers_preserve_quickjs_rules() {
        let text = JsString::try_from_utf16([0x41, 0xd83d, 0xde80, 0xd800, 0x42, 0xdc00]).unwrap();
        assert_eq!(text.code_unit_at(0), Some(0x41));
        assert_eq!(text.code_unit_at(6), None);
        assert_eq!(text.code_point_at(1), Some(0x1f680));
        assert_eq!(text.code_point_at(2), Some(0xde80));
        assert_eq!(text.code_point_at(3), Some(0xd800));
        assert!(!text.is_well_formed());
        assert_eq!(
            text.to_well_formed().utf16_units().collect::<Vec<_>>(),
            vec![0x41, 0xd83d, 0xde80, 0xfffd, 0x42, 0xfffd]
        );

        let well_formed = JsString::try_from_utf16([0xd83d, 0xde80]).unwrap();
        assert!(well_formed.is_well_formed());
        assert_eq!(well_formed.to_well_formed(), well_formed);
    }

    #[test]
    fn sub_string_linearizes_ranges_and_preserves_quickjs_width_rules() {
        let latin1 = JsString::from_static("abcdef");
        let latin1_full = latin1.sub_string(0, latin1.len());
        assert!(latin1_full.same_representation(&latin1));
        let latin1_range = latin1.sub_string(1, 4);
        assert_eq!(latin1_range, JsString::from_static("bcd"));
        assert!(matches!(latin1_range.0.data(), StringData::Latin1(_)));

        let forced_wide = JsString(StringRepr::allocate(StringData::Utf16(
            [u16::from(b'a'), u16::from(b'b'), 0xd800, u16::from(b'c')]
                .into_iter()
                .collect::<Vec<_>>(),
        )));
        let narrowed = forced_wide.sub_string(0, 2);
        assert!(matches!(narrowed.0.data(), StringData::Latin1(_)));
        assert_eq!(
            narrowed.utf16_units().collect::<Vec<_>>(),
            [u16::from(b'a'), u16::from(b'b')]
        );
        let still_wide = forced_wide.sub_string(1, 3);
        assert!(matches!(still_wide.0.data(), StringData::Utf16(_)));
        assert_eq!(
            still_wide.utf16_units().collect::<Vec<_>>(),
            [u16::from(b'b'), 0xd800]
        );
        let empty = forced_wide.sub_string(2, 2);
        assert!(matches!(empty.0.data(), StringData::Latin1(units) if units.is_empty()));

        let left = JsString::try_from_utf8(&"x".repeat(8_193)).unwrap();
        let right = JsString::try_from_utf16([0xd83d, 0xde00, u16::from(b'z')]).unwrap();
        let rope = left.try_concat(&right).unwrap();
        assert!(!rope.is_flat());
        let full = rope.sub_string(0, rope.len());
        assert!(full.is_flat());
        assert_eq!(full, rope);
        let cached_flat = rope.linearize();
        assert!(full.same_representation(&cached_flat));
        let crossed = rope.sub_string(8_193, 8_196);
        assert!(crossed.is_flat());
        assert_eq!(
            crossed.utf16_units().collect::<Vec<_>>(),
            [0xd83d, 0xde00, u16::from(b'z')]
        );
    }

    #[test]
    fn repeat_builds_one_flat_string_with_quickjs_width_and_limit_rules() {
        let latin1 = JsString::from_static("ab");
        let repeated = latin1.repeat_with_limit(3, JsString::MAX_LEN).unwrap();
        assert_eq!(repeated, JsString::from_static("ababab"));
        assert!(repeated.is_flat());
        assert!(matches!(repeated.0.data(), StringData::Latin1(_)));
        let once = latin1.repeat_with_limit(1, 1).unwrap();
        assert!(once.same_representation(&latin1));

        let wide = JsString(StringRepr::allocate(StringData::Utf16(
            [u16::from(b'A'), 0xd800].into_iter().collect(),
        )));
        let repeated = wide.repeat_with_limit(3, JsString::MAX_LEN).unwrap();
        assert_eq!(
            repeated.utf16_units().collect::<Vec<_>>(),
            [0x41, 0xd800, 0x41, 0xd800, 0x41, 0xd800],
        );
        assert!(matches!(repeated.0.data(), StringData::Utf16(_)));
        let zero = wide.repeat_with_limit(0, JsString::MAX_LEN).unwrap();
        assert!(matches!(zero.0.data(), StringData::Latin1(units) if units.is_empty()));

        let left = JsString::try_from_utf8(&"x".repeat(8_193)).unwrap();
        let rope = left.try_concat(&JsString::from_static("yz")).unwrap();
        assert!(!rope.is_flat());
        let once = rope.repeat_with_limit(1, JsString::MAX_LEN).unwrap();
        assert!(once.is_flat());
        assert!(once.same_representation(&rope.linearize()));
        let twice = rope.repeat_with_limit(2, JsString::MAX_LEN).unwrap();
        assert!(twice.is_flat());
        assert_eq!(twice.len(), rope.len() * 2);
        assert_eq!(twice.code_unit_at(rope.len()), Some(u16::from(b'x')));

        let empty = JsString::from_static("");
        let empty_repeat = empty.repeat_with_limit(2_147_483_647, 0).unwrap();
        assert!(empty_repeat.same_representation(&empty));
        assert_eq!(latin1.repeat_with_limit(3, 5), Err(JsStringError::TooLong));

        super::fail_next_repeat_reservation_for_test();
        let forced_fast = latin1.repeat_with_limit(1, 1).unwrap();
        assert!(forced_fast.same_representation(&latin1));
        assert_eq!(
            latin1.repeat_with_limit(2, JsString::MAX_LEN),
            Err(JsStringError::OutOfMemory),
        );
        assert_eq!(
            latin1.repeat_with_limit(2, JsString::MAX_LEN).unwrap(),
            JsString::from_static("abab"),
            "the fail-next reservation hook was not consumed exactly once",
        );
    }

    #[test]
    fn pad_uses_quickjs_order_width_limit_and_recoverable_reservations() {
        let source = JsString::from_static("ab");
        let filler = JsString::from_static("xy");
        assert_eq!(
            source
                .pad_with_limit(7, Some(&filler), true, JsString::MAX_LEN)
                .unwrap(),
            JsString::from_static("abxyxyx"),
        );
        assert_eq!(
            source
                .pad_with_limit(7, Some(&filler), false, JsString::MAX_LEN)
                .unwrap(),
            JsString::from_static("xyxyxab"),
        );
        assert_eq!(
            source
                .pad_with_limit(4, None, true, JsString::MAX_LEN)
                .unwrap(),
            JsString::from_static("ab  "),
        );

        let forced_wide_filler = JsString(StringRepr::allocate(StringData::Utf16(
            [u16::from(b'z'), 0x100].into_iter().collect(),
        )));
        let narrow = source
            .pad_with_limit(3, Some(&forced_wide_filler), false, JsString::MAX_LEN)
            .unwrap();
        assert_eq!(narrow, JsString::from_static("zab"));
        assert!(matches!(narrow.0.data(), StringData::Latin1(_)));
        let wide = source
            .pad_with_limit(4, Some(&forced_wide_filler), false, JsString::MAX_LEN)
            .unwrap();
        assert_eq!(
            wide.utf16_units().collect::<Vec<_>>(),
            [u16::from(b'z'), 0x100, u16::from(b'a'), u16::from(b'b')],
        );
        assert!(matches!(wide.0.data(), StringData::Utf16(_)));

        let early = source
            .pad_with_limit(2, Some(&filler), true, 0)
            .expect("source-length early return must precede the cap");
        assert!(early.same_representation(&source));
        let empty = JsString::from_static("");
        let empty_filler = source
            .pad_with_limit(100, Some(&empty), false, 0)
            .expect("empty-filler early return must precede the cap");
        assert!(empty_filler.same_representation(&source));
        assert_eq!(
            source.pad_with_limit(3, Some(&filler), true, 2),
            Err(JsStringError::TooLong),
        );

        super::fail_next_pad_reservation_for_test();
        let forced_fast = source.pad_with_limit(2, Some(&filler), true, 0).unwrap();
        assert!(forced_fast.same_representation(&source));
        let forced_empty = source.pad_with_limit(100, Some(&empty), false, 0).unwrap();
        assert!(forced_empty.same_representation(&source));
        assert_eq!(
            source.pad_with_limit(3, Some(&filler), true, 2),
            Err(JsStringError::TooLong),
            "length rejection must precede and preserve the reservation hook",
        );
        assert_eq!(
            source.pad_with_limit(3, Some(&filler), true, JsString::MAX_LEN),
            Err(JsStringError::OutOfMemory),
        );
        assert_eq!(
            source
                .pad_with_limit(3, Some(&filler), true, JsString::MAX_LEN)
                .unwrap(),
            JsString::from_static("abx"),
            "the fail-next pad reservation hook was not consumed exactly once",
        );
    }

    #[test]
    fn trim_uses_quickjs_whitespace_width_identity_and_reservation_rules() {
        let whitespace = [
            0x0009, 0x000a, 0x000b, 0x000c, 0x000d, 0x0020, 0x00a0, 0x1680, 0x2000, 0x2001, 0x2002,
            0x2003, 0x2004, 0x2005, 0x2006, 0x2007, 0x2008, 0x2009, 0x200a, 0x2028, 0x2029, 0x202f,
            0x205f, 0x3000, 0xfeff,
        ];
        for unit in whitespace {
            let source = JsString::try_from_utf16([unit, u16::from(b'x'), unit]).unwrap();
            assert_eq!(
                source.trim_whitespace(true, true).unwrap(),
                JsString::from_static("x"),
                "U+{unit:04X} was not trimmed",
            );
        }
        for unit in [0x0000, 0x0085, 0x180e, 0x200b] {
            let source = JsString::try_from_utf16([unit, u16::from(b'x'), unit]).unwrap();
            assert!(
                source
                    .trim_whitespace(true, true)
                    .unwrap()
                    .same_representation(&source),
                "U+{unit:04X} was incorrectly trimmed",
            );
        }

        let source = JsString::from_static("  value  ");
        assert_eq!(
            source.trim_whitespace(true, false).unwrap(),
            JsString::from_static("value  "),
        );
        assert_eq!(
            source.trim_whitespace(false, true).unwrap(),
            JsString::from_static("  value"),
        );
        assert_eq!(
            source.trim_whitespace(true, true).unwrap(),
            JsString::from_static("value"),
        );

        let forced_wide_latin1 = JsString(StringRepr::allocate(StringData::Utf16(
            [0x20, u16::from(b'a'), u16::from(b'b'), 0x20]
                .into_iter()
                .collect(),
        )));
        let narrowed = forced_wide_latin1.trim_whitespace(true, true).unwrap();
        assert_eq!(narrowed, JsString::from_static("ab"));
        assert!(matches!(narrowed.0.data(), StringData::Latin1(_)));
        let forced_wide = JsString::try_from_utf16([0x20, 0x100, 0x20]).unwrap();
        let still_wide = forced_wide.trim_whitespace(true, true).unwrap();
        assert_eq!(still_wide.utf16_units().collect::<Vec<_>>(), [0x100]);
        assert!(matches!(still_wide.0.data(), StringData::Utf16(_)));

        let empty = JsString::from_static(" \u{feff}\u{2029}")
            .trim_whitespace(true, true)
            .unwrap();
        assert!(matches!(empty.0.data(), StringData::Latin1(units) if units.is_empty()));

        let rope = JsString::try_from_utf8(&" ".repeat(8_193))
            .unwrap()
            .try_concat(&JsString::from_static("x "))
            .unwrap();
        assert!(!rope.is_flat());
        assert_eq!(
            rope.trim_whitespace(true, true).unwrap(),
            JsString::from_static("x"),
        );

        super::fail_next_trim_reservation_for_test();
        let unchanged = JsString::from_static("value");
        assert!(
            unchanged
                .trim_whitespace(true, true)
                .unwrap()
                .same_representation(&unchanged),
            "full-range return must not consume the reservation hook",
        );
        let all_space = JsString::from_static("  ");
        assert!(all_space.trim_whitespace(true, true).unwrap().is_empty());
        assert_eq!(
            source.trim_whitespace(true, true),
            Err(JsStringError::OutOfMemory),
        );
        assert_eq!(
            source.trim_whitespace(true, true).unwrap(),
            JsString::from_static("value"),
            "the fail-next trim reservation hook was not consumed exactly once",
        );
    }

    #[test]
    fn rope_thresholds_fringe_merges_and_content_identity_match_quickjs() {
        let flat_8192 = JsString::try_from_utf8(&"a".repeat(8192)).unwrap();
        let short_512 = JsString::try_from_utf8(&"b".repeat(512)).unwrap();
        let merged_flat = flat_8192.try_concat(&short_512).unwrap();
        assert!(merged_flat.is_flat());
        assert_eq!(merged_flat.len(), 8704);

        let flat_8193 = JsString::try_from_utf8(&"a".repeat(8193)).unwrap();
        let rope = flat_8193.try_concat(&short_512).unwrap();
        assert!(!rope.is_flat());
        assert_eq!(rope.depth(), 1);
        assert_eq!(rope.code_unit_at(8192), Some(u16::from(b'a')));
        assert_eq!(rope.code_unit_at(8193), Some(u16::from(b'b')));

        let expected = JsString::try_from_utf8(&("a".repeat(8193) + &"b".repeat(512))).unwrap();
        assert_eq!(rope, expected);
        assert_eq!(content_hash(&rope), content_hash(&expected));
        let mut units = rope.utf16_units();
        assert_eq!(units.len(), rope.len());
        assert_eq!(units.next(), Some(u16::from(b'a')));
        assert_eq!(units.len(), rope.len() - 1);

        let one = JsString::from_static("c");
        let tail_merged = rope.try_concat(&one).unwrap();
        assert_eq!(tail_merged.depth(), 1);
        let super::StringData::Rope(tail) = tail_merged.0.data() else {
            panic!("short right fringe did not remain a rope");
        };
        let (_, tail_right) = JsString::rope_children(tail);
        assert_eq!(tail_right.len(), 513);
        assert!(tail_right.is_flat());

        let right_513 = JsString::try_from_utf8(&"z".repeat(513)).unwrap();
        let threshold_rope = flat_8192.try_concat(&right_513).unwrap();
        assert!(!threshold_rope.is_flat());
        let no_tail_merge = threshold_rope.try_concat(&one).unwrap();
        assert_eq!(no_tail_merge.depth(), 2);
        let super::StringData::Rope(no_tail_merge_rope) = no_tail_merge.0.data() else {
            panic!("513-unit right fringe did not remain nested");
        };
        let (unmerged_left, _) = JsString::rope_children(no_tail_merge_rope);
        assert!(Rc::ptr_eq(&unmerged_left.0, &threshold_rope.0));

        let left_513_rope = right_513.try_concat(&flat_8192).unwrap();
        let no_head_merge = one.try_concat(&left_513_rope).unwrap();
        assert_eq!(no_head_merge.depth(), 2);
        let super::StringData::Rope(no_head_merge_rope) = no_head_merge.0.data() else {
            panic!("513-unit left fringe did not remain nested");
        };
        let (_, unmerged_right) = JsString::rope_children(no_head_merge_rope);
        assert!(Rc::ptr_eq(&unmerged_right.0, &left_513_rope.0));

        let right_rope = short_512.try_concat(&right_513).unwrap();
        assert_eq!(right_rope.depth(), 1);
        let head_merged = one.try_concat(&right_rope).unwrap();
        assert_eq!(head_merged.depth(), 1);
        let super::StringData::Rope(head) = head_merged.0.data() else {
            panic!("short left fringe did not remain a rope");
        };
        let (head_left, _) = JsString::rope_children(head);
        assert_eq!(head_left.len(), 513);
        assert!(head_left.is_flat());

        let empty = JsString::from_static("");
        let large_flat = JsString::try_from_utf8(&"q".repeat(8193)).unwrap();
        let empty_plus_flat = empty.try_concat(&large_flat).unwrap();
        assert!(empty_plus_flat.is_flat());
        assert!(Rc::ptr_eq(&empty_plus_flat.0, &large_flat.0));
        let rope = large_flat.try_concat(&one).unwrap();
        let empty_plus_rope = empty.try_concat(&rope).unwrap();
        assert!(Rc::ptr_eq(&empty_plus_rope.0, &rope.0));
        let rope_plus_empty = rope.try_concat(&empty).unwrap();
        assert!(Rc::ptr_eq(&rope_plus_empty.0, &rope.0));

        let cached_flat = rope.linearize();
        assert!(cached_flat.is_flat());
        let cached_concat = rope.try_concat(&JsString::from_static("!")).unwrap();
        assert!(!cached_concat.is_flat());
        assert_eq!(
            cached_concat.code_unit_at(cached_concat.len() - 1),
            Some(0x21)
        );
    }

    #[test]
    fn rope_code_points_and_well_formed_scans_cross_leaf_boundaries() {
        let mut left = vec![u16::from(b'a'); 8192];
        left.push(0xd83d);
        let mut right = vec![0xde80];
        right.extend(std::iter::repeat_n(u16::from(b'b'), 512));
        let valid = JsString::try_from_utf16(left)
            .unwrap()
            .try_concat(&JsString::try_from_utf16(right).unwrap())
            .unwrap();
        assert!(!valid.is_flat());
        assert_eq!(valid.code_point_at(8192), Some(0x1f680));
        assert!(valid.is_well_formed());

        let invalid =
            JsString::try_from_utf16(std::iter::repeat_n(u16::from(b'a'), 8192).chain([0xd800]))
                .unwrap()
                .try_concat(
                    &JsString::try_from_utf16(
                        [u16::from(b'x')]
                            .into_iter()
                            .chain(std::iter::repeat_n(u16::from(b'b'), 512)),
                    )
                    .unwrap(),
                )
                .unwrap();
        assert!(!invalid.is_well_formed());
        let repaired = invalid.to_well_formed();
        assert_eq!(repaired.code_unit_at(8192), Some(0xfffd));
        assert_eq!(repaired.len(), invalid.len());
    }

    #[test]
    fn rope_rebalances_and_reaches_the_quickjs_length_guard_without_flattening() {
        let marker_chunk = |index: usize| {
            let marker = char::from(b'A' + u8::try_from(index % 26).unwrap());
            JsString::try_from_utf8(&marker.to_string().repeat(8193)).unwrap()
        };
        let mut deep = marker_chunk(0);
        for index in 1..101 {
            deep = deep.try_concat(&marker_chunk(index)).unwrap();
            assert!(deep.depth() <= JsString::ROPE_MAX_DEPTH);
        }
        for index in 0..101 {
            let marker = u16::from(b'A' + u8::try_from(index % 26).unwrap());
            assert_eq!(deep.code_unit_at(index * 8193), Some(marker));
            assert_eq!(deep.code_unit_at((index + 1) * 8193 - 1), Some(marker));
        }

        let mut prepended = marker_chunk(100);
        for index in (0..100).rev() {
            prepended = marker_chunk(index).try_concat(&prepended).unwrap();
            assert!(prepended.depth() <= JsString::ROPE_MAX_DEPTH);
        }
        for index in 0..101 {
            let marker = u16::from(b'A' + u8::try_from(index % 26).unwrap());
            assert_eq!(prepended.code_unit_at(index * 8193), Some(marker));
            assert_eq!(prepended.code_unit_at((index + 1) * 8193 - 1), Some(marker));
        }

        let before_hash = content_hash(&deep);
        let flat = deep.linearize();
        assert!(flat.is_flat());
        assert_eq!(deep, flat);
        assert_eq!(before_hash, content_hash(&deep));
        assert_eq!(before_hash, content_hash(&flat));

        let chunk = JsString::try_from_utf8(&"x".repeat(8193)).unwrap();
        let mut near_limit = chunk;
        for _ in 0..16 {
            near_limit = near_limit.try_concat(&near_limit).unwrap();
        }
        assert_eq!(near_limit.len(), 536_936_448);
        assert!(matches!(
            near_limit.try_concat(&near_limit),
            Err(JsStringError::TooLong)
        ));
        assert!(matches!(
            near_limit.clone().concat_owned(&near_limit),
            Err(JsStringError::TooLong)
        ));
        let error = Error::from(JsStringError::TooLong);
        assert_eq!(error.kind(), ErrorKind::JsInternal);
        assert_eq!(error.message(), "string too long");

        let mut powers = Vec::with_capacity(30);
        let mut power = JsString::from_static("x");
        powers.push(power.clone());
        for _ in 1..30 {
            power = power.try_concat(&power).unwrap();
            powers.push(power.clone());
        }
        let mut exact_max = JsString::from_static("");
        for power in powers.into_iter().rev() {
            exact_max = exact_max.try_concat(&power).unwrap();
        }
        assert_eq!(exact_max.len(), JsString::MAX_LEN);
        assert!(matches!(
            exact_max.try_concat(&JsString::from_static("x")),
            Err(JsStringError::TooLong)
        ));
        let exact_plus_empty = exact_max.try_concat(&JsString::from_static("")).unwrap();
        assert!(Rc::ptr_eq(&exact_plus_empty.0, &exact_max.0));
    }

    #[test]
    fn number_uses_int_fast_path_without_losing_negative_zero() {
        assert!(matches!(
            PrimitiveValue::number(42.0),
            PrimitiveValue::Int(42)
        ));
        assert!(
            matches!(PrimitiveValue::number(-0.0), PrimitiveValue::Float(value) if value.is_sign_negative())
        );
    }

    #[test]
    fn number_formatting_uses_ecmascript_thresholds() {
        assert_eq!(number_to_string(-0.0), "0");
        assert_eq!(number_to_string(f64::NAN), "NaN");
        assert_eq!(number_to_string(1e20), "100000000000000000000");
        assert_eq!(number_to_string(1e21), "1e+21");
    }
}
