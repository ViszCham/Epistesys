#!/usr/bin/env sh
# lc631 host stop-hook fresh-host test (POSIX sh, macOS/Linux)
# usage: lc631-host-hook-test.sh <PLUGIN_ROOT>
set -u
PLUGIN_ROOT="${1:?usage: $0 <plugin_root>}"
export PLUGIN_ROOT
HOOK="$PLUGIN_ROOT/hooks/run-stop-hook.sh"
WORK="$(mktemp -d)"
pass=0; fail=0

report() { # name expected_zero actual_exit extra
  if [ "$2" = "zero" ] && [ "$3" -eq 0 ]; then r=PASS; pass=$((pass+1));
  elif [ "$2" = "nonzero" ] && [ "$3" -ne 0 ]; then r=PASS; pass=$((pass+1));
  else r=FAIL; fail=$((fail+1)); fi
  printf '%s | %s | exit=%s | %s\n' "$r" "$1" "$3" "${4:-}"
}

event() { # turn_id
  printf '{"session_id":"lc631-freshhost-test","transcript_path":null,"cwd":"%s","hook_event_name":"Stop","model":"claude-fable-5","turn_id":"%s","permission_mode":"default","stop_hook_active":false,"last_assistant_message":"fresh host hook test message"}' "$WORK" "$1"
}

# T1 happy path: fresh PLUGIN_DATA, valid event -> exit 0, {"continue":true}, ledger appended
export PLUGIN_DATA="$WORK/data1"
mkdir -p "$PLUGIN_DATA"
out="$(event turn-1 | sh "$HOOK" 2>"$WORK/t1.err")"; ec=$?
extra="stdout=$out"
[ -f "$PLUGIN_DATA/host-output-hook/host-output-v2.jsonl" ] && extra="$extra ledger_lines=$(wc -l < "$PLUGIN_DATA/host-output-hook/host-output-v2.jsonl" | tr -d ' ')" || extra="$extra ledger=MISSING"
case "$out" in *'"continue":true'*) : ;; *) ec=98 ;; esac
report "T1_happy_path" zero "$ec" "$extra"

# T2 second append: same ledger, new turn -> exit 0, ledger grows
out="$(event turn-2 | sh "$HOOK" 2>"$WORK/t2.err")"; ec=$?
lines=$(wc -l < "$PLUGIN_DATA/host-output-hook/host-output-v2.jsonl" 2>/dev/null | tr -d ' ')
[ "${lines:-0}" -ge 2 ] || ec=98
report "T2_second_append" zero "$ec" "ledger_lines=$lines"

# T3 fault: malformed JSON -> nonzero
printf 'not-json' | sh "$HOOK" >"$WORK/t3.out" 2>"$WORK/t3.err"; ec=$?
report "T3_malformed_json" nonzero "$ec" "err=$(head -c 120 "$WORK/t3.err" | tr '\n' ' ')"

# T4 fault: PLUGIN_DATA unset -> nonzero
ec=$(unset PLUGIN_DATA CLAUDE_PLUGIN_DATA; event turn-4 | sh "$HOOK" >/dev/null 2>"$WORK/t4.err"; echo $?)
report "T4_no_plugin_data" nonzero "$ec" "err=$(head -c 120 "$WORK/t4.err" | tr '\n' ' ')"

# T5 fault: tampered ledger -> nonzero (chain verification must catch)
LEDGER="$PLUGIN_DATA/host-output-hook/host-output-v2.jsonl"
if [ -f "$LEDGER" ]; then
  sed '1s/./X/20' "$LEDGER" > "$LEDGER.tmp" && mv "$LEDGER.tmp" "$LEDGER"
  event turn-5 | sh "$HOOK" >"$WORK/t5.out" 2>"$WORK/t5.err"; ec=$?
  report "T5_tampered_ledger" nonzero "$ec" "err=$(head -c 120 "$WORK/t5.err" | tr '\n' ' ')"
else
  report "T5_tampered_ledger" nonzero 0 "ledger missing, cannot tamper"
fi

# T6 platform dispatch info (not an assertion of failure)
printf 'platform=%s:%s binary=' "$(uname -s)" "$(uname -m)"
case "$(uname -s):$(uname -m)" in
  Darwin:arm64) b="$PLUGIN_ROOT/scripts/lc631/bin/macos-arm64/lc631" ;;
  Linux:x86_64) b="$PLUGIN_ROOT/scripts/lc631/bin/linux-x86_64/lc631" ;;
  *) b="(unsupported)" ;;
esac
if [ -x "$b" ]; then echo "present"; else echo "MISSING ($b)"; fi

echo "RESULT pass=$pass fail=$fail work=$WORK"
[ "$fail" -eq 0 ]
