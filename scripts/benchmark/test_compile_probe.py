"""Frozen source identity must not silently inherit an ancestor checkout."""
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
from build_compile_probe import exact_git_root, manifest_files, validate_frozen_source
from run import digest


class CompileProbeIdentityTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.source = self.root / "lib.rs"
        self.source.write_text("pub fn answer() -> u8 { 42 }\n")
        self.files = {"lib.rs": digest(self.source)}

    def test_manifest_admits_exact_export_and_rejects_edits_or_new_inputs(self):
        validate_frozen_source(self.root, manifest_files({"files": self.files}))
        validate_frozen_source(self.root, manifest_files({"source_files": self.files}))
        extra = self.root / "Cargo.toml"
        extra.write_text("[package]\n")
        with self.assertRaisesRegex(ValueError, "inventory"):
            validate_frozen_source(self.root, self.files)
        extra.unlink()
        self.source.write_text("pub fn answer() -> u8 { 41 }\n")
        with self.assertRaisesRegex(ValueError, "source changed"):
            validate_frozen_source(self.root, self.files)

    def test_ancestor_git_checkout_is_not_the_export_identity(self):
        result = subprocess.CompletedProcess([], 0, str(self.root.parent) + "\n", "")
        with patch("build_compile_probe.subprocess.run", return_value=result):
            self.assertFalse(exact_git_root(self.root))
        result.stdout = str(self.root) + "\n"
        with patch("build_compile_probe.subprocess.run", return_value=result):
            self.assertTrue(exact_git_root(self.root))

    def test_manifest_cannot_name_files_outside_the_export(self):
        for name in ["../lib.rs", "/lib.rs", "./lib.rs"]:
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "unsafe"):
                manifest_files({"files": {name: self.files["lib.rs"]}})

    def test_target_directory_inputs_are_part_of_frozen_source(self):
        generated = self.root / "target/generated.rs"
        generated.parent.mkdir()
        generated.write_text("pub const INPUT: u8 = 1;\n")
        with self.assertRaisesRegex(ValueError, "inventory"):
            validate_frozen_source(self.root, self.files)
        self.files["target/generated.rs"] = digest(generated)
        validate_frozen_source(self.root, self.files)
        generated.write_text("pub const INPUT: u8 = 2;\n")
        with self.assertRaisesRegex(ValueError, "source changed"):
            validate_frozen_source(self.root, self.files)

    def test_manifest_must_be_an_object(self):
        with self.assertRaisesRegex(ValueError, "object"):
            manifest_files([])


if __name__ == "__main__":
    unittest.main()
