// Keep the Object oracle implementations in separate modules so their private
// helpers remain isolated while Cargo builds one integration target.

mod oracle_object_assign;
mod oracle_object_assignment;
mod oracle_object_descriptors;
mod oracle_object_enumeration;
mod oracle_object_extensibility;
mod oracle_object_from_entries;
mod oracle_object_group_by;
mod oracle_object_has_own;
mod oracle_object_integrity;
mod oracle_object_intrinsic;
mod oracle_object_is;
mod oracle_object_literals;
mod oracle_object_rest;
