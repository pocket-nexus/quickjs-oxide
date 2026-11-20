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
fn a_single_hot_loop_promotes_once_and_resumes_at_its_back_edge() {
    let image = compile_script_image(b"let n = 0; while (n < 20000) ++n; n;", "loop.js").unwrap();
    let mut rt = Runtime::new(RuntimeOptions::default()).unwrap();
    let script = rt.load_image(&image).unwrap();
    let before = rt.memory_report().code;
    assert_eq!(rt.run_script(script).unwrap().as_int32(), Some(20000));
    let after = rt.memory_report().code;
    assert_eq!(
        after.instruction_bytes - before.instruction_bytes,
        before.instructions * size_of::<oxide_core::Insn>()
    );
}

#[test]
fn mixed_frames_resume_after_getters_throws_and_generator_yields() {
    use quickjs_oxide::engine::api::image::compile_script_for_runtime;

    let mut rt = Runtime::new(RuntimeOptions::default()).unwrap();
    let setup = compile_script_for_runtime(
        &mut rt,
        b"function twice(f, x) { return f(x) + f(x); }",
        "source.js",
    )
    .unwrap();
    rt.run_script(setup).unwrap();
    let source = br#"
        function read(obj) { return obj.x; }
        function* flow() {
            yield twice(read, {x: 5});
            try { twice(read, {get x() { throw 17; }}); }
            catch (v) { yield v; }
            return 15;
        }
        function walk(n) {
            const g = flow();
            return g.next().value + g.next().value + g.next().value + n;
        }
        walk(1);
        walk(0);
    "#;
    let image = compile_script_image(source, "mixed.js").unwrap();
    let script = rt.load_image(&image).unwrap();
    assert_eq!(rt.run_script(script).unwrap().as_int32(), Some(42));
    let after = compile_script_for_runtime(&mut rt, b"twice(walk, 0)", "after.js").unwrap();
    assert_eq!(rt.run_script(after).unwrap().as_int32(), Some(84));
}

#[test]
fn a_unit_larger_than_the_decode_budget_keeps_running_from_the_image() {
    let mut source = String::from("var n = 0; for (let i = 0; i < 2; ++i) {");
    for _ in 0..4500 {
        source.push_str("++n;");
    }
    source.push_str("} n;");
    let image = compile_script_image(source.as_bytes(), "large.js").unwrap();
    let mut rt = Runtime::new(RuntimeOptions::default()).unwrap();
    let script = rt.load_image(&image).unwrap();
    let before = rt.memory_report().code.instruction_bytes;
    for _ in 0..2 {
        assert_eq!(rt.run_script(script).unwrap().as_int32(), Some(9000));
    }
    assert_eq!(rt.memory_report().code.instruction_bytes, before);
}

#[test]
fn one_verified_image_honors_each_runtimes_decode_policy() {
    use oxide_core::ImageDecodePolicy::{Adaptive, Eager, Packed};

    let bytes = compile_script_image(
        b"function f(o) { return o.answer; } let v = { answer: 42 }; f(v);",
        "policy.js",
    )
    .unwrap();
    let image = ScriptImage::decode(&bytes).unwrap();
    let mut packed_size = None;
    for policy in [Packed, Adaptive { max_bytes: 0 }, Eager] {
        let mut rt = Runtime::new_with_image_policy(RuntimeOptions::default(), policy).unwrap();
        assert_eq!(rt.image_decode_policy(), policy);
        let script = image.clone().instantiate(&mut rt).unwrap();
        let installed = rt.memory_report().code;
        let packed = *packed_size.get_or_insert(installed.instruction_bytes);
        let expanded = installed.instructions * size_of::<oxide_core::Insn>();
        assert_eq!(
            installed.instruction_bytes,
            packed + if policy == Eager { expanded } else { 0 }
        );
        assert_eq!(rt.run_script(script).unwrap().as_int32(), Some(42));
        assert_eq!(
            rt.memory_report().code.instruction_bytes,
            installed.instruction_bytes
        );
    }
}

#[test]
fn decoded_images_permanently_consume_their_share_of_the_runtime_budget() {
    use oxide_core::ImageDecodePolicy::{Adaptive, Packed};
    let sources: [&[u8]; 2] = [
        b"var n=0; while(n<9000) ++n; n;",
        b"var n=0; while(n<9000) { ++n; if(n<0) n=n*17+5; } n;",
    ];
    let images = sources.map(|s| compile_script_image(s, "phase.js").unwrap());
    let sizes = images.each_ref().map(|b| {
        let mut rt = Runtime::new_with_image_policy(RuntimeOptions::default(), Packed).unwrap();
        rt.load_image(b).unwrap();
        rt.memory_report().code.instructions * size_of::<oxide_core::Insn>()
    });
    assert_ne!(sizes[0], sizes[1]);
    let total = sizes.iter().sum::<usize>();
    for (budget, reverse) in [
        (*sizes.iter().max().unwrap(), false),
        (total - 1, true),
        (total, true),
    ] {
        let mut rt = Runtime::new_with_image_policy(
            RuntimeOptions::default(),
            Adaptive { max_bytes: budget },
        )
        .unwrap();
        let programs = images.each_ref().map(|b| {
            let script = rt.load_image(b).unwrap();
            let root = rt.hold(Value::object(script));
            (script, root)
        });
        let packed = rt.memory_report().code.instruction_bytes;
        let first = usize::from(reverse);
        for step in 0..4 {
            let index = (step % 2) ^ first;
            assert_eq!(
                rt.run_script(programs[index].0).unwrap().as_int32(),
                Some(9000)
            );
            assert_eq!(
                rt.memory_report().code.instruction_bytes - packed,
                if budget == total && step > 0 {
                    total
                } else {
                    sizes[first]
                }
            );
        }
        assert_eq!(rt.image_decode_policy(), Adaptive { max_bytes: budget });
        for (_, root) in programs {
            rt.release(root);
        }
    }
}

#[test]
fn nested_callback_keeps_its_callers_decoded_instructions_alive() {
    use oxide_core::ImageDecodePolicy::{Adaptive, Packed};
    let image = compile_script_image(
        br#"
        function inner(v) { for (var i=0; i<12000; ++i) v += 1; return v; }
        var outer=0; while(outer<12000) ++outer;
        outer + [1, 2].map(inner)[1];
    "#,
        "nested.js",
    )
    .unwrap();
    let options = RuntimeOptions::default();
    let mut probe = Runtime::new_with_image_policy(options, Packed).unwrap();
    probe.load_image(&image).unwrap();
    let sizes = probe
        .code()
        .units()
        .map(|(_, u)| u.instruction_count() * size_of::<oxide_core::Insn>())
        .collect::<Vec<_>>();
    let [inner, outer] = sizes[..] else {
        panic!("expected two functions")
    };
    let budget = outer.max(inner);
    let mut rt = Runtime::new_with_image_policy(options, Adaptive { max_bytes: budget }).unwrap();
    let script = rt.load_image(&image).unwrap();
    let packed = rt.memory_report().code.instruction_bytes;
    assert_eq!(rt.run_script(script).unwrap().as_int32(), Some(24002));
    assert_eq!(rt.memory_report().code.instruction_bytes - packed, outer);
}

#[test]
fn short_runs_decode_on_second_entry_and_keep_the_original_resident() {
    use oxide_core::ImageDecodePolicy::{Adaptive, Packed};
    let images = [b"var x=1; x+1;" as &[u8], b"var x=1; x=x+2; x=x+3; x;"]
        .map(|src| compile_script_image(src, "short-phase.js").unwrap());
    let mut size_probe = Runtime::new_with_image_policy(RuntimeOptions::default(), Packed).unwrap();
    for image in &images {
        size_probe.load_image(image).unwrap();
    }
    let sizes = size_probe
        .code()
        .units()
        .map(|(_, unit)| unit.instruction_count() * size_of::<oxide_core::Insn>())
        .collect::<Vec<_>>();
    assert!(sizes[0] < sizes[1]);
    let mut rt = Runtime::new_with_image_policy(
        RuntimeOptions::default(),
        Adaptive {
            max_bytes: sizes[1],
        },
    )
    .unwrap();
    let programs = images.each_ref().map(|image| {
        let script = rt.load_image(image).unwrap();
        let root = rt.hold(Value::object(script));
        (script, root)
    });
    let packed = rt.memory_report().code.instruction_bytes;
    for (phase, index) in [0, 1, 0].into_iter().enumerate() {
        for entry in 0..16 {
            assert_eq!(
                rt.run_script(programs[index].0).unwrap().as_int32(),
                Some([2, 6][index])
            );
            let expected = if phase == 0 && entry == 0 {
                0
            } else {
                sizes[0]
            };
            assert_eq!(rt.memory_report().code.instruction_bytes - packed, expected);
        }
    }
    for (_, root) in programs {
        rt.release(root);
    }
}

#[test]
fn source_frame_reentry_preserves_the_original_decoded_image() {
    use oxide_core::ImageDecodePolicy::{Adaptive, Packed};
    use quickjs_oxide::engine::api::image::compile_script_for_runtime;
    let image = compile_script_image(
        br#"
        function inner(v) { for (var i=0; i<12000; ++i) v += 1; return v; }
        var outer=0; while(outer<12000) ++outer;
        outer + sourceCallback(inner);
    "#,
        "image.js",
    )
    .unwrap();
    let options = RuntimeOptions::default();
    let mut probe = Runtime::new_with_image_policy(options, Packed).unwrap();
    probe.load_image(&image).unwrap();
    let sizes = probe
        .code()
        .units()
        .map(|(_, u)| u.instruction_count() * size_of::<oxide_core::Insn>())
        .collect::<Vec<_>>();
    let [inner, outer] = sizes[..] else {
        panic!("expected two functions")
    };
    assert_ne!(inner, outer);
    let mut rt = Runtime::new_with_image_policy(
        options,
        Adaptive {
            max_bytes: outer.max(inner),
        },
    )
    .unwrap();
    let callback = compile_script_for_runtime(
        &mut rt,
        b"function sourceCallback(f) { return [1,2].map(f)[1]; }",
        "source.js",
    )
    .unwrap();
    rt.run_script(callback).unwrap();
    let script = rt.load_image(&image).unwrap();
    let before = rt.memory_report().code.instruction_bytes;
    assert_eq!(rt.run_script(script).unwrap().as_int32(), Some(24002));
    assert_eq!(rt.memory_report().code.instruction_bytes - before, outer);
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
