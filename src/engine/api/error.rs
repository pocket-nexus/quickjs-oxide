use crate::source::SourceSpan;
use std::error::Error as StdError;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    Syntax,
    Type,
    Reference,
    Range,
    /// A JavaScript-visible QuickJS `InternalError`, distinct from an engine
    /// invariant or implementation fault.
    JsInternal,
    Internal,
    Io,
    /// A conformance decision which cannot yet be made because compilation
    /// reached the current implementation frontier.
    ///
    /// This is deliberately not a JavaScript-visible native error kind. Public
    /// compiler and runtime APIs preserve this provenance so ordinary callers
    /// and conformance tooling cannot mistake an implementation gap for a
    /// conforming early error.
    Unsupported,
}

/// QuickJS native Error subclasses, in the same stable order as
/// `JSErrorEnum` in the 2026-06-04 baseline.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[repr(u8)]
pub enum NativeErrorKind {
    Eval,
    Range,
    Reference,
    Syntax,
    Type,
    Uri,
    Internal,
    Aggregate,
}

/// Raw bytes produced by QuickJS's stack-local `char buf[256]` native-error
/// formatter. The payload silently stops at 255 bytes; JavaScript String
/// construction observes only the prefix before the first formatted NUL.
#[derive(Clone, Eq, PartialEq)]
pub struct NativeErrorMessage {
    bytes: [u8; 256],
    len: usize,
}

impl fmt::Debug for NativeErrorMessage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("NativeErrorMessage")
            .field(&&self.bytes[..self.len])
            .finish()
    }
}

impl Default for NativeErrorMessage {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeErrorMessage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            bytes: [0; 256],
            len: 0,
        }
    }

    #[must_use]
    pub fn from_utf8(value: &str) -> Self {
        let mut message = Self::new();
        message.push_bytes(value.bytes());
        message
    }

    pub fn push_bytes(&mut self, bytes: impl IntoIterator<Item = u8>) {
        for byte in bytes {
            if self.len == self.bytes.len() - 1 {
                break;
            }
            self.bytes[self.len] = byte;
            self.len += 1;
        }
    }

    pub fn push_c_string_bytes(&mut self, bytes: impl IntoIterator<Item = u8>) {
        for byte in bytes {
            if byte == 0 || self.len == self.bytes.len() - 1 {
                break;
            }
            self.bytes[self.len] = byte;
            self.len += 1;
        }
    }

    pub fn push_utf8(&mut self, value: &str) {
        self.push_bytes(value.bytes());
    }

    #[must_use]
    pub fn visible_bytes(&self) -> &[u8] {
        let visible_len = self.bytes[..self.len]
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(self.len);
        &self.bytes[..visible_len]
    }
}

impl NativeErrorKind {
    pub const ALL: [Self; 8] = [
        Self::Eval,
        Self::Range,
        Self::Reference,
        Self::Syntax,
        Self::Type,
        Self::Uri,
        Self::Internal,
        Self::Aggregate,
    ];
    pub const COUNT: usize = Self::ALL.len();

    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Eval => "EvalError",
            Self::Range => "RangeError",
            Self::Reference => "ReferenceError",
            Self::Syntax => "SyntaxError",
            Self::Type => "TypeError",
            Self::Uri => "URIError",
            Self::Internal => "InternalError",
            Self::Aggregate => "AggregateError",
        }
    }

    /// Classify an engine-facing error kind which has JavaScript throw
    /// semantics. `Unsupported`, `Internal`, and `Io` are deliberately
    /// excluded: implementation frontiers and engine faults must not become
    /// catchable JavaScript exceptions through this conversion.
    #[must_use]
    pub const fn from_javascript_error(kind: ErrorKind) -> Option<Self> {
        match kind {
            ErrorKind::Syntax => Some(Self::Syntax),
            ErrorKind::Type => Some(Self::Type),
            ErrorKind::Reference => Some(Self::Reference),
            ErrorKind::Range => Some(Self::Range),
            ErrorKind::JsInternal => Some(Self::Internal),
            ErrorKind::Unsupported | ErrorKind::Internal | ErrorKind::Io => None,
        }
    }
}

/// An engine error. JavaScript exceptions will eventually carry a heap value;
/// this type is also usable before a context exists (lexer and decoder errors).
#[derive(Clone, Eq, PartialEq)]
pub struct Error(Box<ErrorData>);

// Keep diagnostic storage on the error path instead of widening every successful
// Result in the interpreter. This allocation never owns a runtime or JS edge.
#[derive(Clone, Eq, PartialEq)]
struct ErrorData {
    kind: ErrorKind,
    message: String,
    native_message: Option<Box<NativeErrorMessage>>,
    span: Option<SourceSpan>,
}

impl Error {
    #[must_use]
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        let message = message.into();
        Self(Box::new(ErrorData {
            kind,
            message,
            native_message: None,
            span: None,
        }))
    }

    #[must_use]
    pub fn from_native_message(kind: ErrorKind, native_message: NativeErrorMessage) -> Self {
        let message = native_message.to_utf8_lossy();
        Self(Box::new(ErrorData {
            kind,
            message,
            native_message: Some(Box::new(native_message)),
            span: None,
        }))
    }

    #[must_use]
    pub fn syntax(message: impl Into<String>, span: SourceSpan) -> Self {
        Self::new(ErrorKind::Syntax, message).with_span(span)
    }

    #[must_use]
    pub fn unsupported(message: impl Into<String>, span: SourceSpan) -> Self {
        Self::new(ErrorKind::Unsupported, message).with_span(span)
    }

    #[must_use]
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Internal, message)
    }

    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        self.0.kind
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.0.message
    }

    #[must_use]
    pub fn native_message(&self) -> Option<&NativeErrorMessage> {
        self.0.native_message.as_deref()
    }

    #[must_use]
    pub const fn span(&self) -> Option<SourceSpan> {
        self.0.span
    }

    #[must_use]
    pub const fn with_span(mut self, span: SourceSpan) -> Self {
        self.0.span = Some(span);
        self
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Error")
            .field("kind", &self.0.kind)
            .field("message", &self.0.message)
            .field("native_message", &self.0.native_message)
            .field("span", &self.0.span)
            .finish()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(span) = self.0.span {
            if self.0.kind == ErrorKind::JsInternal {
                write!(
                    formatter,
                    "InternalError at {}:{}: {}",
                    span.start.line, span.start.column, self.0.message
                )
            } else {
                write!(
                    formatter,
                    "{:?}Error at {}:{}: {}",
                    self.0.kind, span.start.line, span.start.column, self.0.message
                )
            }
        } else {
            match self.0.kind {
                ErrorKind::JsInternal => write!(formatter, "InternalError: {}", self.0.message),
                _ => write!(formatter, "{:?}Error: {}", self.0.kind, self.0.message),
            }
        }
    }
}

impl StdError for Error {}

#[cfg(test)]
mod tests {
    use super::{Error, ErrorKind, NativeErrorMessage};

    #[test]
    fn boxed_error_preserves_const_accessors_clone_and_thread_traits() {
        use crate::source::{SourceLocation, SourceSpan};

        // Compile this wrapper as const even though error construction remains
        // a runtime operation. All three existing const methods must stay usable.
        const fn attach_span(error: Error, span: SourceSpan) -> Error {
            let _ = (error.kind(), error.span());
            error.with_span(span)
        }
        fn assert_traits<
            T: Send + Sync + Unpin + std::panic::UnwindSafe + std::panic::RefUnwindSafe,
        >() {
        }
        assert_traits::<Error>();

        let original = Error::new(ErrorKind::JsInternal, "diagnostic");
        let span = SourceSpan::new(SourceLocation::new(10, 2, 3), SourceLocation::new(12, 2, 5));
        let located = attach_span(original.clone(), span);
        assert_eq!(original.span(), None);
        assert_eq!(located.span(), Some(span));
        assert_eq!(located.kind(), original.kind());
        assert_eq!(located.message(), original.message());
        assert_ne!(located, original);
        assert_eq!(located.clone(), located);
        assert_eq!(format!("{original}"), "InternalError: diagnostic");
        assert_eq!(format!("{located}"), "InternalError at 2:3: diagnostic");
    }

    #[test]
    fn boxed_error_keeps_hot_results_within_two_words() {
        use crate::engine::value::JsValue;
        use std::mem::size_of;

        assert_eq!(size_of::<Error>(), size_of::<usize>());
        assert!(size_of::<Result<bool, Error>>() <= 2 * size_of::<usize>());
        assert!(size_of::<Result<usize, Error>>() <= 2 * size_of::<usize>());
        assert!(size_of::<Result<JsValue, Error>>() <= size_of::<JsValue>());
        assert!(size_of::<Result<Option<JsValue>, Error>>() <= size_of::<JsValue>());
    }

    #[test]
    fn error_equality_and_debug_preserve_exact_native_payload() {
        let mut raw = NativeErrorMessage::new();
        raw.push_bytes([0x80, b'A']);
        let exact = Error::from_native_message(ErrorKind::Type, raw);
        let public = Error::new(ErrorKind::Type, "\u{fffd}");

        assert_eq!(exact.message(), "\u{fffd}");
        assert_ne!(exact, public);
        assert_ne!(format!("{exact:?}"), format!("{public:?}"));
        assert_eq!(
            format!("{exact:?}"),
            "Error { kind: Type, message: \"�\", native_message: Some(NativeErrorMessage([128, 65])), span: None }"
        );
        assert_eq!(format!("{exact}"), "TypeError: �");
        assert_eq!(exact.clone(), exact);
        assert_eq!(
            exact.native_message().unwrap().visible_bytes(),
            [0x80, b'A']
        );
        assert!(public.native_message().is_none());
    }
}
