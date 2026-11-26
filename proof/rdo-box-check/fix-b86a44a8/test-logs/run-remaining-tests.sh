#!/usr/bin/env bash
set +e
ROOT=/home/ubuntu/forms-validation/rdo-fix-deef60d8
WRAP=/home/ubuntu/forms-validation/nsh2404-rdo-fix-deef60d8.sh
OUT=/home/ubuntu/forms-validation/rdo-fix-deef-proof/test-logs
COREPID=$(cat "$OUT/tests.pid")
while kill -0 "$COREPID" 2>/dev/null; do sleep 5; done
run() {
  local name="$1"; shift
  echo "START $(date -u +%FT%TZ) $name" >> "$OUT/summary.txt"
  "$@" > "$OUT/$name.log" 2>&1
  local rc=$?
  echo "$rc" > "$OUT/$name.exit"
  echo "END $(date -u +%FT%TZ) $name rc=$rc" >> "$OUT/summary.txt"
}
run cargo-bir-app-api-rdo-lists "$WRAP" bash -lc 'cd .. && cargo test --locked -p bir-app-api --lib rdo_lists'
for i in 1 2 3; do
  run "screen-form-page-no-rdo-$i" "$WRAP" bash -lc "env -u BIR_DATABASE_PATH nsh test screens/form/FormPageNoRdo.test.tsx"
done
cp "$ROOT/crates/bir-app-api/src/screen_form_module/catalogs.rs" "$OUT/catalogs.fixed.rs"
python3 - <<'PY'
from pathlib import Path
p=Path('/home/ubuntu/forms-validation/rdo-fix-deef60d8/crates/bir-app-api/src/screen_form_module/catalogs.rs')
s=p.read_text()
s=s.replace('if let Some(o) = schema_list(&field) {\n        return Some(o.iter().filter_map(sel_option_value).collect());\n    }','if let Some(o) = field["o"].as_array().filter(|o| !o.is_empty()) {\n        return Some(o.iter().filter_map(sel_option_value).collect());\n    }')
s=s.replace('if schema_list(&field).is_some() {\n        // A schema list judges only when the binding is the same list.','if field["o"].as_array().is_some_and(|o| !o.is_empty()) {\n        // A schema list judges only when the binding is the same list.')
s=s.replace('&& schema_list(field).is_none()\n            && let Some(rows) = field_options(kind, key, draft)','&& field["o"].as_array().is_none_or(Vec::is_empty)\n            && let Some(rows) = field_options(kind, key, draft)')
p.write_text(s)
PY
run negative-cargo-bir-app-api-rdo-lists "$WRAP" bash -lc 'cd .. && cargo test --locked -p bir-app-api --lib rdo_lists'
run negative-screen-form-page-no-rdo "$WRAP" bash -lc 'env -u BIR_DATABASE_PATH nsh test screens/form/FormPageNoRdo.test.tsx'
cd "$ROOT" && git checkout HEAD -- crates/bir-app-api/src/screen_form_module/catalogs.rs
if git diff --quiet -- crates/bir-app-api/src/screen_form_module/catalogs.rs; then echo restored > "$OUT/catalogs-restored.txt"; else echo NOT_RESTORED > "$OUT/catalogs-restored.txt"; fi
echo DONE $(date -u +%FT%TZ) >> "$OUT/summary.txt"
