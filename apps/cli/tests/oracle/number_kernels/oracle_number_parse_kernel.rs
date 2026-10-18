use std::process::Command;

use quickjs_oxide::engine::api::core::JsString;

#[test]
fn number_parse_kernel_matches_pinned_quickjs() {
    let oracle = require_qjs_oracle!("Number parser differential");

    let mut inputs = [
        "",
        " ",
        "+",
        "-",
        ".",
        ".e2",
        "NaN",
        "Infinity",
        "+Infinitytail",
        "-Infinity and beyond",
        "0",
        "-0",
        "000",
        "-000tail",
        "0x10",
        "-0x10tail",
        "0b11",
        "10102",
        "zZ!",
        "1.25e2tail",
        "1.e2",
        ".5e+2",
        "1e",
        "1e+",
        "1e-",
        "9007199254740993",
        "9007199254740995",
        "300000000000000031025361333325263798273",
        "1.0000000000000001110223024625156540423631668090820313",
        "2.4703282292062327e-324",
        "2.4703282292062328e-324",
        "1.7976931348623158e308",
        "1.7976931348623159e308",
        "-1e-9999",
    ]
    .into_iter()
    .map(|value| JsString::try_from_utf8(value).unwrap())
    .collect::<Vec<_>>();
    inputs.extend([
        JsString::try_from_utf16([0x00a0, u16::from(b'1')]).unwrap(),
        JsString::try_from_utf16([0xfeff, u16::from(b'-'), u16::from(b'0')]).unwrap(),
        JsString::try_from_utf16([0x0085, u16::from(b'1')]).unwrap(),
        JsString::try_from_utf16([u16::from(b'1'), u16::from(b'2'), 0, u16::from(b'3')]).unwrap(),
        JsString::try_from_utf16([0xd800, u16::from(b'1')]).unwrap(),
        JsString::try_from_utf16([u16::from(b'1'), 0xd800]).unwrap(),
        JsString::try_from_utf8(&format!("1{}", "0".repeat(400))).unwrap(),
        JsString::try_from_utf8(&format!("0.{}1e10001", "0".repeat(10_000))).unwrap(),
        JsString::try_from_utf8(&format!("1{}e-10000", "0".repeat(10_000))).unwrap(),
    ]);

    let alphabet = b"0123456789abcdefXYZ+-.eE x";
    let mut state = 0xa409_3822_299f_31d0_u64;
    for index in 0..128 {
        state = next_random(state);
        let length = 1 + usize::try_from(state % 64).unwrap();
        let mut units = Vec::with_capacity(length);
        for position in 0..length {
            state = next_random(state);
            let unit = if (index + position) % 53 == 0 {
                0
            } else {
                u16::from(alphabet[usize::try_from(state % alphabet.len() as u64).unwrap()])
            };
            units.push(unit);
        }
        inputs.push(JsString::try_from_utf16(units).unwrap());
    }

    let mut radices = vec![0_i32];
    radices.extend(2..=36);
    radices.extend([1, 37, -2]);
    let string_literals = inputs
        .iter()
        .map(js_utf16_literal)
        .collect::<Vec<_>>()
        .join(",");
    let radix_literals = radices
        .iter()
        .map(i32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let source = format!(
        r#"
var inputs = [{string_literals}];
var radices = [{radix_literals}];
var storage = new ArrayBuffer(8);
var view = new DataView(storage);
function bits(value) {{
    if (Number.isNaN(value)) return "NaN";
    view.setFloat64(0, value, true);
    var text = view.getBigUint64(0, true).toString(16);
    return ("0000000000000000" + text).slice(-16);
}}
for (var i = 0; i < inputs.length; i++)
    for (var j = 0; j < radices.length; j++) print(bits(parseInt(inputs[i], radices[j])));
for (var i = 0; i < inputs.length; i++) print(bits(parseFloat(inputs[i])));
"#
    );
    // Keep the generated corpus out of argv: Linux CI has a substantially
    // smaller ARG_MAX than macOS.
    let path = std::env::temp_dir().join(format!(
        "quickjs-oxide-number-parse-{}.js",
        std::process::id()
    ));
    std::fs::write(&path, &source).expect("write the Number parser script");
    let run = |engine: &std::ffi::OsStr| {
        let output = Command::new(engine)
            .arg(&path)
            .output()
            .expect("run the Number parser script");
        assert!(
            output.status.success(),
            "Number parser script failed on {engine:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    };
    let (oxide, quickjs) = (run(env!("CARGO_BIN_EXE_qjs").as_ref()), run(&oracle));
    let _ = std::fs::remove_file(&path);
    assert!(
        oxide == quickjs,
        "parseInt/parseFloat differ from pinned QuickJS"
    );
}

const fn next_random(state: u64) -> u64 {
    state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407)
}

fn js_utf16_literal(value: &JsString) -> String {
    let mut output = String::from("\"");
    for unit in value.utf16_units() {
        output.push_str(&format!("\\u{unit:04x}"));
    }
    output.push('"');
    output
}
