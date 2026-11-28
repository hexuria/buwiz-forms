"""An agent build never goes up to App Store Connect by accident.

An agent build (`just testflight-agent`, `--agent`) carries the driver: gpui-agent's socket and, on the Mac,
com.apple.security.network.server. App Store Connect's own Submit cannot be guarded from here, so the uploads are:

  check mac PKG [--provenance FILE] [--allow-agent]   (testflight-upload; PKG is the .pkg that is uploaded, or an .app)
  check ios IPA [--provenance FILE]                   (ios-appstore-upload; IPA is the .ipa, or an .app; and
                                                       ios-appstore, on the .app it just built)

It reads the artifact itself, not only files beside it, and refuses (exit 1) when any of these shows an agent build:
  - its Info.plist has NSHAgentBuild true (written by `_package-mac-appstore --agent`);
  - its executable holds the agent's marker string, AGENT_MARKER: gpui-agent's hello feature list, which only the
    `agent` feature links (no app code has it), so a build made before the Info.plist mark existed, or one made with a
    CARGO wrapper that adds the feature, is still found;
  - on the Mac, its signed entitlements carry com.apple.security.network.server while STORE_GMAIL is not 1 (after the
    STORE_GMAIL switch that entitlement exists only for Gmail Connect or an agent build);
  - the provenance file says agent=1.
`--allow-agent` (the Mac only: iOS has no agent build) lets an agent build up for internal TestFlight; it is a recipe
argument that only `just testflight-agent` passes. ALLOW_AGENT_UPLOAD in the environment is ignored on purpose: one line
in .env or a shell profile must not switch the guard off for every upload.
"""

from __future__ import annotations

import os
import plistlib
import shutil
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path

AGENT_MARKER = b"snapshot_compact"
NETWORK_SERVER = "com.apple.security.network.server"


def verdict(kind: str, info: dict, executable: bytes, entitlements: dict | None, provenance: str | None, env,
            allow_agent: bool) -> tuple[bool, list[str]]:
    """(may upload, what shows an agent build)."""
    signs = []
    if info.get("NSHAgentBuild") is True:
        signs.append("Info.plist says NSHAgentBuild")
    if AGENT_MARKER in executable:
        signs.append(f"the executable holds the agent's marker ({AGENT_MARKER.decode()})")
    if kind == "mac":
        if entitlements is None:
            signs.append("its signed entitlements could not be read")
        elif entitlements.get(NETWORK_SERVER) and env.get("STORE_GMAIL", "") != "1":
            signs.append(f"its entitlements carry {NETWORK_SERVER} without STORE_GMAIL=1")
    if provenance and "agent=1" in provenance:
        signs.append("its provenance says agent=1")
    if not signs:
        return True, []
    return (kind == "mac" and allow_agent), signs


def _app_in(folder: Path) -> Path:
    apps = sorted(p for p in folder.rglob("*.app") if p.is_dir())
    if not apps:
        raise FileNotFoundError(f"no .app inside {folder}")
    return apps[0]


def _read_app(app: Path, kind: str) -> tuple[dict, bytes, dict | None]:
    info_path = app / "Contents" / "Info.plist" if kind == "mac" else app / "Info.plist"
    info = plistlib.loads(info_path.read_bytes())
    exe_dir = app / "Contents" / "MacOS" if kind == "mac" else app
    executable = (exe_dir / info.get("CFBundleExecutable", "")).read_bytes()
    entitlements = None
    if kind == "mac":
        out = subprocess.run(["codesign", "-d", "--entitlements", "-", "--xml", str(app)], capture_output=True)
        if out.returncode == 0:
            entitlements = plistlib.loads(out.stdout) if out.stdout.strip() else {}
    return info, executable, entitlements


def read_artifact(kind: str, artifact: Path) -> tuple[dict, bytes, dict | None]:
    if artifact.is_dir():
        return _read_app(artifact, kind)
    if kind == "mac":
        # The .pkg that is uploaded: expanded, and the .app inside read (its signature, its plist, its binary).
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / "expanded"
            subprocess.run(["pkgutil", "--expand-full", str(artifact), str(out)], check=True, capture_output=True)
            return _read_app(_app_in(out), kind)
    with tempfile.TemporaryDirectory() as tmp:
        with zipfile.ZipFile(artifact) as ipa:
            ipa.extractall(tmp)
        return _read_app(_app_in(Path(tmp) / "Payload"), kind)


def check(kind: str, artifact: Path, provenance: Path | None, allow_agent: bool, env) -> tuple[bool, str]:
    try:
        info, executable, entitlements = read_artifact(kind, artifact)
    except (OSError, subprocess.CalledProcessError, zipfile.BadZipFile, plistlib.InvalidFileException, KeyError,
            ValueError) as error:
        return False, f"{artifact} could not be read ({type(error).__name__}: {error}), so it is not uploaded"
    text = provenance.read_text(encoding="utf-8") if provenance and provenance.exists() else None
    ok, signs = verdict(kind, info, executable, entitlements, text, env, allow_agent)
    if not signs:
        return True, f"{artifact.name}: not an agent build."
    why = "; ".join(signs)
    if ok:
        return True, f"AGENT BUILD {artifact.name} ({why}): internal TestFlight only, never for review."
    if kind == "ios":
        return False, f"{artifact.name} is an agent build ({why}). iOS has no agent build for the store: rebuild with just ios-appstore."
    return False, (f"{artifact.name} is an agent build ({why}). It must never be sent to review, so it is not uploaded. "
                   "Rebuild with: just app (or upload it for internal testing with: just testflight-agent).")


def main(argv: list[str]) -> int:
    args = list(argv)
    allow = "--allow-agent" in args
    if allow:
        args.remove("--allow-agent")
    provenance = None
    if "--provenance" in args:
        at = args.index("--provenance")
        provenance = Path(args[at + 1])
        del args[at:at + 2]
    if len(args) != 3 or args[0] != "check" or args[1] not in ("mac", "ios") or (allow and args[1] != "mac"):
        print(__doc__, file=sys.stderr)
        return 2
    env = {k: v for k, v in os.environ.items() if k != "ALLOW_AGENT_UPLOAD"}
    ok, message = check(args[1], Path(args[2]), provenance, allow, env)
    print(("OK " if ok else "FAIL ") + message, file=sys.stderr)
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
