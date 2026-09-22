"""Build receipts must authenticate the source used, including uncommitted exports."""
import argparse
import io
import json
import os
import subprocess
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import build
from run import digest


class BuildIdentityTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.source = self.root / "export"
        self.source.mkdir()
        (self.source / "Cargo.toml").write_text("[workspace]\n")
        (self.source / "Cargo.lock").write_text("version = 4\n")
        (self.source / "lib.rs").write_text("pub fn answer() -> u8 { 42 }\n")
        self.manifest = self.root / "source.json"
        self.manifest.write_text(json.dumps({
            "base": "parent-is-not-the-export-identity",
            "files": {p.name: digest(p) for p in self.source.iterdir()},
        }))
        self.args = argparse.Namespace(repo=self.source, source_manifest=self.manifest,
                                       plain_target=self.root / "plain",
                                       profile_target=self.root / "profile", mode="both", jobs=2)

    def cargo(self, command, *, cwd, env, check, stdout, text):
        self.assertEqual(cwd, self.source)
        self.assertTrue(check)
        target = Path(command[command.index("--target-dir") + 1])
        binary = target / "release/qjs"
        binary.parent.mkdir(parents=True, exist_ok=True)
        binary.write_bytes(b"synthetic CLI " + str(target).encode())
        self.assertEqual(env["QUICKJS_OXIDE_BUILD_COMMIT"], f"frozen:{digest(self.manifest)}")
        return self.artifact(command, binary)

    def artifact(self, command, binary):
        return subprocess.CompletedProcess(command, 0, json.dumps({
            "reason": "compiler-artifact", "target": {"name": "qjs", "kind": ["bin"]},
            "executable": str(binary),
        }) + "\n")

    def invoke(self, cargo=None):
        with patch("build.subprocess.run", side_effect=cargo or self.cargo), \
                patch("build.command_output", return_value={"exit_code": 0, "stdout": "compiler"}), \
                patch("sys.stdout", new_callable=io.StringIO):
            return build.build(self.args)

    def test_export_receipts_keep_exact_manifest_and_profile_flags(self):
        with patch.dict(os.environ, {"CARGO_PROFILE_RELEASE_LTO": "fat",
                                    "CARGO_PROFILE_RELEASE_CODEGEN_UNITS": "1"}):
            manifests = self.invoke()
        self.assertEqual(len(manifests), 2)
        for binary, manifest in manifests:
            self.assertIsNone(manifest["commit"])
            self.assertEqual(manifest["source_identity"]["kind"], "frozen-export")
            self.assertEqual(manifest["binary_sha256"], digest(binary))
            self.assertEqual(manifest["source_identity"]["manifest_sha256"], digest(self.manifest))
            self.assertEqual(binary.with_suffix(".source.json").read_bytes(), self.manifest.read_bytes())
            self.assertEqual(manifest["environment"]["CARGO_PROFILE_RELEASE_LTO"], "fat")
            self.assertEqual(manifest["environment"]["CARGO_PROFILE_RELEASE_CODEGEN_UNITS"], "1")
            self.assertEqual(json.loads(binary.with_suffix(".build.json").read_text()), manifest)

    def test_new_untracked_module_during_build_is_not_admitted(self):
        for target in [self.args.plain_target, self.args.profile_target]:
            receipt = target / "release/qjs.build.json"
            receipt.parent.mkdir(parents=True)
            receipt.write_text("old receipt")
        def mutate(*args, **kwargs):
            result = self.cargo(*args, **kwargs)
            (self.source / "new.rs").write_text("// unrecorded code\n")
            return result
        with self.assertRaisesRegex(ValueError, "inventory"):
            self.invoke(mutate)
        for target in [self.args.plain_target, self.args.profile_target]:
            self.assertFalse((target / "release/qjs.build.json").exists())

    def test_second_build_change_invalidates_both_modes(self):
        count = 0
        def mutate(*args, **kwargs):
            nonlocal count
            result = self.cargo(*args, **kwargs)
            count += 1
            if count == 2:
                (self.source / "lib.rs").write_text("pub fn answer() -> u8 { 0 }\n")
            return result
        with self.assertRaisesRegex(ValueError, "source changed"):
            self.invoke(mutate)
        for target in [self.args.plain_target, self.args.profile_target]:
            self.assertFalse((target / "release/qjs.build.json").exists())

    def test_manifest_replacement_during_build_is_rejected(self):
        def mutate(*args, **kwargs):
            result = self.cargo(*args, **kwargs)
            self.manifest.write_text(self.manifest.read_text() + "\n")
            return result
        with self.assertRaisesRegex(ValueError, "manifest changed"):
            self.invoke(mutate)

    def test_build_output_inside_export_is_rejected_before_cargo(self):
        self.args.plain_target = self.source / "target"
        with patch("build.subprocess.run") as cargo, self.assertRaisesRegex(ValueError, "outside"):
            build.build(self.args)
        cargo.assert_not_called()

    def test_plain_only_does_not_build_profiling(self):
        self.args.mode = "plain"
        manifests = self.invoke()
        self.assertEqual([m["mode"] for _, m in manifests], ["plain"])
        self.assertFalse(self.args.profile_target.exists())

    def test_configured_target_certifies_actual_artifact_not_stale_host(self):
        self.args.mode = "plain"
        host = self.args.plain_target / "release/qjs"
        host.parent.mkdir(parents=True)
        host.write_bytes(b"stale host binary")
        host.with_suffix(".build.json").write_text("old receipt")
        binary = self.args.plain_target / "target-triple/release/qjs"
        binary.parent.mkdir(parents=True)
        binary.with_suffix(".build.json").write_text("old target receipt")
        def cargo(command, **kwargs):
            binary.write_bytes(b"current target binary")
            return self.artifact(command, binary)
        manifests = self.invoke(cargo)
        self.assertEqual(manifests[0][0], binary)
        self.assertEqual(manifests[0][1]["binary_sha256"], digest(binary))
        self.assertFalse(host.with_suffix(".build.json").exists())

    def test_missing_or_external_cargo_artifact_is_not_admitted(self):
        self.args.mode = "plain"
        for output in ["", self.artifact([], self.source / "lib.rs").stdout]:
            with self.subTest(output=output), self.assertRaises(ValueError):
                self.invoke(lambda *a, **kw: subprocess.CompletedProcess([], 0, output))

    def test_non_git_export_requires_manifest_and_never_uses_ancestor_identity(self):
        self.args.source_manifest = None
        with patch("build.exact_git_root", return_value=False), \
                self.assertRaisesRegex(ValueError, "source-manifest"):
            self.invoke()

    def test_checkout_must_remain_clean_at_same_revision(self):
        self.args.source_manifest = None
        clean = {"commit": {"exit_code": 0, "stdout": "a" * 40},
                 "status": {"exit_code": 0, "stdout": ""}}
        changed = {"commit": {"exit_code": 0, "stdout": "b" * 40},
                   "status": {"exit_code": 0, "stdout": ""}}
        with patch("build.exact_git_root", return_value=True), \
                patch("build.git_metadata", side_effect=[clean, changed]), \
                self.assertRaisesRegex(ValueError, "checkout changed"):
            self.invoke()


if __name__ == "__main__":
    unittest.main()
