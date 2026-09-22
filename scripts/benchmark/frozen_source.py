"""Source identity shared by CLI and compilation-probe builders."""
import os
import subprocess
from pathlib import Path
from run import digest


def manifest_files(receipt):
    if not isinstance(receipt, dict):
        raise ValueError("source manifest must be an object")
    files = receipt.get("files", receipt.get("source_files"))
    if not isinstance(files, dict) or not files:
        raise ValueError("source manifest must contain nonempty files or source_files hashes")
    for name, sha256 in files.items():
        path = Path(name)
        if path.is_absolute() or ".." in path.parts or name != path.as_posix() or name == ".":
            raise ValueError(f"unsafe source manifest path: {name}")
        if not isinstance(sha256, str) or len(sha256) != 64 or any(c not in "0123456789abcdef" for c in sha256):
            raise ValueError(f"invalid source hash: {name}")
    return files


def validate_frozen_source(repo, files):
    # Frozen inputs must be a complete export. A new unrecorded Rust module or
    # Cargo config is as significant as an edit to a recorded source file.
    if not repo.is_dir():
        raise ValueError(f"source export is not a directory: {repo}")
    actual = set()
    for directory, directories, names in os.walk(repo):
        if Path(directory) == repo:
            directories[:] = [name for name in directories if name != ".git"]
        if any((Path(directory) / name).is_symlink() for name in directories):
            raise ValueError("frozen source directory symlinks are not supported")
        for name in names:
            path = Path(directory) / name
            if Path(directory) == repo and name == ".git":
                continue
            if not path.resolve().is_relative_to(repo):
                raise ValueError(f"source link escapes export: {path}")
            actual.add(path.relative_to(repo).as_posix())
    if actual != files.keys():
        raise ValueError(f"source inventory differs from manifest: missing={sorted(files.keys() - actual)}, extra={sorted(actual - files.keys())}")
    for name, sha256 in files.items():
        if digest(repo / name) != sha256:
            raise ValueError(f"frozen source changed: {name}")


def exact_git_root(repo):
    result = subprocess.run(["git", "-C", str(repo), "rev-parse", "--show-toplevel"],
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    return result.returncode == 0 and Path(result.stdout.strip()).resolve() == repo
