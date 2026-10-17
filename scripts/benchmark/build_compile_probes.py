#!/usr/bin/env python3
"""Build the Oxide and pinned QuickJS front-end probes under one fresh output directory.

Oxide reuses build_compile_probe.py and its provenance receipts. QuickJS links a
small C probe against the pinned oracle's libquickjs.a. Benchmark corpora stay
outside the repository.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import time

from run import ROOT, command_output, digest


def run(command, cwd=None, log=None):
    if log is None:
        subprocess.run(command, cwd=cwd, check=True)
        return
    with log.open("wb") as stream:
        subprocess.run(command, cwd=cwd, stdout=stream, stderr=subprocess.STDOUT, check=True)


def require_new_directory(path):
    if path.exists():
        raise ValueError(f"output directory already exists: {path}")
    path.mkdir(parents=True)


def build_oxide(repo, output):
    target = output / "oxide"
    command = [sys.executable, str(ROOT / "scripts/benchmark/build_compile_probe.py"),
               "--repo", str(repo), "--output", str(target)]
    result = subprocess.run(command, check=True, capture_output=True, text=True)
    binary = Path(result.stdout.strip().splitlines()[-1])
    if not binary.is_file():
        raise ValueError(f"oxide probe build did not produce a binary: {binary}")
    return {"binary": str(binary), "build_log": str(target / "build.log")}


QUICKJS_PROBE_SOURCE = r"""/*
 * Pinned-QuickJS compile-only probe for the cross-engine front-end matrix.
 *
 * Measures JS_Eval(..., JS_EVAL_FLAG_COMPILE_ONLY) on one Script-goal source
 * file. Runtime and Context construction, source I/O and teardown stay outside
 * the timed interval, matching the public Oxide compile probe boundary.
 * Output is exactly one line: compile_ns:<integer>.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "quickjs.h"

static unsigned long long now_ns(void)
{
    struct timespec ts;
    if (clock_gettime(CLOCK_MONOTONIC, &ts) != 0) {
        fprintf(stderr, "clock_gettime failed\n");
        exit(2);
    }
    return (unsigned long long)ts.tv_sec * 1000000000ull + (unsigned long long)ts.tv_nsec;
}

static char *read_source(const char *path, size_t *length)
{
    FILE *file = fopen(path, "rb");
    if (file == NULL) {
        fprintf(stderr, "cannot open source: %s\n", path);
        exit(2);
    }
    if (fseek(file, 0, SEEK_END) != 0) {
        fprintf(stderr, "cannot seek source: %s\n", path);
        exit(2);
    }
    long size = ftell(file);
    if (size < 0 || fseek(file, 0, SEEK_SET) != 0) {
        fprintf(stderr, "cannot size source: %s\n", path);
        exit(2);
    }
    char *buffer = malloc((size_t)size + 1);
    if (buffer == NULL) {
        fprintf(stderr, "out of memory reading source\n");
        exit(2);
    }
    if (fread(buffer, 1, (size_t)size, file) != (size_t)size) {
        fprintf(stderr, "short read: %s\n", path);
        exit(2);
    }
    fclose(file);
    buffer[size] = '\0';
    *length = (size_t)size;
    return buffer;
}

int main(int argc, char **argv)
{
    if (argc == 2 && strcmp(argv[1], "--version") == 0) {
        puts("quickjs-compile-probe 1");
        return 0;
    }
    if (argc != 2) {
        fprintf(stderr, "usage: %s FILE\n", argv[0]);
        return 2;
    }

    size_t length = 0;
    char *source = read_source(argv[1], &length);

    JSRuntime *runtime = JS_NewRuntime();
    if (runtime == NULL) {
        fprintf(stderr, "JS_NewRuntime failed\n");
        return 2;
    }
    JSContext *context = JS_NewContext(runtime);
    if (context == NULL) {
        fprintf(stderr, "JS_NewContext failed\n");
        return 2;
    }

    unsigned long long started = now_ns();
    JSValue result = JS_Eval(context, source, length, argv[1],
                             JS_EVAL_TYPE_GLOBAL | JS_EVAL_FLAG_COMPILE_ONLY);
    unsigned long long elapsed = now_ns() - started;

    if (JS_IsException(result)) {
        JSValue exception = JS_GetException(context);
        const char *message = JS_ToCString(context, exception);
        fprintf(stderr, "compile error: %s\n", message != NULL ? message : "unknown");
        if (message != NULL) {
            JS_FreeCString(context, message);
        }
        JS_FreeValue(context, exception);
        JS_FreeValue(context, result);
        JS_FreeContext(context);
        JS_FreeRuntime(runtime);
        free(source);
        return 1;
    }

    printf("compile_ns:%llu\n", elapsed);
    JS_FreeValue(context, result);
    JS_FreeContext(context);
    JS_FreeRuntime(runtime);
    free(source);
    return 0;
}
"""


def build_quickjs(source, output, cc):
    include = source.resolve()
    library = include / "libquickjs.a"
    header = include / "quickjs.h"
    if not library.is_file() or not header.is_file():
        raise ValueError(f"--quickjs-source must contain quickjs.h and libquickjs.a: {include}")
    directory = output / "quickjs"
    directory.mkdir()
    probe_source = directory / "quickjs_compile_probe.c"
    probe_source.write_text(QUICKJS_PROBE_SOURCE)
    binary = directory / "quickjs-compile-probe"
    command = [cc, "-O2", "-std=gnu11", "-o", str(binary), str(probe_source),
               f"-I{include}", str(library), "-lm", "-lpthread", "-ldl"]
    run(command, log=directory / "build.log")
    receipt = {"command": command, "probe_source": str(probe_source), "probe_sha256": digest(probe_source),
               "quickjs_source": str(include), "quickjs_library_sha256": digest(library),
               "qjs_version": command_output([str(include / "qjs"), "--version"]),
               "cc_version": command_output([cc, "--version"]),
               "binary": str(binary), "binary_sha256": digest(binary)}
    (directory / "build.json").write_text(json.dumps(receipt, indent=2) + "\n")
    binary.with_suffix(".build.json").write_text(json.dumps(receipt, indent=2) + "\n")
    return {"binary": str(binary), "build": receipt}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True, help="new probe directory")
    parser.add_argument("--repo", type=Path, default=ROOT)
    parser.add_argument("--quickjs-source", type=Path, required=True,
                        help="pinned oracle source directory containing libquickjs.a")
    parser.add_argument("--skip-oxide", action="store_true")
    parser.add_argument("--cc", default="cc")
    args = parser.parse_args()
    output = args.output.resolve()
    try:
        require_new_directory(output)
        probes = {}
        if not args.skip_oxide:
            probes["oxide"] = build_oxide(args.repo.resolve(), output)
        probes["quickjs"] = build_quickjs(args.quickjs_source, output, args.cc)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        parser.error(str(error))
    receipt = {"schema": "oxide-compile-probes-v1",
               "created_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
               "repository": str(args.repo.resolve()), "probes": probes,
               "runner_sha256": digest(__file__)}
    (output / "probes.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps({name: probe["binary"] for name, probe in probes.items()}, indent=2))


if __name__ == "__main__":
    main()
