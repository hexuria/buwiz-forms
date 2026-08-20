#!/usr/bin/env python3
"""Named 2550M header harvest. Not a corpus-wide text join.

Copies four leftover-unique, dummy-Save-emitted ``serialized_key`` values
onto the matching 2550M page-1 identities. Uniqueness in ``fields.json`` is
not a join; this table is. Do not generalize ``txtRDOCode`` / ``txtAddress``
/ ``txtZipCode`` to other bundles from this module.

Refuses:
- ``txtEmail`` — inventory ``control_kind`` is hidden/workflow-metadata;
  2550M prints no email caption.
- leftover leaf ``txtTaxpayerName`` — silent wrong spelling. Item 7's key
  is ``frm2550m:txtTaxPayerName``.
- Item 6 line of business and Item 8 telephone — printed boxes exist; they
  are not this harvest.
- any key whose inventory row is hidden, whose inventory count is not 1,
  or whose identity is already keyed to something else.

Does not write HTML ``name=``. ``map_tin.py`` maps R1 after this remint.

Usage:
    python tools/formgen/harvest_2550m_headers.py --self-test
    python tools/formgen/harvest_2550m_headers.py --write
"""

from __future__ import annotations

import argparse
import copy
import json
import pathlib
import sys


HERE = pathlib.Path(__file__).resolve().parent
if str(HERE) not in sys.path:
    sys.path.insert(0, str(HERE))

import field_identity as fi  # noqa: E402
import join_census as jc  # noqa: E402
import leftover_keys as lk  # noqa: E402

EVIDENCE_DIR = HERE / "corrections" / "evidence"

BUNDLE = "2550m-2007"
INVENTORY_DIR = "2550m-v2007"
EXPECTED_JOINS = 4
GAP_NO_UNIQUE = jc.GAP_NO_UNIQUE
EVIDENCE_NAME = "harvest-2550m-headers-20260821.json"
W5_EVIDENCE = "tools/formgen/corrections/evidence/w5-2550m-leftover-20260820.json"

# Identity id → exact serialized_key. Captions are from 2550M page-1
# ``div.t`` runs, not from leftover uniqueness.
JOINS = (
    {
        "id": "2550m-2007/p1/text-4",
        "bundle_slug": BUNDLE,
        "html_id_hint": "p1c130",
        "serialized_key": "frm2550m:txtRDOCode",
        "item_number": "5",
        "caption": "5 RDO Code",
        "source_printed_box_pt": [273.12, 118.8, 324.24, 133.68],
        "w5_emitted": True,
    },
    {
        "id": "2550m-2007/p1/text-6",
        "bundle_slug": BUNDLE,
        "html_id_hint": "p1c17",
        "serialized_key": "frm2550m:txtTaxPayerName",
        "item_number": "7",
        "caption": "7 Taxpayer's Name",
        "source_printed_box_pt": [50.4, 145.44, 445.44, 159.36],
        "w5_emitted": True,
    },
    {
        "id": "2550m-2007/p1/text-8",
        "bundle_slug": BUNDLE,
        "html_id_hint": "p1c20",
        "serialized_key": "frm2550m:txtAddress",
        "item_number": "9",
        "caption": "9 Registered Address",
        "source_printed_box_pt": [50.4, 171.12, 445.44, 185.04],
        "w5_emitted": True,
    },
    {
        "id": "2550m-2007/p1/text-9",
        "bundle_slug": BUNDLE,
        "html_id_hint": "p1c21",
        "serialized_key": "frm2550m:txtZipCode",
        "item_number": "10",
        "caption": "10 Zip Code",
        "source_printed_box_pt": [526.32, 171.12, 574.56, 185.04],
        "w5_emitted": True,
    },
)

# Never join these through this harvest, even if leftover-unique.
REFUSED_LEAVES = frozenset({"txtEmail", "txtTaxpayerName"})
REFUSED_KEYS = frozenset({"txtEmail", "frm2550m:txtLineBus", "frm2550m:txtTelephoneNum"})


class HarvestError(ValueError):
    """A refused harvest. The catalog must not be written."""


def write_catalog(catalog: dict) -> None:
    fi.DEFAULT_CATALOG.write_text(
        json.dumps(catalog, indent=2) + "\n", encoding="utf-8"
    )


def write_evidence(name: str, payload: dict) -> None:
    path = EVIDENCE_DIR / name
    path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def table_errors() -> list[str]:
    errors: list[str] = []
    if len(JOINS) != EXPECTED_JOINS:
        errors.append(f"JOINS length {len(JOINS)} != pin {EXPECTED_JOINS}")
    ids = [str(row["id"]) for row in JOINS]
    keys = [str(row["serialized_key"]) for row in JOINS]
    if len(ids) != len(set(ids)):
        errors.append("JOINS identity ids are not unique")
    if len(keys) != len(set(keys)):
        errors.append("JOINS serialized_keys are not unique")
    for row in JOINS:
        ident = row["id"]
        if row["bundle_slug"] != BUNDLE:
            errors.append(f"{ident}: bundle_slug is not {BUNDLE}")
        leaf = lk.leaf_of(str(row["serialized_key"]))
        if leaf in REFUSED_LEAVES:
            errors.append(f"{ident}: refused leaf {leaf}")
        key = str(row["serialized_key"])
        if key in REFUSED_KEYS:
            errors.append(f"{ident}: refused key {key}")
        if not key.startswith("frm2550m:"):
            errors.append(f"{ident}: key {key!r} is not a 2550M frm key")
        if leaf == "txtTaxpayerName":
            errors.append(f"{ident}: wrong-spelling taxpayer leaf")
    if "txtEmail" in keys:
        errors.append("txtEmail must not be in JOINS")
    return errors


def _boxes_equal(left: object, right: object) -> bool:
    if not isinstance(left, list) or not isinstance(right, list):
        return False
    if len(left) != 4 or len(right) != 4:
        return False
    return all(float(a) == float(b) for a, b in zip(left, right))


def _inventory_for_slug(
    slug: str,
    inventories: dict[str, dict[str, object]],
) -> dict[str, object]:
    resolved = jc.resolve_slug(slug, inventories, jc.index_inventories_by_stem(inventories))
    name = resolved.get("inventory")
    if not name:
        raise HarvestError(f"{slug}: no inventory")
    inventory = inventories.get(str(name))
    if inventory is None:
        raise HarvestError(f"{slug}: inventory {name!r} missing from load")
    if str(name) != INVENTORY_DIR:
        raise HarvestError(f"{slug}: resolved {name!r}, expected {INVENTORY_DIR}")
    return inventory


def _field_row(inventory: dict[str, object], key: str) -> dict:
    path = pathlib.Path(str(inventory["path"]))
    payload = json.loads(path.read_text(encoding="utf-8"))
    hits = [
        row
        for row in jc.field_rows(payload)
        if isinstance(row, dict) and jc.serialized_key(row) == key
    ]
    if len(hits) != 1:
        raise HarvestError(f"{key}: inventory rows {len(hits)}, want 1")
    return hits[0]


def apply_joins(
    catalog: dict,
    inventories: dict[str, dict[str, object]],
) -> dict:
    errors = table_errors()
    if errors:
        raise HarvestError("; ".join(errors))

    records = catalog.get("records")
    if not isinstance(records, list):
        raise HarvestError("catalog has no records list")
    by_id = {str(record.get("id")): record for record in records}
    claimed: dict[str, str] = {}
    for record in records:
        key = jc.claimed_key(record)
        if key is None:
            continue
        ident = str(record.get("id"))
        previous = claimed.get(key)
        if previous is not None and previous != ident:
            raise HarvestError(f"{key}: claimed by both {previous} and {ident}")
        claimed[key] = ident

    inventory = _inventory_for_slug(BUNDLE, inventories)
    inventory_keys = [str(key) for key in inventory["keys"]]

    rewritten: list[str] = []
    already: list[str] = []
    for join in JOINS:
        ident = str(join["id"])
        key = str(join["serialized_key"])
        record = by_id.get(ident)
        if record is None:
            errors.append(f"{ident}: missing from catalog")
            continue
        if str(record.get("bundle_slug")) != BUNDLE:
            errors.append(f"{ident}: catalog bundle is {record.get('bundle_slug')!r}")
        if str(record.get("html_id_hint")) != join["html_id_hint"]:
            errors.append(
                f"{ident}: html_id_hint {record.get('html_id_hint')!r} "
                f"!= {join['html_id_hint']!r}"
            )
        if not _boxes_equal(record.get("source_printed_box_pt"), join["source_printed_box_pt"]):
            errors.append(
                f"{ident}: source_printed_box_pt {record.get('source_printed_box_pt')!r} "
                f"!= {join['source_printed_box_pt']!r}"
            )
        try:
            row = _field_row(inventory, key)
        except HarvestError as exc:
            errors.append(str(exc))
            continue
        if str(row.get("item_number")) != join["item_number"]:
            errors.append(
                f"{ident}: inventory item_number {row.get('item_number')!r} "
                f"!= {join['item_number']!r}"
            )
        if row.get("page") not in (1, "1"):
            errors.append(f"{ident}: inventory page {row.get('page')!r} is not 1")
        kind = str(row.get("control_kind") or "")
        if "hidden" in kind.lower() or "workflow" in kind.lower():
            errors.append(f"{ident}: control_kind {kind!r} is not a printed box")
        ownership = lk.unique_inventory_ownership_errors(key, inventory_keys)
        if ownership:
            errors.extend(ownership)
        owner = claimed.get(key)
        existing = jc.claimed_key(record)
        if existing == key:
            if owner not in (None, ident):
                errors.append(f"{ident}: key {key} also claimed by {owner}")
            already.append(ident)
            continue
        if existing:
            errors.append(f"{ident}: already keyed {existing!r}, not {key!r}")
            continue
        if owner is not None and owner != ident:
            errors.append(f"{ident}: key {key} already claimed by {owner}")
            continue
        if errors:
            continue
        record["official_field_key"] = key
        record["official_field_key_gap"] = ""
        claimed[key] = ident
        rewritten.append(ident)

    if errors:
        raise HarvestError("; ".join(errors))
    if len(rewritten) + len(already) != EXPECTED_JOINS:
        raise HarvestError(
            f"applied {len(rewritten)} + already {len(already)} != {EXPECTED_JOINS}"
        )
    return {
        "title": "2550M header harvest",
        "date": "2026-08-21",
        "source_evidence": W5_EVIDENCE,
        "bundle": BUNDLE,
        "rewritten_record_count": len(rewritten),
        "already_applied_count": len(already),
        "joins": [
            {
                "caption": row["caption"],
                "html_id_hint": row["html_id_hint"],
                "id": row["id"],
                "item_number": row["item_number"],
                "serialized_key": row["serialized_key"],
                "source_printed_box_pt": row["source_printed_box_pt"],
                "w5_emitted": row["w5_emitted"],
            }
            for row in JOINS
        ],
        "refused": [
            {
                "key": "txtEmail",
                "reason": (
                    "inventory control_kind is hidden/workflow-metadata; "
                    "2550M prints no email caption"
                ),
            },
            {
                "key": "txtTaxpayerName",
                "reason": (
                    "wrong spelling; leftover-silent. Item 7 is "
                    "frm2550m:txtTaxPayerName"
                ),
            },
            {
                "key": "frm2550m:txtLineBus",
                "reason": "Item 6 Line of Business is not this harvest",
            },
            {
                "key": "frm2550m:txtTelephoneNum",
                "reason": "Item 8 Telephone Number is not this harvest",
            },
        ],
        "notes": [
            "Named rule plus printed box role. Leftover uniqueness is not the join.",
            "Do not generalize txtRDOCode / txtAddress / txtZipCode off 2550M.",
            "Catalog size stays 9990. No HTML name= write.",
        ],
        "rewritten_ids": rewritten,
        "already_applied_ids": already,
    }


def remint_catalog() -> dict:
    catalog, errors = fi.load_catalog(fi.DEFAULT_CATALOG)
    if errors:
        raise HarvestError("; ".join(errors[:20]))
    before = len(catalog["records"])
    payload = apply_joins(catalog, jc.load_inventories(jc.DEFAULT_RULES))
    if len(catalog["records"]) != before:
        raise HarvestError("harvest changed catalog size")
    check_errors = fi.check_catalog(catalog, fi.DEFAULT_CATALOG)
    if check_errors:
        raise HarvestError("; ".join(check_errors[:20]))
    if payload["rewritten_record_count"]:
        write_catalog(catalog)
    write_evidence(EVIDENCE_NAME, payload)
    return payload


def self_test() -> int:
    failed = 0

    def check(name: str, held: bool, detail: str = "") -> None:
        nonlocal failed
        if held:
            print("OK    " + name)
        else:
            failed += 1
            extra = (" — " + detail) if detail else ""
            print("FAIL  " + name + extra)

    check("table is well formed", not table_errors(), "; ".join(table_errors()))
    check("exactly four joins", len(JOINS) == EXPECTED_JOINS)
    check(
        "email is refused, not joined",
        "txtEmail" not in {row["serialized_key"] for row in JOINS}
        and "txtEmail" in REFUSED_LEAVES,
    )
    check(
        "wrong-spelling taxpayer leaf is refused",
        all(lk.leaf_of(str(row["serialized_key"])) != "txtTaxpayerName" for row in JOINS)
        and "txtTaxpayerName" in REFUSED_LEAVES,
    )

    catalog, catalog_errors = fi.load_catalog(fi.DEFAULT_CATALOG)
    check("shipped catalog is well formed", not catalog_errors, "; ".join(catalog_errors[:3]))
    inventories = jc.load_inventories(jc.DEFAULT_RULES)
    if catalog:
        live = apply_joins(copy.deepcopy(catalog), inventories)
        check(
            "live catalog already has the four joins (or is ready to remint)",
            live["rewritten_record_count"] + live["already_applied_count"] == EXPECTED_JOINS,
            str(live),
        )
        by_id = {str(record["id"]): record for record in catalog["records"]}
        for join in JOINS:
            record = by_id.get(str(join["id"]))
            check(
                f"{join['id']} binds {join['serialized_key']}",
                record is not None
                and jc.claimed_key(record) == join["serialized_key"]
                and str(record.get("html_id_hint")) == join["html_id_hint"]
                and _boxes_equal(record.get("source_printed_box_pt"), join["source_printed_box_pt"]),
                str(record.get("official_field_key") if record else None),
            )
            row = _field_row(_inventory_for_slug(BUNDLE, inventories), str(join["serialized_key"]))
            check(
                f"{join['serialized_key']} inventory item {join['item_number']}",
                str(row.get("item_number")) == join["item_number"]
                and row.get("page") in (1, "1")
                and "hidden" not in str(row.get("control_kind") or "").lower(),
                str({"item": row.get("item_number"), "page": row.get("page"),
                     "kind": row.get("control_kind")}),
            )
        email_row = _field_row(_inventory_for_slug(BUNDLE, inventories), "txtEmail")
        check(
            "txtEmail stays hidden workflow metadata",
            email_row.get("page") is None
            and email_row.get("item_number") is None
            and "hidden" in str(email_row.get("control_kind") or "").lower(),
            str(email_row.get("control_kind")),
        )

        gapped = copy.deepcopy(catalog)
        for join in JOINS:
            record = next(item for item in gapped["records"] if item["id"] == join["id"])
            record["official_field_key"] = None
            record["official_field_key_gap"] = GAP_NO_UNIQUE
        from_gap = apply_joins(gapped, inventories)
        check(
            "gapped remint writes exactly the four joins",
            from_gap["rewritten_record_count"] == EXPECTED_JOINS
            and from_gap["already_applied_count"] == 0,
            str(from_gap),
        )
        again = apply_joins(gapped, inventories)
        check(
            "second remint is idempotent",
            again["rewritten_record_count"] == 0
            and again["already_applied_count"] == EXPECTED_JOINS,
            str(again),
        )

        missing = {"records": []}
        try:
            apply_joins(missing, inventories)
            check("missing identity is refused", False)
        except HarvestError as exc:
            check("missing identity is refused", "missing from catalog" in str(exc), str(exc))

        dup_inv = copy.deepcopy(inventories)
        dup_inv[INVENTORY_DIR]["keys"] = list(dup_inv[INVENTORY_DIR]["keys"]) + [
            "frm2550m:txtRDOCode"
        ]
        steal = copy.deepcopy(catalog)
        for join in JOINS:
            record = next(item for item in steal["records"] if item["id"] == join["id"])
            record["official_field_key"] = None
            record["official_field_key_gap"] = GAP_NO_UNIQUE
        try:
            apply_joins(steal, dup_inv)
            check("duplicate inventory hit is refused", False)
        except HarvestError as exc:
            check(
                "duplicate inventory hit is refused",
                "not uniquely owned" in str(exc),
                str(exc),
            )

        other = copy.deepcopy(catalog)
        victim = next(item for item in other["records"] if item["id"] == JOINS[0]["id"])
        victim["official_field_key"] = None
        victim["official_field_key_gap"] = GAP_NO_UNIQUE
        thief = next(
            item
            for item in other["records"]
            if item["bundle_slug"] == BUNDLE and item["id"] != victim["id"]
        )
        thief["official_field_key"] = JOINS[0]["serialized_key"]
        thief["official_field_key_gap"] = ""
        try:
            apply_joins(other, inventories)
            check("key already claimed by another identity is refused", False)
        except HarvestError as exc:
            check(
                "key already claimed by another identity is refused",
                "already claimed" in str(exc) or "claimed by both" in str(exc),
                str(exc),
            )

        stale = copy.deepcopy(catalog)
        hint = next(item for item in stale["records"] if item["id"] == JOINS[0]["id"])
        hint["html_id_hint"] = "p1c999"
        try:
            apply_joins(stale, inventories)
            check("stale html_id_hint is refused", False)
        except HarvestError as exc:
            check("stale html_id_hint is refused", "html_id_hint" in str(exc), str(exc))

    print(("FAIL" if failed else "OK"),
          ("%s self-test(s) failed" % failed) if failed else "self-test")
    return 1 if failed else 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument(
        "--write",
        action="store_true",
        help="remint catalog official_field_key for the four joins",
    )
    args = parser.parse_args()
    if args.self_test:
        return self_test()
    if not args.write:
        parser.error("pass --self-test or --write")
    try:
        payload = remint_catalog()
    except HarvestError as exc:
        print("FAIL  %s" % exc)
        return 1
    print(
        "OK    rewritten %s already %s wrote %s"
        % (
            payload["rewritten_record_count"],
            payload["already_applied_count"],
            EVIDENCE_NAME,
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
