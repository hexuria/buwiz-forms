#!/usr/bin/env python3
"""Whether the Microsoft Store .msix's eBIRForms.exe holds Google's OAuth client.

The Store package ships without Gmail Connect unless the repository variable STORE_GMAIL is exactly 1: Connect asks
for Gmail's restricted scope, which needs Google's verification for a public app. bir-core compiles the client in at
build time (option_env!("GOOGLE_CLIENT_ID") / "GOOGLE_CLIENT_SECRET"), so a build that had it left it in the
executable's bytes.

    audit_google_client.py --self-test        the detector finds a planted id and secret, bare and in an .msix, and
                                              nothing in a clean file
    audit_google_client.py --absent PACKAGE   switch off: exit 1 if the exe holds a client id or a client secret
    audit_google_client.py --present PACKAGE  switch on, the positive control: exit 1 unless it holds both

PACKAGE is an .msix, read as the ZIP it is (its members are deflated, so its own bytes hide the strings), or an
eBIRForms.exe. Nothing found is ever printed: only which of the two kinds was found.

The two patterns and the off/on rule are scripts/store_gmail.py's (`check`, hexuria/buwiz-forms#501), which reads
folders; this reads the .msix itself. Once #501 is on main, this folds into store_gmail.py as an .msix reader.
"""

from __future__ import annotations

import argparse
import re
import sys
import tempfile
import zipfile
from pathlib import Path

# Google's OAuth client ids end in .apps.googleusercontent.com; its client secrets start with GOCSPX-.
CLIENT_ID = re.compile(rb"[0-9]{6,}-[0-9a-z]{16,}\.apps\.googleusercontent\.com")
CLIENT_SECRET = re.compile(rb"GOCSPX-[0-9A-Za-z_-]{16,}")
EXE = "eBIRForms.exe"


def found(data: bytes) -> tuple[bool, bool]:
    """(holds a client id, holds a client secret)."""
    return bool(CLIENT_ID.search(data)), bool(CLIENT_SECRET.search(data))


def executable(package: Path) -> bytes:
    """The bytes of eBIRForms.exe: the file itself, or the one at an .msix's root."""
    if package.name.lower() == EXE.lower():
        return package.read_bytes()
    with zipfile.ZipFile(package) as archive:
        names = [name for name in archive.namelist() if name.lower() == EXE.lower()]
        if len(names) != 1:
            raise SystemExit(f"{package.name}: expected one {EXE} at the package root, found {len(names)}")
        return archive.read(names[0])


def describe(has_id: bool, has_secret: bool) -> str:
    kinds = [kind for kind, has in (("a client id", has_id), ("a client secret", has_secret)) if has]
    return " and ".join(kinds) if kinds else "no client id or secret"


def report(package: Path, want_present: bool) -> int:
    has_id, has_secret = found(executable(package))
    print(f"{package.name}: {EXE} holds {describe(has_id, has_secret)} of Google's OAuth client")
    if want_present:
        return 0 if has_id and has_secret else 1
    return 0 if not (has_id or has_secret) else 1


def self_test() -> int:
    """The detector finds a planted id and secret (bare and inside an .msix) and nothing in a clean file."""
    planted_id = b"000000000000-fixturefixturefixture.apps.googleusercontent.com"
    planted_secret = b"GOCSPX-fixturefixturefixture00"
    filler = bytes(range(256)) * 64
    cases = {
        "both": (filler + planted_id + filler + planted_secret + filler, (True, True)),
        "id": (filler + planted_id + filler, (True, False)),
        "secret": (filler + planted_secret + filler, (False, True)),
        "clean": (filler + b"apps.googleusercontent.com GOCSPX-" + filler, (False, False)),
    }
    failures = []
    with tempfile.TemporaryDirectory() as scratch:
        for name, (data, want) in cases.items():
            exe = Path(scratch, name, EXE)
            exe.parent.mkdir()
            exe.write_bytes(data)
            msix = Path(scratch, f"{name}.msix")
            with zipfile.ZipFile(msix, "w", zipfile.ZIP_DEFLATED) as archive:
                archive.writestr("AppxManifest.xml", "<Package/>")
                archive.writestr(EXE, data)
            for package in (exe, msix):
                if found(executable(package)) != want:
                    failures.append(f"{name} ({'msix' if package.suffix else 'exe'})")
    if failures:
        print("self-test FAILED: " + ", ".join(failures))
        return 1
    print("self-test passed: a planted client id and secret are found, bare and inside an .msix; a clean file has neither")
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
