#!/usr/bin/env python3
"""Run each Rust oracle test in a separate process; never lose tests to SIGABRT.

Build once with cargo test --locked -p quickjs-oxide-cli --test oracle --no-run,
then pass the exact debug test executable to --binary. No builds are performed.
Receipts are diagnostic evidence, not a replacement for the workspace gates.
"""

import argparse
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import time


SUMMARY = re.compile(
    r"test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; "
    r"(\d+) ignored; (\d+) measured; (\d+) filtered out;"
)


def execute(command, env, timeout):
    start = time.monotonic()
    with subprocess.Popen(
        command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        env=env, start_new_session=True,
    ) as process:
        timed_out = False
        try:
            output, _ = process.communicate(timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
            os.killpg(process.pid, signal.SIGKILL)
            output, _ = process.communicate()
        return process.returncode, output.decode("utf-8", errors="replace"), timed_out, time.monotonic() - start


def classify(name, code, output, timed_out):
    if timed_out:
        return "timeout"
    if code < 0:
        return "abort" if code == -signal.SIGABRT else "signal"
    summaries = SUMMARY.findall(output)
    if len(summaries) != 1:
        return "missing-summary"
    result, passed, failed, ignored, measured, _ = summaries[0]
    if int(ignored) or re.search(r"\bSKIP\b", output):
        return "skipped"
    if code or int(failed):
        return "failed"
    if result != "ok" or int(passed) != 1 or int(measured) != 0:
        return "missing-test"
    if not re.search(r"^test " + re.escape(name) + r"(?: - should panic)? \.\.\.(?: |$)", output, re.MULTILINE):
        return "missing-test"
    return "passed"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--jobs", type=int, default=min(4, os.cpu_count() or 1))
    parser.add_argument("--timeout", type=float, default=120)
    parser.add_argument("--filter", default="", help="Substring filter; receipt records incomplete scope")
    args = parser.parse_args()
    if args.jobs < 1 or args.timeout <= 0:
        parser.error("jobs and timeout must be positive")
    binary = args.binary.resolve(strict=True)
    # Diagnostic modes may suppress teardown or release assertions. Never
    # inherit them into acceptance runs, even if the caller set them globally.
    env = dict(os.environ)
    oracle = env.get("QJS_ORACLE")
    if not oracle or not Path(oracle).is_file():
        parser.error("QJS_ORACLE must name the pinned executable; missing or skipped oracle is not a pass")
    removed = sorted(key for key in env if key == "QJS_TEARDOWN_PROBE")
    for key in removed:
        del env[key]
    args.output.mkdir(parents=True, exist_ok=False)
    receipt = {
        "schema": 1, "binary": str(binary),
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "filter": args.filter, "jobs": args.jobs, "timeout_seconds": args.timeout,
        "removed_environment": removed, "tests": [], "complete": False,
    }
    receipt_path = args.output / "receipt.json"

    def save():
        temporary = receipt_path.with_suffix(".tmp")
        temporary.write_text(json.dumps(receipt, indent=2) + "\n")
        temporary.replace(receipt_path)

    save()
    code, listing, timed_out, _ = execute([str(binary), "--list", "--format", "terse"], env, args.timeout)
    (args.output / "list.log").write_text(listing)
    names = [line.removesuffix(": test") for line in listing.splitlines() if line.endswith(": test")]
    if code or timed_out or not names or len(set(names)) != len(names):
        receipt["error"] = "test enumeration failed or was empty/duplicated"
        save()
        return 1
    selected = [name for name in names if args.filter in name]
    receipt.update(enumerated=names, selected=selected, full_scope=len(selected) == len(names))
    save()
    if not selected:
        receipt["error"] = "filter selected no tests"
        save()
        return 1

    def run(item):
        index, name = item
        command = [str(binary), name, "--exact", "--test-threads=1", "--color=never", "--nocapture", "--include-ignored"]
        code, output, timed_out, elapsed = execute(command, env, args.timeout)
        log = f"{index:04d}.log"
        (args.output / log).write_text(output)
        return {"name": name, "status": classify(name, code, output, timed_out),
                "returncode": code, "seconds": elapsed, "log": log, "command": command}

    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as executor:
        futures = {executor.submit(run, item): item[1] for item in enumerate(selected)}
        for future in concurrent.futures.as_completed(futures):
            try:
                result = future.result()
            except Exception as error:
                result = {"name": futures[future], "status": "runner-error", "error": repr(error)}
            receipt["tests"].append(result)
            save()
            print(f"{result['status']}: {result['name']}", flush=True)
    receipt["tests"].sort(key=lambda test: test["name"])
    receipt["complete"] = len(receipt["tests"]) == len(selected)
    receipt["passed"] = receipt["complete"] and all(test["status"] == "passed" for test in receipt["tests"])
    receipt["counts"] = {status: sum(test["status"] == status for test in receipt["tests"])
                         for status in sorted({test["status"] for test in receipt["tests"]})}
    save()
    print(json.dumps(receipt["counts"], sort_keys=True))
    return 0 if receipt["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
