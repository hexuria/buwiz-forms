#!/usr/bin/env python3
"""Ordered per-table content digests of an eBIRForms database (accounts-and-lock PLAN, "A2 rehearsal", M5).

Belongs to neither build: plain Python and its own sqlite3. Prints one JSON object:

    {"tables": {"<table>": {"sha256": "...", "rows": N}, ...},
     "integrity_check": "ok", "user_version": 24, "raw_sha256": "...", "has_wal": false}

- Every table (the app's 24, any left-over legacy table, sqlite_sequence) is read with
  `SELECT * FROM <table> ORDER BY rowid`, or by every column for a table without rowid.
- Each value is hashed with its SQLite type tag (null, integer, real, text, blob) and a length prefix, so
  `'1'` and `1`, or `('ab','c')` and `('a','bc')`, never digest alike.
- The database is never opened where it lies: sqlite3 checkpoints whatever it opens. The file and its
  `-wal` and `-shm` are copied into a private temporary folder first, and the copy is read.

Usage: a2_table_digests.py <database> [--skip-settings key,key]
`--skip-settings` leaves rows of `settings` with these keys out of its digest (the claim's step-6 keys),
and lists them apart under "skipped_settings".
"""

import hashlib
import json
import os
import shutil
import sqlite3
import struct
import sys
import tempfile


class Text(bytes):
    """A TEXT value, kept as its stored bytes (so text that is not valid UTF-8 still digests)."""


def tagged(value):
    if isinstance(value, Text):
        return b"t" + struct.pack(">Q", len(value)) + bytes(value)
    if value is None:
        return b"n"
    if isinstance(value, int):
        return b"i" + struct.pack(">q", value)
    if isinstance(value, float):
        return b"r" + struct.pack(">d", value)
    if isinstance(value, str):
        raw = value.encode("utf-8")
        return b"t" + struct.pack(">Q", len(raw)) + raw
    if isinstance(value, (bytes, bytearray, memoryview)):
        raw = bytes(value)
        return b"b" + struct.pack(">Q", len(raw)) + raw
    raise TypeError(type(value))


def file_sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def table_has_rowid(conn, table):
    try:
        conn.execute(f'SELECT rowid FROM "{table}" LIMIT 1').fetchall()
        return True
    except sqlite3.OperationalError:
        return False


def digests(path, skip_settings):
    raw_sha = file_sha256(path)
    has_wal = os.path.exists(path + "-wal") and os.path.getsize(path + "-wal") > 0
    folder = tempfile.mkdtemp(prefix="a2-digest-")
    try:
        copy = os.path.join(folder, "copy.db")
        shutil.copyfile(path, copy)
        for suffix in ("-wal", "-shm"):
            if os.path.exists(path + suffix):
                shutil.copyfile(path + suffix, copy + suffix)
        conn = sqlite3.connect(copy)
        conn.text_factory = Text
        out = {
            "raw_sha256": raw_sha,
            "has_wal": has_wal,
            "user_version": conn.execute("PRAGMA user_version").fetchone()[0],
            "integrity_check": conn.execute("PRAGMA integrity_check").fetchone()[0].decode("utf-8"),
            "tables": {},
            "skipped_settings": {},
        }
        names = [
            row[0].decode("utf-8")
            for row in conn.execute(
                "SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name"
            )
        ]
        for table in names:
            columns = [row[1].decode("utf-8") for row in conn.execute(f'PRAGMA table_info("{table}")')]
            if table_has_rowid(conn, table):
                order = "rowid"
            else:
                order = ", ".join(f'"{column}"' for column in columns)
            digest = hashlib.sha256()
            digest.update(tagged(Text(table.encode("utf-8"))))
            for column in columns:
                digest.update(tagged(Text(column.encode("utf-8"))))
            rows = 0
            for row in conn.execute(f'SELECT * FROM "{table}" ORDER BY {order}'):
                if table == "settings" and skip_settings and row[0].decode("utf-8", "replace") in skip_settings:
                    out["skipped_settings"][row[0].decode("utf-8", "replace")] = bytes(row[1]).decode("utf-8", "replace")
                    continue
                digest.update(b"(")
                for value in row:
                    digest.update(tagged(value))
                digest.update(b")")
                rows += 1
            out["tables"][table] = {"sha256": digest.hexdigest(), "rows": rows}
        conn.close()
        return out
    finally:
        shutil.rmtree(folder, ignore_errors=True)


def main(argv):
    if len(argv) < 2:
        print(__doc__, file=sys.stderr)
        return 2
    skip = set()
    if "--skip-settings" in argv:
        index = argv.index("--skip-settings")
        skip = {key for key in argv[index + 1].split(",") if key}
    print(json.dumps(digests(argv[1], skip), sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
