"""Reject changed training data or undeclared build differences, without an engine."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import build as builder
import iterate_v8
from pgo_validation import require_build_technique, verify_training
from run import digest


def item(path):
    return {"path": str(path), "sha256": digest(path)}


def fixture(directory):
    root = Path(directory)
    source = {"commit": {"stdout": "a" * 40}, "tree": {"stdout": "b" * 40}, "status": {"stdout": ""}}
    common = {"schema": "oxide-build-v2", "mode": "plain", "commit": "a" * 40, "source": source,
              "features": [], "exit_code": 0, "command": ["cargo", "build", "--release"],
              "rustc": {"stdout": "LLVM version: 20.1.5"}, "cargo": {"stdout": "cargo 1.88"},
              "target": {"requested_triple": "x86_64-unknown-linux-gnu"},
              "cargo_toml_sha256": "c" * 64, "cargo_lock_sha256": "d" * 64,
              "optimization": {"kind": "none"},
              "release_profile": {"manifest": {"lto": "fat", "codegen-units": "1"},
                                  "environment_overrides": {"CARGO_ENCODED_RUSTFLAGS": ""},
                                  "cargo_config_sha256": {},
                                  "qjs_rustc_invocation": {"codegen": {"lto": "fat", "opt-level": "3"}}}}
    generated = copy.deepcopy(common)
    instrumented = root / "generate-qjs"
    instrumented.write_bytes(b"synthetic instrumented binary")
    generated["binary_sha256"] = digest(instrumented)
    generated["mode"] = "profile-generate"
    generated["optimization"] = {"kind": "profile-generate", "data": str(root / "raw")}
    generated["release_profile"]["environment_overrides"]["CARGO_ENCODED_RUSTFLAGS"] = f"-Cprofile-generate={root / 'raw'}"
    generated["release_profile"]["qjs_rustc_invocation"]["codegen"]["profile-generate"] = str(root / "raw")
    generated_path = root / "generate.build.json"
    generated_path.write_text(json.dumps(generated))
    raw = root / "raw.profraw"
    raw.write_bytes(b"original raw profile")
    merged = root / "merged.profdata"
    merged.write_bytes(b"original merged profile")
    stdout, stderr = root / "stdout", root / "stderr"
    stdout.write_bytes(b"1\n")
    stderr.write_bytes(b"")
    receipt = {"schema": "oxide-pgo-training-v1", "source_commit": "a" * 40, "source_tree": "b" * 40,
               "rustc": common["rustc"], "cargo": common["cargo"], "held_out": "original-v8-v7",
               "samples": [{"case": "scope", "size": 64, "status": "ok", "exit_code": 0, "timed_out": False,
                            "stdout": str(stdout), "stdout_sha256": digest(stdout),
                            "stderr": str(stderr), "stderr_sha256": digest(stderr), "profraw": [item(raw)]}],
               "workloads": [{"case": "scope", "size": 64, "expected": "1\n"}],
               "artifacts": [item(generated_path)], "profraw": [item(raw)], "profdata": item(merged),
               "instrumented_build_receipt": item(generated_path), "instrumented_binary": item(instrumented)}
    receipt_path = root / "training.json"
    receipt_path.write_text(json.dumps(receipt))
    candidate = copy.deepcopy(common)
    candidate["mode"] = "profile-use"
    candidate["optimization"] = {"kind": "profile-use", "data": str(merged), "data_sha256": digest(merged),
                                 "training_receipt": item(receipt_path)}
    candidate["release_profile"]["environment_overrides"]["CARGO_ENCODED_RUSTFLAGS"] = f"-Cprofile-use={merged}"
    candidate["release_profile"]["qjs_rustc_invocation"]["codegen"]["profile-use"] = str(merged)
    return common, candidate, receipt_path, raw


class PgoValidationTests(unittest.TestCase):
    def test_matching_immutable_chain_is_accepted_only_as_build_technique(self):
        with tempfile.TemporaryDirectory() as directory:
            left, right, _, _ = fixture(directory)
            require_build_technique({"build": left}, {"build": right})
            with mock.patch.object(iterate_v8, "binary_metadata", return_value={"build": right}):
                with self.assertRaisesRegex(ValueError, "not a plain"):
                    iterate_v8.require_plain_engine(Path(directory) / "qjs")
            hidden = copy.deepcopy(right)
            hidden["mode"] = "plain"
            with mock.patch.object(iterate_v8, "binary_metadata", return_value={"build": hidden}):
                with self.assertRaisesRegex(ValueError, "ordinary source"):
                    iterate_v8.require_plain_engine(Path(directory) / "qjs")

    def test_changed_raw_or_merged_profile_is_rejected(self):
        for which in ("raw", "merged"):
            with self.subTest(which=which), tempfile.TemporaryDirectory() as directory:
                left, right, _, raw = fixture(directory)
                path = raw if which == "raw" else Path(right["optimization"]["data"])
                path.write_bytes(b"changed")
                with self.assertRaisesRegex(ValueError, "artifact changed"):
                    require_build_technique({"build": left}, {"build": right})

    def test_failed_training_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            left, right, receipt_path, _ = fixture(directory)
            receipt = json.loads(receipt_path.read_text())
            receipt["samples"][0]["status"] = "wrong-output"
            receipt_path.write_text(json.dumps(receipt))
            with self.assertRaisesRegex(ValueError, "did not complete"):
                verify_training(receipt_path, right["optimization"]["data"], left["source"])

    def test_source_toolchain_environment_and_effective_flags_are_constraints(self):
        changes = (("source", lambda r: r["source"]["tree"].update(stdout="f" * 40)),
                   ("compiler", lambda r: r.update(rustc={"stdout": "LLVM version: 22.1.8"})),
                   ("environment", lambda r: r["release_profile"]["environment_overrides"].update(RUSTFLAGS="-Ctarget-cpu=native")),
                   ("effective", lambda r: r["release_profile"]["qjs_rustc_invocation"]["codegen"].update(**{"opt-level": "2"})))
        for name, mutate in changes:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                left, right, _, _ = fixture(directory)
                mutate(right)
                with self.assertRaises(ValueError):
                    require_build_technique({"build": left}, {"build": right})

    def test_actual_rustc_profile_flags_are_recorded(self):
        rows = builder.observed_rustc("Running `rustc --crate-name qjs -C opt-level=3 -Cprofile-use=/tmp/merged --target x86_64-unknown-linux-gnu`")
        self.assertEqual(rows[0]["codegen"]["profile-use"], "/tmp/merged")
        self.assertEqual(rows[0]["target"], "x86_64-unknown-linux-gnu")

    def test_each_training_process_must_contribute_a_profile(self):
        with tempfile.TemporaryDirectory() as directory:
            left, right, receipt_path, _ = fixture(directory)
            receipt = json.loads(receipt_path.read_text())
            extra = copy.deepcopy(receipt["samples"][0])
            extra.update(size=128, profraw=[])
            receipt["samples"].append(extra)
            receipt["workloads"].append({"case": "scope", "size": 128, "expected": "1\n"})
            receipt_path.write_text(json.dumps(receipt))
            with self.assertRaisesRegex(ValueError, "produced no raw"):
                verify_training(receipt_path, right["optimization"]["data"], left["source"])

    def test_comparison_tooling_identity_includes_pgo_validation(self):
        self.assertIn("pgo_validation.py", iterate_v8.tooling_identity("build-technique"))
        self.assertNotIn("pgo_validation.py", iterate_v8.tooling_identity("source"))

    def test_changed_training_stdout_is_not_rehashed_into_acceptance(self):
        with tempfile.TemporaryDirectory() as directory:
            left, right, receipt_path, _ = fixture(directory)
            receipt = json.loads(receipt_path.read_text())
            Path(receipt["samples"][0]["stdout"]).write_bytes(b"2\n")
            with self.assertRaisesRegex(ValueError, "artifact changed"):
                verify_training(receipt_path, right["optimization"]["data"], left["source"])

    def test_generate_configuration_must_match_none_and_use(self):
        with tempfile.TemporaryDirectory() as directory:
            left, right, receipt_path, _ = fixture(directory)
            receipt = json.loads(receipt_path.read_text())
            path = Path(receipt["instrumented_build_receipt"]["path"])
            generated = json.loads(path.read_text())
            generated["target"] = {"requested_triple": "other-target"}
            path.write_text(json.dumps(generated))
            receipt["instrumented_build_receipt"] = item(path)
            receipt["artifacts"] = [item(path)]
            receipt_path.write_text(json.dumps(receipt))
            right["optimization"]["training_receipt"] = item(receipt_path)
            with self.assertRaisesRegex(ValueError, "generate and none target"):
                require_build_technique({"build": left}, {"build": right})


if __name__ == "__main__":
    unittest.main()
