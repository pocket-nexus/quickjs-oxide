#!/usr/bin/env bash
# Runs Test262 with one front end and writes the per-variant report:
#
#     scripts/frontend/test262.sh old|new REPORT [--test FILE...]
#
# A development run outside the gate: it builds the runner with a fixed
# fingerprint and needs `scripts/test262/prepare-test262.sh` to have
# prepared the suite. Compare two reports with `scripts/frontend/compare-test262.py`.
set -euo pipefail
front_end=$1
report=$2
shift 2
root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
spec=$root/dev-support/test262/current.conf
value() { awk -F= -v key="$1" '$1 == key { print substr($0, length(key) + 2) }' "$spec"; }
fingerprint=1111111111111111111111111111111111111111111111111111111111111111
QUICKJS_OXIDE_TEST262_ENGINE_SEMANTICS_SHA256=$fingerprint \
    cargo build --quiet --release -p quickjs-oxide-test262 --bin run-test262
suite=$root/target/oracle/quickjs-2026-06-04/test262
selection=(--all)
if [[ $# -gt 0 ]]; then selection=("$@"); fi
OXIDE_FRONTEND=$front_end "$root/target/release/run-test262" --suite "$suite" \
    --config "$suite/../test262.conf" \
    --admissions "$root/$(value admissions)" --admissions-sha256 "$(value admissions_sha256)" \
    --oxide-profile "$root/$(value profile)" \
    --negative-diagnostics "$root/$(value negative_diagnostics)" \
    --negative-diagnostics-sha256 "$(value negative_diagnostics_sha256)" \
    --negative-diagnostic-exemptions "$root/$(value negative_diagnostic_exemptions)" \
    --negative-diagnostic-exemptions-sha256 "$(value negative_diagnostic_exemptions_sha256)" \
    "${selection[@]}" --report "$report" --engine-semantics-sha256 "$fingerprint" \
    --mode "$(value mode)" --workers "${TEST262_WORKERS:-14}" \
    --timeout-ms "$(value timeout_ms)" --allow-failures
