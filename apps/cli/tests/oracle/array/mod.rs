// Keep the Array-method oracle implementations in separate modules so their
// private helpers remain isolated while Cargo builds one integration target.

mod support {
    pub(super) use crate::runtime_observation::{
        primitive_value_text, property_callable_with_read_context as property_callable,
        take_exception_object,
    };
    pub(super) use crate::runtime_oracle::{eval_callable, eval_object};
}

mod oracle_array_concat;
mod oracle_array_copy_within;
mod oracle_array_fill;
mod oracle_array_find;
mod oracle_array_flatten;
mod oracle_array_iteration;
mod oracle_array_map_filter;
mod oracle_array_mutators;
mod oracle_array_reduce;
mod oracle_array_reverse;
mod oracle_array_slice_splice;
mod oracle_array_sort;
mod oracle_array_stringification;
mod oracle_array_with;
