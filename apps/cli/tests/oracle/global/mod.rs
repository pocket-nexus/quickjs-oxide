// Keep the global-semantics oracle implementations in isolated modules while
// Cargo builds one integration target.

mod oracle_global_numeric_predicates;
mod oracle_global_this;
mod oracle_global_to_string_tag;
mod oracle_global_uri_codecs;
