"""Generate reproducible, project-authored fixed-work VM diagnostics.

Expected output is derived from the workload contract, never learned from a
benchmark engine. ``--smoke --oracle node`` checks tiny versions without timing.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
from dataclasses import dataclass
from pathlib import Path
from textwrap import dedent


ROOT = Path(__file__).resolve().parents[3]
PROBES = Path(__file__).resolve().parent
RECEIPT_WORKLOADS = (
    ROOT / "docs/performance/receipts/ordinary-number-writes-2026-09-26/workloads"
)
RECEIPT_COUNTS = {
    "local_move": 6_000_000,
    "argument_move": 6_000_000,
    "number_owner_fallback": 1_000_000,
    "object_move": 1_000_000,
    "empty_loop": 10_000_000,
}
SMOKE_COUNT = 8


@dataclass(frozen=True)
class Case:
    name: str
    count: int
    source: str
    expected: str


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def program(template: str, count: int, *, bits: int | None = None) -> str:
    source = dedent(template).strip().replace("__COUNT__", str(count))
    if bits is not None:
        source = source.replace("__BITS__", str(bits))
    return source + "\n"


def count(full: int, smoke: bool) -> int:
    return SMOKE_COUNT if smoke else full


def cases(smoke: bool) -> list[Case]:
    rows = []

    def add(name: str, full: int, template: str, expected: str | int) -> None:
        iterations = count(full, smoke)
        rows.append(Case(name, iterations, program(template, iterations), f"{expected}\n"))

    n = count(3_000_000, smoke)
    add(
        "fusion_hit", 3_000_000,
        """
        function work(n) {
            var sum=0;
            while(n>0) { sum=sum+1; n=n-1; }
            return sum;
        }
        print(work(__COUNT__));
        """,
        n,
    )
    n = count(180_000, smoke)
    add(
        "fusion_dynamic_miss", 180_000,
        """
        function work(n) {
            var calls=0;
            var object={valueOf:function(){calls++;return 1;}};
            var sum=object;
            while(n>0) { sum=sum+1; sum=object; n=n-1; }
            return calls;
        }
        print(work(__COUNT__));
        """,
        n,
    )
    # The hot loops are deliberately identical: the one-time addition makes
    # only the first function's fusion sidecar nonempty. A truthiness check
    # avoids publishing the independent comparison/branch fusion for n>0.
    plain_loop = """
        function work(n) {
            var plain=41, sum=0;
            __CANDIDATE__
            while(n) { sum=plain; n=n-1; }
            return sum;
        }
        print(work(__COUNT__));
    """
    for name, candidate in (
        ("fusion_flag0", "sum=sum+1;"),
        ("fusion_no_plan", ""),
    ):
        iterations = count(3_000_000, smoke)
        rows.append(Case(
            name, iterations,
            program(plain_loop.replace("__CANDIDATE__", candidate), iterations),
            "41\n",
        ))

    bigint_template = """
        function work(n) {
            var baseline=(1n << __BITS__n)-1n, x=baseline;
            while(n>0) {
                x=((x ^ 5n)+1n)-1n;
                x=x ^ 5n;
                n=n-1;
            }
            return x===baseline;
        }
        print(work(__COUNT__));
    """
    for bits, full in ((32, 120_000), (64, 100_000), (256, 40_000)):
        iterations = count(full, smoke)
        rows.append(Case(
            f"bigint_{bits}", iterations,
            program(bigint_template, iterations, bits=bits),
            "true\n",
        ))

    n = count(2_000_000, smoke)
    add(
        "prop_read", 2_000_000,
        """
        function work(n) {
            var o={x:3}, sum=0;
            for(var i=0;i<n;i++) sum=sum+o.x;
            return sum;
        }
        print(work(__COUNT__));
        """,
        3 * n,
    )
    add(
        "prop_write", 2_000_000,
        """
        function work(n) {
            var o={x:0};
            for(var i=0;i<n;i++) o.x=i;
            return o.x;
        }
        print(work(__COUNT__));
        """,
        n - 1,
    )
    add(
        "array_read", 2_000_000,
        """
        function work(n) {
            var a=[1,2,3,4], sum=0;
            for(var i=0;i<n;i++) sum=sum+a[i&3];
            return sum;
        }
        print(work(__COUNT__));
        """,
        10 * (n // 4),
    )
    add(
        "array_write", 2_000_000,
        """
        function work(n) {
            var a=[0,0,0,0];
            for(var i=0;i<n;i++) a[i&3]=i;
            return a[0]+a[1]+a[2]+a[3];
        }
        print(work(__COUNT__));
        """,
        4 * n - 10,
    )
    n = count(800_000, smoke)
    add(
        "call0", 800_000,
        """
        function unit() { return 1; }
        function work(n) {
            var sum=0;
            for(var i=0;i<n;i++) sum=sum+unit();
            return sum;
        }
        print(work(__COUNT__));
        """,
        n,
    )
    n = count(180_000, smoke)
    add(
        "string_bridge", 180_000,
        """
        function work(n) {
            var sum='a', suffix='b', completed=0;
            for(var i=0;i<n;i++) {
                sum=sum+suffix;
                if(sum!=='ab') return -1;
                sum='a';
                completed++;
            }
            return completed;
        }
        print(work(__COUNT__));
        """,
        n,
    )
    n = count(12_000, smoke)
    add(
        "type_error", 12_000,
        """
        function fault() { var x=1n; x=x+1; return x; }
        function work(n) {
            var caught=0;
            for(var i=0;i<n;i++) {
                try { fault(); }
                catch(error) { if(!(error instanceof TypeError)) throw error; caught++; }
            }
            return caught;
        }
        print(work(__COUNT__));
        """,
        n,
    )
    add(
        "tdz", 12_000,
        """
        function fault() { target=target+1; let target=1; }
        function work(n) {
            var caught=0;
            for(var i=0;i<n;i++) {
                try { fault(); }
                catch(error) { if(!(error instanceof ReferenceError)) throw error; caught++; }
            }
            return caught;
        }
        print(work(__COUNT__));
        """,
        n,
    )

    for name, full in RECEIPT_COUNTS.items():
        source = (RECEIPT_WORKLOADS / f"{name}.js").read_text()
        iterations = count(full, smoke)
        if smoke:
            source, replacements = re.subn(
                r"print\(work\(\d+", f"print(work({iterations}", source, count=1
            )
            if replacements != 1:
                raise ValueError(f"receipt workload call not found: {name}")
        expected = {
            "local_move": 2 * iterations - 2,
            "argument_move": iterations - 1,
            "number_owner_fallback": iterations,
            "object_move": 2,
            "empty_loop": iterations,
        }[name]
        rows.append(Case(name, iterations, source, f"{expected}\n"))
    return rows


def node_check(node: str, path: Path, expected: str) -> None:
    # The five receipt workloads use qjs' print; new workloads use it too.
    adapter = "globalThis.print=console.log;require(process.argv[1]);"
    completed = subprocess.run(
        [node, "-e", adapter, str(path.resolve())],
        capture_output=True, check=True, timeout=20,
    )
    if completed.stdout != expected.encode() or completed.stderr:
        raise ValueError(f"Node result differs for {path.name}: {completed!r}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=PROBES / "fixed")
    parser.add_argument("--check", action="store_true", help="verify generated bytes")
    parser.add_argument("--smoke", action="store_true", help="use eight iterations per case")
    parser.add_argument("--oracle", help="optionally check outputs with Node")
    args = parser.parse_args()
    output = args.output.resolve()
    if args.smoke and output == (PROBES / "fixed").resolve():
        parser.error("--smoke requires a separate --output directory")
    workloads = cases(args.smoke)
    entries = []
    for case in workloads:
        path = output / f"{case.name}.js"
        data = case.source.encode()
        if args.check:
            if not path.is_file() or path.read_bytes() != data:
                raise ValueError(f"generated workload differs: {path}")
        else:
            output.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        try:
            display_path = path.relative_to(ROOT).as_posix()
        except ValueError:
            display_path = str(path)
        entries.append({
            "case": case.name, "size": case.count, "path": display_path,
            "sha256": digest(data), "expected": case.expected,
        })
        if args.oracle:
            node_check(args.oracle, path, case.expected)
    manifest = {
        "metadata": {
            "suite": "project-authored-fixed-vm-diagnostics",
            "generator": "docs/performance/probes/build_fixed_matrix.py",
            "generator_sha256": digest(Path(__file__).read_bytes()),
            "smoke": args.smoke,
            "workloads": {"workloads": entries},
        }
    }
    manifest_path = output / "manifest.json"
    manifest_bytes = (json.dumps(manifest, indent=2) + "\n").encode()
    if args.check:
        if not manifest_path.is_file() or manifest_path.read_bytes() != manifest_bytes:
            raise ValueError(f"generated manifest differs: {manifest_path}")
        extras = sorted(p.name for p in output.glob("*.js") if p.stem not in {c.name for c in workloads})
        if extras:
            raise ValueError(f"unlisted workloads: {extras}")
    else:
        manifest_path.write_bytes(manifest_bytes)
    print(f"{len(workloads)} fixed diagnostics: {manifest_path}")


if __name__ == "__main__":
    main()
