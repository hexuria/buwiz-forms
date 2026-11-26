#!/usr/bin/env bash
set -u
OUT=/home/ubuntu/forms-validation/rdo-fix-deef-proof/test-logs
WRAP=/home/ubuntu/forms-validation/nsh2404-rdo-fix-deef60d8.sh
ADDR=127.0.0.1:17499
TOKEN=$(python3 -c 'import secrets; print(secrets.token_hex(24))')
export GPUI_AGENT_TOKEN="$TOKEN"
rm -f "$OUT"/assistant-*.json "$OUT"/assistant-server.log
"$WRAP" bash -lc 'cd ..; GPUI_AGENT=1 GPUI_AGENT_ADDR=127.0.0.1:17499 BIR_DATABASE_PATH=/home/ubuntu/forms-validation/rdo-fix-deef-proof/assistant-door.db /home/ubuntu/forms-validation/target-rdo-fix-deef60d8/debug/bir serve' >"$OUT/assistant-server.log" 2>&1 &
SPID=$!
ready=0
for i in $(seq 1 80); do
  if gpui-agent hello --addr "$ADDR" >"$OUT/assistant-hello.json" 2>&1; then ready=1; break; fi
  sleep 0.25
done
if [ "$ready" != 1 ]; then echo not-ready > "$OUT/assistant-result.txt"; kill $SPID 2>/dev/null; exit 1; fi
gpui-agent invoke form.open --addr "$ADDR" --arg code=1601C --arg year=2026 --arg period=11 --arg tin=00011124500000 >"$OUT/assistant-open.json" 2>&1
echo $? > "$OUT/assistant-open.exit"
gpui-agent invoke form.fill --addr "$ADDR" --arg 'fields={"frm1601c:txtRDOCode":"000"}' >"$OUT/assistant-fill000.json" 2>&1
echo $? > "$OUT/assistant-fill000.exit"
gpui-agent invoke form.validate --addr "$ADDR" >"$OUT/assistant-validate.json" 2>&1
echo $? > "$OUT/assistant-validate.exit"
gpui-agent invoke form.dismiss --addr "$ADDR" --arg discard=true >"$OUT/assistant-dismiss.json" 2>&1
GPUI_AGENT_TOKEN="$TOKEN" gpui-agent shutdown --addr "$ADDR" >"$OUT/assistant-shutdown.json" 2>&1 || true
sleep 1
kill $SPID 2>/dev/null || true
wait $SPID 2>/dev/null || true
echo done > "$OUT/assistant-result.txt"
