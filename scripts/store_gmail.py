"""Gmail Connect in the store builds: off unless STORE_GMAIL=1.

Gmail Connect asks Google for https://mail.google.com/, a restricted scope: a public app using it needs the Google Cloud
project's consent screen verified (with a security assessment), else people meet "Google hasn't verified this app" and the
project stops at 100 users. So the 1.0.0 store builds ship without it unless the owner turns it on after confirming that
(features/ios-store/CUT-1.0.0.md). Without a client id the app offers no Connect and says to use an app password
(nsh/screens/settings/Google.tsx, taxpayer/logic.ts GOOGLE_UNAVAILABLE); IMAP with an app password keeps receipt tracking.

The id reaches the app only at compile time (option_env! in crates/bir-core/src/email/auth_oauth.rs and google_calendar.rs)
from the environment the store recipe builds in, which sources .env. Three commands, used by `just ios-appstore` and
`just app` (`_package-mac-appstore`):

  env            prints the shell line that drops GOOGLE_CLIENT_ID and GOOGLE_CLIENT_SECRET unless STORE_GMAIL=1:
                 eval "$(python3 scripts/store_gmail.py env)"
  check APP      after packaging (APP: an .app folder, a file, or a zip-based archive such as .ipa/.aab/.msix, read member
                 by member; other archives such as .pkg are refused rather than read as empty, here or inside a zip):
                 with the switch off, no file of APP may hold a Google OAuth client id or client secret; with it on, both must be there (the positive control: the same detector finds what the switch
                 put in, so a detector that finds nothing cannot pass a build by accident). Never prints a value. Its
                 last line says how many files it scanned and which kinds it could not read inside (see NOT_READ_INSIDE
                 for the known limits: compressed streams under other names, stub-prefixed zips, UTF-16 text, no size
                 or depth cap; a zip bomb ends it with a failure).
  entitlements SRC DST [--agent]
                 writes the Mac App Store entitlements: com.apple.security.network.server only with the switch on (the
                 one-shot 127.0.0.1 listener for Google's OAuth redirect, crates/bir-core/src/email/oauth_server.rs) or
                 in an --agent build (the agent's own socket). Nothing else in a store build listens.
"""

from __future__ import annotations

import os
import plistlib
import re
import sys
import zipfile
from pathlib import Path

SWITCH = "STORE_GMAIL"
GOOGLE_VARS = ("GOOGLE_CLIENT_ID", "GOOGLE_CLIENT_SECRET")
NETWORK_SERVER = "com.apple.security.network.server"

# Google's OAuth client ids end in .apps.googleusercontent.com; its client secrets start with GOCSPX-. A secret in Google's
# older style (no GOCSPX- prefix) is not matched: with the switch off the client id still is, and with the switch on such a
# build fails closed ("no client secret"), so an old-style secret cannot pass unseen either way.
CLIENT_ID = re.compile(rb"[0-9]{6,}-[0-9a-z]{16,}\.apps\.googleusercontent\.com")
CLIENT_SECRET = re.compile(rb"GOCSPX-[0-9A-Za-z_-]{16,}")


def gmail_on(env: dict[str, str] | os._Environ[str]) -> bool:
    """Only STORE_GMAIL=1 turns Gmail Connect on; anything else (unset included) is off."""
    return env.get(SWITCH, "") == "1"


def env_lines(env) -> tuple[str, str]:
    """(shell line for stdout, note for stderr)."""
    if gmail_on(env):
        return "", f"Store build WITH Gmail Connect ({SWITCH}=1): the Google client id is compiled in."
    value = env.get(SWITCH, "")
    note = f"Store build without Gmail Connect ({SWITCH}=1 to include it after Google's verification)."
    if value:
        note += f" {SWITCH}={value!r} is not 1, so it is off."
    return "unset " + " ".join(GOOGLE_VARS), note


# A deflated archive hides its contents from a byte scan. Zip-based ones (.ipa, .aab, .msix, .appx, .zip, .jar, .apk) are
# read member by member, nested ones too; the archive kinds below are refused wherever they sit (top level or inside a
# zip), so nobody gets a false "absent". A file that looks like a zip but cannot be read, or a member that cannot be
# read (encrypted, unsupported compression, a bad CRC), is refused too.
REFUSED_ARCHIVES = (".pkg", ".dmg", ".xar", ".tar", ".gz", ".tgz", ".xz", ".bz2", ".7z", ".appxbundle", ".msixbundle")
ZIP_NAMES = (".ipa", ".aab", ".msix", ".appx", ".zip", ".jar", ".apk")
# Files whose contents are compressed in a way a byte scan does not see through. They are scanned as bytes (an id in
# them in plain text is still found) and named in the last line of the output, so "absent" is never claimed for them
# silently. Known limits, not checked at all: compressed streams under other names, a zip with a stub in front and no
# zip name, UTF-16 text; and there is no size or depth cap (a zip bomb ends the check with a failure; nothing is written
# to disk). Rust keeps option_env! values as raw UTF-8 and Mach-O has no compressed sections, so the executables are
# covered.
NOT_READ_INSIDE = (".car", ".woff2", ".png", ".pdf", ".zst", ".lz4", ".lzma", ".br", ".cpio", ".aar", ".rar")


class Unreadable(Exception):
    """A file `check` cannot look inside."""


def _from_bytes(data: bytes, name: str, seen: dict):
    """(name, bytes) of a file, and of every member of a zip-based archive (nested ones too)."""
    lower = name.lower()
    if lower.endswith(REFUSED_ARCHIVES):
        raise Unreadable(f"{name} is an archive this check cannot read inside: point it at the .app (or the "
                         ".ipa/.aab/.msix zip) instead")
    seen["files"] += 1
    for suffix in NOT_READ_INSIDE:
        if lower.endswith(suffix):
            seen["not_read"][suffix] = seen["not_read"].get(suffix, 0) + 1
    yield name, data
    if data[:4] != b"PK\x03\x04" and not lower.endswith(ZIP_NAMES):
        return
    import io
    import zlib

    try:
        archive = zipfile.ZipFile(io.BytesIO(data))
        members = [m for m in archive.infolist() if not m.is_dir()]
    except (zipfile.BadZipFile, zipfile.LargeZipFile, EOFError, ValueError, OSError) as error:
        raise Unreadable(f"{name} looks like a zip but cannot be read ({type(error).__name__}): this check refuses it "
                         "rather than read it as empty") from None
    for member in members:
        inner = f"{name}!{member.filename}"
        try:
            body = archive.read(member)
        except (RuntimeError, NotImplementedError, zipfile.BadZipFile, zlib.error, EOFError, ValueError, OSError) as error:
            raise Unreadable(f"{inner} cannot be read ({type(error).__name__}: encrypted, an unsupported compression "
                             "or a damaged entry): this check refuses it rather than read it as empty") from None
        yield from _from_bytes(body, inner, seen)


def scan(app: Path) -> tuple[list[str], list[str], dict]:
    """The files of `app` (relative paths; `archive!member` inside a zip) that hold a client id, those that hold a
    client secret, and what was scanned ({"files": n, "not_read": {suffix: count}})."""
    ids, secrets = [], []
    seen = {"files": 0, "not_read": {}}
    for path in sorted(app.rglob("*")) if app.is_dir() else [app]:
        if path.is_symlink() or not path.is_file():
            continue
        base = str(path.relative_to(app)) if app.is_dir() else path.name
        for name, data in _from_bytes(path.read_bytes(), base, seen):
            if CLIENT_ID.search(data):
                ids.append(name)
            if CLIENT_SECRET.search(data):
                secrets.append(name)
    return ids, secrets, seen


def coverage(seen: dict) -> str:
    """The output's last line: how many files were scanned and which were not read inside."""
    line = f"Scanned {seen['files']} files"
    if seen["not_read"]:
        kinds = ", ".join(f"{suffix} ({count})" for suffix, count in sorted(seen["not_read"].items()))
        line += f"; not read inside (compressed, scanned as bytes only): {kinds}"
    return line + "."


def check(app: Path, env) -> tuple[bool, str]:
    if not app.exists():
        return False, f"no {app}"
    try:
        ids, secrets, seen = scan(app)
    except Unreadable as error:
        return False, str(error)
    except (MemoryError, RecursionError) as error:
        return False, f"{app} could not be scanned ({type(error).__name__}: too large or nested too deep)"
    tail = "\n" + coverage(seen)
    if gmail_on(env):
        if ids and secrets:
            return True, f"{SWITCH}=1: the Google client id and secret are in the build ({ids[0]}).{tail}"
        missing = " and ".join(n for n, found in (("client id", ids), ("client secret", secrets)) if not found)
        return False, (f"{SWITCH}=1 but no Google {missing} in {app}: the build would offer no Connect. "
                       f"Is GOOGLE_CLIENT_ID/GOOGLE_CLIENT_SECRET in .env?{tail}")
    if ids or secrets:
        where = ", ".join(sorted(set(ids + secrets)))
        return False, f"{SWITCH} is off but a Google OAuth client id or secret is in {where}: rebuild without them.{tail}"
    return True, f"No Google client id or secret in the build: Gmail Connect is off.{tail}"


def entitlements(src: Path, dst: Path, env, agent: bool) -> str:
    with src.open("rb") as handle:
        plist = plistlib.load(handle)
    keep = gmail_on(env) or agent
    if not keep:
        plist.pop(NETWORK_SERVER, None)
    with dst.open("wb") as handle:
        plistlib.dump(plist, handle)
    why = "Gmail Connect" if gmail_on(env) else "the agent's socket" if agent else None
    return f"{NETWORK_SERVER}: kept ({why})" if keep else f"{NETWORK_SERVER}: left out (nothing listens)"


def main(argv: list[str]) -> int:
    if not argv:
        print(__doc__, file=sys.stderr)
        return 2
    command, rest = argv[0], argv[1:]
    if command == "env" and not rest:
        line, note = env_lines(os.environ)
        print(note, file=sys.stderr)
        print(line)
        return 0
    if command == "check" and len(rest) == 1:
        ok, message = check(Path(rest[0]), os.environ)
        print(("OK " if ok else "FAIL ") + message, file=sys.stderr)
        return 0 if ok else 1
    if command == "entitlements" and len(rest) in (2, 3) and rest[2:] in ([], ["--agent"]):
        print(entitlements(Path(rest[0]), Path(rest[1]), os.environ, agent=rest[2:] == ["--agent"]), file=sys.stderr)
        return 0
    print(__doc__, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
