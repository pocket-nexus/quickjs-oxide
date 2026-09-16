# S16 / S17 / S20 implementation coverage review

This review checks the execution plan against reachable production code. It
contains no benchmark or Profile results. Final semantic gates and the joint
measurement must still establish correctness and performance on the final tree.

| Work item | Production path | Semantic coverage |
| --- | --- | --- |
| S16.1 receiver admission | `object/property_ic.rs::ordinary_receiver/locate`, used by the read and write site caches; exhaustive class admission also applies to holders | `exotic_named_storage_is_cached_but_typed_numeric_keys_are_not`, realm/runtime/proxy guard tests |
| S16.2 retained receiver gate | `ordinary_storage/ic.rs::try_property_ic_read_owned` gates cleanup only for replacing reads | `owned_ic_guards_borrow_deferred_work_and_final_receiver_before_promotion`; ownership argument in `architecture/property-cache-validity.md` |
| S16.3 polymorphism and revival | `PropertyReadCache::read/miss`: two guarded locations, move-to-front, 1024-step cooldown | `two_shapes_alternate_and_third_shape_eventually_revives` |
| S16.4 write IC | shared read/write site table → `run::PutField` → scalar cache leaf or `run/property::complete` → `try_property_ic_write_owned` → slot replacement; reference owners remain live outside RunSlots | `resident_property_writes_preserve_descriptors_elements_and_references`; own flags/revision/prototype guard tests; Array length explicitly excluded |
| S16.5 kept element reads and dense writes | GetArrayEl2/3 call `array_kept_immediate_read`, with resident owner fallback; PutArrayEl tries typed/dense scalar then `try_dense_array_write_owned` | same resident property matrix covers reference replacement, increment, freeze and array length |
| S16.6 global lookup | `environment_driver` calls `try_read_unresolved_global`; VarRef cell stores realm/atom/shape/revision/index and still reads the live value | `unresolved_global_leaf_observes_replacement_and_declines_accessors_and_tdz` |
| S17.1 append edges | `store_selected_property_slot` checks `append_transition` before copying shared layout; `replace_layout` recognizes remaining append-only callers; forward/reverse weak edges unlink on shape death or mutation | `append_edges_are_weak_and_unlinked_on_mutation_and_collection`, shared-shape tests |
| S17.2 small layouts/delete | unique threshold is one after transition installation; eligible ordinary deletion enters dictionary once and removes in place; exotic deletion keeps its descriptor-specific path | dictionary deletion/order/edge ownership tests |
| S17.3 hashing | small shape linear scan then Fx table; Atom writes packed index/generation; CollectionIndex identity-hashes already-keyed hashes; Map set does one borrowed record lookup; live record stores hash for removal; weak memo admits short strings | small-shape threshold test, short/long weak memo tests, SameValueZero and ordered collection reference-model tests |
| S17.4 pinned atoms | Runtime creates PinnedAtoms; static call sites use table selectors; Proxy method selector covers all trap names; ToPrimitive selects ToString/ValueOf; iterator index uses immediate key constructor | atom domain/lifetime suite, proxy and primitive conversion suites |
| S17.5 slice/copy | species/coercions finish before `try_copy_dense_slice`; heap publishes whole dense buffer after edge retain; copy cursor moves its key on local completion; own-key snapshots reserve capacity | `dense_slice_preserves_species_holes_descriptors_and_owned_cells`, object copy/accessor/order tests |
| S17 R10 fusion | redundant candidate prescan removed; target marking precedes the single candidate pass; flags allocate at first admitted span | fusion target-entry and update/store span tests |
| S20.1 context payload | NodeData::Context contains Box<ContextData>, all allocator/read/write/tracing paths consume that representation | `arena_layout_is_compact` pins actual arena size; the original approximate size was not an implementation requirement |
| S20.2 zero queue | `drain_zero_queue` takes SlotState once, hands the owned Node to `finish_node`, then recycles the slot | heap GC/weak/cycle suites |
| S20.3 edge storage | object/property/raw-value edge enumeration uses four inline entries with heap overflow, consumed by ownership and GC algorithms | `inline_and_spilled_edges_preserve_order`, GC oracle and cycles |
| S20.4 atom/release bookkeeping | `retain_raw_value_atoms` filters directly into the retained rollback list; `release_or_defer` defers immediately under an outstanding borrow and otherwise drains once | atom rollback/deferred ownership tests |
| S20.5 sparse IC table | bitmap and block ranks map instruction PC to reserved read/write site array | `sparse_site_rank_crosses_words_and_distinguishes_writes` |

Review found and repaired these omissions before final validation: remaining
Proxy trap names and ToPrimitive's two static names still went through interning;
non-storage append callers did not reach the shape transition helper; and the
migrated scalar write decline test had a placeholder PC rather than its PutField
site. Small-shape and short-string memo assertions were added. Read admission is
now an exhaustive ObjectKind match rather than an implicit wildcard.

S15's deferred DefineField/delete work is production-wired through the resident
property helper and the S17 layout transactions. It is covered by the same
property semantics test and documented in `architecture/resident-property-writes.md`.
Performance-counter targets remain targets, pending the single final run.
