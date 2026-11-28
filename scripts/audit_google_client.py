#!/usr/bin/env python3
"""Whether a Windows package holds Google's OAuth client, in any of its files.

The Microsoft Store package ships without Gmail Connect unless the repository variable STORE_GMAIL is exactly 1: Connect
asks for Gmail's restricted scope, which needs Google's verification for a public app. bir-core compiles the client in at
build time (option_env!("GOOGLE_CLIENT_ID") / "GOOGLE_CLIENT_SECRET"), so a build that had it left it in its bytes.

    audit_google_client.py --self-test        the detector finds a planted id and secret, bare, in an .msix member
                                              other than the exe, and in a nested archive; nothing in a clean package;
                                              and an archive it cannot read is refused
    audit_google_client.py --absent PACKAGE   exit 1 if any file of PACKAGE holds a client id or a client secret
    audit_google_client.py --present PACKAGE  exit 1 unless its files hold both (the positive control: the direct
                                              downloads have the client, so their package proves the bytes read are real)

PACKAGE is an .msix or a .zip (read member by member as the ZIP it is: members are deflated, so the archive's own bytes
hide the strings), a folder, or a single file. A member that is itself a ZIP or gzip is read too; one in another archive
format (zstd, xz, 7z, cab) cannot be read, so the audit refuses it (exit 2) rather than pass it unread. Only the names
of the files that hold a kind are printed, never what was found.

The two patterns and the off/on rule are scripts/store_gmail.py's (`check`, hexuria/buwiz-forms#501). Once #501 is on
main, this folds into it.
"""

from __future__ import annotations

import argparse
import gzip
import io
import re
import sys
import tempfile
import zipfile
import zlib
from pathlib import Path
from typing import Iterator

# Google's OAuth client ids end in .apps.googleusercontent.com; its client secrets start with GOCSPX-.
CLIENT_ID = re.compile(rb"[0-9]{6,}-[0-9a-z]{16,}\.apps\.googleusercontent\.com")
CLIENT_SECRET = re.compile(rb"GOCSPX-[0-9A-Za-z_-]{16,}")

ZIP_MAGIC = b"PK\x03\x04"
GZIP_MAGIC = b"\x1f\x8b"
UNREADABLE = {
    b"\x28\xb5\x2f\xfd": "zstd",
    b"\xfd7zXZ\x00": "xz",
    b"7z\xbc\xaf\x27\x1c": "7z",
    b"MSCF": "cab",
}


class Unreadable(Exception):
    pass


def files(name: str, data: bytes, depth: int = 0) -> Iterator[tuple[str, bytes]]:
    """(name, bytes) of every file in `data`, opening ZIP and gzip members; refuses archives it cannot read."""
    if depth > 4:
        raise Unreadable(f"{name}: archives nested more than 4 deep")
    for magic, kind in UNREADABLE.items():
        if data.startswith(magic):
            raise Unreadable(f"{name} is a {kind} archive, which this audit cannot read")
    if data.startswith(ZIP_MAGIC):
        with zipfile.ZipFile(io.BytesIO(data)) as archive:
            for member in archive.infolist():
                if not member.is_dir():
                    yield from files(f"{name}/{member.filename}", archive.read(member), depth + 1)
        return
    if data.startswith(GZIP_MAGIC):
        yield from files(f"{name}(gunzip)", gzip.decompress(data), depth + 1)
        return
    yield name, data


def package_files(package: Path) -> Iterator[tuple[str, bytes]]:
    if package.is_dir():
        for path in sorted(package.rglob("*")):
            if path.is_file() and not path.is_symlink():
                yield from files(str(path.relative_to(package)), path.read_bytes())
        return
    yield from files(package.name, package.read_bytes())


def scan(package: Path) -> tuple[list[str], list[str]]:
    """The files of `package` that hold a client id, and those that hold a client secret."""
    ids, secrets = [], []
    for name, data in package_files(package):
        if CLIENT_ID.search(data):
            ids.append(name)
        if CLIENT_SECRET.search(data):
            secrets.append(name)
    return ids, secrets


def report(package: Path, want_present: bool) -> int:
    if not package.exists():
        print(f"no {package}", file=sys.stderr)
        return 2
    try:
        ids, secrets = scan(package)
    except (Unreadable, zipfile.BadZipFile, OSError, EOFError, zlib.error) as error:
        print(f"{package.name}: refused, not read whole: {error}", file=sys.stderr)
        return 2
    except NotImplementedError as error:
        print(f"{package.name}: refused, a member uses a compression this audit cannot read: {error}", file=sys.stderr)
        return 2
    except RuntimeError as error:
        print(f"{package.name}: refused, a member could not be read (encrypted?): {error}", file=sys.stderr)
        return 2
    for kind, where in (("a client id", ids), ("a client secret", secrets)):
        print(f"{package.name}: {kind} in {', '.join(where)}" if where else f"{package.name}: no {kind[2:]} in any file")
    if want_present:
        return 0 if ids and secrets else 1
    return 0 if not (ids or secrets) else 1


def zip_bytes(members: dict[str, bytes]) -> bytes:
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w", zipfile.ZIP_DEFLATED) as archive:
        for name, data in members.items():
            archive.writestr(name, data)
    return buffer.getvalue()


def self_test() -> int:
    """The detector on planted fixtures: found wherever it sits, nothing in a clean package, unreadable refused."""
    planted_id = b"000000000000-fixturefixturefixture.apps.googleusercontent.com"
    planted_secret = b"GOCSPX-fixturefixturefixture00"
    filler = bytes(range(256)) * 64
    clean_exe = filler + b"apps.googleusercontent.com GOCSPX-" + filler
    cases = {
        "bare exe": (filler + planted_id + filler + planted_secret, True, True),
        "dll beside the exe": (zip_bytes({"eBIRForms.exe": clean_exe, "lib/x.dll": planted_id + planted_secret}), True, True),
        "nested zip": (zip_bytes({"eBIRForms.exe": clean_exe, "nsh/inner.zip": zip_bytes({"a": planted_id})}), True, False),
        "gzip member": (zip_bytes({"eBIRForms.exe": clean_exe, "x.gz": gzip.compress(planted_secret)}), False, True),
        "clean": (zip_bytes({"eBIRForms.exe": clean_exe, "nsh/bundle.nshc": filler}), False, False),
    }
    failures = []
    with tempfile.TemporaryDirectory() as scratch:
        for name, (data, want_id, want_secret) in cases.items():
            path = Path(scratch, name.replace(" ", "-") + ".msix")
            path.write_bytes(data)
            ids, secrets = scan(path)
            if (bool(ids), bool(secrets)) != (want_id, want_secret):
                failures.append(name)
        refused = Path(scratch, "unreadable.msix")
        refused.write_bytes(zip_bytes({"eBIRForms.exe": clean_exe, "x.zst": b"\x28\xb5\x2f\xfd" + filler}))
        try:
            scan(refused)
            failures.append("zstd member was not refused")
        except Unreadable:
            pass
    if failures:
        print("self-test FAILED: " + ", ".join(failures))
        return 1
    print("self-test passed: a planted client id and secret are found bare, beside the exe, in a nested zip and in a"
          " gzip member; a clean package has neither; an archive it cannot read is refused")
    return 0


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--self-test", action="store_true")
    mode.add_argument("--absent", type=Path, metavar="PACKAGE")
    mode.add_argument("--present", type=Path, metavar="PACKAGE")
    args = parser.parse_args(argv)
    if args.self_test:
        return self_test()
    if args.absent is not None:
        return report(args.absent, want_present=False)
    return report(args.present, want_present=True)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
