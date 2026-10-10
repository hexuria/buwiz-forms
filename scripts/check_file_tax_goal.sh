#!/usr/bin/env bash
# Done-condition for the /file-tax goal (.claude/GOAL.md, "Done when").
#
# Exits 0 only when every item holds:
#   1. the bir-desktop agent:: lib tests pass, except the three pre-existing
#      failures listed under GOAL "Blocked" (one of those passing is fine);
#   2. each named feature test exists and passes;
#   3. the bir-mcp tests pass, including the one asserting tools/list has no
#      submit/queue/file/pay tool;
#   4. an end-to-end smoke: bir-headless serve (temp DB + temp registry), then
#      bir-mcp over stdio (scripts/file_tax_smoke.py);
#   5. integrations/opengrok/file-tax/ holds the skill and the MCP config.
#
# Test outcomes are read from cargo's own output (`test ... ok|FAILED` and
# `test result:` lines), never from a wrapper's exit code. Cargo is called
# directly, not through `rtk`. Never weaken this script.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT" || exit 2

WORK="$(mktemp -d "${TMPDIR:-/tmp}/file-tax-goal.XXXXXX")"
cleanup() { rm -rf "$WORK"; }
trap cleanup EXIT

# No failure is allowed: the three stale tests that used to be listed here
# were fixed (dummy TIN -> check-digit-valid TIN, 1,000.00 amount format,
# dismiss before opening a second form).
ALLOWED_PREEXISTING_FAILURES=()

# Named feature tests (GOAL "Done when"). Each must exist and pass.
NAMED_FEATURE_TESTS=(
  one_form_lock_refuses_second_open
  dismiss_clean_form_closes_it
  agent_dismiss_with_unsaved_edits_is_refused
  fill_records_source_and_never_overwrites_user_box
  needs_you_lists_required_empty_boxes
  validate_returns_field_errors
  form_context_exposes_profile_and_past_returns
)

# The bir-mcp test that asserts tools/list exposes no submit/queue/file/pay tool.
BIR_MCP_NO_FORBIDDEN_TOOL_TEST=tools_list_has_no_submit_queue_file_or_pay_tool

MCP_CRATE_DIR=crates/bir-mcp
INTEGRATION_DIR=integrations/opengrok/file-tax

FAILURES=()
fail() {
  FAILURES+=("$1")
  echo "FAIL: $1" >&2
}
pass() { echo "ok:   $1"; }

contains() {
  local needle="$1"
  shift
  local item
  for item in "$@"; do
    [[ "$item" == "$needle" ]] && return 0
  done
  return 1
}

# Parse a cargo test log. Prints "<status> <full::test::path>" per test line,
# and sets RESULT_LINES / RESULT_FAILED_TOTAL from the `test result:` lines.
RESULT_LINES=0
RESULT_FAILED_TOTAL=0
parse_results() {
  local log="$1"
  RESULT_LINES=$(grep -cE '^test result: ' "$log" || true)
  RESULT_FAILED_TOTAL=0
  local n
  while read -r n; do
    RESULT_FAILED_TOTAL=$((RESULT_FAILED_TOTAL + n))
  done < <(sed -nE 's/^test result: [A-Za-z]+\. [0-9]+ passed; ([0-9]+) failed;.*/\1/p' "$log")
}

test_lines() {
  # "<name> <ok|FAILED|ignored>" for every per-test line.
  sed -nE 's/^test ([^ ]+) \.\.\. (ok|FAILED|ignored)$/\1 \2/p' "$1"
}

short_name() { echo "${1##*::}"; }

# ---------------------------------------------------------------- 1 + 2
echo "== 1. bir-desktop agent:: lib tests"
DESKTOP_LOG="$WORK/desktop-agent-tests.log"
cargo test --locked -p bir-desktop --features agent --lib agent:: >"$DESKTOP_LOG" 2>&1
parse_results "$DESKTOP_LOG"
if [[ "$RESULT_LINES" -eq 0 ]]; then
  tail -n 40 "$DESKTOP_LOG" >&2
  fail "bir-desktop agent:: tests produced no 'test result:' line (build failure?)"
else
  grep -E '^test result: ' "$DESKTOP_LOG"
  failed_names=()
  while read -r name status; do
    [[ "$status" == "FAILED" ]] && failed_names+=("$name")
  done < <(test_lines "$DESKTOP_LOG")
  if [[ "${#failed_names[@]}" -ne "$RESULT_FAILED_TOTAL" ]]; then
    fail "bir-desktop: ${#failed_names[@]} FAILED lines but 'test result:' reports $RESULT_FAILED_TOTAL failed"
  fi
  unexpected=0
  for name in "${failed_names[@]+"${failed_names[@]}"}"; do
    if contains "$(short_name "$name")" ${ALLOWED_PREEXISTING_FAILURES[@]+"${ALLOWED_PREEXISTING_FAILURES[@]}"}; then
      echo "note: allowed pre-existing failure: $name"
    else
      fail "bir-desktop test failed: $name"
      unexpected=1
    fi
  done
  [[ "$unexpected" -eq 0 ]] && pass "no failing tests"
fi

echo "== 2. named feature tests"
for want in "${NAMED_FEATURE_TESTS[@]}"; do
  found=""
  while read -r name status; do
    if [[ "$(short_name "$name")" == "$want" ]]; then
      found="$status"
      [[ "$status" != "ok" ]] && break
    fi
  done < <(test_lines "$DESKTOP_LOG")
  case "$found" in
    ok) pass "$want" ;;
    "") fail "named feature test missing: $want" ;;
    *) fail "named feature test $want: $found" ;;
  esac
done

# ---------------------------------------------------------------- 3
echo "== 3. bir-mcp tests"
if [[ ! -f "$MCP_CRATE_DIR/Cargo.toml" ]]; then
  fail "$MCP_CRATE_DIR is missing (increment 6 not landed)"
else
  MCP_LOG="$WORK/bir-mcp-tests.log"
  cargo test --locked -p bir-mcp >"$MCP_LOG" 2>&1
  parse_results "$MCP_LOG"
  if [[ "$RESULT_LINES" -eq 0 ]]; then
    tail -n 40 "$MCP_LOG" >&2
    fail "bir-mcp tests produced no 'test result:' line (build failure?)"
  else
    grep -E '^test result: ' "$MCP_LOG"
    mcp_failed=0
    while read -r name status; do
      if [[ "$status" == "FAILED" ]]; then
        fail "bir-mcp test failed: $name"
        mcp_failed=1
      fi
    done < <(test_lines "$MCP_LOG")
    if [[ "$RESULT_FAILED_TOTAL" -ne 0 && "$mcp_failed" -eq 0 ]]; then
      fail "bir-mcp 'test result:' reports $RESULT_FAILED_TOTAL failed"
    fi
    found=""
    while read -r name status; do
      [[ "$(short_name "$name")" == "$BIR_MCP_NO_FORBIDDEN_TOOL_TEST" ]] && found="$status"
    done < <(test_lines "$MCP_LOG")
    case "$found" in
      ok) pass "$BIR_MCP_NO_FORBIDDEN_TOOL_TEST" ;;
      "") fail "bir-mcp test missing: $BIR_MCP_NO_FORBIDDEN_TOOL_TEST" ;;
      *) fail "bir-mcp test $BIR_MCP_NO_FORBIDDEN_TOOL_TEST: $found" ;;
    esac
  fi
fi

# ---------------------------------------------------------------- 4
echo "== 4. end-to-end smoke (bir-headless + bir-mcp over stdio)"
if [[ ! -f "$MCP_CRATE_DIR/Cargo.toml" ]]; then
  fail "smoke needs $MCP_CRATE_DIR (increment 6 not landed)"
else
  BUILD_LOG="$WORK/smoke-build.log"
  if ! cargo build --locked -p bir-desktop --features agent --bin bir-headless >"$BUILD_LOG" 2>&1 \
    || ! cargo build --locked -p bir-mcp >>"$BUILD_LOG" 2>&1; then
    tail -n 40 "$BUILD_LOG" >&2
    fail "smoke: building bir-headless / bir-mcp failed"
  else
    TARGET_DIR="$(cargo metadata --format-version 1 --no-deps 2>/dev/null \
      | python3 -I -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
    HEADLESS_BIN="$TARGET_DIR/debug/bir-headless"
    MCP_BIN="$TARGET_DIR/debug/bir-mcp"
    if [[ ! -x "$HEADLESS_BIN" || ! -x "$MCP_BIN" ]]; then
      fail "smoke: binaries not found ($HEADLESS_BIN, $MCP_BIN)"
    elif python3 -I "$ROOT/scripts/file_tax_smoke.py" \
      --headless "$HEADLESS_BIN" --mcp "$MCP_BIN" --work "$WORK/smoke"; then
      pass "smoke"
    else
      fail "smoke failed (see output above)"
    fi
  fi
fi

# ---------------------------------------------------------------- 5
echo "== 5. $INTEGRATION_DIR"
if [[ ! -d "$INTEGRATION_DIR" ]]; then
  fail "$INTEGRATION_DIR is missing (increment 7 not landed)"
else
  if [[ -s "$INTEGRATION_DIR/SKILL.md" ]]; then
    pass "skill: $INTEGRATION_DIR/SKILL.md"
  else
    fail "$INTEGRATION_DIR/SKILL.md (the /file-tax skill) is missing or empty"
  fi
  mcp_config=""
  for candidate in "$INTEGRATION_DIR"/*.json; do
    [[ -f "$candidate" ]] || continue
    if python3 -I -c 'import json,sys; json.load(open(sys.argv[1]))' "$candidate" 2>/dev/null \
      && grep -q 'bir-mcp' "$candidate"; then
      mcp_config="$candidate"
      break
    fi
  done
  if [[ -n "$mcp_config" ]]; then
    pass "MCP config: $mcp_config"
  else
    fail "$INTEGRATION_DIR has no valid JSON MCP config that launches bir-mcp"
  fi
fi

echo
if [[ "${#FAILURES[@]}" -eq 0 ]]; then
  echo "file-tax goal: DONE"
  exit 0
fi
echo "file-tax goal: NOT DONE (${#FAILURES[@]} failing checks)"
for item in "${FAILURES[@]}"; do
  echo "  - $item"
done
exit 1
