//! Resumable module linking over the existing DFS/SCC records.
use super::{ModuleBytecodeRef, ModuleDfsFrame, ModuleLinkDfs, ModuleLinkStatus};
use crate::engine::api::{runtime::Runtime, runtime_error::RuntimeError};
use crate::engine::heap::{ContextId, RawModuleLinkRealm, RawModuleRef, RawModuleTransition};
use crate::engine::object::CallableRef;
use crate::engine::value::JsValue;
use crate::engine::vm::Completion;

pub(crate) enum LinkStep {
    Complete(Completion),
    Call {
        realm: ContextId,
        callable: CallableRef,
        resume: Box<LinkResume>,
    },
}
pub(crate) struct LinkResume {
    runtime: Runtime,
    root: ModuleBytecodeRef,
    dfs: ModuleLinkDfs,
    frames: Vec<ModuleDfsFrame>,
    pending: Option<ModuleDfsFrame>,
    armed: bool,
}
impl LinkStep {
    pub(crate) fn start(
        runtime: &Runtime,
        module: RawModuleRef,
        initiating_realm: ContextId,
    ) -> Result<Self, RuntimeError> {
        let root = runtime.root_module(module)?;
        runtime.preflight_module_graph_for_link(module)?;
        runtime.prepare_module_instance(module, initiating_realm)?;
        match runtime.module_record(module)?.link_status {
            ModuleLinkStatus::Linked => {
                return Ok(LinkStep::Complete(Completion::Return(JsValue::Undefined)));
            }
            ModuleLinkStatus::Linking => {
                return Err(RuntimeError::Invariant(
                    "module linking was re-entered by the host",
                ));
            }
            ModuleLinkStatus::Poisoned => {
                return Err(RuntimeError::Invariant(
                    "module linking previously failed inside the engine",
                ));
            }
            ModuleLinkStatus::Unlinked => {}
        }
        let mut resume = Box::new(LinkResume {
            runtime: runtime.clone(),
            root,
            dfs: ModuleLinkDfs::new(),
            frames: Vec::new(),
            pending: None,
            armed: true,
        });
        resume
            .frames
            .push(runtime.enter_module_link_dfs(module, &mut resume.dfs)?);
        resume.advance()
    }
}
impl LinkResume {
    fn advance(mut self: Box<Self>) -> Result<LinkStep, RuntimeError> {
        let runtime = self.runtime.clone();
        let dfs = &mut self.dfs;
        let frames = &mut self.frames;
        while !frames.is_empty() {
            let dependency = {
                let frame = frames.last_mut().ok_or(RuntimeError::Invariant(
                    "module link call stack unexpectedly became empty",
                ))?;
                let dependency = frame.dependencies.get(frame.next_dependency).cloned();
                if dependency.is_some() {
                    frame.next_dependency += 1;
                }
                dependency
            };
            if let Some(dependency) = dependency {
                match runtime.module_record(dependency)?.link_status {
                    ModuleLinkStatus::Linked => {}
                    ModuleLinkStatus::Linking => {
                        let dependency_ancestor = dfs
                            .entries
                            .get(&dependency.module)
                            .map(|entry| entry.ancestor)
                            .ok_or(RuntimeError::Invariant(
                                "linking dependency has no DFS entry",
                            ))?;
                        let current_id = frames.last().map(|frame| frame.module.module).ok_or(
                            RuntimeError::Invariant(
                                "module link call stack unexpectedly became empty",
                            ),
                        )?;
                        let entry = dfs
                            .entries
                            .get_mut(&current_id)
                            .ok_or(RuntimeError::Invariant("linking module lost its DFS entry"))?;
                        entry.ancestor = entry.ancestor.min(dependency_ancestor);
                    }
                    ModuleLinkStatus::Unlinked => {
                        frames.push(runtime.enter_module_link_dfs(dependency, dfs)?);
                    }
                    ModuleLinkStatus::Poisoned => {
                        return Err(RuntimeError::Invariant(
                            "module linking previously failed inside the engine",
                        ));
                    }
                }
                continue;
            }

            let frame = frames.pop().ok_or(RuntimeError::Invariant(
                "module link call stack unexpectedly became empty",
            ))?;
            let realm = runtime
                .module_record(frame.module)?
                .link_realm
                .map(|realm| match realm {
                    RawModuleLinkRealm::Cache => frame.module.cache,
                    RawModuleLinkRealm::Other(realm) => realm,
                })
                .ok_or(RuntimeError::Invariant(
                    "instantiated module has no retained link realm",
                ))?;
            runtime.validate_module_indirect_exports(frame.module, &frame.dependencies, realm)?;
            runtime.link_module_imports(frame.module, &frame.dependencies, realm)?;
            if let Some(callable) = runtime.create_module_callable(frame.module, realm)? {
                self.pending = Some(frame);
                return Ok(LinkStep::Call {
                    realm,
                    callable,
                    resume: self,
                });
            }
            Self::finish_frame(
                &runtime,
                dfs,
                frames,
                frame,
                Completion::Return(JsValue::Undefined),
            )?;
        }
        if !dfs.stack.is_empty() {
            return Err(RuntimeError::Invariant(
                "successful module linking retained an SCC stack",
            ));
        }
        self.armed = false;
        Ok(LinkStep::Complete(Completion::Return(JsValue::Undefined)))
    }
    fn finish_frame(
        runtime: &Runtime,
        dfs: &mut ModuleLinkDfs,
        frames: &mut [ModuleDfsFrame],
        frame: ModuleDfsFrame,
        completion: Completion,
    ) -> Result<(), RuntimeError> {
        match completion {
            Completion::Return(JsValue::Undefined) => {
                let entry = dfs
                    .entries
                    .get(&frame.module.module)
                    .copied()
                    .ok_or(RuntimeError::Invariant("linked module lost its DFS entry"))?;
                if entry.index == entry.ancestor {
                    loop {
                        let member = *dfs
                            .stack
                            .last()
                            .ok_or(RuntimeError::Invariant("module link SCC stack underflow"))?;
                        let member = RawModuleRef {
                            cache: frame.module.cache,
                            module: member,
                        };
                        if !matches!(
                            runtime.module_record(member)?.link_status,
                            ModuleLinkStatus::Linking
                        ) {
                            return Err(RuntimeError::Invariant(
                                "module link SCC contained a non-linking member",
                            ));
                        }
                        runtime
                            .transition_module_record(member, RawModuleTransition::FinishLink)?;
                        let popped = dfs.stack.pop().ok_or(RuntimeError::Invariant(
                            "module link SCC stack underflow after publication",
                        ))?;
                        if popped != member.module {
                            return Err(RuntimeError::Invariant(
                                "module link SCC stack changed during record publication",
                            ));
                        }
                        if member.module == frame.module.module {
                            break;
                        }
                    }
                }
            }
            Completion::Return(_) => {
                runtime.transition_module_record(frame.module, RawModuleTransition::PoisonLink)?;
                return Err(RuntimeError::Invariant(
                    "module link entry returned a non-undefined value",
                ));
            }
            Completion::Throw(exception) => {
                runtime.set_pending_exception_jsvalue(exception)?;
                return Err(RuntimeError::Exception);
            }
        }

        if matches!(
            runtime.module_record(frame.module)?.link_status,
            ModuleLinkStatus::Linking
        ) {
            let dependency_ancestor = dfs
                .entries
                .get(&frame.module.module)
                .map(|entry| entry.ancestor)
                .ok_or(RuntimeError::Invariant(
                    "linking dependency has no DFS entry",
                ))?;
            if let Some(parent) = frames.last() {
                let entry = dfs
                    .entries
                    .get_mut(&parent.module.module)
                    .ok_or(RuntimeError::Invariant("linking module lost its DFS entry"))?;
                entry.ancestor = entry.ancestor.min(dependency_ancestor);
            }
        }
        Ok(())
    }
    pub(crate) fn resume(
        mut self: Box<Self>,
        completion: Completion,
    ) -> Result<LinkStep, RuntimeError> {
        let frame = self
            .pending
            .take()
            .ok_or(RuntimeError::Invariant("module link has no pending prefix"))?;
        Self::finish_frame(
            &self.runtime,
            &mut self.dfs,
            &mut self.frames,
            frame,
            completion,
        )?;
        self.advance()
    }
}
impl Drop for LinkResume {
    fn drop(&mut self) {
        if self.runtime.skip_cleanup() {
            return;
        }
        let _unwind = self.runtime.unwind_guard();
        if !self.armed {
            return;
        }
        // An abandoned or engine-failed active prefix cannot be replayed.
        if let Some(frame) = self.pending.take() {
            if self
                .runtime
                .transition_module_record(frame.module, RawModuleTransition::PoisonLink)
                .is_err()
            {
                self.runtime.0.poisoned.set(true);
                return;
            }
        }
        for id in &self.dfs.stack {
            let member = RawModuleRef {
                cache: self.root.raw.cache,
                module: *id,
            };
            let record = match self.runtime.module_record(member) {
                Ok(record) => record,
                // Construction rollback leaves stable canceled identities in
                // the cache; they have no active link state to restore.
                Err(RuntimeError::AbortedModule) => continue,
                Err(_) => {
                    self.runtime.0.poisoned.set(true);
                    return;
                }
            };
            if matches!(record.link_status, ModuleLinkStatus::Linking)
                && self
                    .runtime
                    .transition_module_record(member, RawModuleTransition::ResetLink)
                    .is_err()
            {
                self.runtime.0.poisoned.set(true);
                return;
            }
        }
    }
}

pub(crate) fn resume_reply(
    runtime: &Runtime,
    result: Result<LinkStep, RuntimeError>,
) -> Result<LinkStep, RuntimeError> {
    match result {
        Err(RuntimeError::Exception) => {
            let reason = runtime
                .take_pending_exception()?
                .ok_or(RuntimeError::Invariant(
                    "module link exception has no pending value",
                ))?;
            Ok(LinkStep::Complete(Completion::Throw(
                runtime.into_jsvalue(reason)?,
            )))
        }
        result => result,
    }
}

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<LinkStep>() <= 64);

#[cfg(all(test, not(target_family = "wasm"), panic = "unwind"))]
mod quarantine_tests {
    use super::*;
    use crate::engine::heap::ModuleId;
    use crate::engine::value::Value;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    #[test]
    fn link_resume_abandonment_respects_quarantine() {
        let Ok(case) = std::env::var("QJS_LINK_RESUME_CHILD") else {
            for case in [
                "normal",
                "poison",
                "unwind",
                "error",
                "probe-error",
                "aborted",
            ] {
                let status = std::process::Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "engine::modules::link::quarantine_tests::link_resume_abandonment_respects_quarantine",
                        "--nocapture",
                    ])
                    .env("QJS_LINK_RESUME_CHILD", case)
                    .status()
                    .unwrap();
                assert!(status.success(), "link cleanup {case}: {status}");
            }
            return;
        };
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let module = context.compile_module("export let value = 1;").unwrap();
        runtime
            .prepare_module_instance(module.raw, context.realm)
            .unwrap();
        runtime
            .transition_module_record(module.raw, RawModuleTransition::BeginLink)
            .unwrap();
        let mut dfs = ModuleLinkDfs::new();
        dfs.stack.push(module.raw.module);
        let mut resume = LinkResume {
            runtime: runtime.clone(),
            root: module.try_clone().unwrap(),
            dfs,
            frames: Vec::new(),
            pending: None,
            armed: true,
        };
        let count = runtime
            .0
            .state
            .borrow()
            .heap
            .context_strong_count(module.raw.cache)
            .unwrap();
        match case.as_str() {
            "normal" | "aborted" => {
                if case == "aborted" {
                    let canceled = context
                        .compile_module_with_filename("export let other = 2;", "canceled.js")
                        .unwrap();
                    let mut state = runtime.0.state.borrow_mut();
                    let cleanup = state
                        .heap
                        .unpublish_loaded_modules(canceled.raw.cache, &[canceled.raw.module])
                        .unwrap();
                    state.apply_cleanup(cleanup).unwrap();
                    resume.dfs.stack.insert(0, canceled.raw.module);
                }
                drop(resume);
                assert!(!runtime.is_poisoned());
                assert!(matches!(
                    runtime.module_record(module.raw).unwrap().link_status,
                    ModuleLinkStatus::Unlinked
                ));
                assert_eq!(context.eval("1 + 2").unwrap(), Value::number(3.0));
            }
            "poison" | "unwind" => {
                let state = runtime.0.state.borrow_mut();
                if case == "poison" {
                    runtime.0.poisoned.set(true);
                    drop(resume);
                } else {
                    assert!(
                        catch_unwind(AssertUnwindSafe(|| {
                            let _resume = resume;
                            panic!("injected link abandonment panic");
                        }))
                        .is_err()
                    );
                }
                assert!(runtime.is_poisoned());
                assert!(matches!(
                    state.heap.loaded_module(module.raw).unwrap().link_status,
                    ModuleLinkStatus::Linking
                ));
                assert_eq!(
                    state.heap.context_strong_count(module.raw.cache).unwrap(),
                    count
                );
                drop(state);
                assert!(matches!(runtime.new_context(), Err(RuntimeError::Poisoned)));
            }
            "error" | "probe-error" => {
                // A corrupt pending record fails before the still-valid SCC
                // member is reset or the guard's cache root is released.
                if case == "error" {
                    resume.pending = Some(ModuleDfsFrame {
                        module: RawModuleRef {
                            cache: module.raw.cache,
                            module: ModuleId(usize::MAX),
                        },
                        dependencies: Vec::new(),
                        next_dependency: 0,
                    });
                } else {
                    resume.dfs.stack.insert(0, ModuleId(usize::MAX));
                }
                drop(resume);
                assert!(runtime.is_poisoned());
                let state = runtime.0.state.borrow();
                assert!(matches!(
                    state.heap.loaded_module(module.raw).unwrap().link_status,
                    ModuleLinkStatus::Linking
                ));
                assert_eq!(
                    state.heap.context_strong_count(module.raw.cache).unwrap(),
                    count
                );
                drop(state);
                assert!(matches!(runtime.new_context(), Err(RuntimeError::Poisoned)));
            }
            _ => panic!("unknown link cleanup case"),
        }
    }
}
