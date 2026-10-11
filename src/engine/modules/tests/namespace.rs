use super::*;

#[test]
fn namespace_cache_preserves_cycles_identity_and_live_cells() {
    let runtime = Runtime::new();
    let (loader, _, _) = MapModuleLoader::new([
        (
            "pkg/a.js",
            "export * as b from './b.js'; export let value = 1; export function bump() { value = 42; }",
        ),
        (
            "pkg/b.js",
            "export * as a from './a.js'; export const marker = 2;",
        ),
    ]);
    let _loader_registration = runtime.set_module_loader(loader);
    let mut context = runtime.new_context().expect("create context");
    let module = context
        .compile_module_with_filename(
            r#"
            import * as a from './a.js';
            import * as b from './b.js';
            globalThis.__namespaceA = a;
            globalThis.__namespaceB = b;
            globalThis.__namespaceBefore = a.value;
            a.bump();
            globalThis.__namespaceAfter = a.value;
            "#,
            "pkg/entry.js",
        )
        .unwrap();

    drop(context.execute_module(&module).unwrap());
    assert_script_true(
        &mut context,
        r#"
        __namespaceA.b === __namespaceB &&
        __namespaceB.a === __namespaceA &&
        __namespaceBefore === 1 && __namespaceAfter === 42 &&
        Object.getPrototypeOf(__namespaceA) === null &&
        Object.isExtensible(__namespaceA) === false &&
        Reflect.ownKeys(__namespaceA).slice(0, 3).join(',') === 'b,bump,value' &&
        Reflect.ownKeys(__namespaceA)[3] === Symbol.toStringTag
        "#,
    );
}

#[test]
fn namespace_prototype_set_writes_the_child_while_direct_set_rejects() {
    let runtime = Runtime::new();
    let (loader, _, _) = MapModuleLoader::new([("pkg/dep.js", "export let counter = 1;")]);
    let _loader_registration = runtime.set_module_loader(loader);
    let mut context = runtime.new_context().expect("create context");
    let module = context
        .compile_module_with_filename(
            "import * as ns from './dep.js'; globalThis.__ns = ns;",
            "pkg/entry.js",
        )
        .unwrap();

    drop(context.execute_module(&module).unwrap());
    assert_script_true(
        &mut context,
        r#"
        (() => {
            'use strict';
            const symbol = Symbol('s');
            for (const key of ['counter', 'brandNew', symbol]) {
                const child = Object.create(__ns);
                child[key] = 42;
                if (!Object.hasOwn(child, key) || child[key] !== 42) return false;
                const grandchild = Object.create(Object.create(__ns));
                grandchild[key] = 7;
                if (!Object.hasOwn(grandchild, key)) return false;
                if (!Reflect.set(Object.create(__ns), key, 9)) return false;
                if (Reflect.set(__ns, key, 1)) return false;
                if (Reflect.set(__ns, key, 1, {})) return false;
                try { __ns[key] = 1; return false; } catch (error) {
                    if (!(error instanceof TypeError)) return false;
                }
            }
            return __ns.counter === 1;
        })()
        "#,
    );
}

#[test]
fn self_namespace_import_export_keeps_the_preallocated_cell() {
    let runtime = Runtime::new();
    let (loader, _, _) = MapModuleLoader::new([(
        "pkg/self.js",
        "import * as self from './self.js'; export { self }; export const answer = 42;",
    )]);
    let _loader_registration = runtime.set_module_loader(loader);
    let mut context = runtime.new_context().expect("create context");
    let module = context
        .compile_module_with_filename(
            "import * as ns from './self.js'; globalThis.__selfNamespace = ns;",
            "pkg/entry.js",
        )
        .unwrap();

    drop(context.execute_module(&module).unwrap());
    assert_script_true(
        &mut context,
        "__selfNamespace.self === __selfNamespace && __selfNamespace.answer === 42",
    );
}

#[test]
fn failed_namespace_build_rolls_back_its_placeholder_for_retry() {
    let runtime = Runtime::new();
    let (loader, _, _) = MapModuleLoader::new([("pkg/dependency.js", "export const present = 1;")]);
    let _loader_registration = runtime.set_module_loader(loader);
    let mut context = runtime.new_context().expect("create context");
    let module = context
        .compile_module_with_filename(
            "export { absent as publicName } from './dependency.js';",
            "pkg/entry.js",
        )
        .unwrap();
    runtime
        .prepare_module_instance(module.raw, context.realm)
        .unwrap();

    for _ in 0..2 {
        assert_eq!(
            runtime.get_module_namespace(&module, context.realm),
            Err(RuntimeError::Exception)
        );
        assert!(matches!(
            runtime.module_record(module.raw).unwrap().namespace,
            ModuleNamespaceState::Empty
        ));
        assert!(matches!(
            context.take_exception().unwrap(),
            Some(Value::Object(_))
        ));
    }
}
