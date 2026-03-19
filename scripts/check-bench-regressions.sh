#!/bin/bash
# Checks benchmark results for BOTH timing and memory regressions.
# Parses bencher-format output (base vs head) for timing regressions.
# Parses memory.txt (base vs head) for memory regressions.
#
# Exit codes:
#   0 = no regressions above block threshold
#   1 = one or more regressions exceed 15% (block threshold)
#
# Emits GitHub Actions annotations:
#   ::warning:: for regressions between 5% and 15%
#   ::error::   for regressions above 15%
#
# Usage: ./scripts/check-bench-regressions.sh <base-output-file> <base-memory-file>
#   base-output-file: path to baseline bencher-format output.txt
#   base-memory-file: path to baseline memory.txt
set -euo pipefail

BLOCK_THRESHOLD=15
WARN_THRESHOLD=5
BLOCKED=0

HEAD_OUTPUT="bench-results/output.txt"
HEAD_MEMORY="bench-results/memory.txt"
BASE_OUTPUT="${1:-}"
BASE_MEMORY="${2:-}"

echo "::group::Benchmark regression analysis"
echo "Thresholds: warn at ${WARN_THRESHOLD}%, block at ${BLOCK_THRESHOLD}%"
echo ""

# --- Timing regression check ---
# Bencher format lines look like:
#   test <name> ... bench: <ns_per_iter> ns/iter (+/- <variance>)
# We extract bench name and ns/iter, compare base vs head.

echo "=== Timing Regressions ==="

if [[ -z "$BASE_OUTPUT" || ! -f "$BASE_OUTPUT" ]]; then
  echo "No baseline timing file provided or found. Skipping timing regression check."
  echo "(Timing baselines are established after the first base-branch benchmark run.)"
else
  if [[ ! -f "$HEAD_OUTPUT" ]]; then
    echo "::error::bench-results/output.txt not found. Benchmark run failed."
    echo "::endgroup::"
    exit 1
  fi

  # Parse bencher-format: "test <name> ... bench: <value> ns/iter (+/- <variance>)"
  # Build associative arrays of bench_name -> ns/iter for base and head
  declare -A BASE_TIMING
  declare -A HEAD_TIMING

  while IFS= read -r line; do
    if [[ "$line" =~ ^test[[:space:]]+(.+)[[:space:]]+\.\.\.[[:space:]]+bench:[[:space:]]+([0-9,]+)[[:space:]]+ns/iter ]]; then
      name="${BASH_REMATCH[1]}"
      # Remove commas from numbers (bencher format uses them for thousands)
      value="${BASH_REMATCH[2]//,/}"
      BASE_TIMING["$name"]="$value"
    fi
  done < "$BASE_OUTPUT"

  while IFS= read -r line; do
    if [[ "$line" =~ ^test[[:space:]]+(.+)[[:space:]]+\.\.\.[[:space:]]+bench:[[:space:]]+([0-9,]+)[[:space:]]+ns/iter ]]; then
      name="${BASH_REMATCH[1]}"
      value="${BASH_REMATCH[2]//,/}"
      HEAD_TIMING["$name"]="$value"
    fi
  done < "$HEAD_OUTPUT"

  if [[ ${#BASE_TIMING[@]} -eq 0 ]]; then
    echo "Warning: No benchmarks parsed from base output. Check bencher format."
  else
    echo "Parsed ${#BASE_TIMING[@]} base benchmarks, ${#HEAD_TIMING[@]} head benchmarks."
    echo ""

    for name in "${!HEAD_TIMING[@]}"; do
      head_val="${HEAD_TIMING[$name]}"
      base_val="${BASE_TIMING[$name]:-}"

      if [[ -z "$base_val" || "$base_val" -eq 0 ]]; then
        echo "  $name: ${head_val} ns/iter (new benchmark, no baseline)"
        continue
      fi

      # Calculate percentage change: (head - base) / base * 100
      # Positive = regression (slower), negative = improvement (faster)
      DELTA=$(( (head_val - base_val) * 100 / base_val ))

      if [[ "$DELTA" -gt "$BLOCK_THRESHOLD" ]]; then
        echo "::error::Timing regression: $name regressed ${DELTA}% (${base_val} -> ${head_val} ns/iter, threshold: ${BLOCK_THRESHOLD}%)"
        BLOCKED=1
      elif [[ "$DELTA" -gt "$WARN_THRESHOLD" ]]; then
        echo "::warning::Timing regression: $name regressed ${DELTA}% (${base_val} -> ${head_val} ns/iter)"
      elif [[ "$DELTA" -lt "-${WARN_THRESHOLD}" ]]; then
        echo "  $name: improved by $(( -DELTA ))% (${base_val} -> ${head_val} ns/iter)"
      else
        echo "  $name: ${DELTA}% change (${base_val} -> ${head_val} ns/iter) -- within tolerance"
      fi
    done
  fi
fi

echo ""

# --- Memory regression check ---
echo "=== Memory Regressions ==="

if [[ -z "$BASE_MEMORY" || ! -f "$BASE_MEMORY" ]]; then
  echo "No baseline memory file provided or found. Skipping memory regression check."
  echo "(Memory baselines are established after the first base-branch benchmark run.)"
else
  if [[ ! -f "$HEAD_MEMORY" ]]; then
    echo "::error::bench-results/memory.txt not found. Benchmark memory tracking failed."
    echo "::endgroup::"
    exit 1
  fi

  echo "Comparing memory: $HEAD_MEMORY vs baseline: $BASE_MEMORY"
  echo ""

  while IFS= read -r line; do
    # Parse MEMORY:<bench-name>:<peak_rss>kB
    if [[ "$line" =~ ^MEMORY:([^:]+):([0-9]+)kB$ ]]; then
      BENCH="${BASH_REMATCH[1]}"
      CURRENT="${BASH_REMATCH[2]}"

      # Find matching baseline
      BASE=$(grep "^MEMORY:${BENCH}:" "$BASE_MEMORY" | sed 's/.*:\([0-9]*\)kB/\1/' || echo "")

      if [[ -z "$BASE" || "$BASE" -eq 0 ]]; then
        echo "  $BENCH: ${CURRENT}kB (no baseline, skipping)"
        continue
      fi

      # Calculate percentage change: (current - base) / base * 100
      DELTA=$(( (CURRENT - BASE) * 100 / BASE ))

      if [[ "$DELTA" -gt "$BLOCK_THRESHOLD" ]]; then
        echo "::error::Memory regression: $BENCH regressed ${DELTA}% (${BASE}kB -> ${CURRENT}kB, threshold: ${BLOCK_THRESHOLD}%)"
        BLOCKED=1
      elif [[ "$DELTA" -gt "$WARN_THRESHOLD" ]]; then
        echo "::warning::Memory regression: $BENCH regressed ${DELTA}% (${BASE}kB -> ${CURRENT}kB)"
      elif [[ "$DELTA" -lt "-${WARN_THRESHOLD}" ]]; then
        echo "  $BENCH: improved by $(( -DELTA ))% (${BASE}kB -> ${CURRENT}kB)"
      else
        echo "  $BENCH: ${DELTA}% change (${BASE}kB -> ${CURRENT}kB) -- within tolerance"
      fi
    fi
  done < "$HEAD_MEMORY"
fi

echo ""
echo "::endgroup::"

if [[ "$BLOCKED" -eq 1 ]]; then
  echo "::error::One or more benchmarks exceed the ${BLOCK_THRESHOLD}% regression threshold. Add the 'bench-ok' label to override."
  exit 1
fi

echo "All benchmarks within thresholds."
exit 0
