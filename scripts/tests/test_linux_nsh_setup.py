import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "setup-linux-nsh.sh"


def function(name):
    source = SCRIPT.read_text()
    start = source.index(f"{name}() {{")
    return source[start:source.index("\n}", start) + 2]


@unittest.skipUnless(sys.platform.startswith("linux"), "Linux-only Bash setup tests")
class LinuxNshSetupTests(unittest.TestCase):
    def test_runtime_requires_current_build_settings_and_all_artifacts(self):
        artifacts = ["classes.dex", "nsh-build", "nsh-ref"] + [
            f"lib/{abi}/libnsh.so" for abi in ("arm64-v8a", "armeabi-v7a", "x86_64")
        ]
        cases = ("valid", "empty", "stale", "other-ndk", "debug", "other-ref", "link", *artifacts)
        for variant in ("dev", "release"):
            for case in cases:
                with self.subTest(variant=variant, case=case), tempfile.TemporaryDirectory() as tmp:
                    # A runtime that is a link into a checkout's build folder follows whatever it builds next.
                    built = Path(tmp) / ("nsh/target/android/built" if case == "link" else f"tools/runtime/android/{variant}")
                    built.mkdir(parents=True)
                    if case == "link":
                        (Path(tmp) / "tools/runtime/android").mkdir(parents=True)
                        (Path(tmp) / f"tools/runtime/android/{variant}").symlink_to(built)
                    if case != "empty":
                        for artifact in artifacts:
                            if artifact != case:
                                file = built / artifact
                                file.parent.mkdir(parents=True, exist_ok=True)
                                stamp = {"stale": "old\n27\n0\n",
                                         "other-ndk": "current\n28\n0\n",
                                         "debug": "current\n27\n2\n"}.get(case, "current\n27\n0\n")
                                ref = "other\n" if case == "other-ref" else "current\n"
                                file.write_text({"nsh-build": stamp, "nsh-ref": ref}.get(artifact, "built"))
                    result = self.run_shell(function("android_runtime_stamp") + "\n" +
                                            function("nsh_android_runtime_current") + f'''
WORK="$PWD"
TOOLS="$WORK/tools"
REF=current
NDK_VERSION=27
ANDROID_RUNTIME_DEBUG=0
nsh_android_runtime_current {variant}
''', tmp)
                    self.assertEqual(result.returncode == 0, case == "valid", result.stderr)

    def test_runtime_install_writes_pin_and_skips_current_runtime(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for built in ("debug-dev", "release"):
                runtime = root / "nsh/target/android" / built
                for artifact in ("classes.dex", "lib/arm64-v8a/libnsh.so",
                                 "lib/armeabi-v7a/libnsh.so", "lib/x86_64/libnsh.so"):
                    file = runtime / artifact
                    file.parent.mkdir(parents=True, exist_ok=True)
                    file.write_text(built)
            # A runtime left as a link by hand is replaced by a copy.
            (root / "tools/runtime/android").mkdir(parents=True)
            (root / "tools/runtime/android/release").symlink_to(root / "nsh/target/android/release")
            result = self.run_shell(function("android_runtime_stamp") + "\n" +
                                    function("nsh_android_runtime_current") + "\n" +
                                    function("install_android_runtime") + r'''
WORK="$PWD"
TOOLS="$WORK/tools"
NSH="$PWD/nsh"
REF=current
ANDROID_HOME="$PWD/sdk"
NDK_VERSION=27
ANDROID_RUNTIME_DEBUG=0
log() { :; }
die() { echo "$*" >&2; exit 1; }
sdk_mount() { :; }
in_image() {
    [[ " $* " == *" -e ANDROID_NDK_HOME=$ANDROID_HOME/ndk/27 "* ]]
    [[ " $* " == *" -e CARGO_PROFILE_DEV_DEBUG=0 "* ]]
    echo "build ${@: -1}" >> builds
}
install_android_runtime
install_android_runtime
''', tmp)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual((root / "builds").read_text(), "build --dev\nbuild --release\n")
            for variant, built in (("dev", "debug-dev"), ("release", "release")):
                dest = root / "tools/runtime/android" / variant
                self.assertFalse(dest.is_symlink(), variant)
                self.assertEqual((dest / "nsh-build").read_text(), "current\n27\n0\n")
                self.assertEqual((dest / "nsh-ref").read_text(), "current\n")
                self.assertEqual((dest / "classes.dex").read_text(), built)

    def test_tools_folder_of_its_own_takes_the_cli_and_runtimes(self):
        # NSH_TOOLS: a fresh folder for the CLI and both runtimes, outside ~/BuwizNsh.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for built in ("debug-dev", "release"):
                for artifact in ("classes.dex", "lib/arm64-v8a/libnsh.so",
                                 "lib/armeabi-v7a/libnsh.so", "lib/x86_64/libnsh.so"):
                    file = root / "nsh/target/android" / built / artifact
                    file.parent.mkdir(parents=True, exist_ok=True)
                    file.write_text(built)
            result = self.run_shell("\n".join(function(name) for name in (
                "android_runtime_stamp", "nsh_android_runtime_current", "install_android_runtime",
                "install_nsh_cli")) + r'''
WORK="$PWD/work"
TOOLS="$PWD/fresh-tools"
NSH="$PWD/nsh"
REF=current
ANDROID_HOME="$PWD/sdk"
NDK_VERSION=27
ANDROID_RUNTIME_DEBUG=0
log() { :; }
die() { echo "$*" >&2; exit 1; }
sdk_mount() { :; }
nsh_cli_current() { return 1; }
in_image() { echo "$*" >> calls; }
install_nsh_cli
install_android_runtime
''', tmp)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn(f"--root {root}/fresh-tools", (root / "calls").read_text())
            for variant in ("dev", "release"):
                self.assertTrue((root / "fresh-tools/runtime/android" / variant / "nsh-ref").is_file(), variant)
            self.assertFalse((root / "work/tools").exists())

    def test_the_sdk_platform_is_the_one_nsh_json_targets(self):
        for target, ok in (("36", True), ('"thirty-six"', False), (None, False)):
            with self.subTest(target=target), tempfile.TemporaryDirectory() as tmp:
                nsh = Path(tmp) / "repo/nsh"
                nsh.mkdir(parents=True)
                android = {} if target is None else {"targetSdk": "__T__"}
                text = '{"android": ' + json.dumps(android).replace('"__T__"', target or "") + "}"
                (nsh / "nsh.json").write_text(text)
                result = self.run_shell(function("android_api") + r'''
REPO="$PWD/repo"
die() { echo "$*" >&2; exit 1; }
android_api
''', tmp)
                self.assertEqual(result.returncode == 0, ok, result.stderr)
                if ok:
                    self.assertEqual(result.stdout.strip(), "36")

    def test_checkout_accepts_git_directory_or_worktree_file(self):
        for linked in (False, True):
            with self.subTest(linked=linked), tempfile.TemporaryDirectory() as tmp:
                git = Path(tmp) / ".git"
                if linked:
                    git.write_text("gitdir: elsewhere\n")
                else:
                    git.mkdir()
                result = self.run_shell(function("checkout_nsh") + r'''
NSH="$PWD"
REF=current
git() { [ "$*" = "-C $NSH rev-parse HEAD" ]; echo current; }
checkout_nsh
''', tmp)
                self.assertEqual(result.returncode, 0, result.stderr)

    def run_shell(self, source, directory, *args):
        return subprocess.run(
            ["bash", "-euo", "pipefail", "-c", source, "test", *args],
            cwd=directory, text=True, capture_output=True,
            env={**os.environ, "JAVA_HOME": "/old-java-11"},
        )

    def test_wrapper_passes_literal_paths_and_pinned_ndk(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            repo = root / 'repo # & "quote" $(false)'
            sdk = root / 'SDK # & "quote" $HOME'
            (repo / "nsh").mkdir(parents=True)
            sdk.mkdir()
            bin_dir = root / "bin"
            bin_dir.mkdir()
            docker = bin_dir / "docker"
            docker.write_text('#!/bin/bash\nprintf "%s\\n" "$@"\n')
            docker.chmod(0o755)
            result = self.run_shell(
                "\n".join(function(name) for name in
                          ("outside_home", "sdk_mount", "write_wrapper")) + r'''
export HOME="$PWD/home" PATH="$PWD/bin:$PATH"
REPO="$1"
ANDROID_HOME="$2"
NDK_VERSION=27
WORK="$PWD"
TOOLS="$WORK/tools"
MOUNTS=(-v "$HOME:$HOME")
write_wrapper
bash -n "$WORK/nsh2404.sh"
bash "$WORK/nsh2404.sh" nsh run --target android
''',
                tmp, str(repo), str(sdk),
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            args = result.stdout.splitlines()
            self.assertIn(str(repo / "nsh"), args)
            self.assertIn(f"ANDROID_HOME={sdk}", args)
            self.assertIn(f"{sdk}:{sdk}", args)
            self.assertIn(f"ANDROID_NDK_HOME={sdk}/ndk/27", args)
            self.assertIn("BUWIZ_ANDROID_DEV_ENV=EBIR_TEST_ENV=1;"
                          "BUWIZ_NO_KEYCHAIN=1;BIR_SFTP_DRY_RUN=1", args)
            # The release profile `just play-bundle` sets reaches the container (without it Cargo silently builds
            # its default release: a 123 MB library instead of 88 MB).
            for name in ("CARGO_PROFILE_RELEASE_LTO", "CARGO_PROFILE_RELEASE_CODEGEN_UNITS",
                         "CARGO_PROFILE_RELEASE_OPT_LEVEL"):
                self.assertEqual(args[args.index(name) - 1], "-e", name)
            # The tools folder setup used is the one on the container's PATH.
            self.assertTrue(any(a.startswith(f"PATH={root}/tools/bin:") for a in args), args)

    def test_wrapper_bakes_only_safe_test_settings_and_local_addresses(self):
        # Review of #452 (W1, W2b): a caller's address or Android test settings reach the build only when they keep it
        # a dry-run test build that talks to this machine alone.
        default = "BUWIZ_ANDROID_DEV_ENV=EBIR_TEST_ENV=1;BUWIZ_NO_KEYCHAIN=1;BIR_SFTP_DRY_RUN=1"
        local = "EBIR_TEST_ENV=1;BIR_SFTP_DRY_RUN=1;BUWIZ_SERVER_URL=http://localhost:3008;GPUI_AGENT=1"
        cases = (
            # (dev env, server url, the baked dev env, is the server url passed)
            (local, "http://127.0.0.1:3008", "BUWIZ_ANDROID_DEV_ENV=" + local, True),
            (local, "https://api.buwiz.com", "BUWIZ_ANDROID_DEV_ENV=" + local, False),
            (local, "http://localhost:80@evil.example", "BUWIZ_ANDROID_DEV_ENV=" + local, False),
            ("EBIR_TEST_ENV=1;BIR_SFTP_DRY_RUN=1;BUWIZ_SERVER_URL=https://api.buwiz.com", "", default, False),
            ("EBIR_TEST_ENV=1;BIR_SFTP_DRY_RUN=1;BIR_SFTP_DRY_RUN=0", "", default, False),
            ("EBIR_TEST_ENV=1;BIR_SFTP_DRY_RUN=1;BIR_SFTP_LIVE=1", "", default, False),
            ("EBIR_TEST_ENV=1;BIR_SFTP_DRY_RUN=0", "", default, False),
            ("BIR_SFTP_DRY_RUN=1", "", default, False),
            ("EBIR_TEST_ENV=1;BIR_SFTP_DRY_RUN=1;BUWIZ_RELAY_SEND=1", "", default, False),
            ("EBIR_TEST_ENV=1;BIR_SFTP_DRY_RUN=1;BIR_DATABASE_PATH=x\nBUWIZ_SERVER_URL=https://api.buwiz.com", "",
             default, False),
        )
        for dev_env, url, baked, passed in cases:
            with self.subTest(dev_env=dev_env, url=url), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                (root / "repo" / "nsh").mkdir(parents=True)
                (root / "sdk").mkdir()
                (root / "bin").mkdir()
                docker = root / "bin" / "docker"
                docker.write_text('#!/bin/bash\nprintf "%s\\n" "$@"\n')
                docker.chmod(0o755)
                result = self.run_shell(
                    "\n".join(function(name) for name in
                              ("outside_home", "sdk_mount", "write_wrapper")) + r'''
export HOME="$PWD/home" PATH="$PWD/bin:$PATH"
REPO="$PWD/repo"
ANDROID_HOME="$PWD/sdk"
NDK_VERSION=27
WORK="$PWD"
TOOLS="$WORK/tools"
MOUNTS=(-v "$HOME:$HOME")
write_wrapper
export BUWIZ_ANDROID_DEV_ENV="$(printf '%b' "$1")"
if [ -n "$2" ]; then export BUWIZ_SERVER_URL="$2"; else unset BUWIZ_SERVER_URL; fi
unset BUWIZ_ACCOUNT_URL
bash "$WORK/nsh2404.sh" nsh run --target android
''',
                    tmp, dev_env, url,
                )
                self.assertEqual(result.returncode, 0, result.stderr)
                args = result.stdout.splitlines()
                self.assertIn(baked, args)
                self.assertEqual("BUWIZ_SERVER_URL" in args, passed, args)
                self.assertNotIn("BUWIZ_ACCOUNT_URL", args)
                if baked == default:
                    self.assertIn("BUWIZ_ANDROID_DEV_ENV refused", result.stderr)
                if url and not passed:
                    self.assertIn("BUWIZ_SERVER_URL is not a local address", result.stderr)

    def test_a_host_git_proxy_is_left_outside_the_container(self):
        # buwiz-forms-linux-6: the Devin git proxy's rewrite in ~/.gitconfig failed Cargo's first fetch in the image.
        settings = ("CARGO_NET_GIT_FETCH_WITH_CLI=true", "GIT_CONFIG_GLOBAL=/dev/null")
        for case, config in (("plain", "[user]\n\tname = someone\n"),
                             ("rewrite", '[url "https://proxy.example/github.com/"]\n\tinsteadOf = https://github.com/\n'),
                             ("helper", "[credential]\n\thelper = store\n")):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                (root / "repo/nsh").mkdir(parents=True)
                (root / "home").mkdir()
                (root / "home/.gitconfig").write_text(config)
                (root / "bin").mkdir()
                docker = root / "bin/docker"
                docker.write_text('#!/bin/bash\nprintf "%s\\n" "$@"\n')
                docker.chmod(0o755)
                result = self.run_shell(
                    "\n".join(function(name) for name in
                              ("outside_home", "sdk_mount", "write_wrapper", "in_image")) + r'''
unset GIT_CONFIG_GLOBAL
export HOME="$PWD/home" XDG_CONFIG_HOME="$PWD/xdg" PATH="$PWD/bin:$PATH"
REPO="$PWD/repo" ANDROID_HOME="$PWD/sdk" NDK_VERSION=27 WORK="$PWD" TOOLS="$PWD/tools" IMAGE=buwiz-nsh-2404
MOUNTS=(-v "$HOME:$HOME")
write_wrapper
bash "$WORK/nsh2404.sh" nsh check
echo ---
in_image "$PWD" -- cargo install
''', tmp)
                self.assertEqual(result.returncode, 0, result.stderr)
                wrapper, setup = (part.splitlines() for part in result.stdout.split("---\n"))
                for args in (wrapper, setup):
                    for setting in settings:
                        self.assertEqual(setting in args, case != "plain", (case, setting))

    def test_sdk_selects_java_home_then_path_then_install(self):
        for case in ("home", "path", "old", "missing"):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                for home, major in (("custom-jdk", 21), ("path-jdk", 17 if case == "path" else 11)):
                    java = root / home / "bin/java"
                    java.parent.mkdir(parents=True)
                    java.write_text(f'#!/bin/bash\necho \'openjdk version "{major}.0.1"\' >&2\n'
                                    f'echo \'    java.home = {root / home}\' >&2\n')
                    java.chmod(0o755)
                tools = root / "sdk/cmdline-tools/latest/bin"
                tools.mkdir(parents=True)
                manager = tools / "sdkmanager"
                manager.write_text('#!/bin/bash\necho "$JAVA_HOME" >> java-used\n')
                manager.chmod(0o755)
                emulator = root / "sdk/emulator/emulator"
                emulator.parent.mkdir()
                emulator.write_text('#!/bin/bash\necho buwiz\n')
                emulator.chmod(0o755)
                result = self.run_shell(function("find_java_home") + "\n" +
                                        function("install_android_sdk") + f'\ncase={case}\n' + r'''
JAVA_HOME="$PWD/not-installed"
[ "$case" != home ] || JAVA_HOME="$PWD/custom-jdk"
command() {
    if [ "$*" = '-v java' ]; then
        [ "$case" != missing ] || return 1
        echo "$PWD/path-jdk/bin/java"
    else builtin command "$@"; fi
}
ANDROID_HOME="$PWD/sdk"
NDK_VERSION=27
SYSTEM_IMAGE=image
AVD=buwiz
SUDO=""
log() { :; }
unzip() { :; }
apt-get() {
    echo "$*" > apt-called
    mkdir -p "$PWD/jdk17/bin"; touch "$PWD/jdk17/bin/java"; chmod +x "$PWD/jdk17/bin/java"
}
dpkg-query() { echo "$PWD/jdk17/bin/javac"; }
rustup() { :; }
install_android_sdk
''', tmp)
                self.assertEqual(result.returncode, 0, result.stderr)
                selected = {"home": "custom-jdk", "path": "path-jdk"}.get(case, "jdk17")
                self.assertEqual((root / "java-used").read_text().splitlines(),
                                 [str(root / selected)] * 2)
                self.assertEqual((root / "apt-called").exists(), case in ("old", "missing"))


if __name__ == "__main__":
    unittest.main()
