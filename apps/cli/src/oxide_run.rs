//! `oxide-run`: runs a script on the new core (#122, phase 2, section 5).
//!
//! ```text
//! oxide-run [--disassemble] [--gc-stress=N] [--heap-cap-mib N] FILE
//! ```
//!
//! The script is compiled with the engine's front end, lowered to register
//! code and run with `print` on the global object. An uncaught exception
//! prints its string to stderr; a construct outside the phase 2 surface
//! prints `unsupported: <construct>`; both exit with status 1. `--gc-stress`
//! needs a build with the `new-core-gc-stress` feature.

use std::process::ExitCode;

use quickjs_oxide::engine::api::new_core::{self, NewCoreOptions};

const USAGE: &str = "usage: oxide-run [--disassemble] [--gc-stress=N] [--heap-cap-mib N] FILE";

struct Arguments {
    file: String,
    disassemble: bool,
    options: NewCoreOptions,
}

fn parse(mut args: impl Iterator<Item = String>) -> Result<Arguments, String> {
    let mut file = None;
    let mut disassemble = false;
    let mut options = NewCoreOptions::default();
    while let Some(arg) = args.next() {
        if arg == "--disassemble" {
            disassemble = true;
        } else if let Some(value) = arg.strip_prefix("--gc-stress=") {
            let interval = value
                .parse()
                .map_err(|_| format!("--gc-stress: not a count: {value}"))?;
            options.gc_stress = Some(interval);
        } else if arg == "--gc-stress" {
            options.gc_stress = Some(1);
        } else if arg == "--heap-cap-mib" {
            let value = args.next().ok_or("--heap-cap-mib needs a value")?;
            let mib: usize = value
                .parse()
                .map_err(|_| format!("--heap-cap-mib: not a count: {value}"))?;
            options.heap_cap = Some(mib << 20);
        } else if arg.starts_with('-') && arg != "-" {
            return Err(format!("unknown option {arg}"));
        } else if file.replace(arg).is_some() {
            return Err("only one file".into());
        }
    }
    Ok(Arguments {
        file: file.ok_or("no file")?,
        disassemble,
        options,
    })
}

fn main() -> ExitCode {
    if std::env::args().any(|arg| arg == "--help" || arg == "-h") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let arguments = match parse(std::env::args().skip(1)) {
        Ok(arguments) => arguments,
        Err(message) => {
            eprintln!("oxide-run: {message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let source = match std::fs::read(&arguments.file) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("oxide-run: {}: {error}", arguments.file);
            return ExitCode::from(2);
        }
    };
    let result = if arguments.disassemble {
        new_core::disassemble(&source, &arguments.file).map(|text| print!("{text}"))
    } else {
        new_core::run(&source, &arguments.file, &arguments.options)
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
