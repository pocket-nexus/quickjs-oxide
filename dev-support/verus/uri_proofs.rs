// A mathematical decoder on UTF-16 code units. Radix arithmetic specifies
// hex, UTF-8 and surrogate pairs independently of the production bit shifts.
verus! {
type Text = Seq<u16>;
type Decoding = Result<Text, UriCodecError>;

spec fn digit(c: u16) -> Option<u8> {
    if 48 <= c <= 57 { Some((c - 48) as u8) }
    else if 65 <= c <= 70 { Some((c - 55) as u8) }
    else if 97 <= c <= 102 { Some((c - 87) as u8) }
    else { None }
}

spec fn hex_pair(a: u16, b: u16) -> Option<u16> {
    match (digit(a), digit(b)) {
        (Some(h), Some(l)) => Some((16 * h + l) as u16),
        _ => None,
    }
}

spec fn octet(s: Text, i: int) -> Result<u8, UriCodecError> {
    if !(0 <= i < s.len()) || s[i] != 37 { Err(UriCodecError::ExpectingPercent) }
    else if i + 2 >= s.len() { Err(UriCodecError::ExpectingHexDigit) }
    else {
        match hex_pair(s[i + 1], s[i + 2]) {
            Some(b) => Ok(b as u8),
            None => Err(UriCodecError::ExpectingHexDigit),
        }
    }
}

spec fn reserved(b: u8) -> bool {
    matches!(b, b';' | b'/' | b'?' | b':' | b'@' | b'&' | b'=' | b'+' | b'$' | b',' | b'#')
}

spec fn utf16(cp: int) -> Text {
    if cp < 65536 { seq![cp as u16] }
    else { seq![(55296 + (cp - 65536) / 1024) as u16, (56320 + (cp - 65536) % 1024) as u16] }
}

spec fn width(b: u8) -> int {
    if 192 <= b < 224 { 2 } else if 224 <= b < 240 { 3 }
    else if 240 <= b < 248 { 4 } else { 1 }
}

spec fn payload(b: u8) -> int {
    let w = width(b);
    if w == 2 { b as int % 32 }
    else if w == 3 { b as int % 16 }
    else if w == 4 { b as int % 8 } else { 0 }
}

spec fn scalar_min(width: int) -> int {
    if width == 2 { 128 } else if width == 3 { 2048 } else if width == 4 { 65536 } else { 1 }
}

spec fn scalar_tail(s: Text, i: int, left: int, cp: int, min: int) -> Decoding
    decreases left,
{
    if left <= 0 {
        if cp < min || cp > 0x10ffff || 0xd800 <= cp <= 0xdfff {
            Err(UriCodecError::MalformedUtf8)
        } else { Ok(utf16(cp)) }
    } else {
        match octet(s, i) {
            Err(e) => Err(e),
            Ok(b) => if 128 <= b < 192 {
                scalar_tail(s, i + 3, left - 1, 64 * cp + b as int - 128, min)
            } else { Err(UriCodecError::MalformedUtf8) },
        }
    }
}

spec fn append(prefix: Text, rest: Decoding) -> Decoding {
    match rest { Ok(text) => Ok(prefix + text), Err(e) => Err(e) }
}

spec fn join(token: Decoding, rest: Decoding) -> Decoding {
    match token { Ok(t) => append(t, rest), Err(e) => Err(e) }
}

spec fn decode_from(s: Text, i: int, component: bool) -> Decoding
    recommends i >= 0,
    decreases (if i < s.len() { s.len() - i } else { 0 }),
{
    if i >= s.len() { Ok(Seq::empty()) }
    else if s[i] != 37 {
        append(seq![s[i]], decode_from(s, i + 1, component))
    } else {
        match octet(s, i) {
            Err(e) => Err(e),
            Ok(b) => {
                let w = width(b);
                let text = if b < 128 {
                    Ok(if !component && reserved(b) { s.subrange(i, i + 3) } else { seq![b as u16] })
                } else { scalar_tail(s, i + 3, w - 1, payload(b), scalar_min(w)) };
                join(text, decode_from(s, i + 3 * w, component))
            },
        }
    }
}

spec fn result_text(r: Result<Vec<u16>, UriCodecError>) -> Decoding {
    match r { Ok(v) => Ok(v@), Err(e) => Err(e) }
}

proof fn append_assoc(p: Text, q: Text, r: Decoding)
    ensures append(p, append(q, r)) == append(p + q, r)
{
    if let Ok(t) = r { assert((p + (q + t)) =~= ((p + q) + t)); }
}
}

verus! {
proof fn reserved_bytes(b: u8)
    ensures b";/?:@&=+$,#".contains(&b) == reserved(b),
{
    reveal_byteslit(b";/?:@&=+$,#");
    let table = b";/?:@&=+$,#";
    assert forall|i: int| 0 <= i < 11 && #[trigger] table[i] == b implies reserved(b) by {
        if i == 0 {} else if i == 1 {} else if i == 2 {} else if i == 3 {} else if i == 4 {}
        else if i == 5 {} else if i == 6 {} else if i == 7 {} else if i == 8 {} else if i == 9 {} else {}
    }
    if reserved(b) {
        let i: int = if b == b';' {0} else if b == b'/' {1} else if b == b'?' {2}
            else if b == b':' {3} else if b == b'@' {4} else if b == b'&' {5}
            else if b == b'=' {6} else if b == b'+' {7} else if b == b'$' {8}
            else if b == b',' {9} else {10};
        assert(table[i] == b);
    }
}
}

// Attach a postcondition to the existing Option::and_then closure without
// replacing its body or the surrounding lookup and error conversion.
macro_rules! specify_octet_conversion {
    ($parse:ident($digits:ident).and_then(|$v:ident| $body:expr).ok_or($err:expr)) => {
        $parse($digits).and_then(
            #[verus_spec(r => ensures r == if $v <= 255 { Some($v as u8) } else { None })]
            |$v: u16| -> Option<u8> { $body }
        ).ok_or($err)
    };
}

verus! {
// Every actual &[u16] satisfies the index-arithmetic bound on decode.
// size_of_val witnesses that its byte count fits in a usize.
fn decode_any_slice(units: &[u16], component: bool) -> (r: Result<Vec<u16>, UriCodecError>)
    ensures result_text(r) == decode_from(units@, 0, component),
{
    let _bytes = core::mem::size_of_val(units);
    proof {
        broadcast use vstd::layout::layout_of_primitives;
        vstd::layout::layout_of_slices(units);
        assert(2 * units@.len() <= usize::MAX);
    }
    decode(units, component)
}

spec fn scalar(cp: u32) -> bool { cp <= 0x10ffff && !(0xd800 <= cp <= 0xdfff) }

spec fn hex_char(d: int) -> u16 {
    if d < 10 { (48 + d) as u16 } else { (55 + d) as u16 }
}

spec fn escaped_byte(b: int) -> Text {
    seq![37u16, hex_char(b / 16), hex_char(b % 16)]
}

// Canonical UTF-8, with every byte escaped (including ASCII). This encoder
// is mathematical and does not call the decoder or its width or tail rules.
spec fn escaped_scalar(cp: u32) -> Text {
    let n = cp as int;
    let last = escaped_byte(128 + n % 64);
    let next = escaped_byte(128 + (n / 64) % 64);
    if n < 128 { escaped_byte(n) }
    else if n < 2048 { escaped_byte(192 + n / 64) + last }
    else if n < 65536 { escaped_byte(224 + n / 4096) + next + last }
    else { escaped_byte(240 + n / 262144) + escaped_byte(128 + (n / 4096) % 64) + next + last }
}

spec fn escaped_text(points: Seq<u32>) -> Text
    decreases points.len(),
{
    if points.len() == 0 { seq![] }
    else { escaped_scalar(points[0]) + escaped_text(points.drop_first()) }
}

spec fn utf16_text(points: Seq<u32>) -> Text
    decreases points.len(),
{
    if points.len() == 0 { seq![] }
    else { utf16(points[0] as int) + utf16_text(points.drop_first()) }
}

proof fn read_escaped_byte(s: Text, i: int, b: int)
    requires 0 <= i, i + 3 <= s.len(), 0 <= b <= 255, s.subrange(i, i + 3) == escaped_byte(b),
    ensures octet(s, i) == Ok(b as u8),
{
    assert(s[i] == 37 && s[i + 1] == hex_char(b / 16) && s[i + 2] == hex_char(b % 16));
    assert(digit(s[i + 1]) == Some((b / 16) as u8));
    assert(digit(s[i + 2]) == Some((b % 16) as u8));
}

proof fn consume_byte(s: Text, i: int, b: int, left: int, cp: int, min: int)
    requires left > 0, 128 <= b < 192, octet(s, i) == Ok(b as u8),
    ensures scalar_tail(s, i, left, cp, min) == scalar_tail(s, i + 3, left - 1, 64 * cp + b - 128, min),
{
    reveal_with_fuel(scalar_tail, 2);
}

proof fn split_at(s: Text, i: int, a: Text, b: Text)
    requires 0 <= i, i + a.len() + b.len() <= s.len(),
        s.subrange(i, i + a.len() + b.len()) == a + b,
    ensures s.subrange(i, i + a.len()) == a, s.subrange(i + a.len(), i + a.len() + b.len()) == b,
{
    let whole = s.subrange(i, i + a.len() + b.len());
    assert forall|k: int| 0 <= k < a.len() implies s[i + k] == #[trigger] a[k] by {
        assert(whole[k] == (a + b)[k]);
    }
    assert forall|k: int| 0 <= k < b.len() implies s[i + a.len() + k] == #[trigger] b[k] by {
        assert(whole[a.len() + k] == (a + b)[a.len() + k]);
    }
    assert(s.subrange(i, i + a.len()) =~= a);
    assert(s.subrange(i + a.len(), i + a.len() + b.len()) =~= b);
}

proof fn read_ascii(s: Text, i: int, cp: u32)
    requires cp < 128, scalar(cp), 0 <= i, i + escaped_scalar(cp).len() <= s.len(),
        s.subrange(i, i + escaped_scalar(cp).len()) == escaped_scalar(cp),
    ensures decode_from(s, i, true) == append(utf16(cp as int),
        decode_from(s, i + escaped_scalar(cp).len(), true)),
{
    let n = cp as int;
    reveal_with_fuel(decode_from, 2);
    assert(s[i] == 37) by {
        assert(escaped_scalar(cp)[0] == 37);
    }
    read_escaped_byte(s, i, n);
}

proof fn read_two(s: Text, i: int, cp: u32)
    requires 128 <= cp < 2048, scalar(cp), 0 <= i, i + escaped_scalar(cp).len() <= s.len(),
        s.subrange(i, i + escaped_scalar(cp).len()) == escaped_scalar(cp),
    ensures decode_from(s, i, true) == append(utf16(cp as int),
        decode_from(s, i + escaped_scalar(cp).len(), true)),
{
    let n = cp as int;
    reveal_with_fuel(decode_from, 2);
    assert(s[i] == 37) by {
        assert(escaped_scalar(cp)[0] == 37);
    }
    split_at(s, i, escaped_byte(192 + n / 64), escaped_byte(128 + n % 64));
    read_escaped_byte(s, i, 192 + n / 64);
    read_escaped_byte(s, i + 3, 128 + n % 64);
    consume_byte(s, i + 3, 128 + n % 64, 1, n / 64, 128);
}

proof fn read_three(s: Text, i: int, cp: u32)
    requires 2048 <= cp < 65536, 0 <= i, i + escaped_scalar(cp).len() <= s.len(),
        s.subrange(i, i + escaped_scalar(cp).len()) == escaped_scalar(cp),
    ensures decode_from(s, i, true) == if 0xd800 <= cp <= 0xdfff {
        Err(UriCodecError::MalformedUtf8)
    } else { append(utf16(cp as int), decode_from(s, i + escaped_scalar(cp).len(), true)) },
{
    let n = cp as int;
    reveal_with_fuel(decode_from, 2);
    assert(s[i] == 37) by {
        assert(escaped_scalar(cp)[0] == 37);
    }
    split_at(s, i, escaped_byte(224 + n / 4096) + escaped_byte(128 + n / 64 % 64), escaped_byte(128 + n % 64));
    split_at(s, i, escaped_byte(224 + n / 4096), escaped_byte(128 + n / 64 % 64));
    read_escaped_byte(s, i, 224 + n / 4096);
    read_escaped_byte(s, i + 3, 128 + n / 64 % 64);
    read_escaped_byte(s, i + 6, 128 + n % 64);
    consume_byte(s, i + 3, 128 + n / 64 % 64, 2, n / 4096, 2048);
    consume_byte(s, i + 6, 128 + n % 64, 1, n / 64, 2048);
}

#[verifier::rlimit(35)]
proof fn combine_four(s: Text, i: int, cp: u32)
    requires 65536 <= cp <= 0x10ffff, 0 <= i < s.len(), s[i] == 37,
        octet(s, i) == Ok((240 + cp as int / 262144) as u8),
        octet(s, i + 3) == Ok((128 + cp as int / 4096 % 64) as u8),
        octet(s, i + 6) == Ok((128 + cp as int / 64 % 64) as u8),
        octet(s, i + 9) == Ok((128 + cp as int % 64) as u8),
    ensures decode_from(s, i, true) == append(utf16(cp as int), decode_from(s, i + 12, true)),
{
    let n = cp as int;
    assert(decode_from(s, i, true) == join(
        scalar_tail(s, i + 3, 3, n / 262144, 65536), decode_from(s, i + 12, true)
    )) by {
        let lead = (240 + n / 262144) as u8;
        assert(240 <= lead <= 244);
        assert(width(lead) == 4 && payload(lead) == n / 262144);
        reveal_with_fuel(decode_from, 1);
    }
    assert((n / 262144) * 64 + n / 4096 % 64 == n / 4096);
    assert((n / 4096) * 64 + n / 64 % 64 == n / 64);
    assert(n / 64 * 64 + n % 64 == n);
    consume_byte(s, i + 3, 128 + n / 4096 % 64, 3, n / 262144, 65536);
    consume_byte(s, i + 6, 128 + n / 64 % 64, 2, n / 4096, 65536);
    consume_byte(s, i + 9, 128 + n % 64, 1, n / 64, 65536);
    assert(scalar_tail(s, i + 12, 0, n, 65536) == Ok(utf16(n)));
}

proof fn read_four(s: Text, i: int, cp: u32)
    requires 65536 <= cp, scalar(cp), 0 <= i, i + escaped_scalar(cp).len() <= s.len(),
        s.subrange(i, i + escaped_scalar(cp).len()) == escaped_scalar(cp),
    ensures decode_from(s, i, true) == append(utf16(cp as int),
        decode_from(s, i + escaped_scalar(cp).len(), true)),
{
    let n = cp as int;
    reveal_with_fuel(decode_from, 2);
    assert(s[i] == 37) by {
        assert(escaped_scalar(cp)[0] == 37);
    }
    let lead = escaped_byte(240 + n / 262144);
    let second = escaped_byte(128 + n / 4096 % 64);
    let third = escaped_byte(128 + n / 64 % 64);
    let last = escaped_byte(128 + n % 64);
    split_at(s, i, lead + second + third, last);
    split_at(s, i, lead + second, third);
    split_at(s, i, lead, second);
    read_escaped_byte(s, i, 240 + n / 262144);
    read_escaped_byte(s, i + 3, 128 + n / 4096 % 64);
    read_escaped_byte(s, i + 6, 128 + n / 64 % 64);
    read_escaped_byte(s, i + 9, 128 + n % 64);
    combine_four(s, i, cp);
}

proof fn read_scalar(s: Text, i: int, cp: u32)
    requires scalar(cp), 0 <= i, i + escaped_scalar(cp).len() <= s.len(),
        s.subrange(i, i + escaped_scalar(cp).len()) == escaped_scalar(cp),
    ensures decode_from(s, i, true) == append(utf16(cp as int),
        decode_from(s, i + escaped_scalar(cp).len(), true)),
{
    if cp < 128 { read_ascii(s, i, cp); }
    else if cp < 2048 { read_two(s, i, cp); }
    else if cp < 65536 { read_three(s, i, cp); }
    else { read_four(s, i, cp); }
}

proof fn read_text(s: Text, i: int, points: Seq<u32>)
    requires
        0 <= i <= s.len(), s.subrange(i, s.len() as int) == escaped_text(points),
        forall|j: int| 0 <= j < points.len() ==> scalar(#[trigger] points[j]),
    ensures decode_from(s, i, true) == Ok(utf16_text(points)),
    decreases points.len(),
{
    reveal_with_fuel(escaped_text, 2);
    reveal_with_fuel(utf16_text, 2);
    if points.len() > 0 {
        assert(escaped_text(points) == escaped_scalar(points[0]) + escaped_text(points.drop_first()));
        assert(s.len() - i == escaped_text(points).len());
        let cp = points[0];
        let next = i + escaped_scalar(cp).len();
        split_at(s, i, escaped_scalar(cp), escaped_text(points.drop_first()));
        read_scalar(s, i, cp);
        let rest = points.drop_first();
        assert(s.subrange(next, s.len() as int) =~= escaped_text(rest));
        assert forall|j: int| 0 <= j < rest.len() implies scalar(#[trigger] rest[j]) by {
            assert(rest[j] == points[j + 1]);
        }
        read_text(s, next, rest);
    } else {
        assert(s.subrange(i, s.len() as int).len() == s.len() - i);
        assert(i == s.len());
        reveal_with_fuel(decode_from, 2);
    }
}

// A client of the production decoder, quantified over arbitrary sequences.
fn decode_canonical_component(units: &[u16], Ghost(points): Ghost<Seq<u32>>)
    -> (r: Result<Vec<u16>, UriCodecError>)
    requires
        units@ == escaped_text(points),
        forall|j: int| 0 <= j < points.len() ==> scalar(#[trigger] points[j]),
    ensures result_text(r) == Ok(utf16_text(points)),
{
    proof {
        assert(units@.subrange(0, units@.len() as int) =~= units@);
        read_text(units@, 0, points);
    }
    decode_any_slice(units, true)
}
}
