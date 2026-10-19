//! Links `qjs` with `qjs.order` first in `.text`: the functions that realm
//! initialization, a script's compile and the loop's common paths execute
//! (`scripts/release/gen-symbol-order.py` writes the file). Placed together,
//! they touch few of the kernel's fault-around windows over the binary's file
//! pages. A name the binary no longer defines is ignored, so a stale file
//! orders less and never breaks the link. The names are the mangled names of
//! an `x86_64-unknown-linux-gnu` build, so the file is passed on that target
//! only.

use std::env;
use std::path::Path;

fn main() {
    let order = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("qjs.order");
    println!("cargo:rerun-if-changed={}", order.display());
    const LINKER: &str = "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER";
    println!("cargo:rerun-if-env-changed=CARGO_ENCODED_RUSTFLAGS");
    println!("cargo:rerun-if-env-changed={LINKER}");
    // The toolchain links this target with its bundled lld; a linker chosen
    // in the flags or the environment may not take the option.
    let flags = env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default();
    if env::var("TARGET").unwrap() != "x86_64-unknown-linux-gnu"
        || flags.contains("linker")
        || env::var_os(LINKER).is_some()
    {
        return;
    }
    println!(
        "cargo:rustc-link-arg-bins=-Wl,--symbol-ordering-file={}",
        order.display()
    );
    println!("cargo:rustc-link-arg-bins=-Wl,--no-warn-symbol-ordering");
}
