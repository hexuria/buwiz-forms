//! No test anywhere in the repository may reach the person's real keychain
//! (service `com.buwiz.bir` and the others), real data folder or real logs.
//!
//! bir-core's test-build rule (`platform::keychain_disabled`,
//! `platform::data_dir`, `log_files::log_dir`) holds only where the tests
//! are built with bir-core's `test-support` feature or `cfg(test)`. This file
//! fails when that stops being true:
//! - a crate whose tests run bir-core (directly, or through another crate
//!   such as bir-app-api) does not give them `test-support`;
//! - a new keychain call site appears, in any crate's `src/`, `tests/`,
//!   `examples/`, `benches/` or `build.rs`, that this list has not reviewed;
//! - this test build itself does not have the keychain off, or makes the
//!   system's data folder it does not use.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use toml_edit::{DocumentMut, Item};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root")
}

/// Every Cargo.toml in the repository that can use bir-core.
fn manifests(root: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = fs::read_dir(root.join("crates"))
        .expect("crates/")
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("Cargo.toml"))
        .filter(|path| path.is_file())
        .collect();
    found.push(root.join("nsh/Cargo.toml"));
    found.sort();
    found
}

/// What the rule needs from one Cargo.toml, read with a real TOML parser.
struct Manifest {
    name: String,
    /// The packages of `[dependencies]`, and of every `[target.….dependencies]`.
    normal: BTreeSet<String>,
    /// The packages of `[dev-dependencies]`, and of every `[target.….dev-dependencies]`.
    dev: BTreeSet<String>,
    /// An unconditional `[dev-dependencies]` entry for bir-core turns on `test-support`.
    test_support: bool,
}

/// Reads one manifest. A dependency's package is its `package = "…"` when renamed, else its key; it is found in an
/// inline table (`bir-core = { … }`), a table (`[dependencies.bir-core]`) or a plain version, at the top or under any
/// `[target.'cfg(…)']`.
fn read_manifest(text: &str) -> Manifest {
    let doc: DocumentMut = text.parse().expect("Cargo.toml is valid TOML");
    let mut manifest = Manifest {
        name: doc
            .get("package")
            .and_then(|package| package.get("name"))
            .and_then(Item::as_str)
            .unwrap_or_default()
            .to_owned(),
        normal: BTreeSet::new(),
        dev: BTreeSet::new(),
        test_support: false,
    };
    let targets = doc
        .get("target")
        .and_then(Item::as_table_like)
        .into_iter()
        .flat_map(|targets| targets.iter().map(|(_, target)| (false, target)));
    for (top, section) in std::iter::once((true, doc.as_item())).chain(targets) {
        for (kind, dev) in [
            ("dependencies", false),
            ("dev-dependencies", true),
            ("dev_dependencies", true),
        ] {
            let Some(table) = section.get(kind).and_then(Item::as_table_like) else {
                continue;
            };
            for (key, entry) in table.iter() {
                let spec = entry.as_table_like();
                let package = spec
                    .and_then(|spec| spec.get("package"))
                    .and_then(Item::as_str)
                    .unwrap_or(key);
                if dev {
                    manifest.dev.insert(package.to_owned());
                } else {
                    manifest.normal.insert(package.to_owned());
                }
                let test_support = spec
                    .and_then(|spec| spec.get("features"))
                    .and_then(Item::as_array)
                    .is_some_and(|features| {
                        features
                            .iter()
                            .any(|feature| feature.as_str() == Some("test-support"))
                    });
                manifest.test_support |= top && dev && package == "bir-core" && test_support;
            }
        }
    }
    manifest
}

/// The manifests (labelled) whose tests run bir-core without an unconditional
/// `bir-core = { …, features = ["test-support"] }` under `[dev-dependencies]`.
///
/// The rule: a crate's tests run bir-core when the crate is bir-core, or when a
/// normal or dev dependency of it links bir-core. A crate links bir-core when
/// it is bir-core or a normal dependency links it, so bir-app-api's users count
/// as well as bir-core's. A dev-dependency of bir-core on the crate itself is
/// enough: cargo unifies it into the one bir-core of that test build. Only an
/// unconditional `[dev-dependencies]` entry counts, so every target gets it;
/// one under `[target.'cfg(…)'.dev-dependencies]` does not.
fn missing_test_support(manifests: &[(String, Manifest)]) -> Vec<String> {
    let mut links: BTreeSet<&str> = BTreeSet::from(["bir-core"]);
    loop {
        let before = links.len();
        for (_, manifest) in manifests {
            if manifest
                .normal
                .iter()
                .any(|package| links.contains(package.as_str()))
            {
                links.insert(&manifest.name);
            }
        }
        if links.len() == before {
            break;
        }
    }
    manifests
        .iter()
        .filter(|(_, manifest)| {
            let tests_run_bir_core = links.contains(manifest.name.as_str())
                || manifest
                    .dev
                    .iter()
                    .any(|package| links.contains(package.as_str()));
            tests_run_bir_core && !manifest.test_support
        })
        .map(|(label, _)| label.clone())
        .collect()
}

#[test]
fn every_crate_that_uses_bir_core_tests_with_its_test_build_rule() {
    let root = repo_root();
    let read: Vec<(String, Manifest)> = manifests(&root)
        .iter()
        .map(|path| {
            let label = path.strip_prefix(&root).unwrap().display().to_string();
            (
                label,
                read_manifest(&fs::read_to_string(path).expect("read Cargo.toml")),
            )
        })
        .collect();
    let missing = missing_test_support(&read);
    assert!(
        missing.is_empty(),
        "these crates' tests would reach the real keychain and data folder; add \
         `bir-core = {{ path = …, default-features = false, features = [\"test-support\"] }}` \
         under [dev-dependencies]: {missing:?}"
    );
}

/// The manifest rule sees bir-core however it is named or reached, and a test-support entry that misses a target.
#[test]
fn the_manifest_rule_sees_every_way_to_reach_bir_core() {
    const TESTED: &str = "[dev-dependencies]\n\
        bir-core = { path = \"../bir-core\", default-features = false, features = [\"test-support\"] }\n";
    let app_api = format!(
        "[package]\nname = \"bir-app-api\"\n[dependencies]\nbir-core = {{ path = \"../bir-core\" }}\n{TESTED}"
    );
    let cases = [
        (
            "a [dependencies.bir-core] table",
            "[package]\nname = \"a\"\n[dependencies.bir-core]\npath = \"../bir-core\"\n",
        ),
        (
            "a renamed package",
            "[package]\nname = \"b\"\n[dependencies]\ncore = { package = \"bir-core\", path = \"../bir-core\" }\n",
        ),
        (
            "bir-core through bir-app-api",
            "[package]\nname = \"c\"\n[dependencies]\nbir-app-api = { path = \"../bir-app-api\" }\n",
        ),
        (
            "bir-app-api as a dev-dependency",
            "[package]\nname = \"d\"\n[dev-dependencies]\nbir-app-api = { path = \"../bir-app-api\" }\n",
        ),
        (
            "a target-specific dev-dependency",
            "[package]\nname = \"e\"\n[target.'cfg(unix)'.dev-dependencies]\nbir-core = { path = \"../bir-core\" }\n",
        ),
    ];
    for (case, text) in cases {
        let read = |text: &str| {
            vec![
                ("bir-app-api".to_owned(), read_manifest(&app_api)),
                (case.to_owned(), read_manifest(text)),
            ]
        };
        assert_eq!(missing_test_support(&read(text)), [case], "{case}");
        // The entry as a table, beside a `[dev-dependencies]` the case may already have.
        let tested = format!(
            "{text}[dev-dependencies.bir-core]\npath = \"../bir-core\"\nfeatures = [\"test-support\"]\n"
        );
        assert!(
            missing_test_support(&read(&tested)).is_empty(),
            "{case}, with test-support"
        );
    }

    let only_on_unix = "[package]\nname = \"f\"\n[dependencies]\nbir-core = { path = \"../bir-core\" }\n\
        [target.'cfg(unix)'.dev-dependencies]\n\
        bir-core = { path = \"../bir-core\", features = [\"test-support\"] }\n";
    assert_eq!(
        missing_test_support(&[("f".to_owned(), read_manifest(only_on_unix))]),
        ["f"],
        "test-support on one target leaves the others' tests without it"
    );

    // A crate that only dev-depends on one whose tests use bir-core does not run bir-core itself.
    let tests_only =
        "[package]\nname = \"g\"\n[dev-dependencies]\nbir-core = { path = \"../bir-core\" }\n";
    let user = "[package]\nname = \"h\"\n[dependencies]\ng = { path = \"../g\" }\n";
    assert_eq!(
        missing_test_support(&[
            ("g".to_owned(), read_manifest(tests_only)),
            ("h".to_owned(), read_manifest(user)),
        ]),
        ["g"]
    );
}

/// What a line that reaches an OS keychain contains: a `keyring` entry, the
/// macOS Security framework, the `security` command, or the iOS Keychain's
/// `SecItem*` calls. Comment lines do not count.
const KEYCHAIN_MARKS: &[&str] = &[
    "keyring::Entry::new(",
    "security_framework::",
    "SecKeychain::",
    "SecItemCopyMatching(",
    "SecItemAdd(",
    "SecItemDelete(",
    "\"/usr/bin/security\"",
];

/// Each reviewed keychain call site: the file, how many of its lines carry a
/// [`KEYCHAIN_MARKS`] mark (or a bare `Entry::new("…")` after `use keyring::Entry`),
/// and why no test reaches the OS through it.
const REVIEWED_KEYCHAIN_CALLS: &[(&str, usize, &str)] = &[
    (
        "crates/bir-core/src/cloud_account.rs",
        1,
        "keyring_load, keyring_save and keyring_delete return first on platform::keychain_disabled()",
    ),
    (
        "crates/bir-core/src/db/mod.rs",
        2,
        "the SQLCipher master key: only with the sqlcipher feature, which no build turns on",
    ),
    (
        "crates/bir-core/src/dev_mode.rs",
        2,
        "the test-server password: both return first on platform::keychain_disabled()",
    ),
    (
        "crates/bir-core/src/email_sign_in.rs",
        3,
        "keyring_load, keyring_save and keyring_delete return first on platform::keychain_disabled()",
    ),
    (
        "crates/bir-core/src/google_calendar.rs",
        3,
        "keychain_set, keychain_get and keychain_delete: their callers return first on platform::keychain_disabled()",
    ),
    (
        "crates/bir-core/src/keychain.rs",
        8,
        "NativeKeychain (Security framework) and SecurityCli (/usr/bin/security): every read, add, upsert and delete \
         returns first on keychain_off(), which is platform::keychain_disabled(); status_text only formats an error",
    ),
    (
        "crates/bir-submit/src/ios_storage.rs",
        3,
        "iOS only (cfg(target_os = \"ios\")), and only with use_keychain, which bir-core sets from \
         !platform::keychain_disabled() (install_id.rs, platform/ios.rs)",
    ),
    (
        "crates/bir-submit/src/secrets.rs",
        1,
        "only with use_keychain, which bir-core sets from !platform::keychain_disabled() (relay.rs)",
    ),
];

/// The Rust files of one crate: `src/`, `tests/`, `examples/`, `benches/` and `build.rs`.
fn crate_rust_files(crate_dir: &Path, out: &mut Vec<PathBuf>) {
    for folder in ["src", "tests", "examples", "benches"] {
        rust_files(&crate_dir.join(folder), out);
    }
    let build = crate_dir.join("build.rs");
    if build.is_file() {
        out.push(build);
    }
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "target") {
                continue;
            }
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// How many lines of `text` reach an OS keychain.
fn keychain_lines(text: &str) -> usize {
    let bare_entry = text.contains("use keyring::Entry");
    text.lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .filter(|line| {
            KEYCHAIN_MARKS.iter().any(|mark| line.contains(mark))
                || (bare_entry && line.contains(" Entry::new(\""))
        })
        .count()
}

#[test]
fn every_keychain_call_site_is_reviewed() {
    let root = repo_root();
    let mut files = Vec::new();
    for crate_dir in fs::read_dir(root.join("crates"))
        .expect("crates/")
        .filter_map(Result::ok)
    {
        crate_rust_files(&crate_dir.path(), &mut files);
    }
    crate_rust_files(&root.join("nsh"), &mut files);
    let mut found: Vec<(String, usize)> = files
        .iter()
        .filter_map(|path| {
            let relative = path.strip_prefix(&root).unwrap();
            let relative = relative.to_string_lossy().replace('\\', "/");
            // This file names every mark itself.
            if relative == "crates/bir-core/tests/test_isolation_guard.rs" {
                return None;
            }
            let count = keychain_lines(&fs::read_to_string(path).ok()?);
            (count > 0).then_some((relative, count))
        })
        .collect();
    found.sort();
    let reviewed: Vec<(String, usize)> = REVIEWED_KEYCHAIN_CALLS
        .iter()
        .map(|(file, count, _why)| ((*file).to_owned(), *count))
        .collect();
    assert_eq!(
        found, reviewed,
        "a keychain call site was added, moved or removed. Put it behind \
         `platform::keychain_disabled()` (always true in a test build), then update \
         REVIEWED_KEYCHAIN_CALLS with the reason"
    );
}

#[test]
fn the_keychain_marks_cover_every_kind_of_call() {
    let lines = "let a = keyring::Entry::new(SERVICE, ACCOUNT);\n\
        use security_framework::os::macos::passwords::find_generic_password;\n\
        let k = SecKeychain::default();\n\
        check(unsafe { SecItemCopyMatching(query, &mut result) });\n\
        check(unsafe { SecItemAdd(attributes, null) });\n\
        check(unsafe { SecItemDelete(query) });\n\
        Command::new(\"/usr/bin/security\")\n\
        // keyring::Entry::new( in a comment\n\
        let other = Entry::new(\"x\");\n";
    assert_eq!(keychain_lines(lines), 7);
    assert_eq!(keychain_lines(&format!("use keyring::Entry;\n{lines}")), 8);
}

/// A test build looks at the system's data folder without making it: with a
/// home outside the system temp folder, `data_dir()` answers this process's
/// scratch folder and nothing appears in that home.
#[test]
fn a_test_build_never_creates_the_data_folder_it_does_not_use() {
    // Inside the build folder, so outside the system temp folder (unless the build folder is itself in it).
    let home = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("home-{}", std::process::id()));
    let _ = fs::remove_dir_all(&home);
    fs::create_dir_all(&home).unwrap();
    let temp = std::env::temp_dir().canonicalize().unwrap();
    if home.canonicalize().unwrap().starts_with(&temp) {
        eprintln!("skipped: the build folder is inside the system temp folder");
        return;
    }
    let dir = temp_env::with_vars(
        [
            ("HOME", Some(home.as_os_str())),
            ("XDG_DATA_HOME", None),
            ("LOCALAPPDATA", Some(home.as_os_str())),
            ("USERPROFILE", Some(home.as_os_str())),
        ],
        bir_core::platform::data_dir,
    );
    let made: Vec<PathBuf> = fs::read_dir(&home)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    fs::remove_dir_all(&home).unwrap();
    assert!(
        dir.canonicalize().unwrap().starts_with(&temp),
        "{} is not a scratch folder",
        dir.display()
    );
    assert!(made.is_empty(), "the data folder was made: {made:?}");
}

#[test]
fn this_test_build_has_the_keychain_off() {
    temp_env::with_var("BUWIZ_NO_KEYCHAIN", None::<&str>, || {
        assert!(
            bir_core::platform::keychain_disabled(),
            "bir-core's own integration tests must be built with test-support"
        );
    });
}
