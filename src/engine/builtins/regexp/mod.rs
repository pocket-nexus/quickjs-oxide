//! Pinned QuickJS 2026-06-04 `RegExp` intrinsic.
//!
//! The pure matcher lives in [`crate::regexp`].  This module owns the
//! observable ECMAScript shell around it: realm-local constructor/prototype
//! identities, derived allocation, accessors, `lastIndex`, and match result
//! objects. RegExp literals, the legacy `compile` method, `RegExp.escape`,
//! and the standard RegExp Symbol protocols are linked; Unicode-sets (`v`)
//! syntax remains a separate parity slice.

mod compile;

pub(crate) use compile::{RegExpCompileResume, RegExpCompileStep};
mod constructor;

pub(crate) use constructor::{RegExpConstructorResume, RegExpConstructorStep};
mod escape;
mod exec;

pub(crate) use exec::{RegExpExecResume, RegExpExecStep};
mod iterator_next;

pub(crate) use iterator_next::{RegExpIteratorResume, RegExpIteratorStep};
mod match_all;
mod match_all_protocol;

pub(crate) use match_all_protocol::{RegExpMatchAllResume, RegExpMatchAllStep};
mod match_protocol;

pub(crate) use match_protocol::{RegExpMatchResume, RegExpMatchStep};
mod prototype;

pub(crate) use prototype::{RegExpPresentationResume, RegExpPresentationStep};
mod replace;

pub(crate) use replace::{RegExpReplaceResume, RegExpReplaceStep};
mod result;
mod search;

pub(crate) use search::{RegExpSearchResume, RegExpSearchStep};
mod species;

pub(crate) use species::{RegExpSpeciesResume, RegExpSpeciesStep};
mod split;

pub(crate) use split::{RegExpSplitResume, RegExpSplitStep};
#[cfg(test)]
mod tests;

use crate::engine::builtins::native::{RegExpFlagKind, RegExpNativeKind};
use crate::engine::heap::RegExpRealmData;

use crate::engine::{
    api::{runtime::Runtime, runtime_error::RuntimeError},
    atom::AtomIdx,
    builtins::native::NativeFunctionId,
    heap::ContextId,
    object::{
        ObjectRef, WellKnownSymbol,
        builtin_properties::{
            BuiltinEntry, METHOD_FLAGS, builtin_getter, method, species_getter, symbol_method,
            to_string_tag,
        },
        shape::{PropertyFlags, ShapeEntry},
    },
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};

impl Runtime {
    /// Install the linked subset of pinned `js_regexp_funcs` and
    /// `js_regexp_proto_funcs`.
    ///
    /// This routine creates `%RegExp.prototype%` as an ordinary object, then
    /// atomically attaches the constructor, prototype, and initial instance
    /// shape to `ContextData` after their public graph is complete.
    pub(crate) fn initialize_regexp_intrinsic(
        &self,
        realm: ContextId,
        function_prototype: &ObjectRef,
        object_prototype: &ObjectRef,
        iterator_prototype: &ObjectRef,
        global_object: &ObjectRef,
    ) -> Result<(), RuntimeError> {
        use NativeFunctionId::RegExp as F;
        use RegExpFlagKind as Flag;
        use RegExpNativeKind as K;
        use WellKnownSymbol as S;
        // Preserve pinned source-table order.  OrdinaryOwnPropertyKeys later
        // performs its own integer/String/Symbol grouping.
        const PROTOTYPE: &[BuiltinEntry] = &[
            builtin_getter!(F(K::Flags), "flags"),
            builtin_getter!(F(K::Source), "source"),
            builtin_getter!(F(K::Flag(Flag::Global)), "global"),
            builtin_getter!(F(K::Flag(Flag::IgnoreCase)), "ignoreCase"),
            builtin_getter!(F(K::Flag(Flag::Multiline)), "multiline"),
            builtin_getter!(F(K::Flag(Flag::DotAll)), "dotAll"),
            builtin_getter!(F(K::Flag(Flag::Unicode)), "unicode"),
            builtin_getter!(F(K::Flag(Flag::UnicodeSets)), "unicodeSets"),
            builtin_getter!(F(K::Flag(Flag::Sticky)), "sticky"),
            builtin_getter!(F(K::Flag(Flag::HasIndices)), "hasIndices"),
            method(F(K::Exec), "exec", 1, 1),
            method(F(K::Compile), "compile", 2, 2),
            method(F(K::Test), "test", 1, 1),
            method(F(K::ToString), "toString", 0, 0),
            symbol_method(
                S::Replace,
                F(K::Replace),
                "[Symbol.replace]",
                2,
                METHOD_FLAGS,
            ),
            symbol_method(S::Match, F(K::Match), "[Symbol.match]", 1, METHOD_FLAGS),
            symbol_method(
                S::MatchAll,
                F(K::MatchAll),
                "[Symbol.matchAll]",
                1,
                METHOD_FLAGS,
            ),
            symbol_method(S::Search, F(K::Search), "[Symbol.search]", 1, METHOD_FLAGS),
            symbol_method(S::Split, F(K::Split), "[Symbol.split]", 2, METHOD_FLAGS),
        ];
        const STRING_ITERATOR_PROTOTYPE: &[BuiltinEntry] = &[
            method(NativeFunctionId::RegExpStringIteratorNext, "next", 0, 0),
            to_string_tag("RegExp String Iterator"),
        ];
        const CONSTRUCTOR: &[BuiltinEntry] = &[
            method(F(K::Escape), "escape", 1, 1),
            species_getter(F(K::Species)),
        ];
        let regexp_prototype = self.new_object(Some(object_prototype))?;
        let regexp_string_iterator_prototype = self.new_object(Some(iterator_prototype))?;
        self.define_builtin_entries(
            &regexp_string_iterator_prototype,
            realm,
            STRING_ITERATOR_PROTOTYPE,
        )?;
        self.define_builtin_entries(&regexp_prototype, realm, PROTOTYPE)?;
        let constructor =
            self.new_native_builtin(function_prototype, realm, F(K::Constructor), 2, "RegExp", 2)?;
        self.define_builtin_entries(constructor.as_object(), realm, CONSTRUCTOR)?;

        // JS_NewCConstructor publishes the global before installing the two
        // constructor relationship properties.
        self.define_global_function(global_object, "RegExp", &constructor)?;
        self.define_constructor_relationship(&constructor, &regexp_prototype)?;

        // QuickJS retains a realm-local canonical shape for literal-created
        // RegExp objects.  Constructors with a custom derived prototype use a
        // shape with the same property layout but that explicit prototype.
        let last_index =
            self.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::LastIndex)?;
        let entries = [ShapeEntry {
            atom: AtomIdx::from_raw(last_index.atom().raw()),
            flags: PropertyFlags::data(true, false, false),
        }];
        let object_shape = self
            .0
            .state
            .borrow_mut()
            .get_or_create_shape(Some(regexp_prototype.object_id()), &entries)?;
        let mut result_shapes = Vec::new();
        for names in [
            &["index", "input", "groups"][..],
            &["index", "input", "groups", "indices"][..],
            &["groups"][..],
        ] {
            let keys = std::iter::once("length")
                .chain(names.iter().copied())
                .map(|name| self.intern_property_key(name))
                .collect::<Result<Vec<_>, _>>()?;
            let entries = keys
                .iter()
                .enumerate()
                .map(|(index, key)| ShapeEntry {
                    atom: AtomIdx::from_raw(key.atom().raw()),
                    flags: if index == 0 {
                        PropertyFlags::data(true, false, false)
                    } else {
                        PropertyFlags::data(true, true, true)
                    },
                })
                .collect::<Vec<_>>();
            let mut state = self.0.state.borrow_mut();
            let prototype = state.heap.context(realm)?.array_prototype;
            match state.get_or_create_shape(Some(prototype), &entries) {
                Ok(shape) => result_shapes.push(shape),
                Err(error) => {
                    for shape in result_shapes.into_iter().chain([object_shape]) {
                        let cleanup = state.heap.release_shape(shape)?;
                        state.apply_cleanup(cleanup)?;
                    }
                    return Err(error);
                }
            }
        }
        let result_shapes: [_; 3] = result_shapes
            .try_into()
            .expect("three RegExp result layouts");
        let mut state = self.0.state.borrow_mut();
        let attached = state.heap.attach_regexp_intrinsics(
            realm,
            RegExpRealmData {
                prototype: regexp_prototype.object_id(),
                constructor: constructor.as_object().object_id(),
                object_shape,
                result_shapes: Some(result_shapes),
                string_iterator_prototype: regexp_string_iterator_prototype.object_id(),
            },
            last_index.atom(),
        );
        // `get_or_create_shape` returned one construction reference. The
        // Context owns the durable edge only when `attached` succeeded; on
        // failure this release reclaims the unpublished shape instead.
        for shape in result_shapes.into_iter().chain([object_shape]) {
            let cleanup = state.heap.release_shape(shape)?;
            state.apply_cleanup(cleanup)?;
        }
        attached?;
        Ok(())
    }

    /// Typed top-level dispatcher for every native identity installed above.
    pub(crate) fn call_regexp_native(
        &self,
        realm: ContextId,
        kind: RegExpNativeKind,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        match kind {
            RegExpNativeKind::Constructor => {
                self.call_regexp_constructor(realm, invocation, arguments)
            }
            RegExpNativeKind::Escape => self.call_regexp_escape(realm, &invocation, arguments),
            RegExpNativeKind::Species => self.call_species_getter(&invocation),
            RegExpNativeKind::Source | RegExpNativeKind::Flags | RegExpNativeKind::Flag(_) => {
                self.call_regexp_accessor(realm, kind, invocation)
            }
            RegExpNativeKind::Exec | RegExpNativeKind::Test => {
                self.call_regexp_exec_native(realm, kind, invocation, arguments)
            }
            RegExpNativeKind::Compile => self.call_regexp_compile(realm, invocation, arguments),
            RegExpNativeKind::ToString => self.call_regexp_to_string(realm, invocation),
            RegExpNativeKind::Replace => {
                self.call_regexp_symbol_replace(realm, invocation, arguments)
            }
            RegExpNativeKind::Match => self.call_regexp_symbol_match(realm, invocation, arguments),
            RegExpNativeKind::MatchAll => {
                self.call_regexp_symbol_match_all(realm, invocation, arguments)
            }
            RegExpNativeKind::Search => {
                self.call_regexp_symbol_search(realm, invocation, arguments)
            }
            RegExpNativeKind::Split => self.call_regexp_symbol_split(realm, invocation, arguments),
        }
    }
}
