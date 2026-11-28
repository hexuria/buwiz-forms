import contextlib
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
PREBUILT_SH = ROOT / "scripts/lib/prebuilt.sh"
PREBUILT_PS1 = ROOT / "scripts/lib/Prebuilt.ps1"
PIN_FILE = ROOT / "scripts/impeccable.pin"
UPDATE_PINS = ROOT / "scripts/update-prebuilt-pins.py"
PWSH = shutil.which("pwsh")


def read_pin(key, pin_file=PIN_FILE):
    for line in pin_file.read_text().splitlines():
        if line.startswith(key + "="):
            return line.split("=", 1)[1]
    raise AssertionError(f"{key} missing from {pin_file}")


# The gpui-agent-cli version the pin file names: the helpers refuse any other.
CLI = read_pin("GPUI_AGENT_CLI_VERSION")


def version_tuple(text):
    return tuple(int(part) for part in text.split("."))


def write_executable(path, text):
    path.write_text(text)
    path.chmod(0o755)


def sha256_bytes(data):
    return hashlib.sha256(data).hexdigest()


def tool_pin_prefix(tool):
    return tool.upper().replace("-", "_")


def digest_pin_key(tool, target):
    return f"{tool_pin_prefix(tool)}_SHA256_{target.replace('-', '_')}"


def powershell_target_table():
    table = {}
    crate = None
    in_targets = False
    for line in PREBUILT_PS1.read_text().splitlines():
        crate_match = re.match(r"\s*'([^']+)'\s*=\s*@\{\s*$", line)
        if crate_match:
            crate = crate_match.group(1)
            table[crate] = []
            in_targets = False
            continue
        if crate and re.match(r"\s*Targets\s*=\s*@\(\s*$", line):
            in_targets = True
            continue
        if in_targets:
            target_match = re.match(r"\s*'([^']+)',?\s*$", line)
            if target_match:
                table[crate].append(target_match.group(1))
            elif line.strip().startswith(")"):
                in_targets = False
    return table


def replace_pin_line(pin_text, key, value):
    pin_text, count = re.subn(
        rf"(?m)^{re.escape(key)}=.*$",
        f"{key}={value}",
        pin_text,
    )
    if count != 1:
        raise AssertionError(f"{key} did not appear exactly once in the copied pin file")
    return pin_text


@contextlib.contextmanager
def copied_prebuilt_tree(pin_text):
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        lib = root / "scripts/lib"
        lib.mkdir(parents=True)
        shutil.copyfile(PREBUILT_SH, lib / "prebuilt.sh")
        shutil.copyfile(PREBUILT_PS1, lib / "Prebuilt.ps1")
        (root / "scripts/impeccable.pin").write_text(pin_text)
        yield lib / "prebuilt.sh", lib / "Prebuilt.ps1"


def pwsh_archive_hash_override():
    return r'''
function Get-FileHash {
    [CmdletBinding()]
    param([string]$LiteralPath, [string]$Algorithm)
    $name = Split-Path -Leaf $LiteralPath
    if (($name -like 'gpui-agent-cli-*') -or ($name -like 'impeccable-*')) {
        return [pscustomobject]@{ Hash = $env:FAKE_PS_ARCHIVE_HASH }
    }
    $sha256 = [System.Security.Cryptography.SHA256]::Create()
    try {
        $stream = [System.IO.File]::OpenRead($LiteralPath)
        try {
            $bytes = $sha256.ComputeHash($stream)
        } finally {
            $stream.Dispose()
        }
    } finally {
        $sha256.Dispose()
    }
    return [pscustomobject]@{ Hash = ([BitConverter]::ToString($bytes) -replace '-', '').ToLowerInvariant() }
}
'''


class FakeToolchain:
    """A temp PATH with small fakes for the external commands the helpers use."""

    def __init__(self, *, archive_paths=("gpui-agent-cli/gpui-agent",),
                 archive_content=b"fake archive bytes",
                 archive_exit=0, cargo_install_exit=0,
                 install_list="", system="Linux", machine="x86_64",
                 no_prebuilt=False):
        self.archive_paths = archive_paths
        self.archive_content = archive_content
        self.archive_exit = archive_exit
        self.cargo_install_exit = cargo_install_exit
        self.install_list = install_list
        self.system = system
        self.machine = machine
        self.no_prebuilt = no_prebuilt

    def __enter__(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        self.fakebin = self.root / "fakebin"
        self.fakebin.mkdir()
        self.cargo_home = self.root / "cargo-home"
        self.cargo_bin = self.cargo_home / "bin"
        self.cargo_bin.mkdir(parents=True)
        self.home = self.root / "home"
        self.home.mkdir()
        self.tmpdir = self.root / "tmp"
        self.tmpdir.mkdir()
        self.log = self.root / "log"
        self.install_list_file = self.root / "install-list"
        self.install_list_file.write_text(self.install_list)
        self.tar_list_file = self.root / "archive-list"
        self.tar_list_file.write_text("\n".join(self.archive_paths) + "\n")
        self.extract_tree = self.root / "extract-tree"
        self.extract_tree.mkdir()
        for relative in self.archive_paths:
            if relative.endswith("/"):
                (self.extract_tree / relative).mkdir(parents=True, exist_ok=True)
                continue
            file = self.extract_tree / relative
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_bytes(b"prebuilt binary payload")
            file.chmod(0o755)
        self._write_fakes()
        return self

    def __exit__(self, exc_type, exc, tb):
        self._tmp.cleanup()

    def _write_fakes(self):
        write_executable(self.fakebin / "cargo", r'''#!/bin/sh
printf 'cargo' >> "$LOG"
for arg in "$@"; do printf ' [%s]' "$arg" >> "$LOG"; done
printf '\n' >> "$LOG"
if [ "$1" = "install" ] && [ "$2" = "--list" ]; then
    [ ! -f "$FAKE_INSTALL_LIST_FILE" ] || cat "$FAKE_INSTALL_LIST_FILE"
    exit 0
fi
if [ "$1" = "install" ]; then
    [ "${FAKE_CARGO_INSTALL_EXIT:-0}" = "0" ] || exit "${FAKE_CARGO_INSTALL_EXIT:-0}"
    crate="$2"
    version=""
    previous=""
    for arg in "$@"; do
        if [ "$previous" = "--version" ]; then version="$arg"; fi
        previous="$arg"
    done
    binary="$crate"
    if [ "$crate" = "gpui-agent-cli" ]; then binary="gpui-agent"; fi
    bin="${CARGO_INSTALL_ROOT:-${CARGO_HOME:-$HOME/.cargo}}/bin"
    mkdir -p "$bin"
    printf 'source-%s-%s\n' "$crate" "${version#=}" > "$bin/$binary"
    printf 'source-%s-%s\n' "$crate" "${version#=}" > "$bin/$binary.exe"
fi
exit 0
''')
        write_executable(self.fakebin / "curl", r'''#!/bin/sh
printf 'curl' >> "$LOG"
for arg in "$@"; do printf ' [%s]' "$arg" >> "$LOG"; done
printf '\n' >> "$LOG"
out=""
url=""
while [ "$#" -gt 0 ]; do
    if [ "$1" = "-o" ]; then
        shift
        out="${1:-}"
    else
        url="$1"
    fi
    shift
done
[ "${FAKE_CURL_ARCHIVE_EXIT:-0}" = "0" ] || exit "${FAKE_CURL_ARCHIVE_EXIT:-0}"
printf '%s' "$FAKE_ARCHIVE_CONTENT" > "$out"
''')
        write_executable(self.fakebin / "tar", r'''#!/bin/sh
printf 'tar' >> "$LOG"
for arg in "$@"; do printf ' [%s]' "$arg" >> "$LOG"; done
printf '\n' >> "$LOG"
if [ "${1:-}" = "-tzf" ]; then
    cat "$FAKE_TAR_LIST_FILE"
    exit 0
fi
if [ "${1:-}" = "-xzf" ]; then
    dest=""
    while [ "$#" -gt 0 ]; do
        if [ "$1" = "-C" ]; then
            shift
            dest="${1:-}"
        fi
        shift
    done
    [ -n "$dest" ] || exit 2
    mkdir -p "$dest"
    cp -R "$FAKE_EXTRACT_TREE/." "$dest/"
    exit 0
fi
exit 2
''')
        write_executable(self.fakebin / "unzip", r'''#!/bin/sh
printf 'unzip' >> "$LOG"
for arg in "$@"; do printf ' [%s]' "$arg" >> "$LOG"; done
printf '\n' >> "$LOG"
if [ "${1:-}" = "-Z1" ]; then
    cat "$FAKE_TAR_LIST_FILE"
    exit 0
fi
dest=""
while [ "$#" -gt 0 ]; do
    if [ "$1" = "-d" ]; then
        shift
        dest="${1:-}"
    fi
    shift
done
[ -n "$dest" ] || exit 2
mkdir -p "$dest"
cp -R "$FAKE_EXTRACT_TREE/." "$dest/"
exit 0
''')
        hash_script = r'''#!/bin/sh
file=""
if [ "$(basename "$0")" = "shasum" ]; then
    while [ "$#" -gt 0 ]; do
        case "$1" in
            -a) shift 2 ;;
            *) file="$1"; shift ;;
        esac
    done
else
    file="$1"
fi
if [ -n "${FAKE_HASH_OUTPUT:-}" ]; then
    case "$(basename "$file")" in
        gpui-agent-cli-*|impeccable-*|cargo-binstall-*)
            printf '%s  %s\n' "$FAKE_HASH_OUTPUT" "$file"
            exit 0
            ;;
    esac
fi
python3 -c 'import hashlib,sys; print(hashlib.sha256(open(sys.argv[1],"rb").read()).hexdigest() + "  " + sys.argv[1])' "$file"
'''
        write_executable(self.fakebin / "sha256sum", hash_script)
        write_executable(self.fakebin / "shasum", hash_script)
        write_executable(self.fakebin / "uname", r'''#!/bin/sh
case "${1:-}" in
    -s) printf '%s\n' "${FAKE_UNAME_S:-Linux}" ;;
    -m) printf '%s\n' "${FAKE_UNAME_M:-x86_64}" ;;
    *) printf '%s\n' "${FAKE_UNAME_S:-Linux}" ;;
esac
''')

    def env(self):
        env = {
            "PATH": os.pathsep.join([str(self.fakebin), "/usr/bin", "/bin"]),
            "HOME": str(self.home),
            "LOG": str(self.log),
            "CARGO_HOME": str(self.cargo_home),
            "TMPDIR": str(self.tmpdir),
            "FAKE_CARGO_INSTALL_EXIT": str(self.cargo_install_exit),
            "FAKE_CURL_ARCHIVE_EXIT": str(self.archive_exit),
            "FAKE_INSTALL_LIST_FILE": str(self.install_list_file),
            "FAKE_TAR_LIST_FILE": str(self.tar_list_file),
            "FAKE_EXTRACT_TREE": str(self.extract_tree),
            "FAKE_ARCHIVE_CONTENT": self.archive_content.decode(),
            "FAKE_UNAME_S": self.system,
            "FAKE_UNAME_M": self.machine,
            "PROCESSOR_ARCHITECTURE": "AMD64",
        }
        if self.no_prebuilt:
            env["BUWIZ_SETUP_NO_PREBUILT"] = "1"
        return env

    def install_binary(self, crate, binary, content=b"installed binary", *,
                       version=CLI, route="prebuilt", stamp=True, exe=False):
        name = binary + (".exe" if exe else "")
        path = self.cargo_bin / name
        path.write_bytes(content)
        path.chmod(0o755)
        if stamp:
            digest = sha256_bytes(content)
            (self.cargo_bin / f".{crate}.buwiz-installed").write_text(
                f"{version}\n{route}\n{digest}\n"
            )

    def collect(self, result):
        return {
            "result": result,
            "log": self.log.read_text() if self.log.exists() else "",
            "output": result.stdout + result.stderr,
            "stdout": result.stdout,
            "stderr": result.stderr,
            "bin_contents": {
                path.name: path.read_bytes()
                for path in sorted(self.cargo_bin.iterdir())
                if path.is_file() and not path.name.startswith(".")
            },
            "stamps": {
                path.name: path.read_text()
                for path in sorted(self.cargo_bin.iterdir())
                if path.is_file() and path.name.startswith(".")
            },
            "tmp_entries": sorted(path.name for path in self.tmpdir.iterdir()),
        }


def run_bash_helper(fake, script, env_extra=None, helper=PREBUILT_SH):
    env = fake.env()
    if env_extra:
        env.update(env_extra)
    result = subprocess.run(
        ["bash", "-c", f'. "{helper}"\n' + script],
        env=env, text=True, capture_output=True,
    )
    return fake.collect(result)


def run_pwsh_helper(fake, script, helper=PREBUILT_PS1):
    result = subprocess.run(
        [PWSH, "-NoProfile", "-Command", f'. "{helper}"\n' + script],
        env=fake.env(), text=True, capture_output=True,
    )
    return fake.collect(result)


# The helper is bash for Linux and macOS; Windows has its own (Prebuilt.ps1), checked below.
@unittest.skipIf(sys.platform == "win32", "prebuilt.sh is for Linux and macOS")
class PrebuiltShellTests(unittest.TestCase):
    def test_direct_download_success_writes_a_prebuilt_stamp_without_calling_cargo(self):
        pin = read_pin("GPUI_AGENT_CLI_SHA256_x86_64_unknown_linux_gnu")
        with FakeToolchain() as fake:
            data = run_bash_helper(
                fake,
                f"install_pinned_tool gpui-agent-cli {CLI}\n",
                {"FAKE_HASH_OUTPUT": pin},
            )
        self.assertEqual(data["result"].returncode, 0, data["output"])
        self.assertNotIn("cargo ", data["log"])
        archive = f"gpui-agent-cli-x86_64-unknown-linux-gnu-v{CLI}.tgz"
        self.assertIn(
            f"https://github.com/hexuria/gpui-agent/releases/download/v{CLI}/{archive}",
            data["log"],
        )
        self.assertNotIn(".sha256", data["log"])
        self.assertIn("--max-time", data["log"])
        self.assertIn("[60]", data["log"])
        self.assertEqual(data["tmp_entries"], [])
        digest = sha256_bytes(b"prebuilt binary payload")
        self.assertEqual(
            data["stamps"][".gpui-agent-cli.buwiz-installed"].splitlines(),
            [CLI, "prebuilt", digest],
        )
        self.assertEqual(data["bin_contents"]["gpui-agent"], b"prebuilt binary payload")

    def test_digest_mismatch_builds_from_source_once_and_leaves_no_downloads(self):
        with FakeToolchain() as fake:
            data = run_bash_helper(
                fake,
                f"install_pinned_tool gpui-agent-cli {CLI}\n",
                {"FAKE_HASH_OUTPUT": "0" * 64},
            )
        self.assertEqual(data["result"].returncode, 0, data["output"])
        self.assertNotIn(".sha256", data["log"])
        cargo_calls = [line for line in data["log"].splitlines() if line.startswith("cargo ")]
        self.assertEqual(
            cargo_calls,
            [f"cargo [install] [gpui-agent-cli] [--version] [={CLI}]"],
        )
        self.assertEqual(data["output"].count("building from source"), 1, data["output"])
        self.assertIn("digest", data["output"])
        self.assertEqual(data["tmp_entries"], [])
        digest = sha256_bytes(f"source-gpui-agent-cli-{CLI}\n".encode())
        self.assertEqual(
            data["stamps"][".gpui-agent-cli.buwiz-installed"].splitlines(),
            [CLI, "source", digest],
        )

    def test_missing_archive_builds_from_source(self):
        with FakeToolchain(archive_exit=22) as fake:
            data = run_bash_helper(fake, f"install_pinned_tool gpui-agent-cli {CLI}\n")
        self.assertEqual(data["result"].returncode, 0, data["output"])
        self.assertNotIn("tar ", data["log"])
        self.assertEqual(
            [line for line in data["log"].splitlines() if line.startswith("cargo ")],
            [f"cargo [install] [gpui-agent-cli] [--version] [={CLI}]"],
        )
        self.assertEqual(data["output"].count("building from source"), 1, data["output"])
        self.assertIn("download", data["output"])

    def test_a_missing_target_digest_pin_never_downloads_and_builds_from_source(self):
        key = "GPUI_AGENT_CLI_SHA256_x86_64_unknown_linux_gnu"
        pin_text = re.sub(rf"(?m)^{re.escape(key)}=.*\n", "", PIN_FILE.read_text())
        with copied_prebuilt_tree(pin_text) as (bash_helper, _):
            with FakeToolchain() as fake:
                data = run_bash_helper(
                    fake,
                    f"install_pinned_tool gpui-agent-cli {CLI}\n",
                    helper=bash_helper,
                )
        self.assertEqual(data["result"].returncode, 0, data["output"])
        self.assertNotIn("curl ", data["log"])
        self.assertIn("digest pin", data["output"])
        self.assertEqual(data["output"].count("building from source"), 1, data["output"])
        self.assertEqual(
            [line for line in data["log"].splitlines() if line.startswith("cargo ")],
            [f"cargo [install] [gpui-agent-cli] [--version] [={CLI}]"],
        )

    def test_sha256_for_version_mismatch_refuses_prebuilt_in_both_directions(self):
        base_pin = PIN_FILE.read_text()
        cases = (
            (
                "version pin moved without its digests",
                replace_pin_line(base_pin, "GPUI_AGENT_CLI_VERSION", "0.4.2"),
                "0.4.2",
            ),
            (
                "digest generation moved without the version pin",
                replace_pin_line(base_pin, "GPUI_AGENT_CLI_SHA256_FOR", "0.4.2"),
                CLI,
            ),
            (
                "install version is not the version pin",
                base_pin,
                "0.4.2",
            ),
        )
        for name, pin_text, install_version in cases:
            with self.subTest(name=name), copied_prebuilt_tree(pin_text) as (bash_helper, _):
                with FakeToolchain() as fake:
                    data = run_bash_helper(
                        fake,
                        f"install_pinned_tool gpui-agent-cli {install_version}\n",
                        helper=bash_helper,
                    )
            self.assertEqual(data["result"].returncode, 0, data["output"])
            self.assertNotIn("curl ", data["log"])
            self.assertIn("pin version", data["output"])
            self.assertEqual(data["output"].count("building from source"), 1, data["output"])
            self.assertEqual(
                [line for line in data["log"].splitlines() if line.startswith("cargo ")],
                [f"cargo [install] [gpui-agent-cli] [--version] [={install_version}]"],
            )

    def test_a_crate_outside_the_table_never_downloads(self):
        with FakeToolchain() as fake:
            data = run_bash_helper(fake, "install_pinned_tool just 1.45.0\n")
        self.assertEqual(data["result"].returncode, 0, data["output"])
        self.assertNotIn("curl ", data["log"])
        self.assertEqual(
            [line for line in data["log"].splitlines() if line.startswith("cargo ")],
            ["cargo [install] [just] [--version] [=1.45.0]"],
        )
        self.assertEqual(
            data["stamps"][".just.buwiz-installed"].splitlines()[1],
            "source",
        )

    def test_no_prebuilt_goes_straight_to_the_source_build(self):
        with FakeToolchain(no_prebuilt=True) as fake:
            data = run_bash_helper(fake, f"install_pinned_tool gpui-agent-cli {CLI}\n")
        self.assertEqual(data["result"].returncode, 0, data["output"])
        self.assertNotIn("curl ", data["log"])
        self.assertEqual(
            [line for line in data["log"].splitlines() if line.startswith("cargo ")],
            [f"cargo [install] [gpui-agent-cli] [--version] [={CLI}]"],
        )

    def test_skip_when_the_binary_and_stamp_are_already_at_the_pin(self):
        content = b"already installed binary"
        with FakeToolchain() as fake:
            fake.install_binary("gpui-agent-cli", "gpui-agent", content)
            data = run_bash_helper(fake, f"install_pinned_tool gpui-agent-cli {CLI}\n")
        self.assertEqual(data["result"].returncode, 0, data["output"])
        self.assertIn(f"already at {CLI}", data["output"])
        self.assertNotIn("curl ", data["log"])
        self.assertNotIn("cargo ", data["log"])

    def test_a_replaced_binary_invalidates_the_stamp(self):
        content = b"original binary"
        with FakeToolchain() as fake:
            fake.install_binary("gpui-agent-cli", "gpui-agent", content)
            (fake.cargo_bin / "gpui-agent").write_bytes(b"replaced binary")
            data = run_bash_helper(
                fake,
                'installed_pinned_version gpui-agent-cli gpui-agent\n',
            )
        self.assertEqual(data["result"].stdout.strip(), "")

    def test_a_bracketed_install_list_line_is_not_the_pin_but_a_plain_line_is(self):
        with FakeToolchain(
            install_list=f"gpui-agent-cli v{CLI} (/tmp/local-checkout):\n"
        ) as fake:
            fake.install_binary("gpui-agent-cli", "gpui-agent", stamp=False)
            bracketed = run_bash_helper(
                fake,
                'installed_pinned_version gpui-agent-cli gpui-agent\n',
            )
        self.assertEqual(bracketed["result"].stdout.strip(), "")
        with FakeToolchain(install_list=f"gpui-agent-cli v{CLI}:\n") as fake:
            fake.install_binary("gpui-agent-cli", "gpui-agent", stamp=False)
            plain = run_bash_helper(
                fake,
                'installed_pinned_version gpui-agent-cli gpui-agent\n',
            )
        self.assertEqual(plain["result"].stdout.strip(), CLI)

    def test_an_unexpected_archive_layout_builds_from_source(self):
        pin = read_pin("GPUI_AGENT_CLI_SHA256_x86_64_unknown_linux_gnu")
        with FakeToolchain(archive_paths=("too/deep/gpui-agent",)) as fake:
            data = run_bash_helper(
                fake,
                f"install_pinned_tool gpui-agent-cli {CLI}\n",
                {"FAKE_HASH_OUTPUT": pin},
            )
        self.assertEqual(data["result"].returncode, 0, data["output"])
        self.assertIn("layout", data["output"])
        self.assertEqual(
            [line for line in data["log"].splitlines() if line.startswith("cargo ")],
            [f"cargo [install] [gpui-agent-cli] [--version] [={CLI}]"],
        )

    def test_impeccable_accepts_the_binary_at_the_top_level_or_one_folder_down(self):
        pin = read_pin("IMPECCABLE_SHA256_x86_64_unknown_linux_musl")
        for path in ("impeccable", "impeccable-bin/impeccable"):
            with self.subTest(path=path), FakeToolchain(archive_paths=(path,)) as fake:
                data = run_bash_helper(
                    fake,
                    "install_pinned_tool impeccable 0.2.1 --locked\n",
                    {"FAKE_HASH_OUTPUT": pin},
                )
            self.assertEqual(data["result"].returncode, 0, data["output"])
            self.assertNotIn("cargo [install]", data["log"])
            self.assertIn("impeccable", data["bin_contents"])

    def test_check_mode_report_matches_mismatches_and_reports_unknown(self):
        content = b"check binary"
        cases = (
            (CLI, 0, f"gpui-agent  {CLI}  (pin {CLI})"),
            ("0.4.0", 1, f"gpui-agent  0.4.0  (pin {CLI})"),
        )
        for stamp_version, expected_rc, expected_line in cases:
            with self.subTest(stamp_version=stamp_version), FakeToolchain() as fake:
                fake.install_binary(
                    "gpui-agent-cli", "gpui-agent", content, version=stamp_version
                )
                data = run_bash_helper(
                    fake,
                    f"prebuilt_report_pinned gpui-agent gpui-agent-cli gpui-agent {CLI}\n",
                )
            self.assertEqual(data["result"].returncode, expected_rc, data["output"])
            self.assertIn(expected_line, data["output"])
        with FakeToolchain() as fake:
            unknown = run_bash_helper(
                fake,
                f"prebuilt_report_pinned gpui-agent gpui-agent-cli gpui-agent {CLI}\n",
            )
        self.assertEqual(unknown["result"].returncode, 1, unknown["output"])
        self.assertIn(f"gpui-agent  unknown  (pin {CLI})", unknown["output"])

    def test_ensure_cargo_binstall_checks_the_pinned_linux_digest_before_unpacking(self):
        digest_pin = read_pin("CARGO_BINSTALL_SHA256_x86_64_unknown_linux_musl")
        with FakeToolchain(archive_paths=("cargo-binstall",)) as fake:
            binstall = fake.extract_tree / "cargo-binstall"
            binstall.write_text(
                '#!/bin/sh\nif [ "${1:-}" = "-V" ]; then\n'
                f'    echo "cargo-binstall {read_pin("CARGO_BINSTALL_VERSION")}"\n'
                'fi\n'
            )
            binstall.chmod(0o755)
            data = run_bash_helper(
                fake,
                "ensure_cargo_binstall\n",
                {"FAKE_HASH_OUTPUT": digest_pin},
            )
        self.assertEqual(data["result"].returncode, 0, data["output"])
        self.assertIn("cargo-binstall-x86_64-unknown-linux-musl.tgz", data["log"])
        self.assertIn("cargo-binstall", data["bin_contents"])
        with FakeToolchain(archive_paths=("cargo-binstall",)) as fake:
            bad = run_bash_helper(fake, "ensure_cargo_binstall\n")
        self.assertNotEqual(bad["result"].returncode, 0, bad["output"])
        self.assertNotIn("cargo-binstall", bad["bin_contents"])
        self.assertEqual(bad["tmp_entries"], [])


class PrebuiltPinTests(unittest.TestCase):
    def test_cli_pin_covers_every_recipe_needs_cli_and_the_library_pin(self):
        cli_pin = read_pin("GPUI_AGENT_CLI_VERSION")
        needed = []
        for file in (ROOT / "agent-recipes").rglob("*.json"):
            try:
                document = json.loads(file.read_text())
            except json.JSONDecodeError:
                continue
            stack = [document]
            while stack:
                value = stack.pop()
                if isinstance(value, dict):
                    for key, child in value.items():
                        if key == "needs_cli":
                            needed.append((file, child))
                        else:
                            stack.append(child)
                elif isinstance(value, list):
                    stack.extend(value)
        self.assertTrue(needed, "no needs_cli values found under agent-recipes")
        for file, needed_version in needed:
            self.assertGreaterEqual(
                version_tuple(cli_pin),
                version_tuple(needed_version),
                f"{file} needs CLI {needed_version}",
            )
        manifest = (ROOT / "Cargo.toml").read_text()
        match = re.search(
            r'gpui-agent\s*=\s*\{\s*version\s*=\s*"=(\d+\.\d+\.\d+)"',
            manifest,
        )
        self.assertIsNotNone(match)
        self.assertGreaterEqual(version_tuple(cli_pin), version_tuple(match.group(1)))

    def test_release_digest_pins_cover_every_helper_target_exactly(self):
        expected_targets = {
            "gpui-agent-cli": {
                "aarch64-apple-darwin",
                "x86_64-apple-darwin",
                "aarch64-unknown-linux-gnu",
                "x86_64-unknown-linux-gnu",
                "x86_64-pc-windows-msvc",
            },
            "impeccable": {
                "aarch64-apple-darwin",
                "x86_64-apple-darwin",
                "aarch64-unknown-linux-musl",
                "x86_64-unknown-linux-musl",
                "x86_64-pc-windows-msvc",
                "aarch64-pc-windows-msvc",
            },
        }
        actual_targets = {
            tool: set(targets) for tool, targets in powershell_target_table().items()
        }
        self.assertEqual(actual_targets, expected_targets)
        expected_keys = set()
        for tool, targets in expected_targets.items():
            for target in targets:
                key = digest_pin_key(tool, target)
                expected_keys.add(key)
                self.assertRegex(read_pin(key), r"^[0-9a-f]{64}$", key)

        actual_keys = set()
        sha_for_keys = {
            f"{tool_pin_prefix(tool)}_SHA256_FOR" for tool in expected_targets
        }
        for line in PIN_FILE.read_text().splitlines():
            key = line.split("=", 1)[0]
            if key in sha_for_keys:
                continue
            if re.match(r"^(GPUI_AGENT_CLI|IMPECCABLE)_SHA256_", key):
                actual_keys.add(key)
        self.assertEqual(actual_keys, expected_keys)

    def test_sha256_for_tracks_each_tool_version_pin(self):
        for tool in ("gpui-agent-cli", "impeccable"):
            prefix = tool_pin_prefix(tool)
            self.assertEqual(
                read_pin(f"{prefix}_SHA256_FOR"),
                read_pin(f"{prefix}_VERSION"),
                tool,
            )

    def test_binstall_digest_pins_cover_only_the_two_linux_musl_targets(self):
        pin_text = (ROOT / "scripts/impeccable.pin").read_text()
        self.assertIn(
            "CARGO_BINSTALL_SHA256_x86_64_unknown_linux_musl="
            "7ffe1832c4bc3148d00f02d51c1d96c9cccdec76a0fa20e6e5692201d78bcc0b",
            pin_text,
        )
        self.assertIn(
            "CARGO_BINSTALL_SHA256_aarch64_unknown_linux_musl="
            "c2c44aba65d402ac43ee821387c49246bc574bde65d3d7f0f7a9e666c5a69f7f",
            pin_text,
        )
        self.assertNotIn("CARGO_BINSTALL_SHA256_x86_64_apple_darwin", pin_text)
        self.assertNotIn("CARGO_BINSTALL_SHA256_x86_64_pc_windows_msvc", pin_text)


class PrebuiltSetupTextTests(unittest.TestCase):
    def test_shell_setup_scripts_use_the_direct_download_helper_and_plain_cargo_for_other_crates(self):
        linux = (ROOT / "scripts/setup-linux.sh").read_text()
        macos = (ROOT / "scripts/setup-macos.sh").read_text()
        impeccable = (ROOT / "scripts/setup-impeccable.sh").read_text()
        self.assertIn('install_pinned_tool gpui-agent-cli "$version"', linux)
        self.assertIn('install_pinned_tool gpui-agent-cli "$version"', macos)
        self.assertIn('install_pinned_tool impeccable "$version" --locked', impeccable)
        self.assertIn('cargo install just --version =1.45.0', linux)
        for script in (linux, macos):
            self.assertIn('prebuilt_pin GPUI_AGENT_CLI_VERSION', script)
            self.assertIn('prebuilt_report_pinned gpui-agent gpui-agent-cli gpui-agent', script)
            for crate, pin in (
                ("cargo-audit", "0.22.1"),
                ("cargo-outdated", "0.19.0"),
                ("cargo-machete", "0.9.2"),
            ):
                self.assertIn(f'cargo install {crate} --version ={pin}', script)
            self.assertNotIn('install_prebuilt_or_build', script)
        self.assertNotIn('install_prebuilt_or_build', impeccable)

    def test_powershell_helper_and_call_sites_use_boolean_status_and_no_binstall(self):
        helper = PREBUILT_PS1.read_text()
        setup = (ROOT / "scripts/setup-windows.ps1").read_text()
        e2e = (ROOT / "scripts/e2e/machines/windows.ps1").read_text()
        for text in (
            "function Install-PinnedTool",
            "function Get-PrebuiltInstalledVersion",
            "Install-PinnedTool 'gpui-agent-cli'",
            "| Out-Host",
            "BUWIZ_SETUP_NO_PREBUILT",
            "pc-windows-msvc",
        ):
            self.assertIn(text, helper + setup)
        self.assertNotIn("cargo binstall", helper)
        self.assertNotIn("Ensure-CargoBinstall", helper)
        self.assertIn("[switch]$Check", setup)
        self.assertIn("Read-PrebuiltPin 'GPUI_AGENT_CLI_VERSION'", setup)
        self.assertRegex(setup, r"if \(-not \(Install-PinnedTool 'gpui-agent-cli'")
        self.assertNotIn("if ($LASTEXITCODE -ne 0) { Exit-Native \"gpui-agent install failed\" }", setup)
        self.assertIn("Get-PrebuiltInstalledVersion", e2e)

    def test_binstall_is_only_in_the_linux_blueprint_and_is_digest_checked(self):
        for relative in (".devin/blueprint.yaml", "docs/platforms/devin-blueprint.yml"):
            blueprint = (ROOT / relative).read_text()
            self.assertIn("ensure_cargo_binstall", blueprint)
        helper = PREBUILT_SH.read_text()
        self.assertIn("CARGO_BINSTALL_SHA256_", helper)
        self.assertNotIn("apple-darwin.zip", helper)
        self.assertNotIn("pc-windows-msvc.zip", PREBUILT_PS1.read_text())

    def test_e2e_version_detection_uses_the_shared_stamp_reader(self):
        e2e = (ROOT / "scripts/e2e/machines/common.sh").read_text()
        self.assertIn("installed_pinned_version gpui-agent-cli gpui-agent", e2e)

    def test_docs_describe_the_direct_download_route_and_its_limits(self):
        docs = (ROOT / "docs/platforms/devin-environment.md").read_text()
        self.assertIn("## Prebuilt tools", docs)
        self.assertIn("gpui-agent-cli", docs)
        self.assertIn("impeccable", docs)
        self.assertIn("releases/download", docs)
        self.assertIn("scripts/impeccable.pin", docs)
        self.assertIn("scripts/update-prebuilt-pins.py", docs)
        self.assertIn("BUWIZ_SETUP_NO_PREBUILT=1", docs)
        self.assertIn("GPUI_AGENT_CLI_VERSION", docs)
        self.assertIn("release changed", docs)
        self.assertIn("source build", docs)
        self.assertNotIn("downloads the sibling", docs)


# These run Prebuilt.ps1's functions against FAKE curl, tar, unzip and cargo, which are POSIX scripts: PowerShell on
# Windows cannot start them ("Cannot run a document in the middle of a pipeline"), so there the class is skipped.
# On Windows the helper was proven on a real machine instead (the direct download, the tamper check, the source
# route and check mode, 2026-11-28); the text tests below still run everywhere.
@unittest.skipIf(PWSH is None or sys.platform == "win32", "needs pwsh and POSIX fake tools")
class PrebuiltPowerShellRunTests(unittest.TestCase):
    def test_download_success_returns_one_boolean_and_writes_the_stamp(self):
        pin = read_pin("GPUI_AGENT_CLI_SHA256_x86_64_pc_windows_msvc")
        with FakeToolchain(archive_paths=("gpui-agent-cli/gpui-agent.exe",)) as fake:
            data = run_pwsh_helper(
                fake,
                pwsh_archive_hash_override()
                + f"$env:FAKE_PS_ARCHIVE_HASH = '{pin}'\n"
                + f"$out = @(Install-PinnedTool 'gpui-agent-cli' '{CLI}')\n"
                "'COUNT:' + $out.Count\n"
                "'TYPE:' + $out[0].GetType().Name\n"
                "'VALUE:' + $out[0]\n",
            )
        self.assertEqual(data["result"].returncode, 0, data["output"])
        self.assertIn("COUNT:1", data["stdout"], data["output"])
        self.assertIn("TYPE:Boolean", data["stdout"], data["output"])
        self.assertIn("VALUE:True", data["stdout"], data["output"])
        self.assertNotIn("cargo ", data["log"])
        self.assertNotIn(".sha256", data["log"])
        digest = sha256_bytes(b"prebuilt binary payload")
        self.assertEqual(
            data["stamps"][".gpui-agent-cli.buwiz-installed"].splitlines(),
            [CLI, "prebuilt", digest],
        )

    def test_digest_mismatch_falls_back_to_source_and_returns_boolean(self):
        with FakeToolchain(archive_paths=("gpui-agent-cli/gpui-agent.exe",)) as fake:
            data = run_pwsh_helper(
                fake,
                f"$out = @(Install-PinnedTool 'gpui-agent-cli' '{CLI}')\n"
                "'COUNT:' + $out.Count\n"
                "'VALUE:' + $out[0]\n",
            )
        self.assertEqual(data["result"].returncode, 0, data["output"])
        self.assertIn("COUNT:1", data["stdout"], data["output"])
        self.assertIn("VALUE:True", data["stdout"], data["output"])
        self.assertNotIn(".sha256", data["log"])
        self.assertEqual(data["output"].count("building from source"), 1, data["output"])
        self.assertIn(
            f"cargo [install] [gpui-agent-cli] [--version] [={CLI}]",
            data["log"],
        )

    def test_sha256_for_mismatch_refuses_the_windows_prebuilt_route(self):
        pin_text = replace_pin_line(
            PIN_FILE.read_text(),
            "GPUI_AGENT_CLI_SHA256_FOR",
            "0.4.0",
        )
        with copied_prebuilt_tree(pin_text) as (_, powershell_helper):
            with FakeToolchain(archive_paths=("gpui-agent-cli/gpui-agent.exe",)) as fake:
                data = run_pwsh_helper(
                    fake,
                    f"$out = @(Install-PinnedTool 'gpui-agent-cli' '{CLI}')\n"
                    "'VALUE:' + $out[0]\n",
                    helper=powershell_helper,
                )
        self.assertEqual(data["result"].returncode, 0, data["output"])
        self.assertIn("VALUE:True", data["stdout"], data["output"])
        self.assertNotIn("curl ", data["log"])
        self.assertIn("pin version", data["output"])
        self.assertEqual(data["output"].count("building from source"), 1, data["output"])
        self.assertIn(
            f"cargo [install] [gpui-agent-cli] [--version] [={CLI}]",
            data["log"],
        )

    def test_impeccable_windows_zip_uses_unzip_and_accepts_one_folder_down(self):
        pin = read_pin("IMPECCABLE_SHA256_x86_64_pc_windows_msvc")
        with FakeToolchain(archive_paths=("impeccable-bin/impeccable.exe",)) as fake:
            data = run_pwsh_helper(
                fake,
                pwsh_archive_hash_override()
                + f"$env:FAKE_PS_ARCHIVE_HASH = '{pin}'\n"
                + "$out = @(Install-PinnedTool 'impeccable' '0.2.1' '--locked')\n"
                "'VALUE:' + $out[0]\n",
            )
        self.assertEqual(data["result"].returncode, 0, data["output"])
        self.assertIn("VALUE:True", data["stdout"], data["output"])
        self.assertIn("unzip ", data["log"])
        self.assertIn("impeccable.exe", data["bin_contents"])
        self.assertNotIn("cargo [install]", data["log"])

    def test_skip_when_current_returns_true_without_downloading_or_calling_cargo(self):
        content = b"already installed windows binary"
        with FakeToolchain() as fake:
            fake.install_binary(
                "gpui-agent-cli", "gpui-agent", content, exe=True
            )
            data = run_pwsh_helper(
                fake,
                f"$out = @(Install-PinnedTool 'gpui-agent-cli' '{CLI}')\n"
                "'VALUE:' + $out[0]\n",
            )
        self.assertEqual(data["result"].returncode, 0, data["output"])
        self.assertIn("VALUE:True", data["stdout"], data["output"])
        self.assertNotIn("curl ", data["log"])
        self.assertNotIn("cargo ", data["log"])

    def test_windows_check_report_matches_mismatches_and_reports_unknown(self):
        cli_pin = read_pin("GPUI_AGENT_CLI_VERSION")

        def report(fake):
            return run_pwsh_helper(
                fake,
                f"$out = @(Write-PrebuiltPinnedReport 'gpui-agent' 'gpui-agent-cli' 'gpui-agent' '{cli_pin}')\n"
                "'VALUE:' + $out[0]\n",
            )

        with FakeToolchain() as fake:
            fake.install_binary("gpui-agent-cli", "gpui-agent", b"gpui current", exe=True)
            matched = report(fake)
        self.assertIn("VALUE:True", matched["stdout"], matched["output"])
        self.assertIn(f"gpui-agent  {cli_pin}  (pin {cli_pin})", matched["output"])

        with FakeToolchain() as fake:
            fake.install_binary("gpui-agent-cli", "gpui-agent", b"gpui old", version="0.4.0", exe=True)
            mismatched = report(fake)
        self.assertIn("VALUE:False", mismatched["stdout"], mismatched["output"])
        self.assertIn(f"gpui-agent  0.4.0  (pin {cli_pin})", mismatched["output"])

        with FakeToolchain() as fake:
            unknown = report(fake)
        self.assertIn("VALUE:False", unknown["stdout"], unknown["output"])
        self.assertIn(f"gpui-agent  unknown  (pin {cli_pin})", unknown["output"])


@unittest.skipIf(sys.platform == "win32", "the fake gh executable is a shell script")
class UpdatePrebuiltPinsTests(unittest.TestCase):
    TARGETS = powershell_target_table()

    @staticmethod
    def asset_name(tool, target, version):
        if tool == "gpui-agent-cli":
            return f"gpui-agent-cli-{target}-v{version}.tgz"
        if target.endswith("-pc-windows-msvc"):
            return f"impeccable-{target}-v{version}.zip"
        return f"impeccable-{target}-v{version}.tar.gz"

    @classmethod
    def release_payload(cls, tool, version, *, missing_asset=None, missing_digest=None):
        assets = []
        for target in cls.TARGETS[tool]:
            if target == missing_asset:
                continue
            asset = {
                "name": cls.asset_name(tool, target, version),
                "digest": "sha256:" + sha256_bytes(f"{tool}-{target}-{version}".encode()),
            }
            if target == missing_digest:
                del asset["digest"]
            assets.append(asset)
        return {"assets": assets}

    @staticmethod
    @contextlib.contextmanager
    def copied_update_tree(payload, gh_exit=0):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            scripts = root / "scripts"
            scripts.mkdir()
            script = scripts / "update-prebuilt-pins.py"
            pin_file = scripts / "impeccable.pin"
            shutil.copyfile(UPDATE_PINS, script)
            shutil.copyfile(PIN_FILE, pin_file)
            payload_file = root / "release.json"
            payload_file.write_text(json.dumps(payload))
            fakebin = root / "fakebin"
            fakebin.mkdir()
            gh_log = root / "gh-log"
            write_executable(fakebin / "gh", r'''#!/bin/sh
printf 'gh' >> "$GH_LOG"
for arg in "$@"; do printf ' [%s]' "$arg" >> "$GH_LOG"; done
printf '\n' >> "$GH_LOG"
if [ "${FAKE_GH_EXIT:-0}" != "0" ]; then
    echo 'fake gh failure' >&2
    exit "$FAKE_GH_EXIT"
fi
cat "$FAKE_GH_PAYLOAD"
''')
            env = {
                "PATH": os.pathsep.join([str(fakebin), "/usr/bin", "/bin"]),
                "GH_LOG": str(gh_log),
                "FAKE_GH_PAYLOAD": str(payload_file),
                "FAKE_GH_EXIT": str(gh_exit),
            }
            yield script, pin_file, gh_log, env

    def run_update(self, script, env, tool, version):
        return subprocess.run(
            [sys.executable, str(script), tool, version],
            env=env,
            text=True,
            capture_output=True,
        )

    def test_gpui_update_rewrites_version_generation_and_every_digest(self):
        version = "0.4.2"
        payload = self.release_payload("gpui-agent-cli", version)
        with self.copied_update_tree(payload) as (script, pin_file, gh_log, env):
            result = self.run_update(script, env, "gpui-agent-cli", version)
            self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
            self.assertIn(
                f"repos/hexuria/gpui-agent/releases/tags/v{version}",
                gh_log.read_text(),
            )
            self.assertEqual(read_pin("GPUI_AGENT_CLI_VERSION", pin_file), version)
            self.assertEqual(read_pin("GPUI_AGENT_CLI_SHA256_FOR", pin_file), version)
            for target in self.TARGETS["gpui-agent-cli"]:
                key = digest_pin_key("gpui-agent-cli", target)
                self.assertEqual(
                    read_pin(key, pin_file),
                    sha256_bytes(f"gpui-agent-cli-{target}-{version}".encode()),
                    key,
                )
                self.assertEqual(
                    sum(line.startswith(key + "=") for line in pin_file.read_text().splitlines()),
                    1,
                    key,
                )
            self.assertEqual(read_pin("IMPECCABLE_VERSION", pin_file), read_pin("IMPECCABLE_VERSION"))

    def test_impeccable_update_uses_the_release_archive_names_for_every_target(self):
        version = "0.2.2"
        payload = self.release_payload("impeccable", version)
        with self.copied_update_tree(payload) as (script, pin_file, gh_log, env):
            result = self.run_update(script, env, "impeccable", version)
            self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
            self.assertIn(
                f"repos/hexuria/impeccable-rust/releases/tags/v{version}",
                gh_log.read_text(),
            )
            self.assertEqual(read_pin("IMPECCABLE_VERSION", pin_file), version)
            self.assertEqual(read_pin("IMPECCABLE_SHA256_FOR", pin_file), version)
            for target in self.TARGETS["impeccable"]:
                self.assertEqual(
                    read_pin(digest_pin_key("impeccable", target), pin_file),
                    sha256_bytes(f"impeccable-{target}-{version}".encode()),
                    target,
                )

    def test_a_missing_asset_refuses_and_leaves_the_pin_file_untouched(self):
        version = "0.4.2"
        payload = self.release_payload(
            "gpui-agent-cli", version, missing_asset="aarch64-apple-darwin"
        )
        with self.copied_update_tree(payload) as (script, pin_file, _, env):
            before = pin_file.read_bytes()
            result = self.run_update(script, env, "gpui-agent-cli", version)
            self.assertNotEqual(result.returncode, 0, result.stdout)
            self.assertIn("aarch64-apple-darwin", result.stderr)
            self.assertIn("gpui-agent-cli-aarch64-apple-darwin-v0.4.2.tgz", result.stderr)
            self.assertEqual(pin_file.read_bytes(), before)

    def test_a_missing_digest_refuses_and_leaves_the_pin_file_untouched(self):
        version = "0.2.2"
        payload = self.release_payload(
            "impeccable", version, missing_digest="x86_64-pc-windows-msvc"
        )
        with self.copied_update_tree(payload) as (script, pin_file, _, env):
            before = pin_file.read_bytes()
            result = self.run_update(script, env, "impeccable", version)
            self.assertNotEqual(result.returncode, 0, result.stdout)
            self.assertIn("x86_64-pc-windows-msvc", result.stderr)
            self.assertIn("digest", result.stderr)
            self.assertEqual(pin_file.read_bytes(), before)

    def test_a_gh_failure_refuses_and_leaves_the_pin_file_untouched(self):
        version = "0.4.2"
        payload = self.release_payload("gpui-agent-cli", version)
        with self.copied_update_tree(payload, gh_exit=7) as (script, pin_file, _, env):
            before = pin_file.read_bytes()
            result = self.run_update(script, env, "gpui-agent-cli", version)
            self.assertNotEqual(result.returncode, 0, result.stdout)
            self.assertIn("gh api", result.stderr)
            self.assertEqual(pin_file.read_bytes(), before)


if __name__ == "__main__":
    unittest.main()
