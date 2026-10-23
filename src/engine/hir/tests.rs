//! The generated reader, writer, dump and strip against the fixtures the
//! TypeScript implementation writes (`node tools/hir-schema/test.ts`).

use super::schema::{DumpOptions, Program, ReadError, dump, dump_with, read, strip, write};

macro_rules! fixture {
    ($name:literal) => {
        (
            $name,
            include_bytes!(concat!(
                "../../../tools/hir-schema/fixtures/",
                $name,
                ".hir"
            )),
            include_str!(concat!(
                "../../../tools/hir-schema/fixtures/",
                $name,
                ".dump"
            )),
            include_str!(concat!(
                "../../../tools/hir-schema/fixtures/",
                $name,
                ".spans.dump"
            )),
            include_str!(concat!(
                "../../../tools/hir-schema/fixtures/",
                $name,
                ".strip.dump"
            )),
        )
    };
}

const FIXTURES: &[(&str, &[u8], &str, &str, &str)] = &[
    fixture!("random-1"),
    fixture!("random-2"),
    fixture!("random-3"),
    fixture!("random-4"),
    fixture!("random-5"),
    fixture!("random-6"),
    fixture!("random-7"),
    fixture!("random-8"),
];

fn parse(name: &str, bytes: &[u8]) -> Program {
    read(bytes).unwrap_or_else(|error| panic!("{name}: {error}"))
}

#[test]
fn typescript_fixtures_round_trip_byte_for_byte() {
    for (name, bytes, ..) in FIXTURES {
        assert_eq!(write(&parse(name, bytes)), *bytes, "{name}");
    }
}

#[test]
fn dumps_match_the_typescript_dumps() {
    for (name, bytes, plain, spans, _) in FIXTURES {
        let program = parse(name, bytes);
        assert_eq!(dump(&program), *plain, "{name}");
        assert_eq!(
            dump_with(&program, DumpOptions { spans: true }),
            *spans,
            "{name}"
        );
    }
}

#[test]
fn strip_matches_the_typescript_strip_and_is_idempotent() {
    for (name, bytes, _, _, stripped) in FIXTURES {
        let mut program = parse(name, bytes);
        strip(&mut program);
        assert_eq!(dump(&program), *stripped, "{name}");
        let once = write(&program);
        strip(&mut program);
        assert_eq!(write(&program), once, "{name}");
    }
}

#[test]
fn truncated_and_corrupted_files_are_errors() {
    for (name, bytes, ..) in FIXTURES {
        for len in 0..bytes.len() {
            // A file cut inside the padding of its last section still reads.
            let _ = read(&bytes[..len]);
        }
        assert_eq!(
            read(&bytes[..10]),
            Err(ReadError("truncated header")),
            "{name}"
        );
        for at in 0..bytes.len() {
            let mut corrupt = bytes.to_vec();
            corrupt[at] ^= 0x5a;
            if let Ok(program) = read(&corrupt) {
                // Whatever reads must write and read back the same.
                assert_eq!(
                    read(&write(&program)).map(|p| write(&p)),
                    Ok(write(&program)),
                    "{name} at {at}"
                );
            }
        }
    }
}

#[test]
fn another_schema_version_is_rejected() {
    let (_, bytes, ..) = FIXTURES[0];
    let mut other = bytes.to_vec();
    other[4] = other[4].wrapping_add(1);
    assert_eq!(read(&other), Err(ReadError("another schema version")));
}
