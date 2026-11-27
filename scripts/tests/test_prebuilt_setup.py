import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
PREBUILT_SH = ROOT / "scripts/lib/prebuilt.sh"
PREBUILT_PS1 = ROOT / "scripts/lib/Prebuilt.ps1"


def read_pin(key):
    for line in (ROOT / "scripts/impeccable.pin").read_text().splitlines():
        if line.startswith(key + "="):
            return line.split("=", 1)[1]
    raise AssertionError(f"{key} missing from scripts/impeccable.pin")


PIN = read_pin("CARGO_BINSTALL_VERSION")


def write_executable(path, text):
    path.write_text(text)
    path.chmod(0o755)


def run_helper(script, *, include_binstall=True, binstall_exit=0, curl_exit=0,
               no_prebuilt=False, install_list="", binary=False,
               system="Linux", machine="x86_64"):
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        fakebin = root / "fakebin"
        fakebin.mkdir()
        cargo_home = root / "cargo-home"
        (cargo_home / "bin").mkdir(parents=True)
        log = root / "log"

        write_executable(fakebin / "cargo", r'''#!/bin/sh
printf 'cargo' >> "$LOG"
for arg in "$@"; do printf ' [%s]' "$arg" >> "$LOG"; done
printf '\n' >> "$LOG"
if [ "$1" = "install" ] && [ "$2" = "--list" ]; then
    [ ! -f "$FAKE_INSTALL_LIST_FILE" ] || cat "$FAKE_INSTALL_LIST_FILE"
    exit 0
fi
if [ "$1" = "binstall" ]; then
    exit "${FAKE_BINSTALL_EXIT:-0}"
fi
exit 0
''')
        if include_binstall:
            write_executable(fakebin / "cargo-binstall", r'''#!/bin/sh
printf 'cargo-binstall' >> "$LOG"
for arg in "$@"; do printf ' [%s]' "$arg" >> "$LOG"; done
printf '\n' >> "$LOG"
if [ "${1:-}" = "-V" ]; then
    printf '%s\n' "${FAKE_BINSTALL_VERSION:-1.24.0}"
fi
exit 0
''')
        write_executable(fakebin / "curl", r'''#!/bin/sh
printf 'curl' >> "$LOG"
for arg in "$@"; do printf ' [%s]' "$arg" >> "$LOG"; done
printf '\n' >> "$LOG"
out=""
while [ "$#" -gt 0 ]; do
    if [ "$1" = "-o" ]; then
        shift
        out="${1:-}"
    fi
    shift
done
if [ "${FAKE_CURL_EXIT:-0}" != "0" ]; then
    exit "${FAKE_CURL_EXIT:-0}"
fi
if [ -n "$out" ]; then
    printf 'archive' > "$out"
else
    printf 'archive'
fi
''')
        write_executable(fakebin / "tar", r'''#!/bin/sh
printf 'tar' >> "$LOG"
for arg in "$@"; do printf ' [%s]' "$arg" >> "$LOG"; done
printf '\n' >> "$LOG"
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
cat > "$dest/cargo-binstall" <<'EOF'
#!/bin/sh
printf 'cargo-binstall' >> "$LOG"
for arg in "$@"; do printf ' [%s]' "$arg" >> "$LOG"; done
printf '\n' >> "$LOG"
if [ "${1:-}" = "-V" ]; then
    printf '%s\n' "${FAKE_BINSTALL_VERSION:-1.24.0}"
fi
exit 0
EOF
chmod +x "$dest/cargo-binstall"
''')
        write_executable(fakebin / "unzip", r'''#!/bin/sh
printf 'unzip' >> "$LOG"
for arg in "$@"; do printf ' [%s]' "$arg" >> "$LOG"; done
printf '\n' >> "$LOG"
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
cat > "$dest/cargo-binstall" <<'EOF'
#!/bin/sh
printf 'cargo-binstall' >> "$LOG"
for arg in "$@"; do printf ' [%s]' "$arg" >> "$LOG"; done
printf '\n' >> "$LOG"
if [ "${1:-}" = "-V" ]; then
    printf '%s\n' "${FAKE_BINSTALL_VERSION:-1.24.0}"
fi
exit 0
EOF
chmod +x "$dest/cargo-binstall"
''')
        write_executable(fakebin / "uname", r'''#!/bin/sh
case "${1:-}" in
    -s) printf '%s\n' "${FAKE_UNAME_S:-Linux}" ;;
    -m) printf '%s\n' "${FAKE_UNAME_M:-x86_64}" ;;
    *) printf '%s\n' "${FAKE_UNAME_S:-Linux}" ;;
esac
''')
        if binary:
            write_executable(cargo_home / "bin/gpui-agent", "#!/bin/sh\n")
        install_list_file = root / "install-list"
        install_list_file.write_text(install_list)

        env = {
            "PATH": os.pathsep.join([str(fakebin), "/usr/bin", "/bin"]),
            "HOME": str(root / "home"),
            "LOG": str(log),
            "CARGO_HOME": str(cargo_home),
            "FAKE_BINSTALL_EXIT": str(binstall_exit),
            "FAKE_CURL_EXIT": str(curl_exit),
            "FAKE_BINSTALL_VERSION": PIN,
            "FAKE_INSTALL_LIST_FILE": str(install_list_file),
            "FAKE_UNAME_S": system,
            "FAKE_UNAME_M": machine,
        }
        if no_prebuilt:
            env["BUWIZ_SETUP_NO_PREBUILT"] = "1"
        result = subprocess.run(
            ["bash", "-c", f'. "{PREBUILT_SH}"\n' + script],
            env=env, text=True, capture_output=True,
        )
        return {
            "result": result,
            "log": log.read_text() if log.exists() else "",
            "output": result.stdout + result.stderr,
            "bin": sorted(path.name for path in (cargo_home / "bin").iterdir()),
        }


class PrebuiltShellTests(unittest.TestCase):
    def test_binstall_success_skips_cargo_install_and_uses_the_exact_arguments(self):
        data = run_helper("install_prebuilt_or_build gpui-agent-cli 0.4.0\n")
        self.assertEqual(data["result"].returncode, 0, data["output"])
        self.assertEqual(
            [line for line in data["log"].splitlines() if line.startswith("cargo ")],
            ["cargo [binstall] [--no-confirm] [--strategies] [crate-meta-data] [--version] [0.4.0] [gpui-agent-cli]"],
        )

    def test_binstall_failure_falls_back_once_to_cargo_install(self):
        data = run_helper(
            "install_prebuilt_or_build impeccable 0.2.1 --locked\n",
            binstall_exit=1,
        )
        self.assertEqual(data["result"].returncode, 0, data["output"])
        self.assertEqual(
            [line for line in data["log"].splitlines() if line.startswith("cargo ")],
            [
                "cargo [binstall] [--no-confirm] [--strategies] [crate-meta-data] [--version] [0.2.1] [impeccable]",
                "cargo [install] [impeccable] [--version] [0.2.1] [--locked]",
            ],
        )
        self.assertEqual(data["output"].count("falling back to cargo install"), 1, data["output"])

    def test_missing_binstall_downloads_the_linux_musl_archive_then_uses_it(self):
        data = run_helper(
            "install_prebuilt_or_build gpui-agent-cli 0.4.0\n",
            include_binstall=False,
        )
        self.assertEqual(data["result"].returncode, 0, data["output"])
        self.assertIn(
            f"https://github.com/cargo-bins/cargo-binstall/releases/download/v{PIN}/cargo-binstall-x86_64-unknown-linux-musl.tgz",
            data["log"],
        )
        self.assertIn("tar ", data["log"])
        self.assertIn("cargo-binstall", data["bin"])
        self.assertEqual(
            [line for line in data["log"].splitlines() if line.startswith("cargo ")],
            ["cargo [binstall] [--no-confirm] [--strategies] [crate-meta-data] [--version] [0.4.0] [gpui-agent-cli]"],
        )

    def test_missing_binstall_downloads_the_macos_zip_archive_then_uses_it(self):
        data = run_helper(
            "install_prebuilt_or_build gpui-agent-cli 0.4.0\n",
            include_binstall=False,
            system="Darwin",
            machine="arm64",
        )
        self.assertEqual(data["result"].returncode, 0, data["output"])
        self.assertIn(
            f"https://github.com/cargo-bins/cargo-binstall/releases/download/v{PIN}/cargo-binstall-aarch64-apple-darwin.zip",
            data["log"],
        )
        self.assertIn("unzip ", data["log"])
        self.assertIn("cargo-binstall", data["bin"])
        self.assertEqual(
            [line for line in data["log"].splitlines() if line.startswith("cargo ")],
            ["cargo [binstall] [--no-confirm] [--strategies] [crate-meta-data] [--version] [0.4.0] [gpui-agent-cli]"],
        )

    def test_missing_binstall_and_failed_download_fall_back_once(self):
        data = run_helper(
            "install_prebuilt_or_build gpui-agent-cli 0.4.0\n",
            include_binstall=False,
            curl_exit=22,
        )
        self.assertEqual(data["result"].returncode, 0, data["output"])
        self.assertIn("cargo-binstall-x86_64-unknown-linux-musl.tgz", data["log"])
        self.assertNotIn("tar ", data["log"])
        self.assertEqual(
            [line for line in data["log"].splitlines() if line.startswith("cargo ")],
            ["cargo [install] [gpui-agent-cli] [--version] [0.4.0]"],
        )
        self.assertEqual(data["output"].count("falling back to cargo install"), 1, data["output"])

    def test_no_prebuilt_goes_straight_to_cargo_install(self):
        data = run_helper(
            "install_prebuilt_or_build gpui-agent-cli 0.4.0\n",
            no_prebuilt=True,
        )
        self.assertEqual(data["result"].returncode, 0, data["output"])
        self.assertEqual(
            [line for line in data["log"].splitlines() if line.startswith("cargo ")],
            ["cargo [install] [gpui-agent-cli] [--version] [0.4.0]"],
        )
        self.assertNotIn("curl ", data["log"])
        self.assertIn("prebuilt installs disabled", data["output"])

    def test_install_list_wins_and_the_stamp_covers_a_binstall_install(self):
        listed = run_helper(
            "install_prebuilt_or_build gpui-agent-cli 0.3.0 >/dev/null\n"
            "printf '%s\\n' 'gpui-agent-cli v0.4.0:' > \"$FAKE_INSTALL_LIST_FILE\"\n"
            "prebuilt_installed_version gpui-agent-cli gpui-agent\n",
            binary=True,
        )
        self.assertEqual(listed["result"].stdout.strip(), "0.4.0")
        stamped = run_helper(
            "install_prebuilt_or_build gpui-agent-cli 0.4.0 >/dev/null\n"
            ": > \"$FAKE_INSTALL_LIST_FILE\"\n"
            "prebuilt_installed_version gpui-agent-cli gpui-agent\n",
            binary=True,
        )
        self.assertEqual(stamped["result"].stdout.strip(), "0.4.0")


class PrebuiltSetupTextTests(unittest.TestCase):
    def test_shell_setup_scripts_use_the_shared_helper(self):
        linux = (ROOT / "scripts/setup-linux.sh").read_text()
        macos = (ROOT / "scripts/setup-macos.sh").read_text()
        impeccable = (ROOT / "scripts/setup-impeccable.sh").read_text()
        self.assertIn('install_prebuilt_or_build just ', linux)
        for script in (linux, macos):
            self.assertIn('install_prebuilt_or_build gpui-agent-cli "$version"', script)
            for crate in ("cargo-audit", "cargo-outdated", "cargo-machete"):
                self.assertIn(f'install_prebuilt_or_build {crate} ', script)
            self.assertNotIn("cargo install gpui-agent-cli", script)
            self.assertNotIn("cargo install cargo-audit cargo-outdated cargo-machete", script)
        self.assertIn('install_prebuilt_or_build impeccable "$version" --locked', impeccable)

    def test_powershell_helper_and_call_sites_are_wired(self):
        helper = PREBUILT_PS1.read_text()
        setup = (ROOT / "scripts/setup-windows.ps1").read_text()
        e2e = (ROOT / "scripts/e2e/machines/windows.ps1").read_text()
        for text in (
            "function Ensure-CargoBinstall",
            "function Install-PrebuiltOrBuild",
            "--strategies crate-meta-data",
            "BUWIZ_SETUP_NO_PREBUILT",
            "pc-windows-msvc.zip",
            "Expand-Archive",
        ):
            self.assertIn(text, helper)
        self.assertIn(". (Join-Path $PSScriptRoot 'lib\\Prebuilt.ps1')", setup)
        self.assertRegex(setup, r"Install-PrebuiltOrBuild\s+'gpui-agent-cli'\s+\$gpuiVersion")
        self.assertRegex(setup, r"Install-PrebuiltOrBuild\s+'impeccable'\s+\$impeccableVersion\s+'--locked'")
        self.assertNotIn("install gpui-agent-cli --version", setup)
        self.assertIn("Get-PrebuiltInstalledVersion", e2e)

    def test_blueprint_blocks_call_the_shared_helper_in_both_copies(self):
        for relative in (".devin/blueprint.yaml", "docs/platforms/devin-blueprint.yml"):
            blueprint = (ROOT / relative).read_text()
            self.assertIn(". scripts/lib/prebuilt.sh", blueprint)
            self.assertIn("ensure_cargo_binstall", blueprint)
            self.assertNotIn("cargo-binstall-$(uname -m)-unknown-linux-musl.tgz", blueprint)

    def test_e2e_shell_version_detection_uses_the_shared_install_list_and_stamp_reader(self):
        e2e = (ROOT / "scripts/e2e/machines/common.sh").read_text()
        self.assertIn("prebuilt_installed_version gpui-agent-cli gpui-agent", e2e)

    def test_docs_describe_prebuilt_tools_and_the_source_build_escape(self):
        docs = (ROOT / "docs/platforms/devin-environment.md").read_text()
        self.assertIn("## Prebuilt tools", docs)
        self.assertIn("BUWIZ_SETUP_NO_PREBUILT=1", docs)
        self.assertIn("crate-meta-data", docs)
        self.assertIn("fall", docs)


if __name__ == "__main__":
    unittest.main()
