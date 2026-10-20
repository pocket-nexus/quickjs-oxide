//! Atom-table size for a Test262-sized key set.
//!
//! Interns every identifier name and string-literal body of the given
//! JavaScript files (a superset of the property keys and names a run of those
//! files interns) and reports the table's size twice: for one runtime that
//! runs every file (the worst case for a strong table), and for the largest
//! single file with the harness prelude (one runtime per test).
//!
//! `cargo run --release -p quickjs-oxide-core --example atom_census -- <test262>/harness <test262>/test`

use std::fs;
use std::path::{Path, PathBuf};

use oxide_core::Heap;

fn collect(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        let mut entries: Vec<_> = fs::read_dir(path)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        entries.sort();
        for entry in entries {
            collect(&entry, out);
        }
    } else if path.extension().is_some_and(|extension| extension == "js") {
        out.push(path.to_path_buf());
    }
}

/// Identifier names and string-literal bodies; comments are skipped.
fn tokens(source: &str) -> Vec<&str> {
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let mut index = 0;
    let ident_start = |c: char| c.is_alphabetic() || c == '_' || c == '$';
    let ident_part = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    while index < bytes.len() {
        let rest = &source[index..];
        let c = rest.chars().next().unwrap();
        if rest.starts_with("//") {
            index += rest.find('\n').unwrap_or(rest.len());
        } else if rest.starts_with("/*") {
            index += rest.find("*/").map_or(rest.len(), |end| end + 2);
        } else if c == '"' || c == '\'' || c == '`' {
            let mut end = 1;
            let body = &rest.as_bytes()[1..];
            let mut escaped = false;
            for &byte in body {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == c as u8 || (byte == b'\n' && c != '`') {
                    break;
                }
                end += 1;
            }
            out.push(&rest[1..end.min(rest.len())]);
            index += (end + 1).min(rest.len());
        } else if ident_start(c) {
            let len = rest.find(|c: char| !ident_part(c)).unwrap_or(rest.len());
            out.push(&rest[..len]);
            index += len;
        } else {
            index += c.len_utf8();
        }
    }
    out
}

struct Size {
    atoms: usize,
    content: usize,
    bytes: usize,
}

fn measure(heap: &Heap) -> Size {
    let table = heap.atom_table();
    let content: usize = table
        .ids()
        .map(|id| heap.string(id).repr().content_bytes())
        .sum();
    Size {
        atoms: table.len(),
        content,
        // Records, contents and the table itself.
        bytes: table.len() * size_of::<oxide_core::heap::string::StringRecord>()
            + content
            + table.bytes(),
    }
}

fn main() {
    let args: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    assert!(
        args.len() == 2,
        "usage: atom_census <harness dir> <test dir>"
    );
    let mut harness = Vec::new();
    collect(&args[0], &mut harness);
    let mut tests = Vec::new();
    collect(&args[1], &mut tests);
    let prelude: Vec<String> = ["assert.js", "sta.js", "doneprintHandle.js"]
        .iter()
        .filter_map(|name| fs::read_to_string(args[0].join(name)).ok())
        .collect();

    let mut union = Heap::default();
    let mut token_count = 0usize;
    for file in harness.iter().chain(&tests) {
        let source = fs::read_to_string(file).unwrap_or_default();
        for token in tokens(&source) {
            token_count += 1;
            union.property_key(token).unwrap();
        }
    }
    let mut largest = (
        0,
        Size {
            atoms: 0,
            content: 0,
            bytes: 0,
        },
        PathBuf::new(),
    );
    let mut per_file = Vec::with_capacity(tests.len());
    for file in &tests {
        let mut heap = Heap::default();
        for source in prelude.iter().cloned().chain(fs::read_to_string(file).ok()) {
            for token in tokens(&source) {
                heap.property_key(token).unwrap();
            }
        }
        let size = measure(&heap);
        per_file.push(size.atoms);
        if size.atoms > largest.0 {
            largest = (size.atoms, size, file.clone());
        }
    }
    let all = measure(&union);
    per_file.sort_unstable();
    let quantile = |q: f64| per_file[((per_file.len() - 1) as f64 * q) as usize];
    println!(
        "files: {} test + {} harness, tokens: {token_count}",
        tests.len(),
        harness.len()
    );
    println!(
        "one runtime for every file: {} atoms, {} content bytes, {} bytes total ({:.1} MiB)",
        all.atoms,
        all.content,
        all.bytes,
        all.bytes as f64 / (1 << 20) as f64
    );
    println!(
        "largest single test with prelude: {} atoms, {} content bytes, {} bytes total ({})",
        largest.1.atoms,
        largest.1.content,
        largest.1.bytes,
        largest
            .2
            .strip_prefix(&args[1])
            .unwrap_or(&largest.2)
            .display()
    );
    println!(
        "atoms per test with prelude: median {}, p99 {}",
        quantile(0.5),
        quantile(0.99)
    );
}
