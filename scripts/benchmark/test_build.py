"""Build receipts retain source/tooling identity and the actual Cargo invocation."""
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

import build


class BuildReceiptTests(unittest.TestCase):
    def test_plain_only_external_worktree_receipt(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = (root / "historical-source").resolve()
            source.mkdir()
            (source / "Cargo.toml").write_text('[profile.release]\nlto = "fat"\ncodegen-units = 1\n')
            (source / "Cargo.lock").write_text("# fixed lock\n")
            target = (root / "plain").resolve()

            def git_metadata(path):
                path = Path(path).resolve()
                return {"commit": {"exit_code": 0, "stdout": "a" * 40 if path == source else "b" * 40},
                        "tree": {"exit_code": 0, "stdout": "c" * 40},
                        "top_level": {"exit_code": 0, "stdout": str(path)},
                        "status": {"exit_code": 0, "stdout": ""}}

            def command_output(command, _cwd):
                return {"exit_code": 0, "stdout": "rustc 1.94.1\nhost: aarch64-apple-darwin" if command[0] == "rustc" else "cargo 1.94.1"}

            def run(command, **kwargs):
                self.assertEqual(kwargs["cwd"], source)
                self.assertIn("--verbose", command)
                self.assertNotIn("--features", command)
                kwargs["stdout"].write(b"complete stdout\n")
                kwargs["stderr"].write(
                    b"Running `rustc --crate-name qjs -C lto=fat -C codegen-units=1 "
                    b"-C target-cpu=apple-m1 -C target-feature=+neon`\n")
                binary = target / "release" / "qjs"
                binary.write_bytes(b"qjs binary")
                return SimpleNamespace(returncode=0)

            argv = ["build.py", "--repo", str(source), "--plain-only",
                    "--plain-target", str(target), "--profile-target", str(root / "profile")]
            with mock.patch.object(sys, "argv", argv), \
                 mock.patch.object(build, "git_metadata", side_effect=git_metadata), \
                 mock.patch.object(build, "command_output", side_effect=command_output), \
                 mock.patch.object(build, "cargo_configuration_hashes", return_value={}), \
                 mock.patch.object(build.subprocess, "run", side_effect=run):
                build.main()
            receipt = json.loads((target / "release" / "qjs.build.json").read_text())
            self.assertEqual(receipt["source"]["commit"]["stdout"], "a" * 40)
            self.assertEqual(receipt["tooling"]["repository"]["commit"]["stdout"], "b" * 40)
            self.assertEqual(Path(receipt["tooling"]["snapshots"]["build.py"]["path"]).read_bytes(),
                             Path(build.__file__).read_bytes())
            self.assertEqual(receipt["release_profile"]["manifest"], {"lto": '"fat"', "codegen-units": "1"})
            self.assertEqual(receipt["release_profile"]["qjs_rustc_invocation"]["codegen"]["lto"], "fat")
            self.assertEqual(receipt["release_profile"]["observed_rustc_compilations"], 1)
            self.assertEqual(receipt["release_profile"]["observed_codegen_groups"][0]["crates"], ["qjs"])
            self.assertEqual(receipt["target"]["observed_target_cpu"], "apple-m1")
            self.assertEqual(receipt["target"]["observed_target_feature"], "+neon")
            self.assertEqual(Path(receipt["stdout"]["path"]).read_bytes(), b"complete stdout\n")
            self.assertTrue(Path(receipt["stderr"]["path"]).is_file())
            self.assertFalse((root / "profile").exists())


if __name__ == "__main__":
    unittest.main()
