// The native loop is the shared source. Create a second loop with image
// frame and instruction reads at the corresponding native operations.
use std::{env, fs, path::Path};

pub fn image_dispatch() {
    let path = "src/vm/interp/dispatch.rs";
    println!("cargo:rerun-if-changed={path}");
    let source = fs::read_to_string(path).unwrap();
    let mut image = String::new();
    let mut patches = [0; 10];
    for line in source.lines() {
        let text = line.trim_start();
        let indent = &line[..line.len() - text.len()];
        let (index, replacement) = match text {
            "pub(super) fn run_at(cx: &mut Cx<'_>, start: usize) -> Result<Value, Throw> {" => (
                0,
                "fn run_image_at(cx: &mut Cx<'_>, start: usize) -> Result<Value, Throw> {",
            ),
            "cx.static_code.get_native(id)" => (1, "cx.static_code.get(id)"),
            "let mut code: &[Insn] = &unit.code;" => {
                image.push_str(line);
                image.push('\n');
                (
                    2,
                    "let mut image = cx.static_code.image_execution(entry.unit);",
                )
            }
            "let mut ic_base = unit.ic_base as usize;" => {
                (3, "let mut ic_base = unit.cache_base(cx.heap);")
            }
            "ic_base = unit.ic_base as usize;" => (4, "ic_base = unit.cache_base(cx.heap);"),
            "let insn = &code[pc];" => (
                5,
                concat!(
                    "let decoded;\n",
                    "let insn = if let Some(image) = &image {\n",
                    "    decoded = image.get(pc).expect(\"an admitted image instruction\");\n",
                    "    &decoded\n",
                    "} else {\n",
                    "    &code[pc]\n",
                    "};",
                ),
            ),
            "if let Some(read) = code.get(pc).copied()" => (
                6,
                concat!(
                    "if let Some(read) = if let Some(image) = &image {\n",
                    "    image.get(pc)\n",
                    "} else {\n",
                    "    code.get(pc).copied()\n",
                    "}",
                ),
            ),
            "let (next, plan) = cx.static_code.call_entry_native(id);" => {
                (7, "let (next, plan) = cx.static_code.call_entry(id);")
            }
            "rare_op::<false>(cx, &frame, unit, *insn)" => {
                (9, "rare_op::<true>(cx, &frame, unit, *insn)")
            }
            _ => {
                image.push_str(line);
                image.push('\n');
                if let Some(id) = text
                    .strip_prefix("base = ")
                    .and_then(|line| line.strip_suffix(".base as usize;"))
                {
                    image.push_str(indent);
                    image.push_str("image = cx.static_code.image_execution(");
                    image.push_str(id);
                    image.push_str(".unit);\n");
                    patches[8] += 1;
                }
                continue;
            }
        };
        for line in replacement.lines() {
            image.push_str(indent);
            image.push_str(line);
            image.push('\n');
        }
        patches[index] += 1;
    }
    assert_eq!(
        patches,
        [1, 1, 1, 1, 3, 1, 1, 1, 3, 1],
        "native dispatch shape changed"
    );
    let output = env::var_os("OUT_DIR").unwrap();
    fs::write(Path::new(&output).join("image-dispatch.rs"), image).unwrap();
}
