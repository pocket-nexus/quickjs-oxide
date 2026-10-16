//! Symbol records.

use super::arena::Record;
use super::object::NONE;

#[derive(Clone, Copy, Debug)]
pub struct SymbolRecord {
    /// The description string, or [`NONE`].
    pub description: u32,
    /// Private names: never reported as own keys.
    pub private: bool,
}

impl Record for SymbolRecord {
    fn vacant() -> Self {
        SymbolRecord {
            description: NONE,
            private: false,
        }
    }
}
