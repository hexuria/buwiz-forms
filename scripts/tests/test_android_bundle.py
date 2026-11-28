"""scripts/android_bundle.py against bundles made from real aapt2 manifests.

The fixtures are the protobuf manifests `aapt2 link --proto-format` (build-tools 36.0.0) wrote, the form a bundle carries:
- android_bundle_manifest.pb: the Play shape (com.buwiz.ebirforms, versionCode 27, target 36, backup off, as
  nsh.json `android.allowBackup: false` makes NSH write it).
- android_bundle_manifest_dev.pb: a development shape (dev.goldcoders.bir, versionCode 26, target 35, backup as Android
  has it, debuggable).
"""

import io
import sys
import tempfile
import unittest
import zipfile
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import android_bundle  # noqa: E402

FIXTURES = Path(__file__).resolve().parent / "fixtures"
PLAY = (FIXTURES / "android_bundle_manifest.pb").read_bytes()
DEV = (FIXTURES / "android_bundle_manifest_dev.pb").read_bytes()
PLAY_ARGS = {"package": "com.buwiz.ebirforms", "version_code": 27, "target_sdk": 36}


# A library as a release build's reads: ELF, the production addresses, nothing a debug build carries.
RELEASE_LIBRARY = b"\x7fELF\0https://api.buwiz.com\0https://account.buwiz.com\0"


def write_bundle(path: Path, manifest: bytes, abis=("arm64-v8a",), updater=True, signed=True, library=None) -> None:
    with zipfile.ZipFile(path, "w") as zf:
        zf.writestr("BundleConfig.pb", b"\x0a\x08\x0a\x061.18.1", compress_type=zipfile.ZIP_DEFLATED)
        zf.writestr(android_bundle.MANIFEST, manifest, compress_type=zipfile.ZIP_DEFLATED)
        zf.writestr("base/dex/classes.dex", b"dex\n035", compress_type=zipfile.ZIP_DEFLATED)
        zf.writestr("base/assets/nsh/bundle.nshc", b"bytecode", compress_type=zipfile.ZIP_DEFLATED)
        for abi in abis:
            # Native libraries are stored, as NSH writes them (extractNativeLibs="false").
            zf.writestr(f"base/lib/{abi}/libnsh.so", library or RELEASE_LIBRARY + abi.encode(),
                        compress_type=zipfile.ZIP_STORED)
        if updater:
            zf.writestr(android_bundle.UPDATER_ASSET, b'{"github":"hexuria/buwiz-forms"}')
        if signed:
            zf.writestr("META-INF/MANIFEST.MF", b"Manifest-Version: 1.0\n")
            zf.writestr("META-INF/ANDROIDD.SF", b"Signature-Version: 1.0\n")
            zf.writestr("META-INF/ANDROIDD.RSA", b"\x30\x82")


class ManifestTests(unittest.TestCase):
    def test_reads_aapt2_protobuf_manifest(self):
        manifest = android_bundle.read_manifest(PLAY)
        self.assertEqual(manifest.get("package", ""), "com.buwiz.ebirforms")
        self.assertEqual(manifest.get("versionCode"), "27")
        self.assertEqual(manifest.find("uses-sdk")[0].get("targetSdkVersion"), "36")
        self.assertEqual(manifest.find("uses-sdk")[0].get("minSdkVersion"), "26")
        app = manifest.find("application")[0]
        self.assertEqual(app.get("allowBackup"), "false")
        self.assertEqual(app.get("fullBackupContent"), "false")
        self.assertEqual(app.get("dataExtractionRules"), "@xml/nsh_no_backup")
        self.assertEqual([c.name for c in app.children], ["activity"])

    def test_a_file_that_is_no_manifest_is_refused(self):
        with self.assertRaises(android_bundle.BundleError):
            android_bundle.read_manifest(b"")
        with self.assertRaises(android_bundle.BundleError):
            android_bundle.read_manifest(b"\x0a\x80")  # a length past the end


class BundleTests(unittest.TestCase):
    def setUp(self):
        self.dir = Path(tempfile.mkdtemp())
        self.built = self.dir / "built.aab"
        self.ready = self.dir / "ready.aab"

    def test_prepared_play_bundle_passes(self):
        write_bundle(self.built, PLAY)
        android_bundle.prepare(self.built, self.ready)
        self.assertEqual(android_bundle.check(self.ready, **PLAY_ARGS), [])

    def test_a_permission_the_listing_does_not_declare_is_a_problem(self):
        write_bundle(self.built, PLAY)
        android_bundle.prepare(self.built, self.ready)
        real = android_bundle.read_manifest

        def with_camera(data):
            manifest = real(data)
            manifest.children.append(android_bundle.Element(
                "uses-permission", {(android_bundle.ANDROID_NS, "name"): "android.permission.CAMERA"}, []))
            return manifest

        android_bundle.read_manifest = with_camera
        try:
            problems = android_bundle.check(self.ready, **PLAY_ARGS)
        finally:
            android_bundle.read_manifest = real
        self.assertEqual(len(problems), 1, problems)
        self.assertIn("android.permission.CAMERA", problems[0])

    def test_the_library_must_be_a_release_build(self):
        # The positive control: the same detector that passes a release library finds each development marker.
        self.assertEqual(android_bundle.library_problems("arm64-v8a", 10, RELEASE_LIBRARY), [])
        for marker in android_bundle.DEBUG_MARKERS:
            with self.subTest(marker=marker):
                problems = android_bundle.library_problems("arm64-v8a", 10, RELEASE_LIBRARY + b"\0" + marker)
                self.assertEqual(len(problems), 1, problems)
                self.assertIn("debug or development build", problems[0])
        for marker in android_bundle.RELEASE_MARKERS:
            with self.subTest(missing=marker):
                problems = android_bundle.library_problems("arm64-v8a", 10, RELEASE_LIBRARY.replace(marker, b""))
                self.assertEqual(len(problems), 1, problems)
                self.assertIn("not a release build's addresses", problems[0])

    def test_the_sentinel_is_dev_envs_and_named_nowhere_else(self):
        root = Path(__file__).resolve().parents[2]
        dev_env = (root / "nsh/src/dev_env.rs").read_text(encoding="utf-8")
        self.assertIn(f'const SENTINEL_TEXT: &str = "{android_bundle.DEV_LIST_SENTINEL.decode()}";', dev_env)
        named = [path for folder in ("crates", "nsh/src") for path in (root / folder).rglob("*.rs")
                 if android_bundle.DEV_LIST_SENTINEL in path.read_bytes()]
        self.assertEqual(named, [root / "nsh/src/dev_env.rs"])

    def test_text_a_release_library_keeps_is_no_marker(self):
        # A release library keeps this error message (nsh/src/door.rs use_database, bir-app-api agent/headless.rs):
        # the test list's own text cannot be the marker for a baked test list.
        release = RELEASE_LIBRARY + b"useDatabase is for test environments (EBIR_TEST_ENV=1).\0"
        self.assertEqual(android_bundle.library_problems("arm64-v8a", 10, release), [])
        self.assertNotIn(b"attempt to add with overflow", android_bundle.DEBUG_MARKERS)

    def test_a_google_client_is_compiled_in_exactly_when_gmail_is_on(self):
        with_client = RELEASE_LIBRARY + b"1234-abc.apps.googleusercontent.com\0"
        # Off (the store default): a client id in the library is a problem; none is right.
        self.assertEqual(android_bundle.library_problems("arm64-v8a", 10, RELEASE_LIBRARY), [])
        off = android_bundle.library_problems("arm64-v8a", 10, with_client)
        self.assertEqual(len(off), 1, off)
        self.assertIn("STORE_GMAIL=1", off[0])
        # On: the same detector finds it (the positive control), and its absence is the problem.
        self.assertEqual(android_bundle.library_problems("arm64-v8a", 10, with_client, gmail=True), [])
        on = android_bundle.library_problems("arm64-v8a", 10, RELEASE_LIBRARY, gmail=True)
        self.assertEqual(len(on), 1, on)
        self.assertIn("no Google client id", on[0])

    def test_a_library_over_its_ceiling_is_a_problem(self):
        # Cargo's default release profile: 122,876,080 bytes on arm64 (job 59), over the ceiling.
        problems = android_bundle.library_problems("arm64-v8a", 122_876_080, RELEASE_LIBRARY)
        self.assertEqual(len(problems), 1, problems)
        self.assertIn("did not reach the build", problems[0])
        self.assertEqual(android_bundle.library_problems("arm64-v8a", 88_396_528, RELEASE_LIBRARY), [])
        self.assertEqual(android_bundle.library_problems("armeabi-v7a", 76_143_608, RELEASE_LIBRARY), [])
        self.assertEqual(len(android_bundle.library_problems("armeabi-v7a", 99_077_356, RELEASE_LIBRARY)), 1)

    def test_a_bundle_whose_library_is_a_debug_build_fails_the_check(self):
        write_bundle(self.built, PLAY, library=RELEASE_LIBRARY + b"http://localhost:3008")
        android_bundle.prepare(self.built, self.ready)
        problems = android_bundle.check(self.ready, **PLAY_ARGS)
        self.assertEqual(len(problems), 1, problems)
        self.assertIn("localhost:3008", problems[0])

    def test_prepare_drops_only_the_signature_and_the_updater(self):
        write_bundle(self.built, PLAY, abis=("arm64-v8a", "armeabi-v7a"))
        self.assertEqual(android_bundle.prepare(self.built, self.ready), 4)
        with zipfile.ZipFile(self.built) as before, zipfile.ZipFile(self.ready) as after:
            kept = [i for i in before.infolist() if not i.filename.startswith("META-INF/")
                    and i.filename != android_bundle.UPDATER_ASSET]
            self.assertEqual([i.filename for i in after.infolist()], [i.filename for i in kept])
            for info in kept:
                self.assertEqual(after.read(info.filename), before.read(info.filename), info.filename)
                self.assertEqual(after.getinfo(info.filename).compress_type, info.compress_type, info.filename)

    def test_prepare_refuses_to_overwrite_its_input(self):
        write_bundle(self.built, PLAY)
        with self.assertRaises(android_bundle.BundleError):
            android_bundle.prepare(self.built, self.built)

    def test_an_unprepared_bundle_still_carries_the_updater(self):
        write_bundle(self.built, PLAY)
        problems = android_bundle.check(self.built, **PLAY_ARGS)
        self.assertEqual(len(problems), 1, problems)
        self.assertIn("nsh-updater.json", problems[0])

    def test_development_shape_is_refused_on_every_count(self):
        write_bundle(self.built, DEV, abis=("arm64-v8a", "x86_64"), updater=False)
        problems = "\n".join(android_bundle.check(self.built, **PLAY_ARGS))
        for expected in ("package is 'dev.goldcoders.bir'", "versionCode is '26'", "targetSdkVersion is '35'",
                         "allowBackup is None", "no dataExtractionRules", "debuggable", "x86_64"):
            self.assertIn(expected, problems)

    def test_wrong_expectations_are_named(self):
        write_bundle(self.built, PLAY, updater=False)
        problems = android_bundle.check(self.built, package="dev.goldcoders.bir", version_code=28, target_sdk=35)
        self.assertEqual(len(problems), 3, problems)

    def test_a_bundle_without_native_code_is_refused(self):
        write_bundle(self.built, PLAY, abis=(), updater=False)
        self.assertIn("no native code under base/lib/", android_bundle.check(self.built, **PLAY_ARGS))

    def test_an_apk_is_not_a_bundle(self):
        with zipfile.ZipFile(self.built, "w") as zf:
            zf.writestr("AndroidManifest.xml", PLAY)
        self.assertIn("not an Android App Bundle", android_bundle.check(self.built, **PLAY_ARGS)[0])

    def test_command_line_exit_codes(self):
        write_bundle(self.built, PLAY)
        out, err = io.StringIO(), io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            self.assertEqual(android_bundle.main(["prepare", str(self.built), str(self.ready)]), 0)
            self.assertEqual(android_bundle.main(["check", str(self.ready), "--package", "com.buwiz.ebirforms",
                                                  "--version-code", "27", "--target-sdk", "36"]), 0)
            self.assertEqual(android_bundle.main(["check", str(self.built), "--package", "com.buwiz.ebirforms",
                                                  "--version-code", "27", "--target-sdk", "36"]), 1)
            self.assertEqual(android_bundle.main(["check", str(self.dir / "missing.aab"), "--package", "x",
                                                  "--version-code", "1", "--target-sdk", "36"]), 1)
        self.assertIn("backup off", out.getvalue())


if __name__ == "__main__":
    unittest.main()
