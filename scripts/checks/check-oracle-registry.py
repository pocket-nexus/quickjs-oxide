#!/usr/bin/env python3
"""Check the oracle module tree and, optionally, Cargo's compiled inventories."""
import argparse
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
ORACLE = ROOT / 'apps/cli/tests/oracle'
HOST_MODULES = {'test262_create_realm', 'test262_host_gc', 'test262_is_html_dda'}
MODULE = re.compile(r'^(?:pub(?:\([^)]*\))?\s+)?mod (\w+);$', re.MULTILINE)
TEST = re.compile(r'^\s*#\[test\]', re.MULTILINE)


def fail(message):
    raise SystemExit('error: ' + message)


def inventory():
    visited = set()
    host_modules = set()
    counts = {False: 0, True: 0}

    def visit(path, host=False):
        if path.is_symlink() or not path.is_file():
            fail(f'module must be a regular file: {path.relative_to(ROOT)}')
        path = path.resolve()
        if path in visited:
            fail(f'duplicate module registration: {path.relative_to(ROOT)}')
        if not path.is_relative_to(ROOT / 'apps/cli/tests') and path != ROOT / 'tests/common/mod.rs':
            fail(f'oracle module escapes tests/: {path}')
        visited.add(path)
        source = path.read_text()
        counts[host] += len(TEST.findall(source))
        for match in MODULE.finditer(source):
            name = match[1]
            prefix = source[:match.start()].splitlines()
            attributes = []
            while prefix and prefix[-1].startswith('#['):
                attributes.insert(0, prefix.pop())
            gates = [a for a in attributes if a.startswith('#[cfg')]
            gated = gates == ['#[cfg(feature = "test262-host")]']
            if gates and not gated:
                fail(f'unexpected module gate for {name}: {gates}')
            if name in HOST_MODULES:
                if path != ORACLE / 'main.rs' or not gated:
                    fail(f'{name} must be gated at the oracle entry point')
                host_modules.add(name)
            elif gated:
                fail(f'unexpected host-only module: {name}')
            paths = [a for a in attributes if a.startswith('#[path')]
            if paths:
                allowed = {
                    (ORACLE / 'main.rs', '#[path = "../common/mod.rs"]'): '../common/mod.rs',
                    (ROOT / 'apps/cli/tests/common/mod.rs', '#[path = "../../../../tests/common/mod.rs"]'): '../../../../tests/common/mod.rs',
                }
                relative = allowed.get((path, paths[0])) if len(paths) == 1 else None
                if relative is None:
                    fail(f'use ordinary module declarations for {name}')
                child = path.parent / relative
            else:
                directory = path.parent if path.name in ('main.rs', 'mod.rs') else path.with_suffix('')
                candidates = [directory / (name + '.rs'), directory / name / 'mod.rs']
                existing = [p for p in candidates if p.exists()]
                if len(existing) != 1:
                    fail(f'{name} must resolve to exactly one source file: {path.relative_to(ROOT)}')
                child = existing[0]
            visit(child, host or gated)

    visit(ORACLE / 'main.rs')
    if host_modules != HOST_MODULES:
        fail(f'host module inventory drifted: {sorted(host_modules)}')
    orphans = set(ORACLE.rglob('*.rs')) - visited
    if orphans:
        fail('unregistered oracle sources: ' + ', '.join(str(p.relative_to(ROOT)) for p in sorted(orphans)))
    if list((ROOT / 'apps/cli/tests').glob('oracle_*.rs')):
        fail('oracle sources must live under apps/cli/tests/oracle/')
    case_inventory()
    print(f'Oracle module tree covers {counts[False]} default + {counts[True]} host tests across {len(visited)} sources.')
    return counts


def case_inventory():
    """Every JS fixture directory under cases/ needs its own runner test."""
    cases = ORACLE / 'cases'
    if not cases.exists():
        return 0
    runner = (ORACLE / 'cases.rs').read_text() if (ORACLE / 'cases.rs').exists() else ''
    total = 0
    for path in sorted(cases.iterdir()):
        if not path.is_dir() or path.is_symlink():
            fail(f'oracle case fixtures must live in a directory: {path.relative_to(ROOT)}')
        if not re.search(r'#\[test\]\s*fn ' + re.escape(path.name) + r'\(\)', runner):
            fail(f'oracle case directory has no runner test: {path.relative_to(ROOT)}')
        for fixture in sorted(path.rglob('*')):
            if fixture.is_file() and fixture.suffix != '.js':
                fail(f'oracle case fixtures must be .js files: {fixture.relative_to(ROOT)}')
            total += fixture.suffix == '.js'
    return total


def compiled_inventory(features=False, test_filter=None):
    command = ['cargo', 'test', '--locked', '-p', 'quickjs-oxide-cli', '--test', 'oracle']
    if features:
        command += ['--features', 'test262-host']
    if test_filter:
        command.append(test_filter)
    result = subprocess.check_output(command + ['--', '--list'], cwd=ROOT, text=True)
    return {line.removesuffix(': test') for line in result.splitlines() if line.endswith(': test')}


LEDGER = ROOT / 'docs/deviations.md'
FIXTURE_GATES = ROOT / 'dev-support/quickjs-fixture-gates.tsv'
LEDGER_ENTRY = re.compile(r'^### ([A-Z0-9]+(?:-[A-Z0-9]+)*-\d{3})$', re.MULTILINE)
# The fields every new-core entry carries (new-core phase 6, section 5).
NEWCORE_FIELDS = ('Status', 'Surface', 'Authority', 'Required behavior',
                  'Baseline evidence', 'Acceptance', 'Specification')


def ledger_entries():
    """The deviation ledger's entry identifiers; checks the new-core entries'
    form and that every fixture attribution names an entry."""
    text = LEDGER.read_text()
    found = LEDGER_ENTRY.findall(text)
    if len(found) != len(set(found)):
        fail('deviation ledger repeats an entry identifier')
    entries = set(found)
    bodies = re.split(r'^### ', text, flags=re.MULTILINE)[1:]
    for body in bodies:
        name = body.split('\n', 1)[0].strip()
        if not name.startswith('NEWCORE-'):
            continue
        for field in NEWCORE_FIELDS:
            if not re.search(rf'^- {re.escape(field)}:\s', body, re.MULTILINE):
                fail(f'deviation ledger entry {name} lacks its {field} field')
    newcore = {entry for entry in entries if entry.startswith('NEWCORE-')}
    expected = {'NEWCORE-WITH-001', 'NEWCORE-UNSCOPABLES-001', 'NEWCORE-DIRECT-EVAL-001',
                'NEWCORE-REALMS-001', 'NEWCORE-SPECIES-001'}
    if newcore != expected:
        fail(f'deviation ledger new-core entries differ: {sorted(newcore ^ expected)}')
    rows = FIXTURE_GATES.read_text().splitlines()
    header = rows[0].split('\t')
    if 'deviation' in header:
        column = header.index('deviation')
        for row in rows[1:]:
            value = row.split('\t')[column]
            if value != '-' and value not in entries:
                fail(f'fixture gate {row.split(chr(9))[0]} names no ledger entry: {value}')
    return entries


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiled', action='store_true')
    args = parser.parse_args()
    counts = inventory()
    entries = ledger_entries()
    subprocess.run(['node', 'scripts/checks/check-oracle-helper-duplication.mjs'], cwd=ROOT, check=True)
    if args.compiled:
        default = compiled_inventory()
        host = compiled_inventory(True)
        filtered = compiled_inventory(True, 'test262_')
        if len(default) != counts[False] or len(host) != sum(counts.values()):
            fail('compiled test counts do not match registered source tests')
        if not default <= host or host - default != filtered or len(filtered) != counts[True]:
            fail('host feature/filter changes tests outside the registered host modules')
        print(f'Deviation ledger: {len(entries)} entries.')
        print(f'Compiled oracle inventory matches: {len(default)} default / {len(host)} host / {len(filtered)} host-filtered tests.')


if __name__ == '__main__':
    main()
