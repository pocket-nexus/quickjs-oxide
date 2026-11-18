// The code image ABI is tied to the core that interprets the instructions.
// Hash source paths and contents, independently of target and build profile.
mod dispatch;
use std::{fs, path::Path};

fn fingerprint(path: &Path, hash: &mut u64) {
    let mut entries: Vec<_> = fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    entries.sort();
    for entry in entries {
        if entry.is_dir() {
            fingerprint(&entry, hash);
        } else if entry.extension().is_some_and(|ext| ext == "rs") {
            add(
                hash,
                entry.file_name().unwrap().to_str().unwrap().as_bytes(),
            );
            add(hash, &fs::read(&entry).unwrap());
        }
    }
}

fn add(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes.iter().copied().chain([0]) {
        *hash = (*hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
    }
}

fn main() {
    dispatch::image_dispatch();
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=Cargo.toml");
    let mut hash = 0xcbf2_9ce4_8422_2325;
    add(&mut hash, env!("CARGO_PKG_VERSION").as_bytes());
    fingerprint(Path::new("src"), &mut hash);
    add(&mut hash, &fs::read("dispatch.rs").unwrap());
    println!("cargo:rustc-env=OXIDE_IMAGE_FINGERPRINT={hash:016x}");
}
