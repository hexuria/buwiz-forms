//! B2's host-enforced room lock, through the real host entry points.

use std::sync::{Arc, Mutex};

use bir_core::db::Database;
use bir_core::profile::TaxpayerType;
use bir_core::tax_profile::{BranchFacts, NewTaxpayer};
use gpui_shell::{HostArguments, HostValue};

use super::*;

const ROOT: &str = "123456788";
const TIN: &str = "12345678800000";
const OTHER_ROOT: &str = "987654321";
const OTHER_TIN: &str = "98765432100000";

struct LegacyRoom {
    _turn: std::sync::MutexGuard<'static, ()>,
    folder: tempfile::TempDir,
    handle: Arc<Mutex<Database>>,
}

impl LegacyRoom {
    fn open() -> Self {
        let turn = crate::open_room::room_test_turn();
        let folder = tempfile::tempdir().unwrap();
        bir_core::platform::set_device_dir_override(Some(folder.path().to_path_buf()));
        let layout = bir_core::rooms::Layout::at(folder.path().to_path_buf());
        std::fs::create_dir_all(layout.device_dir()).unwrap();
        let (handle, _) = crate::session::open_legacy_in_place_for_test(
            &layout,
            "lock-test".into(),
            "https://api.buwiz.test".into(),
        )
        .unwrap();
        crate::account_gate::set_unlocked(true);
        Self {
            _turn: turn,
            folder,
            handle,
        }
    }
}

impl Drop for LegacyRoom {
    fn drop(&mut self) {
        crate::session::close(crate::session::Reason::SignOut);
        bir_core::platform::set_device_dir_override(None);
        crate::account_gate::clear_client_scope();
    }
}

/// These tests exercise the process-wide room state. Run each in a child process so it cannot
/// lock or scope the unrelated thread-local databases of the other unit tests.
fn isolated(name: &str, body: impl FnOnce()) {
    const CHILD: &str = "BIR_LOCK_TEST_CHILD";
    if std::env::var_os(CHILD).is_some_and(|running| running == name) {
        body();
        return;
    }
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            &format!("bir_module::lock_tests::{name}"),
            "--nocapture",
        ])
        .env(CHILD, name)
        .env("EBIR_TEST_ENV", "1")
        .env("BIR_AUTH_BYPASS", "1")
        .env("BUWIZ_NO_KEYCHAIN", "1")
        .env("BIR_SFTP_DRY_RUN", "1")
        .env_remove("BIR_SFTP_LIVE")
        .status()
        .unwrap();
    assert!(status.success(), "{name}: the child failed");
}

fn args(values: Vec<HostValue>) -> HostArguments {
    HostArguments::new(values)
}

fn text(value: &str) -> HostValue {
    HostValue::Str(value.into())
}

fn lock_now() -> bool {
    crate::owner_lock_module::register(gpui_shell::HostModule::new("bir"))
        .call("lock_now", &HostArguments::new(vec![]))
        .unwrap()
        == HostValue::Bool(true)
}

fn create_taxpayer(root: &str, name: &str) {
    with_database(|db| {
        db.create_taxpayer(NewTaxpayer {
            tin_root: root.into(),
            name: name.into(),
            taxpayer_type: TaxpayerType::Corporation,
            birth_date: None,
            head_office: BranchFacts {
                rdo_code: "040".into(),
                registered_address: "1 Example Street, Pasig".into(),
                zip_code: "1600".into(),
                phone: String::new(),
                email: String::new(),
            },
        })
        .map_err(|error| HostError::new(error.to_string()))?;
        Ok(())
    })
    .unwrap();
}

/// A solo legacy room whose owner PIN is configured. It starts locked: only a host check may open it.
fn locked_legacy_room() -> LegacyRoom {
    let room = LegacyRoom::open();
    create_taxpayer(ROOT, "Example Trading Corp");
    setup_solo(&args(vec![text(ROOT)])).unwrap();
    set_taxpayer_pin(&args(vec![text(ROOT), text("0817")])).unwrap();
    assert!(lock_now(), "the room has its owner PIN configured");
    room
}

fn locked<T>(result: Result<T, HostError>, way: &str) {
    let Err(error) = result else {
        panic!("{way}: the locked room answered");
    };
    assert_eq!(
        error.message(),
        crate::account_gate::LOCKED_MESSAGE,
        "{way}"
    );
}

#[test]
fn a_locked_legacy_room_refuses_every_filing_path_until_the_admin_check() {
    isolated(
        "a_locked_legacy_room_refuses_every_filing_path_until_the_admin_check",
        || {
            let room = locked_legacy_room();
            assert!(!crate::account_gate::room_unlocked());

            locked(list_profiles(), "bir list_profiles");
            locked(database_handle(), "database_handle");
            locked(with_database(|_| Ok(())), "with_database");
            locked(with_reader(|_| Ok(())), "with_reader");
            locked(
                crate::tax_profile_module::register(gpui_shell::HostModule::new("tax"))
                    .call("list_taxpayers", &HostArguments::new(vec![])),
                "tax_profile list_taxpayers",
            );
            locked(
                crate::dashboard_module::register(gpui_shell::HostModule::new("dash"))
                    .call("list_returns", &args(vec![text("all")])),
                "dashboard list_returns",
            );
            locked(
                crate::legacy_import_module::register(gpui_shell::HostModule::new("legacy"))
                    .call("list_legacy_filed_returns", &args(vec![text(TIN)])),
                "legacy list_legacy_filed_returns",
            );
            let export = room.folder.path().join("locked-export.zip");
            locked(export_profile_to(TIN, Some("0817"), &export), "export");
            assert!(!export.exists(), "the locked export wrote nothing");
            let scan_target = room.folder.path().join("scan-target");
            std::fs::create_dir(&scan_target).unwrap();
            locked(
                crate::legacy_import_module::scan_folder(&scan_target),
                "legacy import scan",
            );

            // The lock page itself still answers while the room is locked.
            assert!(auth_state().is_ok(), "auth_state reads the lock page state");
            assert!(taxpayer_auth(&args(vec![text(ROOT)])).is_ok());
            assert_eq!(
                verify_admin_pin(&args(vec![text("0000")])).unwrap(),
                HostValue::Bool(false),
                "a wrong PIN is an answer, not a lock refusal"
            );
            assert!(!crate::account_gate::room_unlocked());

            assert_eq!(
                verify_admin_pin(&args(vec![text("0817")])).unwrap(),
                HostValue::Bool(true)
            );
            assert!(crate::account_gate::room_unlocked());

            // Positive control: the same paths answer after the host-checked unlock.
            assert!(list_profiles().is_ok());
            assert!(database_handle().is_ok());
            assert!(with_database(|_| Ok(())).is_ok());
            assert!(with_reader(|_| Ok(())).is_ok());
            assert!(
                crate::tax_profile_module::register(gpui_shell::HostModule::new("tax"))
                    .call("list_taxpayers", &HostArguments::new(vec![]))
                    .is_ok()
            );
            assert!(
                crate::dashboard_module::register(gpui_shell::HostModule::new("dash"))
                    .call("list_returns", &args(vec![text("all")]))
                    .is_ok()
            );
            assert!(
                crate::legacy_import_module::register(gpui_shell::HostModule::new("legacy"))
                    .call("list_legacy_filed_returns", &args(vec![text(TIN)]))
                    .is_ok()
            );
            let export = room.folder.path().join("export.zip");
            export_profile_to(TIN, Some("0817"), &export).unwrap();
            assert!(export.exists());
            assert!(crate::legacy_import_module::scan_folder(&scan_target).is_ok());
            drop(room.handle.clone());
        },
    );
}

#[test]
fn a_room_with_no_lock_configured_opens_for_setup_and_solo_without_a_pin() {
    isolated(
        "a_room_with_no_lock_configured_opens_for_setup_and_solo_without_a_pin",
        || {
            let room = LegacyRoom::open();
            crate::account_gate::set_room_unlocked(false);

            // Every new install and the store-review account have no access mode yet: Setup still works.
            assert!(
                with_database(|_| Ok(())).is_ok(),
                "a room with no lock is opened by the host"
            );
            create_taxpayer(ROOT, "Example Trading Corp");
            setup_solo(&args(vec![text(ROOT)])).unwrap();

            // A solo install whose admin has no PIN or authenticator has nothing to unlock with.
            crate::account_gate::set_room_unlocked(false);
            assert!(list_profiles().is_ok());
            assert!(with_reader(|_| Ok(())).is_ok());
            drop(room);
        },
    );
}

#[test]
fn a_client_pin_unlocks_only_that_taxpayers_scope() {
    isolated("a_client_pin_unlocks_only_that_taxpayers_scope", || {
        let room = LegacyRoom::open();
        create_taxpayer(ROOT, "Example Trading Corp");
        create_taxpayer(OTHER_ROOT, "Other Trading Corp");
        setup_solo(&args(vec![text(ROOT)])).unwrap();
        set_taxpayer_pin(&args(vec![text(ROOT), text("0817")])).unwrap();
        set_taxpayer_pin(&args(vec![text(OTHER_ROOT), text("2468")])).unwrap();
        crate::account_gate::set_room_unlocked(false);

        assert_eq!(
            verify_taxpayer_pin(&args(vec![text(OTHER_ROOT), text("2468")])).unwrap(),
            HostValue::Bool(true)
        );
        assert_eq!(
            crate::account_gate::client_scope().as_deref(),
            Some(OTHER_ROOT)
        );
        assert!(list_profiles().is_ok(), "the host check opens the room");

        let own = room.folder.path().join("own.zip");
        export_profile_to(OTHER_TIN, Some("2468"), &own).unwrap();
        let other = room.folder.path().join("other.zip");
        assert!(
            export_profile(&args(vec![text(TIN), text("0817")])).is_err(),
            "the client cannot export outside the signed-in taxpayer"
        );
        assert!(!other.exists());
        assert!(
            crate::legacy_import_module::scan_folder(room.folder.path()).is_err(),
            "the client cannot run a computer-wide import"
        );
        assert!(
            set_admin_pin(&args(vec![text("9999")])).is_err(),
            "the client cannot change Settings"
        );
    });
}

#[test]
fn recovery_key_and_mac_login_each_unlock_through_the_real_verbs() {
    isolated(
        "recovery_key_and_mac_login_each_unlock_through_the_real_verbs",
        || {
            struct OsYes;
            impl crate::hooks::Hooks for OsYes {
                fn os_authenticate(&self, _reason: &str) -> bool {
                    true
                }
            }
            crate::hooks::install_hooks(OsYes);

            let room = LegacyRoom::open();
            create_taxpayer(ROOT, "Example Trading Corp");
            setup_solo(&args(vec![text(ROOT)])).unwrap();
            set_taxpayer_pin(&args(vec![text(ROOT), text("0817")])).unwrap();
            let recovery_key =
                crate::bir_module::to_json(&create_recovery_key().unwrap())["recovery_key"]
                    .as_str()
                    .unwrap()
                    .to_owned();
            assert!(lock_now());
            locked(with_database(|_| Ok(())), "locked before recovery");

            assert_eq!(
                use_recovery_key(&args(vec![text("not-the-key")])).unwrap(),
                HostValue::Bool(false)
            );
            assert!(!crate::account_gate::room_unlocked());
            assert_eq!(
                use_recovery_key(&args(vec![text(&recovery_key)])).unwrap(),
                HostValue::Bool(true),
                "positive control: the recovery key unlocks"
            );
            assert!(with_database(|_| Ok(())).is_ok());

            assert!(lock_now());
            locked(with_database(|_| Ok(())), "locked before mac_login");
            let answer =
                futures::executor::block_on(mac_login(&args(vec![text("unlock")])).unwrap())
                    .unwrap();
            assert_eq!(
                answer,
                HostValue::Bool(true),
                "positive control: mac_login unlocks"
            );
            assert!(with_database(|_| Ok(())).is_ok());
            drop(room);
        },
    );
}
