//! Host-side compilation of classic script bundles for the core image loader.

pub use crate::engine::code::debug::DebugInfoMode;

/// Compile a classic script to an Oxide code image. The input is the exact
/// script bytes, without a package's trailing NUL terminator. Compilation
/// does not execute the script or install any application host bindings.
pub fn compile_script_image(source: &[u8], filename: &str) -> Result<Vec<u8>, String> {
    compile_script_image_with_debug(source, filename, DebugInfoMode::Full)
}

/// Compile an image using the selected source and position retention policy.
pub fn compile_script_image_with_debug(
    source: &[u8],
    filename: &str,
    debug: DebugInfoMode,
) -> Result<Vec<u8>, String> {
    let mut runtime = oxide_core::Runtime::new(oxide_core::RuntimeOptions::default())
        .map_err(|e| e.to_string())?;
    let script = publish_script(&mut runtime, source, filename, debug)?;
    oxide_core::bytecode::image::encode_script(&runtime, script).map_err(|e| e.to_string())
}

/// Publish classic script source directly into a core runtime. A host that
/// supports source fallback may call this after rejecting an image.
pub fn compile_script_for_runtime(
    runtime: &mut oxide_core::Runtime,
    source: &[u8],
    filename: &str,
) -> Result<oxide_core::ObjectId, String> {
    publish_script(runtime, source, filename, DebugInfoMode::Full)
}

fn publish_script(
    runtime: &mut oxide_core::Runtime,
    source: &[u8],
    filename: &str,
    debug: DebugInfoMode,
) -> Result<oxide_core::ObjectId, String> {
    use crate::engine::api::core::compile::ScriptCompileError;
    crate::engine::api::core::compile::compile_script(
        &mut runtime.compiler(),
        source,
        filename,
        debug,
    )
    .map(|script| script.function)
    .map_err(|error| match error {
        ScriptCompileError::Front(error) => error.to_string(),
        ScriptCompileError::Lower(error) => error.to_string(),
    })
}

/// Attach the source compiler used by dynamic JavaScript compilation.
pub fn install_dynamic_compiler(runtime: &mut oxide_core::Runtime) {
    runtime.set_compiler(crate::engine::api::core::compile::compile_for_core);
}
