//! `oxide-run`: runs a script on the new core (#122, phase 2, section 5).
//!
//! ```text
//! oxide-run [--disassemble] [--gc-stress=N] [--heap-cap-mib N] [--ic-stats] [--gc-stats] [--test262] [--cannot-block] [--module] [--std] [-e EXPR] FILE... [-- ARG...]
//! ```
//!
//! Each script is compiled with the engine's front end, lowered to register
//! code and run with `print` on the global object; several files run in
//! order as separate scripts of one realm, and the job queue drains after
//! each. An uncaught exception
//! prints its string to stderr; a construct outside the phase 2 surface
//! prints `unsupported: <construct>`; both exit with status 1. `--gc-stress`
//! needs a build with the `new-core-gc-stress` feature; `--ic-stats`, which
//! prints the inline-cache report to stderr at exit, `new-core-ic-stats`.
//! `--gc-stats` prints the collector's statistics to stderr at exit.
//!
//! As qjs: `-e EXPR` runs `EXPR` after the files, `--std` defines `load`
//! and `read`, `console.log` is `print`, and `scriptArgs` holds the last
//! file (or `-e`) and the arguments after `--`.

use std::process::ExitCode;

use quickjs_oxide::engine::api::new_core::{self, NewCoreOptions};

const USAGE: &str = "usage: oxide-run [--disassemble] [--gc-stress=N] [--heap-cap-mib N] [--ic-stats] [--gc-stats] [--test262] [--cannot-block] [--module] [--std] [-e EXPR] FILE... [-- ARG...]";

struct Arguments {
    /// The scripts, run in order as separate scripts of one realm.
    files: Vec<String>,
    /// `-e`'s source, run after the files.
    eval: Option<String>,
    disassemble: bool,
    options: NewCoreOptions,
}

fn parse(mut args: impl Iterator<Item = String>) -> Result<Arguments, String> {
    let mut files = Vec::new();
    let mut eval = None;
    let mut script_args = Vec::new();
    let mut disassemble = false;
    let mut options = NewCoreOptions {
        services: Some(std::rc::Rc::new(
            quickjs_oxide_host::SystemHostServices::default(),
        )),
        ..NewCoreOptions::default()
    };
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
        } else if arg == "--ic-stats" {
            options.ic_stats = true;
        } else if arg == "--gc-stats" {
            options.gc_stats = true;
        } else if arg == "--test262" {
            options.test262 = true;
        } else if arg == "--cannot-block" {
            options.cannot_block = true;
        } else if arg == "--module" {
            options.module = true;
        } else if arg == "--std" {
            options.std = true;
        } else if arg == "-e" || arg == "--eval" {
            eval = Some(args.next().ok_or("-e needs an expression")?);
        } else if arg == "--" {
            script_args.extend(args.by_ref());
        } else if arg == "--heap-cap-mib" {
            let value = args.next().ok_or("--heap-cap-mib needs a value")?;
            let mib: usize = value
                .parse()
                .map_err(|_| format!("--heap-cap-mib: not a count: {value}"))?;
            options.heap_cap = Some(mib << 20);
        } else if arg.starts_with('-') && arg != "-" {
            return Err(format!("unknown option {arg}"));
        } else {
            files.push(arg);
        }
    }
    if files.is_empty() && eval.is_none() {
        return Err("no file".into());
    }
    let script = if eval.is_some() {
        "<cmdline>".to_owned()
    } else {
        files.last().cloned().unwrap_or_default()
    };
    script_args.insert(0, script);
    options.script_args = Some(script_args);
    Ok(Arguments {
        files,
        eval,
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
    let mut sources = Vec::new();
    for file in &arguments.files {
        match std::fs::read(file) {
            Ok(source) => sources.push((source, file.clone())),
            Err(error) => {
                eprintln!("oxide-run: {file}: {error}");
                return ExitCode::from(2);
            }
        }
    }
    if let Some(source) = &arguments.eval {
        sources.push((source.clone().into_bytes(), "<cmdline>".to_owned()));
    }
    let result = if arguments.disassemble {
        let (source, file) = sources.last().expect("a file");
        new_core::disassemble(source, file).map(|text| print!("{text}"))
    } else {
        new_core::run_scripts(&sources, &arguments.options)
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
