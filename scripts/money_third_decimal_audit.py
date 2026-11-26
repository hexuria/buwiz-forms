#!/usr/bin/env python3
"""Build the report-only third-decimal money audit tables.

Inputs are the rules corpus and the JSON emitted by the Rust
`money_third_decimal_audit_matrix` harness. The script changes no app behavior.
"""

from __future__ import annotations

import csv
import json
import math
import re
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PROBES = [1000.005, 1.005, 2.675]


def load_json(path: Path):
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def money_fields(form_dir: Path) -> list[dict]:
    data = load_json(form_dir / "fields.json")
    out = []
    for field in data.get("fields", []):
        logical = str(field.get("logical_type") or "").lower()
        normalization = " ".join(map(str, field.get("normalization") or [])).lower()
        if (
            "money" in logical
            or "amount" in logical
            or "decimal" in logical
            or "numwithcomma" in normalization
            or "formatcurrency" in normalization
        ):
            out.append(field)
    return out


def text_of_rule(rule: dict) -> str:
    return " ".join(
        str(part)
        for part in [
            rule.get("rule_id"),
            rule.get("phase"),
            rule.get("condition"),
            rule.get("official_behavior"),
            " ".join(map(str, rule.get("fields") or [])),
        ]
        if part
    ).lower()


def blur_rules(form_dir: Path) -> list[dict]:
    path = form_dir / "validations.json"
    if not path.exists():
        return []
    data = load_json(path)
    matches = []
    for rule in data.get("rules", []):
        text = text_of_rule(rule)
        if "blur" not in text and "change" not in text:
            continue
        if not any(
            word in text
            for word in [
                "money",
                "amount",
                "decimal",
                "currency",
                "numwithcomma",
                "formatcurrency",
                "round",
                "tofixed",
            ]
        ):
            continue
        score = 0
        behavior = str(rule.get("official_behavior") or "").lower()
        rule_id = str(rule.get("rule_id") or "").lower()
        if "round" in behavior or "tofixed" in behavior or "formatcurrency" in behavior:
            score += 5
        if "half-up" in behavior or "tofixed(2)" in behavior:
            score += 3
        if "money" in text or "amount" in text:
            score += 2
        if "blur" in rule_id or "input" in rule_id or "amount-format" in rule_id:
            score += 2
        matches.append((score, rule))
    return [rule for score, rule in sorted(matches, key=lambda pair: pair[0], reverse=True) if score >= 5]


def classify_rounding(text: str) -> str:
    lower = text.lower()
    if "rounddown" in lower or "round down" in lower or "rounddowncomputation" in lower:
        return "round-down"
    if (
        "whole-peso" in lower
        or "tofixed(0)" in lower
        or "parseint" in lower
        or "formatcurrencywoc" in lower
        or "math.round to whole peso" in lower
    ):
        return "whole-peso"
    explicit_shared = (
        "half-up" in lower
        or "0.50000000001" in lower
        or "round(...,2)" in lower
        or "round(this,2)" in lower
        or "round/format to two decimals" in lower
    )
    binary = (
        ("parsefloat" in lower and "tofixed(2)" in lower)
        or "number.tofixed(2)" in lower
        or "tofixed(2)" in lower
    ) and not explicit_shared
    shared = explicit_shared or "formatcurrency" in lower
    if binary:
        return "parseFloat/toFixed(2)"
    if shared:
        return "formatCurrency/round"
    if "precisecompute" in lower or "roundamount" in lower:
        return "preciseCompute/roundAmount"
    return "unresolved"


def classify_normalizations(normalizations: list[str]) -> str:
    classes = []
    for item in normalizations:
        cls = classify_rounding(item)
        if cls != "unresolved" and cls not in classes:
            classes.append(cls)
    if not classes:
        return "unresolved"
    if len(classes) == 1:
        return classes[0]
    return "mixed: " + " + ".join(classes)


def computation_rounding(form_dir: Path) -> tuple[str, list[str]]:
    path = form_dir / "calculations.json"
    if not path.exists():
        return "unresolved", []
    data = load_json(path)
    rounding = []
    for calc in data.get("calculations", []):
        text = str(calc.get("rounding") or "")
        if text and text not in rounding:
            rounding.append(text)
    classes = []
    for text in rounding:
        cls = classify_rounding(text)
        if cls not in classes:
            classes.append(cls)
    if not classes:
        return "unresolved", rounding
    if len(classes) == 1:
        return classes[0], rounding
    return "mixed: " + " + ".join(classes), rounding


def inventory() -> dict[str, dict]:
    out = {}
    for form_dir in sorted((ROOT / "rules/forms").iterdir()):
        if not form_dir.is_dir():
            continue
        fields = money_fields(form_dir)
        if not fields:
            continue
        normalizations = []
        for field in fields:
            for item in field.get("normalization") or []:
                item = str(item)
                if item not in normalizations:
                    normalizations.append(item)
        rules = blur_rules(form_dir)
        primary = rules[0] if rules else None
        behavior = str((primary or {}).get("official_behavior") or "")
        rule_text = behavior or " ".join(normalizations)
        blur_class = classify_rounding(rule_text)
        if behavior and blur_class == "unresolved":
            blur_class = classify_normalizations(normalizations)
        elif not behavior:
            blur_class = classify_normalizations(normalizations)
        compute_class, compute_texts = computation_rounding(form_dir)
        counts = Counter(str(field.get("logical_type") or "unknown") for field in fields)
        input_count = sum(
            1
            for field in fields
            if not field.get("computed") and field.get("required") != "computed"
        )
        out[form_dir.name] = {
            "money_field_count": len(fields),
            "input_money_field_count": input_count,
            "logical_type_counts": dict(sorted(counts.items())),
            "normalization": normalizations,
            "blur_rule_id": (primary or {}).get("rule_id"),
            "blur_official_behavior": behavior or None,
            "blur_rule_count": len(rules),
            "all_blur_rule_ids": [rule.get("rule_id") for rule in rules],
            "blur_rounding": blur_class,
            "computation_rounding": compute_class,
            "computation_rounding_texts": compute_texts,
        }
    return out


def as_float(value) -> float | None:
    if value is None:
        return None
    if isinstance(value, (int, float)):
        return float(value)
    if isinstance(value, str):
        try:
            return float(value.replace(",", ""))
        except ValueError:
            return None
    return None


def binary_expected(probe: float) -> float:
    return float(f"{probe:.2f}")


def shared_expected(probe: float) -> float:
    return math.floor(probe * 100 + 0.50000000001) / 100


def down_expected(probe: float) -> float:
    return math.floor(probe * 100) / 100


def close(a: float | None, b: float | None) -> bool:
    return a is not None and b is not None and abs(a - b) < 0.0000001


def expected_for(cls: str, probe: float) -> float | None:
    if cls == "parseFloat/toFixed(2)":
        return binary_expected(probe)
    if cls == "formatCurrency/round":
        return shared_expected(probe)
    if cls == "round-down":
        return down_expected(probe)
    return None


def ours_class(record: dict) -> str:
    if record.get("outcome") == "refused":
        return "refused"
    probe = as_float(record.get("probe"))
    stored = as_float(record.get("stored"))
    if close(stored, probe):
        return "kept-3dp"
    if close(stored, binary_expected(probe or 0.0)):
        return "rounded-parseFloat/toFixed(2)"
    if close(stored, shared_expected(probe or 0.0)):
        return "rounded-formatCurrency/round"
    if close(stored, down_expected(probe or 0.0)):
        return "rounded-down"
    if stored is None:
        return "accepted-no-numeric-store"
    return f"rounded-other:{stored:.15g}"


def compare(record: dict, form_info: dict) -> tuple[str, str]:
    official = form_info["blur_rounding"]
    ours = ours_class(record)
    probe = as_float(record.get("probe"))
    expected = expected_for(official, probe or 0.0)
    if ours == "refused":
        refusal = str(record.get("refusal") or "").lower()
        amountish = any(
            word in refusal
            for word in ["amount", "decimal", "character", "digit", "non-negative", "smaller"]
        )
        if amountish:
            return ours, "mismatch-wrong-refusal"
        return ours, "inconclusive-gated"
    stored = as_float(record.get("stored"))
    if stored is None:
        return ours, "inconclusive-no-numeric-store"
    if stored == 0.0 and (probe or 0.0) >= 1.0:
        return ours, "inconclusive-dropped-in-incomplete-draft"
    if official == "whole-peso":
        if close(stored, probe):
            return ours, "mismatch-kept-3dp"
        if stored is not None and abs(stored - round(stored)) < 0.0000001:
            return ours, "match-whole-peso"
        return ours, "mismatch-different-rounding"
    if expected is None:
        return ours, "unclear-official"
    if close(stored, probe):
        return ours, "mismatch-kept-3dp"
    if close(stored, expected):
        return ours, "match"
    return ours, "mismatch-different-rounding"


def fmt(value) -> str:
    if value is None:
        return "—"
    if isinstance(value, float):
        return f"{value:.15g}"
    text = str(value).replace("\n", " ").replace("|", "\\|")
    return text


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: money_third_decimal_audit.py <harness-json> <out-dir>", file=sys.stderr)
        return 2
    harness = load_json(Path(sys.argv[1]))
    out_dir = Path(sys.argv[2])
    out_dir.mkdir(parents=True, exist_ok=True)
    inv = inventory()
    rows = []
    for record in harness["records"]:
        package = record["package"]
        info = inv.get(package)
        if not info:
            continue
        ours, match = compare(record, info)
        upload = record.get("upload")
        rows.append(
            {
                "form": record["form"],
                "package": package,
                "category": record["category"],
                "box": record["box"],
                "official_blur_rule": info.get("blur_rule_id") or "no validation rule; fields.json normalization",
                "official_blur_rounding": info["blur_rounding"],
                "official_computation_rounding": info["computation_rounding"],
                "probe": record["probe"],
                "outcome": record["outcome"],
                "refusal": record.get("refusal"),
                "stored": record.get("stored"),
                "upload": upload,
                "ours": ours,
                "match": match,
            }
        )

    (out_dir / "official-inventory.json").write_text(
        json.dumps(inv, indent=2, sort_keys=True), encoding="utf-8"
    )
    with (out_dir / "probe-table.csv").open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)

    mismatch_rank = {
        "mismatch-kept-3dp": 1,
        "mismatch-wrong-refusal": 2,
        "mismatch-different-rounding": 3,
        "unclear-official": 4,
        "inconclusive-gated": 5,
        "inconclusive-dropped-in-incomplete-draft": 6,
        "inconclusive-no-numeric-store": 7,
        "match": 9,
        "match-whole-peso": 9,
    }
    worst = {}
    for row in rows:
        key = (row["form"], row["category"], row["box"])
        current = worst.get(key)
        if current is None or mismatch_rank[row["match"]] < mismatch_rank[current["match"]]:
            worst[key] = row
    differing = sorted(
        (row for row in worst.values() if row["match"].startswith("mismatch-")),
        key=lambda row: (mismatch_rank[row["match"]], row["form"], row["category"]),
    )
    caveats = sorted(
        (
            row
            for row in worst.values()
            if row["match"] not in {"match", "match-whole-peso"} and not row["match"].startswith("mismatch-")
        ),
        key=lambda row: (mismatch_rank[row["match"]], row["form"], row["category"]),
    )

    lines = [
        "# Money-box third-decimal audit",
        "",
        "Report only; no behavior is changed. Probe values: `1000.005`, `1.005`, `2.675`.",
        "",
        "## Official blur and computation inventory",
        "",
        "| Form | Money boxes (input/total) | Blur rule | Official blur behavior quoted by the rules | Blur rounding | Computation rounding |",
        "|---|---:|---|---|---|---|",
    ]
    for package, info in sorted(inv.items()):
        rule = info["blur_rule_id"] or "none recorded"
        behavior = info["blur_official_behavior"] or "normalization: " + "; ".join(info["normalization"][:4])
        lines.append(
            f"| {package} | {info['input_money_field_count']}/{info['money_field_count']} | `{rule}` | {fmt(behavior)} | {info['blur_rounding']} | {fmt(info['computation_rounding'])} |"
        )
    lines += [
        "",
        "## Probe table",
        "",
        "| Form | Box | Kind | Official rule | Probe | Ours | Stored | Upload | Match |",
        "|---|---|---|---|---:|---|---:|---:|---|",
    ]
    for row in rows:
        ours = row["ours"] if row["outcome"] != "refused" else "refused: " + (row["refusal"] or "")
        lines.append(
            f"| {row['form']} | `{row['box']}` | {row['category']} | `{row['official_blur_rule']}` | {row['probe']} | {fmt(ours)} | {fmt(row['stored'])} | {fmt(row['upload'])} | {row['match']} |"
        )
    lines += [
        "",
        "## Forms that differ, worst first",
        "",
        "| Rank | Form | Box | Kind | Ours | Why (1 stored three decimals, 2 wrong refusal, 3 different rounding) |",
        "|---:|---|---|---|---|---|",
    ]
    for row in differing:
        rank = mismatch_rank[row["match"]]
        lines.append(
            f"| {rank} | {row['form']} | `{row['box']}` | {row['category']} | {fmt(row['ours'])} | {row['match']} |"
        )
    lines += [
        "",
        "## Official-rule and probe caveats",
        "",
        "These rows are not counted as mismatches: the rules corpus does not settle the exact blur rounding, the first representative was gated, or an incomplete draft dropped/held the value before the probe could measure entry rounding.",
        "",
        "| Rank | Form | Box | Kind | Ours | Why (4 official rule unclear, 5 probe gated, 6 value dropped, 7 no numeric stored value) |",
        "|---:|---|---|---|---|---|",
    ]
    for row in caveats:
        rank = mismatch_rank[row["match"]]
        lines.append(
            f"| {rank} | {row['form']} | `{row['box']}` | {row['category']} | {fmt(row['ours'])} | {row['match']} |"
        )
    lines.append("")
    (out_dir / "REPORT.md").write_text("\n".join(lines), encoding="utf-8")

    print(f"forms inventoried: {len(inv)}")
    print(f"probe rows: {len(rows)}")
    print(f"differing representatives: {len(differing)}")
    for row in differing[:30]:
        print(row["form"], row["category"], row["box"], row["ours"], row["match"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
