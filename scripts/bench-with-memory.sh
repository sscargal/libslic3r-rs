#!/bin/bash
# Runs a single criterion benchmark with /usr/bin/time -v to capture peak RSS.
# Usage: ./scripts/bench-with-memory.sh <package> <bench-name> [extra-cargo-args...]
# Output: Appends bencher-format timing to bench-results/output.txt
#         Appends "MEMORY:<bench-name>:<peak_rss_kb>kB" to bench-results/memory.txt
set -euo pipefail

PACKAGE="$1"
BENCH_NAME="$2"
shift 2

mkdir -p bench-results

TIME_OUTPUT=$(mktemp)

# Run bench: stdout = bencher format timing, stderr = /usr/bin/time stats
/usr/bin/time -v cargo bench -p "$PACKAGE" --bench "$BENCH_NAME" "$@" \
  -- --output-format bencher 2>"$TIME_OUTPUT" | tee -a bench-results/output.txt

# Extract peak RSS from /usr/bin/time output
PEAK_RSS=$(grep "Maximum resident set size" "$TIME_OUTPUT" | awk '{print $NF}')
echo "MEMORY:${BENCH_NAME}:${PEAK_RSS}kB" >> bench-results/memory.txt

cat "$TIME_OUTPUT" >&2
rm -f "$TIME_OUTPUT"
