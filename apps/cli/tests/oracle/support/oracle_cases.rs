// Test-case record shapes shared verbatim by several oracle modules.

pub(crate) struct GroupedCase {
    pub(crate) group: &'static str,
    pub(crate) description: &'static str,
    pub(crate) source: &'static str,
    pub(crate) expected: &'static str,
}

pub(crate) struct SuccessCase {
    pub(crate) description: &'static str,
    pub(crate) source: &'static str,
    pub(crate) expected_stdout: &'static str,
}

pub(crate) struct ExpectedCase {
    pub(crate) description: &'static str,
    pub(crate) source: &'static str,
    pub(crate) expected: &'static str,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct NativeErrorObservation {
    pub(crate) name: Vec<u16>,
    pub(crate) message: Vec<u16>,
}

#[derive(Debug)]
pub(crate) struct StartMatchCase {
    pub(crate) label: &'static str,
    pub(crate) pattern: Vec<u16>,
    pub(crate) flags: &'static str,
    pub(crate) input: Vec<u16>,
    pub(crate) start: usize,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CompileCase {
    pub(crate) label: &'static str,
    pub(crate) pattern: &'static str,
    pub(crate) flags: &'static str,
    pub(crate) expected: &'static str,
}

#[derive(Debug)]
pub(crate) struct ExpectedMatchCase {
    pub(crate) label: &'static str,
    pub(crate) pattern: Vec<u16>,
    pub(crate) flags: &'static str,
    pub(crate) input: Vec<u16>,
    pub(crate) expected: &'static str,
}
