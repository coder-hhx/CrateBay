#!/usr/bin/env bash
# CrateBay Performance Benchmark
# Checks release binary size, CLI startup, runtime command health, and core benchmarks:
#   1. CLI binary size
#   2. CLI startup
#   3. Runtime status command health
#   4. Criterion micro-benchmarks (cratebay-core)
#
# Usage:
#   ./scripts/bench-perf.sh [--release-dir DIR]
#
# Exit code 0 if ALL checks pass, 1 if any fail.

set -euo pipefail

# ── Configuration ─────────────────────────────────────────────────
MAX_BINARY_SIZE_MB=20
MAX_STARTUP_TIME_S=3
STARTUP_RUNS=5

# ── Argument parsing ──────────────────────────────────────────────
RELEASE_DIR="target/release"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --release-dir) RELEASE_DIR="$2"; shift 2 ;;
        *) echo "Unknown argument: $1"; exit 2 ;;
    esac
done

# ── Colour helpers (disabled when not a terminal) ─────────────────
if [[ -t 1 ]]; then
    RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[0;33m'
    BOLD='\033[1m'; RESET='\033[0m'
else
    RED=''; GREEN=''; YELLOW=''; BOLD=''; RESET=''
fi

pass() { printf "${GREEN}PASS${RESET}"; }
fail() { printf "${RED}FAIL${RESET}"; }
warn() { printf "${YELLOW}SKIP${RESET}"; }

# ── State tracking ────────────────────────────────────────────────
FAILURES=0
SKIPS=0

# Results arrays (for summary table)
declare -a RESULT_NAMES=()
declare -a RESULT_VALUES=()
declare -a RESULT_LIMITS=()
declare -a RESULT_STATUSES=()

record() {
    # record NAME VALUE LIMIT STATUS
    RESULT_NAMES+=("$1")
    RESULT_VALUES+=("$2")
    RESULT_LIMITS+=("$3")
    RESULT_STATUSES+=("$4")
}

# ── Helper: get file size in bytes (cross-platform) ──────────────
file_size_bytes() {
    local path="$1"
    if [[ "$(uname)" == "Darwin" ]]; then
        stat -f%z "$path"
    else
        stat -c%s "$path"
    fi
}

# ── Helper: compute median from a file of numbers ────────────────
median() {
    sort -n | awk '{a[NR]=$1} END {
        if (NR%2==1) print a[(NR+1)/2];
        else print (a[NR/2]+a[NR/2+1])/2
    }'
}

echo ""
echo "======================================================"
echo " CrateBay Performance Benchmark"
echo "======================================================"
echo ""
echo "Release dir : $RELEASE_DIR"
echo "Platform    : $(uname -s) $(uname -m)"
echo "Date        : $(date -u '+%Y-%m-%d %H:%M:%S UTC')"
echo ""

# ══════════════════════════════════════════════════════════════════
# 1. Binary Size Check (<20MB)
# ══════════════════════════════════════════════════════════════════
echo "──────────────────────────────────────────────────────"
echo " 1. Binary Size Check (limit: <${MAX_BINARY_SIZE_MB}MB per binary)"
echo "──────────────────────────────────────────────────────"

MAX_BYTES=$((MAX_BINARY_SIZE_MB * 1048576))

for bin in cratebay; do
    path="${RELEASE_DIR}/${bin}"
    if [[ ! -f "$path" ]]; then
        printf "  %-20s [$(warn)] binary not found at %s\n" "$bin" "$path"
        record "$bin size" "N/A" "<${MAX_BINARY_SIZE_MB}MB" "SKIP"
        SKIPS=$((SKIPS + 1))
        continue
    fi

    size_bytes=$(file_size_bytes "$path")
    size_mb=$(echo "scale=2; $size_bytes / 1048576" | bc)

    if [[ "$size_bytes" -gt "$MAX_BYTES" ]]; then
        printf "  %-20s %6s MB  [$(fail)]  exceeds %sMB limit\n" "$bin" "$size_mb" "$MAX_BINARY_SIZE_MB"
        record "$bin size" "${size_mb}MB" "<${MAX_BINARY_SIZE_MB}MB" "FAIL"
        FAILURES=$((FAILURES + 1))
    else
        printf "  %-20s %6s MB  [$(pass)]\n" "$bin" "$size_mb"
        record "$bin size" "${size_mb}MB" "<${MAX_BINARY_SIZE_MB}MB" "PASS"
    fi
done
echo ""

# ══════════════════════════════════════════════════════════════════
# 2. Startup Time Check (<3s)
# ══════════════════════════════════════════════════════════════════
echo "──────────────────────────────────────────────────────"
echo " 2. Startup Time Check (limit: <${MAX_STARTUP_TIME_S}s, median of ${STARTUP_RUNS} runs)"
echo "──────────────────────────────────────────────────────"

CLI_BIN="${RELEASE_DIR}/cratebay"

if [[ ! -f "$CLI_BIN" ]]; then
    printf "  [$(warn)] cratebay binary not found, skipping startup benchmark\n"
    record "startup time" "N/A" "<${MAX_STARTUP_TIME_S}s" "SKIP"
    SKIPS=$((SKIPS + 1))
else
    # Check if hyperfine is available
    if command -v hyperfine &>/dev/null; then
        echo "  Using hyperfine for measurement..."
        # hyperfine outputs JSON; extract median
        HYPERFINE_JSON=$(hyperfine --runs "$STARTUP_RUNS" --export-json /dev/stdout \
            --warmup 1 "$CLI_BIN system info" 2>/dev/null) || true

        if [[ -n "$HYPERFINE_JSON" ]]; then
            MEDIAN_S=$(echo "$HYPERFINE_JSON" | \
                python3 -c "import sys,json; d=json.load(sys.stdin); print(d['results'][0]['median'])" 2>/dev/null) || MEDIAN_S=""
        fi

        if [[ -z "${MEDIAN_S:-}" ]]; then
            echo "  hyperfine JSON parsing failed, falling back to bash timing..."
            MEDIAN_S=""
        fi
    fi

    # Fallback: bash timing
    if [[ -z "${MEDIAN_S:-}" ]]; then
        echo "  Using bash timing (${STARTUP_RUNS} runs)..."
        TIMES_FILE=$(mktemp)
        for i in $(seq 1 "$STARTUP_RUNS"); do
            # Use bash TIMEFORMAT to get wall-clock seconds
            START_NS=$( { date +%s%N 2>/dev/null || python3 -c "import time; print(int(time.time()*1e9))"; } )
            "$CLI_BIN" system info >/dev/null 2>&1 || true
            END_NS=$( { date +%s%N 2>/dev/null || python3 -c "import time; print(int(time.time()*1e9))"; } )
            ELAPSED=$(echo "scale=4; ($END_NS - $START_NS) / 1000000000" | bc)
            echo "$ELAPSED" >> "$TIMES_FILE"
            printf "    run %d: %ss\n" "$i" "$ELAPSED"
        done
        MEDIAN_S=$(median < "$TIMES_FILE")
        rm -f "$TIMES_FILE"
    fi

    # Evaluate result
    EXCEEDED=$(echo "$MEDIAN_S > $MAX_STARTUP_TIME_S" | bc -l)
    if [[ "$EXCEEDED" -eq 1 ]]; then
        printf "  Median: %ss  [$(fail)]  exceeds %ss limit\n" "$MEDIAN_S" "$MAX_STARTUP_TIME_S"
        record "startup time" "${MEDIAN_S}s" "<${MAX_STARTUP_TIME_S}s" "FAIL"
        FAILURES=$((FAILURES + 1))
    else
        printf "  Median: %ss  [$(pass)]\n" "$MEDIAN_S"
        record "startup time" "${MEDIAN_S}s" "<${MAX_STARTUP_TIME_S}s" "PASS"
    fi
fi
echo ""

# ══════════════════════════════════════════════════════════════════
# 3. Runtime Status Command
# ══════════════════════════════════════════════════════════════════
echo "──────────────────────────────────────────────────────"
echo " 3. Runtime Status Command"
echo "──────────────────────────────────────────────────────"

if [[ ! -f "$CLI_BIN" ]]; then
    printf "  [$(warn)] cratebay binary not found, skipping runtime status check\n"
    record "runtime status" "N/A" "exit 0" "SKIP"
    SKIPS=$((SKIPS + 1))
else
    set +e
    RUNTIME_STATUS_OUTPUT="$("$CLI_BIN" runtime status 2>&1)"
    RUNTIME_STATUS_CODE=$?
    set -e
    if [[ "$RUNTIME_STATUS_CODE" -eq 0 ]]; then
        printf "  runtime status  [$(pass)]\n"
        record "runtime status" "exit 0" "exit 0" "PASS"
        echo "$RUNTIME_STATUS_OUTPUT" | sed 's/^/    /'
    else
        printf "  runtime status  [$(fail)]\n"
        record "runtime status" "error" "exit 0" "FAIL"
        echo "$RUNTIME_STATUS_OUTPUT" | sed 's/^/    /'
        FAILURES=$((FAILURES + 1))
    fi
fi
echo ""

# ══════════════════════════════════════════════════════════════════
# 4. Criterion Benchmarks (cratebay-core)
# ══════════════════════════════════════════════════════════════════
echo "──────────────────────────────────────────────────────"
echo " 4. Criterion Benchmarks (cratebay-core)"
echo "──────────────────────────────────────────────────────"

# Check if cargo is available
if ! command -v cargo &>/dev/null; then
    printf "  [$(warn)] cargo not found, skipping Criterion benchmarks\n"
    record "criterion bench" "N/A" "run" "SKIP"
    SKIPS=$((SKIPS + 1))
else
    echo "  Running Criterion benchmarks..."
    BENCH_OUTPUT_FILE=$(mktemp)

    if cargo bench -p cratebay-core 2>&1 | tee "$BENCH_OUTPUT_FILE"; then
        printf "  Criterion benchmarks  [$(pass)]\n"
        record "criterion bench" "completed" "run" "PASS"

        # Extract and display key results (time values from Criterion output)
        echo ""
        echo "  Key benchmark results:"
        # Criterion outputs lines like: "bench_name    time:   [123.45 µs 124.56 µs 125.67 µs]"
        while IFS= read -r line; do
            printf "    %s\n" "$line"
        done < <(grep -E "^[a-z_/]+.*time:" "$BENCH_OUTPUT_FILE" 2>/dev/null || true)
    else
        printf "  Criterion benchmarks  [$(fail)]  cargo bench returned non-zero\n"
        record "criterion bench" "error" "run" "FAIL"
        FAILURES=$((FAILURES + 1))
    fi

    rm -f "$BENCH_OUTPUT_FILE"
fi
echo ""

# ══════════════════════════════════════════════════════════════════
# Summary
# ══════════════════════════════════════════════════════════════════
echo "======================================================"
echo " Summary"
echo "======================================================"
echo ""
printf "  ${BOLD}%-20s  %-12s  %-12s  %-6s${RESET}\n" "Metric" "Value" "Limit" "Result"
printf "  %-20s  %-12s  %-12s  %-6s\n" "────────────────────" "────────────" "────────────" "──────"

for i in "${!RESULT_NAMES[@]}"; do
    status="${RESULT_STATUSES[$i]}"
    case "$status" in
        PASS) colour="$GREEN" ;;
        FAIL) colour="$RED" ;;
        *)    colour="$YELLOW" ;;
    esac
    printf "  %-20s  %-12s  %-12s  ${colour}%-6s${RESET}\n" \
        "${RESULT_NAMES[$i]}" "${RESULT_VALUES[$i]}" "${RESULT_LIMITS[$i]}" "$status"
done
echo ""

if [[ "$FAILURES" -gt 0 ]]; then
    echo "${RED}${BOLD}RESULT: $FAILURES check(s) FAILED${RESET}"
    exit 1
elif [[ "$SKIPS" -gt 0 ]]; then
    echo "${YELLOW}${BOLD}RESULT: All run checks passed ($SKIPS skipped)${RESET}"
    exit 0
else
    echo "${GREEN}${BOLD}RESULT: All checks PASSED${RESET}"
    exit 0
fi
