use std::process::Command;

#[test]
fn number_formatting_kernel_matches_pinned_quickjs() {
    let oracle = require_qjs_oracle!("Number formatting differential");

    let mut bits = vec![
        0_u64,
        1_u64 << 63,
        1,
        2,
        3,
        (1_u64 << 63) | 1,
        (1_u64 << 52) - 2,
        (1_u64 << 52) - 1,
        1_u64 << 52,
        (1_u64 << 52) + 1,
        0x3fb9_9999_9999_999a, // 0.1
        0x3fef_ffff_ffff_ffff, // previous binary64 before 1
        0x3ff0_0000_0000_0001, // next binary64 after 1
        0x3ff0_147a_e147_ae14, // 1.005
        0x4004_0000_0000_0000, // 2.5
        0x7fef_ffff_ffff_fffe,
        0x7fef_ffff_ffff_ffff,
        0xffef_ffff_ffff_ffff,
        f64::INFINITY.to_bits(),
        f64::NEG_INFINITY.to_bits(),
        f64::NAN.to_bits(),
    ];
    let mut state = 0x1319_8a2e_0370_7344_u64;
    for _ in 0..64 {
        state = state
            .wrapping_mul(2_862_933_555_777_941_757)
            .wrapping_add(3_037_000_493);
        bits.push(state);
    }

    let bit_literals = bits
        .iter()
        .map(|bits| format!("0x{bits:016x}n"))
        .collect::<Vec<_>>()
        .join(",");
    let source = format!(
        r#"
var storage = new ArrayBuffer(8);
var view = new DataView(storage);
var bits = [{bit_literals}];
var radices = [];
for (var radix = 2; radix <= 36; radix++) radices.push(radix);
var fixedDigits = [0,1,2,20,100];
var exponentialDigits = [-1,0,2,20,100];
var precisions = [-1,1,2,20,100];
for (var i = 0; i < bits.length; i++) {{
    view.setBigUint64(0, bits[i], true);
    var value = view.getFloat64(0, true);
    for (var j = 0; j < radices.length; j++) print(value.toString(radices[j]));
    for (var j = 0; j < fixedDigits.length; j++) print(value.toFixed(fixedDigits[j]));
    for (var j = 0; j < exponentialDigits.length; j++)
        print(exponentialDigits[j] < 0 ? value.toExponential() : value.toExponential(exponentialDigits[j]));
    for (var j = 0; j < precisions.length; j++)
        print(precisions[j] < 0 ? value.toPrecision() : value.toPrecision(precisions[j]));
}}
"#
    );
    let run = |engine: &std::ffi::OsStr| {
        let output = Command::new(engine)
            .args(["-e", &source])
            .output()
            .expect("run the Number formatting script");
        assert!(
            output.status.success(),
            "Number formatting script failed on {engine:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("Number formatting output is UTF-8")
    };
    assert_eq!(run(env!("CARGO_BIN_EXE_qjs").as_ref()), run(&oracle));
}
