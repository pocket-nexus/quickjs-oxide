#!/usr/bin/env python3
"""Install the pinned Linux x86-64 Verus bundle after checking its SHA-256."""
import hashlib
from pathlib import Path
import platform
import subprocess
import urllib.request
import zipfile

from verus import ROOT, VERSION

SHA256 = "e9af03732d4bdd32b93a8fcf471706739c24c1473fa5086618d96c47237845dd"
URL = (f"https://github.com/verus-lang/verus/releases/download/release/{VERSION}/"
       f"verus-{VERSION}-x86-linux.zip")


def main():
    if platform.system() != "Linux" or platform.machine() != "x86_64":
        raise SystemExit("This pinned proof configuration supports Linux x86-64 only")
    destination = ROOT / "target/verification/tools"
    destination.mkdir(parents=True, exist_ok=True)
    archive = destination / f"verus-{VERSION}.zip"
    if not archive.exists():
        with urllib.request.urlopen(URL) as response:
            archive.write_bytes(response.read())
    if hashlib.sha256(archive.read_bytes()).hexdigest() != SHA256:
        raise SystemExit(f"checksum mismatch: {archive}; remove it before retrying")
    with zipfile.ZipFile(archive) as bundle:
        for entry in bundle.infolist():
            path = Path(entry.filename)
            if path.is_absolute() or ".." in path.parts:
                raise SystemExit("unexpected archive path")
        bundle.extractall(destination)
    for name in ("verus", "rust_verify", "z3"):
        (destination / "verus-x86-linux" / name).chmod(0o755)
    subprocess.run(["rustup", "toolchain", "install", "1.99.1", "--profile", "minimal"], check=True)
    print(destination / "verus-x86-linux/verus")


if __name__ == "__main__":
    main()
