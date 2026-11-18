use oxide_core::bytecode::image::{ImageBytes, ImageError, LoadError, ScriptImage};
use oxide_core::{Runtime, RuntimeOptions, Value};
use quickjs_oxide::engine::api::image::compile_script_image;
use std::rc::Rc;

#[test]
fn image_relocates_literals_onto_a_fresh_host_and_runs_jobs() {
    let source = br#"
        let first;
        function tag(strings) {
            if (first === undefined) first = strings;
            if (first !== strings || !Object.isFrozen(strings.raw) || !Object.isFrozen(strings))
                throw "template identity or descriptors";
            return strings[0] === undefined && strings.raw[0] === "\\unicode";
        }
        function work(n) {
            try {
                if (!/a+/i.test("Aa")) throw 1;
                return () => n + hostInput + (1n << 65n === 36893488147419103232n ? 1 : 0);
            } catch (err) { return () => -1; }
        }
        function template() { return tag`\unicode`; }
        let computed = work(10);
        Promise.resolve().then(() => {
            globalThis.result = template() && template() && "\uD800".charCodeAt(0) === 55296
                && /return/.test(work.toString()) ? computed() : -2;
        });
    "#;
    let image = compile_script_image(source, "bundle.js").unwrap();
    assert_eq!(image, compile_script_image(source, "bundle.js").unwrap());

    let mut runtime = Runtime::new(RuntimeOptions::default()).unwrap();
    let scratch = compile_script_image(
        b"globalThis.unrelatedName = 'unrelated literal';",
        "other.js",
    )
    .unwrap();
    let scratch = runtime.load_image(&scratch).unwrap();
    runtime.run_script(scratch).unwrap();

    let cx = &mut runtime.native_cx();
    let global = cx.global_object();
    let receiver = Value::object(global);
    let input = cx.heap_mut().property_key("hostInput").unwrap();
    assert!(cx.set(global, input, Value::int32(31), receiver).unwrap());

    let script = ScriptImage::decode(&image)
        .unwrap()
        .instantiate(&mut runtime)
        .unwrap();
    runtime.run_script(script).unwrap();
    runtime.run_jobs().unwrap();
    let cx = &mut runtime.native_cx();
    let result = cx.heap_mut().property_key("result").unwrap();
    assert_eq!(
        cx.get(global, result, receiver).unwrap().as_int32(),
        Some(42)
    );
}

#[test]
fn rejects_corrupt_or_incompatible_images_before_publication() {
    let image = compile_script_image(b"globalThis.flag = 1;", "bundle.js").unwrap();
    let mut runtime = Runtime::new(RuntimeOptions::default()).unwrap();
    for end in 0..image.len() {
        assert!(matches!(
            runtime.load_image(&image[..end]),
            Err(LoadError::Rejected(_))
        ));
    }
    for index in 0..image.len() {
        let mut corrupt = image.clone();
        corrupt[index] ^= 1;
        assert!(matches!(
            runtime.load_image(&corrupt),
            Err(LoadError::Rejected(_))
        ));
    }
    let mut old = image;
    old[4] = 1;
    assert!(matches!(
        runtime.load_image(&old),
        Err(LoadError::Rejected(ImageError::Version(1)))
    ));
    assert!(runtime.code().is_empty());
}

#[test]
fn execution_failure_occurs_after_successful_admission() {
    let image = compile_script_image(b"globalThis.ran = 1; throw 42;", "bundle.js").unwrap();
    let mut runtime = Runtime::new(RuntimeOptions::default()).unwrap();
    let script = runtime.load_image(&image).unwrap();
    assert_eq!(
        runtime.run_script(script).unwrap_err().0.as_int32(),
        Some(42)
    );
    let cx = &mut runtime.native_cx();
    let object = cx.global_object();
    let ran = cx.heap_mut().property_key("ran").unwrap();
    assert_eq!(
        cx.get(object, ran, Value::object(object))
            .unwrap()
            .as_int32(),
        Some(1)
    );
}

#[test]
fn compact_positions_preserve_the_source_backtrace() {
    use quickjs_oxide::engine::api::image::compile_script_for_runtime;

    let source = br#"
      function outer() {
          const action = () => {
              throw new Error("position");
          };
          return action();
      }
      outer();
    "#;
    let image = compile_script_image(source, "bundle.js").unwrap();
    let mut runtime = Runtime::new(RuntimeOptions::default()).unwrap();
    let script = runtime.load_image(&image).unwrap();
    let error = runtime.run_script(script).unwrap_err();
    let image_trace = runtime.thrown_summary(error.0).unwrap().stack.unwrap();

    let mut runtime = Runtime::new(RuntimeOptions::default()).unwrap();
    let script = compile_script_for_runtime(&mut runtime, source, "bundle.js").unwrap();
    let error = runtime.run_script(script).unwrap_err();
    let source_trace = runtime.thrown_summary(error.0).unwrap().stack.unwrap();
    assert!(image_trace.contains("bundle.js:4"));
    assert_eq!(image_trace, source_trace);
}

#[test]
fn shared_admission_reuses_exact_bytes_in_separate_realms() {
    let source = br#"
        function read() {
            const obj = { value: 42 };
            return obj.value;
        }
        read();
    "#;
    let image = compile_script_image(source, "bundle.js").unwrap();
    let backing = Rc::new(
        [&b"prefix"[..], &image, &b"suffix"[..]]
            .concat()
            .into_boxed_slice(),
    );
    let end = 6 + image.len();
    let prepared =
        ScriptImage::decode_shared(ImageBytes::new(backing.clone(), 6..end).unwrap()).unwrap();
    assert!(std::ptr::eq(prepared.bytes(), &backing[6..end]));

    for _ in 0..2 {
        let mut runtime = Runtime::new(RuntimeOptions::default()).unwrap();
        let script = prepared.clone().instantiate(&mut runtime).unwrap();
        assert_eq!(runtime.run_script(script).unwrap().as_int32(), Some(42));
    }
}

#[test]
fn repeated_image_frames_keep_the_same_instructions_and_keys() {
    let source = br#"
        (() => {
            function read(object) { return object.answer; }
            const object = { answer: 21 };
            return read(object) + read(object);
        })();
    "#;
    let image = compile_script_image(source, "bundle.js").unwrap();
    let mut runtime = Runtime::new(RuntimeOptions::default()).unwrap();
    let script = runtime.load_image(&image).unwrap();
    let prior = runtime.memory_report().code.instruction_bytes;
    let hold = runtime.hold(Value::object(script));
    assert_eq!(runtime.run_script(script).unwrap().as_int32(), Some(42));
    let cached = runtime.memory_report().code.instruction_bytes - prior;
    assert!((1..=32 * 1024).contains(&cached));
    runtime.heap_mut().collect();
    assert_eq!(runtime.run_script(script).unwrap().as_int32(), Some(42));
    runtime.release(hold);
}
