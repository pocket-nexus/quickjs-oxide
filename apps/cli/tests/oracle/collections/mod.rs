// Keep the collection oracle implementations in separate modules so their
// private helpers remain isolated while Cargo builds one integration target.

mod oracle_map;
mod oracle_set;
mod oracle_weak_collections;
