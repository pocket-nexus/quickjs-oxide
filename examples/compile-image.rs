//! Build-host exporter for a prebundled classic script.

use oxide_core::bytecode::image::ScriptImage;
use quickjs_oxide::engine::api::image::{DebugInfoMode, compile_script_image_with_debug};
use std::{env, fs, io, path::Path};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    let (input, output, debug) = match args.as_slice() {
        [input, output] => (input, output, DebugInfoMode::Full),
        [flag, input, output] if flag == "--strip-source" => {
            (input, output, DebugInfoMode::StripSource)
        }
        _ => {
            return Err(io::Error::other(
                "usage: compile-image [--strip-source] INPUT.js OUTPUT.oxbc",
            )
            .into());
        }
    };
    let source = fs::read(input)?;
    let filename = Path::new(input)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("<input>");
    let image =
        compile_script_image_with_debug(&source, filename, debug).map_err(io::Error::other)?;
    let verified = ScriptImage::decode_shared(image.into_boxed_slice().into())?;
    fs::write(output, verified.bytes())?;
    Ok(())
}
