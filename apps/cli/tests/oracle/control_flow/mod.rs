// Keep the control-flow oracle implementations in isolated modules while
// Cargo builds one integration target.

use crate::quickjs_argv_completion_oracle;

mod oracle_annex_b_statements;
mod oracle_for_in;
mod oracle_for_lexicals;
mod oracle_for_of;
mod oracle_try_catch_finally;
mod oracle_with;
