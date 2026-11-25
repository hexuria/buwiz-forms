#!/usr/bin/env python3
"""The Google Play bundle: check what `nsh package --format aab` built, and ready it for the upload key.

    android_bundle.py check BUNDLE --package ID --version-code N --target-sdk N
        Reads the bundle's manifest (base/manifest/AndroidManifest.xml, aapt2's protobuf form) and fails unless the
        package, versionCode and targetSdkVersion are the ones given, backup is off (allowBackup="false" and data
        extraction rules), the app is not debuggable, and the native code is for phones only (arm64-v8a, armeabi-v7a).

    android_bundle.py prepare BUNDLE OUT
        Writes OUT: the same bundle without its signature (`nsh package` always signs, with the Android debug key when
        it has no keystore) and without the updater's source (`nsh/assets/nsh-updater.json`: Google Play updates the
        app, and the updater never starts on a phone). OUT is what the owner signs with the upload key.

Standard library only: no protobuf package, no bundletool.
"""

from __future__ import annotations

import argparse
import sys
import zipfile
from pathlib import Path

MANIFEST = "base/manifest/AndroidManifest.xml"
ANDROID_NS = "http://schemas.android.com/apk/res/android"
UPDATER_ASSET = "base/assets/nsh/assets/nsh-updater.json"
PHONE_ABIS = {"arm64-v8a", "armeabi-v7a"}


class BundleError(Exception):
    pass


# ---- protobuf wire format, as much as aapt2's XmlNode needs (frameworks/base/tools/aapt2/Resources.proto)


def _varint(data: bytes, at: int) -> tuple[int, int]:
    value = shift = 0
    while True:
        if at >= len(data):
            raise BundleError("truncated protobuf varint")
        byte = data[at]
        at += 1
        value |= (byte & 0x7F) << shift
        if byte < 0x80:
            return value, at
        shift += 7


def _fields(data: bytes):
    """(field number, value) for each field; value is bytes for length-delimited fields, int for varints."""
    at = 0
    while at < len(data):
        key, at = _varint(data, at)
        number, wire = key >> 3, key & 7
        if wire == 0:
            value, at = _varint(data, at)
        elif wire == 2:
            length, at = _varint(data, at)
            value, at = data[at : at + length], at + length
        elif wire == 1:
            value, at = data[at : at + 8], at + 8
        elif wire == 5:
            value, at = data[at : at + 4], at + 4
        else:
            raise BundleError(f"unsupported protobuf wire type {wire}")
        yield number, value


class Element:
    """One XML element of the manifest: its name, its attributes by (namespace, name), its child elements."""

    def __init__(self, name: str, attributes: dict[tuple[str, str], str], children: list[Element]):
        self.name = name
        self.attributes = attributes
        self.children = children

    def get(self, name: str, namespace: str = ANDROID_NS) -> str | None:
        return self.attributes.get((namespace, name))

    def find(self, name: str) -> list[Element]:
        return [child for child in self.children if child.name == name]


def _attribute(data: bytes) -> tuple[tuple[str, str], str]:
    namespace = name = value = ""
    for number, field in _fields(data):
        if number == 1:
            namespace = field.decode()
        elif number == 2:
            name = field.decode()
        elif number == 3:
            value = field.decode()
    return (namespace, name), value


def _element(data: bytes) -> Element:
    name = ""
    attributes: dict[tuple[str, str], str] = {}
    children: list[Element] = []
    for number, field in _fields(data):
        if number == 3:
            name = field.decode()
        elif number == 4:
            key, value = _attribute(field)
            attributes[key] = value
        elif number == 5:
            child = _node(field)
            if child is not None:
                children.append(child)
    return Element(name, attributes, children)


def _node(data: bytes) -> Element | None:
    """An XmlNode: an element (field 1) or text (field 2, which manifests do not use)."""
    for number, field in _fields(data):
        if number == 1:
            return _element(field)
    return None


def read_manifest(data: bytes) -> Element:
    root = _node(data)
    if root is None or root.name != "manifest":
        raise BundleError("the bundle's manifest has no <manifest> element")
    return root


# ---- the two commands


def check(bundle: Path, package: str, version_code: int, target_sdk: int) -> list[str]:
    """What is wrong with `bundle` for a Google Play upload (empty: nothing)."""
    with zipfile.ZipFile(bundle) as zf:
        names = zf.namelist()
        if MANIFEST not in names:
            return [f"no {MANIFEST}: not an Android App Bundle"]
        manifest = read_manifest(zf.read(MANIFEST))
    problems = []
    found = manifest.get("package", "") or ""
    if found != package:
        problems.append(f"package is {found!r}, expected {package!r}")
    if manifest.get("versionCode") != str(version_code):
        problems.append(f"versionCode is {manifest.get('versionCode')!r}, expected {version_code}")
    sdk = manifest.find("uses-sdk")
    target = sdk[0].get("targetSdkVersion") if sdk else None
    if target != str(target_sdk):
        problems.append(f"targetSdkVersion is {target!r}, expected {target_sdk}")
    applications = manifest.find("application")
    if len(applications) != 1:
        problems.append(f"{len(applications)} <application> elements, expected 1")
    else:
        app = applications[0]
        if app.get("allowBackup") != "false":
            problems.append(f'allowBackup is {app.get("allowBackup")!r}, expected "false" (nsh.json android.allowBackup)')
        if not (app.get("dataExtractionRules") or "").startswith("@"):
            problems.append("no dataExtractionRules: Android 12 and later would still move the app's data to a new device")
        if app.get("debuggable") not in (None, "false"):
            problems.append("the app is debuggable")
    abis = {name.split("/")[2] for name in names if name.startswith("base/lib/") and name.count("/") >= 3}
    if not abis:
        problems.append("no native code under base/lib/")
    elif not abis <= PHONE_ABIS:
        problems.append(f"native code for {sorted(abis - PHONE_ABIS)}: a store bundle has phone ABIs only")
    if UPDATER_ASSET in names:
        problems.append(f"{UPDATER_ASSET} is still in the bundle: run `prepare`")
    return problems


def prepare(bundle: Path, out: Path) -> int:
    """Copies `bundle` to `out` without its signature and the updater's source; answers how many entries went."""
    if out.resolve() == bundle.resolve():
        raise BundleError("OUT must be another file than BUNDLE")
    dropped = 0
    with zipfile.ZipFile(bundle) as zin, zipfile.ZipFile(out, "w") as zout:
        for info in zin.infolist():
            if info.filename.startswith("META-INF/") or info.filename == UPDATER_ASSET:
                dropped += 1
                continue
            # The entry as it was, compressed or stored: Google Play reads the bundle's own choices.
            zout.writestr(info, zin.read(info.filename), compress_type=info.compress_type)
    return dropped


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    c = sub.add_parser("check")
    c.add_argument("bundle", type=Path)
    c.add_argument("--package", required=True)
    c.add_argument("--version-code", type=int, required=True)
    c.add_argument("--target-sdk", type=int, required=True)
    p = sub.add_parser("prepare")
    p.add_argument("bundle", type=Path)
    p.add_argument("out", type=Path)
    args = parser.parse_args(argv)
    try:
        if args.command == "prepare":
            dropped = prepare(args.bundle, args.out)
            print(f"android_bundle: {args.out} (left out {dropped} entries: the signature and the updater's source)")
            return 0
        problems = check(args.bundle, args.package, args.version_code, args.target_sdk)
    except (BundleError, zipfile.BadZipFile, OSError) as error:
        print(f"android_bundle: {error}", file=sys.stderr)
        return 1
    for problem in problems:
        print(f"android_bundle: {problem}", file=sys.stderr)
    if problems:
        return 1
    print(f"android_bundle: {args.bundle} is {args.package}, versionCode {args.version_code}, target API {args.target_sdk}, backup off")
    return 0


if __name__ == "__main__":
    sys.exit(main())
