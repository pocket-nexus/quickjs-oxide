/// Read `QJS_ORACLE`, or print the standard skip line and return from the
/// enclosing test. The arguments format the skipped comparison's label.
macro_rules! require_qjs_oracle {
    ($($label:tt)+) => {
        match std::env::var_os("QJS_ORACLE") {
            Some(oracle) => oracle,
            None => {
                eprintln!(
                    "SKIP {}: set QJS_ORACLE to upstream qjs",
                    format_args!($($label)+)
                );
                return;
            }
        }
    };
}

/// [`require_qjs_oracle`] for comparisons that name the pinned upstream qjs.
macro_rules! require_pinned_qjs_oracle {
    ($($label:tt)+) => {
        match std::env::var_os("QJS_ORACLE") {
            Some(oracle) => oracle,
            None => {
                eprintln!(
                    "SKIP {}: set QJS_ORACLE to pinned upstream qjs",
                    format_args!($($label)+)
                );
                return;
            }
        }
    };
}

pub(crate) mod grouped_case_oracle;
pub(crate) mod object_graph_observation;
pub(crate) mod oracle_cases;
pub(crate) mod primitive_probe;
pub(crate) mod quickjs_argv_completion_oracle;
pub(crate) mod quickjs_array_completion_oracle;
pub(crate) mod quickjs_control_value_oracle;
pub(crate) mod quickjs_indexed_oracle;
pub(crate) mod quickjs_object_pattern_oracle;
pub(crate) mod quickjs_object_super_oracle;
pub(crate) mod quickjs_oracle;
pub(crate) mod quickjs_plain_eval_oracle;
pub(crate) mod quickjs_program_property_oracle;
pub(crate) mod quickjs_raw_source_oracle;
pub(crate) mod quickjs_string_result_oracle;
pub(crate) mod quickjs_syntax_diagnostic_oracle;
pub(crate) mod quickjs_typed_array_oracle;
pub(crate) mod regexp_probe_oracle;
pub(crate) mod runtime_completion_oracle;
pub(crate) mod runtime_observation;
pub(crate) mod string_value_cases;
