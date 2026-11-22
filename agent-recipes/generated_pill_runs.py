#!/usr/bin/env python3
"""Combined run: window opens the return, the generated recipe (keep_open) fills it on the data plane,
the window walks the pill, the data plane saves and closes. generated_pill_runs.py OUT WINDOW DATA ROWS [phone]"""
import csv, json, os, subprocess, sys, time
out, W, D, rows_arg = sys.argv[1:5]; phone = "-phone" if len(sys.argv) > 5 else ""
A = os.path.dirname(os.path.abspath(__file__)); CLI = os.environ.get("CLI", "gpui-agent"); T = "12345678800000"
sys.path.insert(0, A)
BOXES = {"sheets": "frm1701q:txtSheets", "filer_type": "frm1701q:optType", "atc": "frm1701q:optATC",
         "birth_month": "frm1701q:txtBirthMonth", "birth_day": "frm1701q:txtBirthDay", "birth_year": "frm1701q:txtBirthYear",
         "citizenship": "frm1701q:txtCitizenship", "deduction": "frm1701q:optMethodOfDeduction", "sales": "frm1701q:txt36A",
         "cost_of_sales": "frm1701q:txt37A", "deductions": "frm1701q:txt39A", "prior_income": "frm1701q:txt42A",
         "non_op_desc": "frm1701q:txt43Desc", "non_op_income": "frm1701q:txt43A", "prior_credits": "frm1701q:txt55A",
         "prior_payments": "frm1701q:txt56A", "withheld_2307": "frm1701q:txt58A"}
SCHEMA = ["--schema", f"{A}/bir.schema.json"]
# A row's expect_error_fields -> the pill recipe that fixes it.
AFTER = {"": "clean", "item_36_taxpayer": "fix-36a", "email": "fix-email"}
os.makedirs(out, exist_ok=True)
def run(*args):
    r = subprocess.run([CLI, *args], capture_output=True, text=True)
    try: return json.loads(r.stdout)
    except ValueError: return {"ok": False, "error": (r.stdout or r.stderr)[:400], "steps": []}
rows = list(csv.DictReader(open(f"{A}/datasets/1701q.csv")))
recipes = {}
for mode in ("strict", "report"):
    made = run("--addr", D, "invoke", "recipe.generate", "--arg", 'code="1701Q"', "--arg", f'tin="{T}"',
               "--arg", "include_unused=true", "--arg", "keep_open=true", "--arg", f'check="{mode}"')
    if not made.get("ok"): sys.exit(f"recipe.generate {mode}: {made}")
    made = made["result"]; json.dump(made, open(f"{out}/generated-{mode}.json", "w"), indent=2)
    json.dump(made["recipe"], open(f"{out}/recipe-{mode}.json", "w"), indent=2)
    v = run("recipe", "validate", f"{out}/recipe-{mode}.json", *SCHEMA)
    print(mode, "validate ok" if v.get("ok") else v, "steps", [s["id"] for s in made["recipe"]["steps"]])
    recipes[mode] = made
lines = ["| run | data set | pass | recipe.generate check | open (window) ms | generated recipe ms (per step) | pill (window) ms | close ms | data-plane verification | screenshots | notes |", "|---|---|---|---|---|---|---|---|---|---|---|"]
allok = True
for n in map(int, rows_arg.split(",")):
    row = rows[n - 1]; bad = row["expect_error_fields"]
    if bad not in AFTER:
        # No pill recipe covers this value: the row fails before anything is opened, and the other rows still run.
        allok = False
        lines.append(f"| {n} | {row['case']} | NO | - | - | - | - | - | - | 0 | expect_error_fields {bad!r} has no pill recipe (known: {', '.join(k for k in AFTER if k)}) |")
        continue
    mode = "report" if bad else "strict"; made = recipes[mode]
    by_box = {box: name for name, box in made["params"].items()}
    vals = {"year": row["year"], "period": row["period"]}
    if "frm1701q:optTaxRate" in by_box: vals[by_box["frm1701q:optTaxRate"]] = "Graduated"
    for col, box in BOXES.items():
        if box in by_box: vals[by_box[box]] = row[col]
    sets = [x for k, v in vals.items() for x in ("--set", f"{k}={v}")]
    d = f"{out}/{n:02d}-{row['case']}"; os.makedirs(d, exist_ok=True); why = []
    def stage(name, addr, recipe, *extra, shots=False):
        cmd = ["--addr", addr, "recipe", "run", recipe, *extra, "--receipt-out", f"{d}/{name}.json"]
        if shots: cmd += ["--screenshot-dir", f"{d}/{name}-shots", "--screenshot-flagged"]
        r = run(*cmd)
        f = next((s for s in r.get("steps", []) if not s.get("ok")), None)
        if not r.get("ok"): why.append(f"{name} {f['id'] + ': ' + str(f.get('error')) if f else r.get('error')}")
        return r
    run("--addr", D, "invoke", "profile.save", "--arg", f'tin="{T}"', "--arg", f'email="{row["email"]}"')
    # The fill writes the date of birth. The window reads the profile when the return opens; with the same date
    # already there it has nothing to ask ("Save date of birth to the tax profile?" would cover the page).
    born = [row[k] for k in ("birth_year", "birth_month", "birth_day")]
    if all(born):
        seeded = run("--addr", D, "invoke", "profile.save", "--arg", f'tin="{T}"', "--arg", f'birth_date="{"-".join(born)}"')
        if not seeded.get("ok"): why.append(f"0-seed profile.save birth_date: {seeded.get('error', seeded)}")
    o = stage("1-open", W, f"{A}/ui/ui-start-1701q-template{phone}.wants", "--set", f"year={row['year']}", "--set", f"period={row['period']}", "--set", f"sheets={row['sheets']}", shots=True)
    g = p = c = {}
    if not why:
        g = stage("2-generated", D, f"{out}/recipe-{mode}.json", *SCHEMA, *sets)
    if not why:
        after = AFTER[bad]
        pill = f"{A}/ui/ui-1701q-pill-{after}{phone}.wants"; text = open(pill).read()
        have = {"sales_shown": f"{float(row['sales']):,.2f}", "sales_fixed": f"{float(row['sales']):.0f}", "fix_email": "fixed@example.com"}
        p = stage("3-pill", W, pill, *[x for k, v in have.items() if f"${k}" in text for x in ("--set", f"{k}={v}")], shots=True)
    c = stage("4-close", D, f"{A}/ui/close-form.json", *SCHEMA)
    v = {}
    verified_ok = False
    if not why and bad:
        opened = run("--addr", D, "invoke", "form.open", "--arg", 'code="1701Q"',
                     "--arg", f"year={row['year']}", "--arg", f"period={row['period']}", "--arg", f'tin="{T}"')
        if not opened.get("ok"):
            why.append(f"5-verify form.open: {opened.get('error', opened)}")
        else:
            try:
                v = run("--addr", D, "invoke", "form.fields", "--arg", "locked=true")
                fields = v.get("result", v)
                key = "frm1701q:txt36A" if bad == "item_36_taxpayer" else "txtEmail"
                expected = f"{float(have['sales_fixed']):,.2f}" if bad == "item_36_taxpayer" else "fixed@example.com"
                box = next((box for box in fields.get("boxes", []) if box.get("key") == key), None)
                actual = box.get("value") if box else None
                if not v.get("ok") or actual != expected:
                    why.append(f"5-verify {key}: expected {expected!r}, got {actual!r}")
                else:
                    verified_ok = True
            finally:
                try:
                    cleanup = run("--addr", D, "invoke", "form.dismiss", "--arg", "discard=true")
                    if not cleanup.get("ok"):
                        why.append(f"5-verify form.dismiss: {cleanup.get('error', cleanup)}")
                except Exception as error:
                    why.append(f"5-verify form.dismiss: {error}")
    nshots = sum(len([s for s in r.get("screenshots", []) if s.get("ok")]) for r in (o, p))
    per = " + ".join(f"{s['id']} {s['elapsed_ms']}" for s in g.get("steps", []))
    allok &= not why
    verified = "skipped (clean)" if not bad else ("yes" if verified_ok else "NO")
    lines.append(f"| {n} | {row['case']} | {'yes' if not why else 'NO'} | {mode} | {o.get('elapsed_ms')} | {g.get('elapsed_ms')} ({per}) | {p.get('elapsed_ms')} | {c.get('elapsed_ms')} | {verified} | {nshots} | {'; '.join(why)[:400]} |")
    time.sleep(1)
open(f"{out}/results.md", "w").write("\n".join(lines) + "\n"); print("\n".join(lines)); sys.exit(0 if allok else 1)
