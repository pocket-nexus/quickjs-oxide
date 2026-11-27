#!/usr/bin/env python3
"""Run complementary Kani proofs on a temporary copy of the production core."""

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
CLIENTS = {
    "vm/bigint.rs": "words_machine.rs",
    "value.rs": "value_machine.rs",
    "heap/arena.rs": "arena_machine.rs",
    "heap/hash.rs": "hash_machine.rs",
    "heap/string.rs": "string_machine.rs",
    "builtins/base64.rs": "base64_machine.rs",
    "builtins/date/parse.rs": "date_parse_machine.rs",
    "builtins/date/format.rs": "date_format_machine.rs",
}

# The two larger bounded experiments are opt-in. Their proof runs have not
# completed. Keep their harnesses available for reproducing the open results.
CHECKED = ("word_arithmetic_contracts", "instruction_byte_contracts", "number_round_trip", "raw_numeric_projection",
           "boxed_observations", "every_raw_tag_and_reader", "initial_hash", "reused_slot_mark_epoch",
           "canonical_index_within_ten_units", "longer_text_is_never_an_index", "invalid_date_result")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--harness", action="append", default=[], help="choose a Kani harness")
    parser.add_argument("--all-features", action="store_true")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="oxide-machine-") as tmp:
        work = Path(tmp)
        # Cargo resolves the original workspace manifest and lockfile. Only
        # core/src receives proof clients. All build products go in the copy.
        for name in ("Cargo.toml", "Cargo.lock"):
            shutil.copyfile(REPO / name, work / name)
        for name in ("src", "tests", "examples", "adapters", "apps", "conformance", "regexp", "README.md"):
            (work / name).symlink_to(REPO / name)
        core = work / "core"
        shutil.copytree(REPO / "core" / "src", core / "src")
        shutil.copyfile(REPO / "core" / "Cargo.toml", core / "Cargo.toml")
        for module, client in CLIENTS.items():
            with (core / "src" / module).open("a") as out:
                out.write("\n" + (HERE / client).read_text())
        cmd = ["cargo", "kani", "-p", "quickjs-oxide-core", "--output-format", "terse"]
        if args.all_features:
            cmd.append("--all-features")
        for harness in (args.harness or CHECKED):
            cmd.extend(["--harness", harness])
        env = {**os.environ, "CARGO_TARGET_DIR": str(work / "target")}
        return subprocess.run(cmd, cwd=work, env=env).returncode


if __name__ == "__main__":
    raise SystemExit(main())
