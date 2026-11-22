#!/usr/bin/env python3
"""Generate a taxpayer's own 1701Q recipe from their filing history, then run it over a CSV data set.

Usage: GPUI_AGENT_TOKEN=... python3 generated_recipe_runs.py --cli path/to/gpui-agent --addr 127.0.0.1:PORT
       [--data datasets/1701q.csv] [--out runs-generated] [--rows 3-10] [--history 2]

Against a host on a fresh scratch database: adds the test taxpayer, files the first --history CSV rows with the hand-written
recipe (the "history"), asks the app for `recipe.generate`, checks the recipe with `gpui-agent recipe validate`,
then runs it for the other rows. Each row is first checked against the generated `input_schema`'s required list
and `form.check` (inside the recipe, strict unless the row expects errors). A run passes when the receipt is ok,
or, for a row that expects validation errors, when the recipe stops at `check` before any write and the check
names every field in the row's `expect_error_fields`. With --tin nothing is filed first unless --history is given.
Writes OUT/recipe.json, OUT/input_schema.json, OUT/results.md.
"""
import argparse, csv, json, os, shlex, subprocess, sys, time

HERE = os.path.dirname(os.path.abspath(__file__))
# Data-set column -> official box.
BOXES = {"sheets": "frm1701q:txtSheets", "filer_type": "frm1701q:optType", "atc": "frm1701q:optATC",
         "birth_month": "frm1701q:txtBirthMonth", "birth_day": "frm1701q:txtBirthDay", "birth_year": "frm1701q:txtBirthYear",
         "citizenship": "frm1701q:txtCitizenship", "deduction": "frm1701q:optMethodOfDeduction", "sales": "frm1701q:txt36A",
         "cost_of_sales": "frm1701q:txt37A", "deductions": "frm1701q:txt39A", "prior_income": "frm1701q:txt42A",
         "non_op_desc": "frm1701q:txt43Desc", "non_op_income": "frm1701q:txt43A", "prior_credits": "frm1701q:txt55A",
         "prior_payments": "frm1701q:txt56A", "withheld_2307": "frm1701q:txt58A"}

def run_shot_cmd(shot_cmd, path):
    # Only {out} is a placeholder: any other brace in the command is the command's own.
    argv = [part.replace("{out}", path) for part in shlex.split(shot_cmd)]
    return subprocess.run(argv)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--cli", required=True)
    ap.add_argument("--addr", required=True)
    ap.add_argument("--data", default=os.path.join(HERE, "datasets", "1701q.csv"))
    ap.add_argument("--out", default="runs-generated")
    ap.add_argument("--rows", default="3-10")
    ap.add_argument("--include-unused", action="store_true",
                    help="keep boxes the history never filled as optional params")
    ap.add_argument("--history", help="how many leading rows to file first as the taxpayer's history "
                                      "(default 2, or 0 with --tin: an existing taxpayer keeps its own history)")
    ap.add_argument("--tin", help="use this existing taxpayer (and whatever it already filed) instead of adding the data set's")
    ap.add_argument("--generate-arg", action="append", default=[], metavar="NAME=JSON",
                    help="extra recipe.generate argument, e.g. keep_open=true or check='\"report\"'")
    ap.add_argument("--recipe-from", metavar="DIR",
                    help="run the recipe an earlier call wrote to DIR instead of generating one")
    ap.add_argument("--expect-stop", metavar="STEP", help="every run must stop at this step (a stale recipe: template)")
    ap.add_argument("--shot-cmd", metavar="CMD",
                    help="command split shell-style and run without a shell; {out} is the PNG to write")
    ap.add_argument("--pause-ms", type=int, default=0, help="pause after each row, outside the timed run")
    a = ap.parse_args()
    schema = ["--schema", os.path.join(HERE, "bir.schema.json")]
    os.makedirs(a.out, exist_ok=True)

    def cli(*args):
        ran = subprocess.run([a.cli, *args], capture_output=True, text=True)
        try:
            return json.loads(ran.stdout)
        except ValueError:
            sys.exit(f"{' '.join(args)}: {ran.stdout or ran.stderr}")

    rows = list(csv.DictReader(open(a.data, encoding="utf-8")))
    first, _, last = a.rows.partition("-")
    seeds = rows[: int(a.history if a.history is not None else (0 if a.tin else 2))]
    tin = a.tin or rows[0]["tin"]
    if not a.tin:
        cli("--addr", a.addr, "recipe", "run", os.path.join(HERE, "add-tax-profile.json"), *schema, "--set", f"tin={tin}",
            "--set", "name=TEST TAXPAYER", "--set", "rdo=040", "--set", "address=1 TEST ST", "--set", "zip=1000")
    hand = json.load(open(os.path.join(HERE, "fill-1701q-dataset.json")))
    for seed in seeds:
        sets = [x for p in hand["params"] for x in ("--set", f"{p}={tin if p == 'tin' else seed[p]}")]
        history = cli("--addr", a.addr, "recipe", "run", os.path.join(HERE, "fill-1701q-dataset.json"), *schema, *sets)
        if not history.get("ok"):
            sys.exit(f"history run failed: {history}")

    # Answers the hand-written recipe hard-codes are part of every data set too.
    fill = next(step for step in hand["steps"] if step.get("name") == "form.fill")
    fixed = {box: value for box, value in fill["args"]["fields"].items() if not str(value).startswith("$")}
    more = ["--arg", "include_unused=true"] if a.include_unused else []
    more += [x for arg in a.generate_arg for x in ("--arg", arg)]
    if a.recipe_from:
        made = json.load(open(os.path.join(a.recipe_from, "generated.json")))
    else:
        made = cli("--addr", a.addr, "invoke", "recipe.generate", "--arg", "code=\"1701Q\"", "--arg", f"tin=\"{tin}\"", *more)
        if not made.get("ok", True):
            sys.exit(f"recipe.generate failed: {made}")
        made = made.get("result", made)
    json.dump(made, open(os.path.join(a.out, "generated.json"), "w"), indent=2)
    recipe_path = os.path.join(a.out, "recipe.json")
    json.dump(made["recipe"], open(recipe_path, "w"), indent=2)
    json.dump(made["input_schema"], open(os.path.join(a.out, "input_schema.json"), "w"), indent=2)
    valid = cli("recipe", "validate", recipe_path, *schema)
    if not valid.get("ok"):
        sys.exit(f"generated recipe is not valid: {valid}")
    by_box = {box: name for name, box in made["params"].items()}
    required = set(made["input_schema"]["required"])
    print(f"generated: {len(made['params'])} params ({len(required)} required), {len(made['constants'])} constants, "
          f"{len(made['left_out'])} boxes left out, stamp {made['template_stamp']}")

    lines = ["| run | data set | pass | stopped at | recipe ms | wall ms | notes |", "|---|---|---|---|---|---|---|"]
    ok_all = True
    for n, row in enumerate(rows, 1):
        if not int(first) <= n <= int(last or first):
            continue
        values = {"year": row["year"], "period": row["period"]}
        unmapped = []
        for box, value in fixed.items():
            if box in by_box:
                values[by_box[box]] = value
        for column, box in BOXES.items():
            if box in by_box:
                values[by_box[box]] = row[column]
            elif box not in made["constants"] and row[column].strip("0.") != "":
                unmapped.append(column)
        missing = sorted(required - set(values))
        # A pinned answer wins over the data set: say so when the row asked for something else.
        pinned_over = [f"{column}={made['constants'][box]} (row has {row[column]})" for column, box in BOXES.items()
                       if box in made["constants"] and str(made["constants"][box]) != row[column]]
        sets = [x for name, value in values.items() for x in ("--set", f"{name}={value}")]
        # The profile is the taxpayer's state, not a recipe input: set it as the row has it.
        cli("--addr", a.addr, "invoke", "profile.save", "--arg", f"tin=\"{tin}\"", "--arg", f"email=\"{row['email']}\"")
        start = time.perf_counter()
        receipt = cli("--addr", a.addr, "recipe", "run", recipe_path, *schema, *sets)
        wall = round((time.perf_counter() - start) * 1000)
        json.dump(receipt, open(os.path.join(a.out, f"{n:02d}-{row['case']}.json"), "w"), indent=2)
        expect_errors = int(row.get("expect_errors") or 0)
        failed = next((s for s in receipt.get("steps", []) if not s.get("ok", True)), None)
        stopped = (failed or {}).get("id", "")
        if a.expect_stop:
            good = not receipt.get("ok") and stopped == a.expect_stop
        elif expect_errors:
            # The stop must be for this row's reason: any other refusal at `check` is a failure of the run.
            why = json.dumps([receipt.get("error", ""), failed])
            wanted = [f for f in (row.get("expect_error_fields") or "").replace(";", ",").split(",") if f.strip()]
            not_named = [f.strip() for f in wanted if f"{f.strip()}: " not in why]
            good = not receipt.get("ok") and stopped == "check" and bool(wanted) and not not_named
        else:
            good = bool(receipt.get("ok"))
        good = good and not unmapped
        ok_all &= good
        note = receipt.get("error", "") if not receipt.get("ok") else ""
        if expect_errors and not a.expect_stop and (not_named or not wanted):
            note += f" check did not name: {not_named or 'no expect_error_fields in the row'}"
        if missing:
            note += f" missing required: {missing}"
        if pinned_over:
            note += f" template pins: {pinned_over}"
        if unmapped:
            note += f" not in recipe: {unmapped}"
        if a.shot_cmd:
            path = os.path.join(a.out, f"{n:02d}-{row['case']}.png")
            shot = run_shot_cmd(a.shot_cmd, path)
            if shot.returncode != 0:
                # The row's evidence is missing: the run does not pass.
                good = ok_all = False
                note += f" screenshot failed (exit {shot.returncode})"
        time.sleep(a.pause_ms / 1000)
        lines.append(f"| {n} | {row['case']} | {'yes' if good else 'NO'} | {stopped} | {receipt.get('elapsed_ms')} | {wall} | {note.strip()[:240]} |")
    print("\n".join(lines))
    open(os.path.join(a.out, "results.md"), "w").write("\n".join(lines) + "\n")
    sys.exit(0 if ok_all else 1)


if __name__ == "__main__":
    main()
