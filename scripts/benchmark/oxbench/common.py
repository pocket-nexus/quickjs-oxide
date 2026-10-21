"""Shared identity, metadata and process sampling for the benchmark tools."""
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import signal
import subprocess
import time
import threading

ROOT = Path(__file__).resolve().parents[3]
V8_V7_SOURCE_COMMIT = "2034d98fc8c5f8044e186267593f5d5ea5232caf"
LOAD = re.compile(r"load\('([^']+)'\);")


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def command_output(command, cwd=None):
    result = subprocess.run(command, cwd=cwd, capture_output=True, text=True, timeout=15)
    return {"command": list(map(str, command)), "exit_code": result.returncode,
            "stdout": result.stdout.strip(), "stderr": result.stderr.strip()}


def git_metadata(path):
    return {"commit": command_output(["git", "rev-parse", "HEAD"], path),
            "tree": command_output(["git", "rev-parse", "HEAD^{tree}"], path),
            "top_level": command_output(["git", "rev-parse", "--show-toplevel"], path),
            "status": command_output(["git", "status", "--porcelain"], path)}


def paired_order(names, repetition, strategy):
    """Choose one half of an ABBA/BAAB block for two engine comparisons."""
    if len(names) != 2 or strategy not in ("abba", "baab", "abba-baab"):
        raise ValueError("paired order requires two engines and abba, baab or abba-baab")
    reverse_first = strategy == "baab" or (strategy == "abba-baab" and repetition // 2 % 2 == 1)
    reverse = reverse_first == (repetition % 2 == 0)
    return list(reversed(names)) if reverse else list(names)


def validate_paired_order(names, repeat, strategy):
    if strategy == "default":
        return
    block_repetitions = 4 if strategy == "abba-baab" else 2
    if len(names) != 2 or repeat % block_repetitions:
        raise ValueError(f"{strategy} requires two engines and repeat divisible by {block_repetitions}")


def binary_metadata(path):
    metadata = {"path": str(path), "sha256": digest(path),
                "version": command_output([str(path), "--version"])}
    receipt = path.with_suffix(".build.json")
    if receipt.exists():
        build = json.loads(receipt.read_text())
        if build.get("binary_sha256") != metadata["sha256"]:
            raise ValueError(f"stale build receipt: {receipt}")
        metadata["build"] = build
    else:
        metadata["build"] = None
    return metadata


def machine_metadata():
    cpu = None
    if Path("/proc/cpuinfo").exists():
        for line in Path("/proc/cpuinfo").read_text().splitlines():
            if line.startswith("model name"):
                cpu = line.split(":", 1)[1].strip()
                break
    affinity = sorted(os.sched_getaffinity(0)) if hasattr(os, "sched_getaffinity") else None
    governors = {}
    for cpu_id in affinity or []:
        path = Path(f"/sys/devices/system/cpu/cpu{cpu_id}/cpufreq/scaling_governor")
        if path.is_file():
            governors[str(cpu_id)] = path.read_text().strip()
    try:
        load_average = list(os.getloadavg())
    except (AttributeError, OSError):
        load_average = None
    macos = None
    if platform.system() == "Darwin":
        brand = command_output(["sysctl", "-n", "machdep.cpu.brand_string"])
        memory = command_output(["sysctl", "-n", "hw.memsize"])
        cpu = brand["stdout"] if brand["exit_code"] == 0 else None
        macos = {"physical_memory_bytes": int(memory["stdout"]) if memory["exit_code"] == 0 and memory["stdout"].isdigit() else None,
                 "power_settings": command_output(["pmset", "-g", "custom"]),
                 "power_source": command_output(["pmset", "-g", "batt"]),
                 "vm_stat": command_output(["vm_stat"])}
    return {"platform": platform.platform(), "machine": platform.machine(), "cpu": cpu,
            "logical_cpus": os.cpu_count(), "python": platform.python_version(),
            "cpu_affinity": affinity, "scaling_governors": governors, "load_average": load_average,
            "macos": macos,
            "runner_sha256": digest(__file__), "repository": git_metadata(ROOT)}


def run_sample(command, cwd, prefix, timeout):
    # A watchdog allows blocking waitpid instead of Popen.wait(timeout)'s
    # exponentially backed-off polling, which quantizes short wall timings.
    expired = threading.Event()
    with prefix.with_suffix(".stdout").open("wb") as stdout, prefix.with_suffix(".stderr").open("wb") as stderr:
        started = time.perf_counter_ns()
        proc = subprocess.Popen(command, cwd=cwd, stdout=stdout, stderr=stderr, start_new_session=True)

        def kill_group():
            try:
                os.killpg(proc.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass

        def expire():
            if proc.poll() is None:
                expired.set()
                kill_group()

        watchdog = threading.Timer(timeout, expire)
        watchdog.daemon = True
        watchdog.start()
        try:
            code = proc.wait()
            elapsed = time.perf_counter_ns() - started
        except BaseException:
            kill_group()
            proc.wait()
            raise
        finally:
            watchdog.cancel()
            watchdog.join()
    return {"command": command, "exit_code": code, "timed_out": expired.is_set(),
            "process_wall_ns": elapsed,
            "stdout": str(prefix.with_suffix(".stdout")), "stderr": str(prefix.with_suffix(".stderr"))}
