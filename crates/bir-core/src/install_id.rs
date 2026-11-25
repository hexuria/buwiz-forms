//! The install id: an id this app makes once per install and sends from the very first sign-in request
//! (`install_id` in `/auth/email/start` and in the browser device flow's start, then in the device registration).
//!
//! It is 16–64 URL-safe characters and never a raw hardware value. It stays across sign-out.
//! - iPhone, iPad: a random id kept by `bir_submit::ios_storage::install_id`: the Keychain first (this device only), so
//!   it survives a delete and reinstall; a file beside the database when the Keychain is switched off
//!   (`BUWIZ_NO_KEYCHAIN`) or cannot be used for any reason.
//! - Android: a salted SHA-256 of `Settings.Secure.ANDROID_ID`, which the app crate reads through its Java access and
//!   registers here ([`crate::platform::set_device_id_source`]).
//! - macOS, Windows, Linux: a salted SHA-256 of the machine's id (IOPlatformUUID, MachineGuid, /etc/machine-id)
//!   together with the OS user name. No new permission.
//! - Otherwise (no machine id): a random id in a file beside the database, logged
//!   as "random install id". The id itself is never logged.
//! - A test environment (`EBIR_TEST_ENV`) always uses a random id in that file, which is then a scratch folder: a test
//!   never reads the real machine id.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use sha2::{Digest, Sha256};

/// The file beside the database that holds a random install id.
pub const INSTALL_ID_FILE: &str = "buwiz-install-id";
/// Mixed into every derived id, so it is this app's and matches no other app's hash of the same machine id.
const SALT: &str = "buwiz-install-id/v1";

/// Where the id came from (logged, never the id).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// Hashed from the machine's (or Android's) id.
    Derived,
    /// The random id in the file beside the database.
    File,
}

/// What [`resolve`] reads. The real one is [`OsSources`]; tests give a fake.
pub(crate) trait Sources {
    fn test_environment(&self) -> bool;
    /// The machine's id (desktops) or `ANDROID_ID` (Android); `None` on iOS and when it cannot be read.
    fn machine_id(&self) -> Option<String>;
    /// The OS user name, on desktops.
    fn user_name(&self) -> Option<String>;
    /// The folder of [`INSTALL_ID_FILE`].
    fn folder(&self) -> PathBuf;
}

/// 16–64 characters, each a letter, a digit, `-` or `_`.
pub fn valid(id: &str) -> bool {
    (16..=64).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// The salted SHA-256 of a machine id (and the user name), base64url without padding: 43 characters.
pub fn derive(machine_id: &str, user_name: Option<&str>) -> String {
    let mut hash = Sha256::new();
    hash.update(SALT.as_bytes());
    hash.update([0]);
    hash.update(machine_id.trim().as_bytes());
    hash.update([0]);
    hash.update(user_name.unwrap_or("").as_bytes());
    data_encoding::BASE64URL_NOPAD.encode(&hash.finalize())
}

/// A new random id: 24 random bytes, base64url: 32 characters.
pub fn random() -> String {
    data_encoding::BASE64URL_NOPAD.encode(&rand::random::<[u8; 24]>())
}

/// A machine id worth hashing: not empty, not all zeros or dashes (a VM or a container that has none).
fn usable_machine_id(id: &str) -> bool {
    let id = id.trim();
    !id.is_empty() && id.chars().any(|c| c.is_ascii_alphanumeric() && c != '0')
}

/// The id by the rules above, and where it came from.
pub(crate) fn resolve(sources: &dyn Sources) -> (String, Origin) {
    if sources.test_environment() {
        return (from_file(&sources.folder()), Origin::File);
    }
    if let Some(machine) = sources.machine_id().filter(|id| usable_machine_id(id)) {
        return (
            derive(&machine, sources.user_name().as_deref()),
            Origin::Derived,
        );
    }
    tracing::info!("random install id");
    (from_file(&sources.folder()), Origin::File)
}

/// The id in the file in `folder`, or a new random one written there (mode 0600). A file that cannot be written
/// still gives an id for this run.
fn from_file(folder: &Path) -> String {
    let path = folder.join(INSTALL_ID_FILE);
    if let Ok(raw) = std::fs::read_to_string(&path)
        && valid(raw.trim())
    {
        return raw.trim().to_owned();
    }
    let id = random();
    if let Err(error) = write_private(&path, &id) {
        tracing::warn!("the install id file could not be written: {error}");
    }
    id
}

fn write_private(path: &Path, value: &str) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(value.as_bytes())
}

/// This install's id (worked out once per process and folder).
pub fn install_id() -> String {
    static CACHE: Mutex<Option<(PathBuf, String)>> = Mutex::new(None);
    let sources = OsSources;
    let folder = sources.folder();
    let mut cache = CACHE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some((cached_folder, id)) = cache.as_ref()
        && *cached_folder == folder
    {
        return id.clone();
    }
    let (id, origin) = resolve_here(&sources, &folder);
    tracing::info!("install id: {origin}");
    *cache = Some((folder, id.clone()));
    id
}

/// [`resolve`], except on iPhone and iPad outside a test environment: there `bir_submit::ios_storage::install_id` keeps
/// it (the Keychain first, else its file beside the database), offered a fresh random id for a first install.
fn resolve_here(sources: &OsSources, folder: &Path) -> (String, String) {
    #[cfg(target_os = "ios")]
    if !sources.test_environment() {
        let kept = bir_submit::ios_storage::install_id(
            folder,
            !crate::platform::keychain_disabled(),
            Some(&random()),
        );
        return (kept.value, format!("{:?}", kept.source));
    }
    let _ = folder;
    let (id, origin) = resolve(sources);
    (id, format!("{origin:?}"))
}

/// The `platform` the server is told: ios, ipados, android, macos, windows or linux.
pub fn platform() -> &'static str {
    #[cfg(target_os = "ios")]
    {
        ios_kind(
            &ios_machine(),
            std::env::var("SIMULATOR_MODEL_IDENTIFIER").ok().as_deref(),
        )
    }
    #[cfg(not(target_os = "ios"))]
    {
        match std::env::consts::OS {
            "android" => "android",
            "macos" => "macos",
            "windows" => "windows",
            _ => "linux",
        }
    }
}

/// iPhone or iPad from the model identifier (`iPad13,4`; on a Simulator `hw.machine` is the Mac's, and the
/// Simulator's model is in `SIMULATOR_MODEL_IDENTIFIER`).
pub fn ios_kind(machine: &str, simulator_model: Option<&str>) -> &'static str {
    let model = simulator_model
        .filter(|model| !model.is_empty())
        .unwrap_or(machine);
    if model.starts_with("iPad") {
        "ipados"
    } else {
        "ios"
    }
}

#[cfg(target_os = "ios")]
fn ios_machine() -> String {
    let mut size: libc::size_t = 0;
    // SAFETY: a size query with a null buffer, then a read into a buffer of that size.
    unsafe {
        if libc::sysctlbyname(
            c"hw.machine".as_ptr(),
            std::ptr::null_mut(),
            &mut size,
            std::ptr::null_mut(),
            0,
        ) != 0
            || size == 0
        {
            return String::new();
        }
        let mut buffer = vec![0u8; size];
        if libc::sysctlbyname(
            c"hw.machine".as_ptr(),
            buffer.as_mut_ptr().cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        ) != 0
        {
            return String::new();
        }
        buffer.truncate(size);
        String::from_utf8_lossy(&buffer)
            .trim_end_matches('\0')
            .to_owned()
    }
}

/// The real sources: the machine's id on desktops, `ANDROID_ID` on Android (iOS goes to `bir_submit::ios_storage`).
pub(crate) struct OsSources;

impl Sources for OsSources {
    fn test_environment(&self) -> bool {
        cfg!(test) || crate::db::test_environment()
    }

    fn machine_id(&self) -> Option<String> {
        if cfg!(test) {
            return None;
        }
        #[cfg(target_os = "android")]
        {
            crate::platform::device_id_source().and_then(|read| read())
        }
        #[cfg(target_os = "macos")]
        {
            macos_platform_uuid()
        }
        #[cfg(target_os = "linux")]
        {
            ["/etc/machine-id", "/var/lib/dbus/machine-id"]
                .iter()
                .find_map(|path| std::fs::read_to_string(path).ok())
                .map(|id| id.trim().to_owned())
        }
        #[cfg(target_os = "windows")]
        {
            windows_machine_guid()
        }
        #[cfg(target_os = "ios")]
        {
            None
        }
    }

    fn user_name(&self) -> Option<String> {
        if cfg!(any(target_os = "ios", target_os = "android")) {
            return None;
        }
        ["USER", "USERNAME", "LOGNAME"].iter().find_map(|name| {
            std::env::var(name)
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
    }

    fn folder(&self) -> PathBuf {
        crate::db::app_database_path()
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

/// `IOPlatformUUID` of `IOPlatformExpertDevice`, through IOKit (no process, no permission).
#[cfg(target_os = "macos")]
fn macos_platform_uuid() -> Option<String> {
    use std::ffi::{c_char, c_void};
    type CFTypeRef = *const c_void;
    #[link(name = "IOKit", kind = "framework")]
    unsafe extern "C" {
        fn IOServiceMatching(name: *const c_char) -> *mut c_void;
        fn IOServiceGetMatchingService(main_port: u32, matching: *mut c_void) -> u32;
        fn IORegistryEntryCreateCFProperty(
            entry: u32,
            key: CFTypeRef,
            allocator: CFTypeRef,
            options: u32,
        ) -> CFTypeRef;
        fn IOObjectRelease(object: u32) -> i32;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFStringCreateWithCString(
            allocator: CFTypeRef,
            text: *const c_char,
            encoding: u32,
        ) -> CFTypeRef;
        fn CFStringGetCString(
            string: CFTypeRef,
            buffer: *mut c_char,
            size: isize,
            encoding: u32,
        ) -> u8;
        fn CFGetTypeID(object: CFTypeRef) -> usize;
        fn CFStringGetTypeID() -> usize;
        fn CFRelease(object: CFTypeRef);
    }
    const UTF8: u32 = 0x0800_0100;
    // SAFETY: IOKit and CoreFoundation calls with valid arguments; every object made here is released once.
    unsafe {
        let service =
            IOServiceGetMatchingService(0, IOServiceMatching(c"IOPlatformExpertDevice".as_ptr()));
        if service == 0 {
            return None;
        }
        let key = CFStringCreateWithCString(std::ptr::null(), c"IOPlatformUUID".as_ptr(), UTF8);
        let value = if key.is_null() {
            std::ptr::null()
        } else {
            IORegistryEntryCreateCFProperty(service, key, std::ptr::null(), 0)
        };
        if !key.is_null() {
            CFRelease(key);
        }
        IOObjectRelease(service);
        if value.is_null() {
            return None;
        }
        let mut buffer = [0 as c_char; 128];
        let read = CFGetTypeID(value) == CFStringGetTypeID()
            && CFStringGetCString(value, buffer.as_mut_ptr(), buffer.len() as isize, UTF8) != 0;
        CFRelease(value);
        if !read {
            return None;
        }
        std::ffi::CStr::from_ptr(buffer.as_ptr())
            .to_str()
            .ok()
            .map(str::to_owned)
    }
}

/// `HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid`, read with `reg query` (no window, no permission).
#[cfg(target_os = "windows")]
fn windows_machine_guid() -> Option<String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let output = std::process::Command::new("reg")
        .args([
            "query",
            r"HKLM\SOFTWARE\Microsoft\Cryptography",
            "/v",
            "MachineGuid",
            "/reg:64",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    machine_guid_from_reg(&String::from_utf8_lossy(&output.stdout))
}

/// The value of the `MachineGuid` line of `reg query`'s answer.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn machine_guid_from_reg(text: &str) -> Option<String> {
    text.lines()
        .find(|line| line.trim_start().starts_with("MachineGuid"))
        .and_then(|line| line.split_whitespace().last())
        .filter(|guid| guid.len() >= 16)
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    struct Fake {
        test_env: bool,
        machine: Option<String>,
        user: Option<String>,
        folder: PathBuf,
        machine_read: RefCell<bool>,
    }

    impl Sources for Fake {
        fn test_environment(&self) -> bool {
            self.test_env
        }
        fn machine_id(&self) -> Option<String> {
            *self.machine_read.borrow_mut() = true;
            self.machine.clone()
        }
        fn user_name(&self) -> Option<String> {
            self.user.clone()
        }
        fn folder(&self) -> PathBuf {
            self.folder.clone()
        }
    }

    fn fake(dir: &Path) -> Fake {
        Fake {
            folder: dir.to_path_buf(),
            ..Fake::default()
        }
    }

    #[test]
    fn ids_are_16_to_64_url_safe_characters() {
        assert!(valid(&"a".repeat(16)));
        assert!(valid(&"Z9-_".repeat(16)));
        assert!(!valid(&"a".repeat(15)));
        assert!(!valid(&"a".repeat(65)));
        assert!(!valid("abcdefghijklmno+"));
        assert!(!valid("abcdefghijklmno="));
        assert!(!valid("abcdefgh ijklmnop"));
        for _ in 0..20 {
            let id = random();
            assert_eq!(id.len(), 32);
            assert!(valid(&id));
        }
        assert_ne!(random(), random());
    }

    #[test]
    fn a_derived_id_is_salted_stable_and_not_the_raw_value() {
        let machine = "6F9C7A1E-2B44-4E0A-9D11-3C5B8E7F0A12";
        let id = derive(machine, Some("juana"));
        assert_eq!(id.len(), 43);
        assert!(valid(&id));
        assert_eq!(id, derive(machine, Some("juana")), "stable");
        assert_eq!(
            id,
            derive(&format!("  {machine}\n"), Some("juana")),
            "the file's newline is not part of it"
        );
        assert_ne!(
            id,
            derive(machine, Some("pedro")),
            "another user, another id"
        );
        assert_ne!(id, derive("another-machine-id-0000", Some("juana")));
        assert!(!id.contains(machine) && !id.to_lowercase().contains("6f9c7a1e"));
        // Not the plain hash of the machine id: the salt is in it.
        let plain = data_encoding::BASE64URL_NOPAD.encode(&Sha256::digest(machine.as_bytes()));
        assert_ne!(id, plain);
        assert_eq!(derive("abc", None), derive("abc", Some("")));
    }

    #[test]
    fn a_test_environment_uses_a_random_file_id_and_never_reads_the_machine() {
        let dir = tempfile::tempdir().unwrap();
        let mut sources = fake(dir.path());
        sources.test_env = true;
        sources.machine = Some("real-machine-id-1234".into());
        let (first, origin) = resolve(&sources);
        assert_eq!(origin, Origin::File);
        assert!(
            !*sources.machine_read.borrow(),
            "the machine id is not read"
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join(INSTALL_ID_FILE)).unwrap(),
            first
        );
        assert_eq!(resolve(&sources).0, first, "kept in the file");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.path().join(INSTALL_ID_FILE))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[test]
    fn a_desktop_or_android_hashes_its_machine_id() {
        let dir = tempfile::tempdir().unwrap();
        let mut sources = fake(dir.path());
        sources.machine = Some("0f3c9a1e2b4d4e0a9d113c5b8e7f0a12\n".into());
        sources.user = Some("juana".into());
        let (id, origin) = resolve(&sources);
        assert_eq!(origin, Origin::Derived);
        assert_eq!(
            id,
            derive("0f3c9a1e2b4d4e0a9d113c5b8e7f0a12", Some("juana"))
        );
        assert!(
            !dir.path().join(INSTALL_ID_FILE).exists(),
            "nothing written"
        );
        // Android: no user name.
        sources.user = None;
        assert_eq!(
            resolve(&sources).0,
            derive("0f3c9a1e2b4d4e0a9d113c5b8e7f0a12", None)
        );
    }

    #[test]
    fn no_machine_id_or_an_empty_one_is_a_random_file_id() {
        for machine in [
            None,
            Some(String::new()),
            Some("  \n".into()),
            Some("00000000-0000-0000-0000-000000000000".into()),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let mut sources = fake(dir.path());
            sources.machine = machine.clone();
            let (id, origin) = resolve(&sources);
            assert_eq!(origin, Origin::File, "{machine:?}");
            assert_eq!(
                std::fs::read_to_string(dir.path().join(INSTALL_ID_FILE)).unwrap(),
                id
            );
            assert_eq!(
                resolve(&sources).0,
                id,
                "{machine:?}: the same id next time"
            );
        }
    }

    #[test]
    fn a_damaged_file_is_replaced_and_an_unwritable_folder_still_gives_an_id() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(INSTALL_ID_FILE), "not an id!").unwrap();
        let id = from_file(dir.path());
        assert!(valid(&id));
        assert_eq!(
            std::fs::read_to_string(dir.path().join(INSTALL_ID_FILE)).unwrap(),
            id
        );
        let blocked = dir.path().join("file-not-folder");
        std::fs::write(&blocked, "x").unwrap();
        assert!(valid(&from_file(&blocked)));
    }

    #[test]
    fn the_real_sources_under_test_never_read_the_machine() {
        assert!(OsSources.test_environment());
        assert_eq!(OsSources.machine_id(), None);
    }

    #[test]
    fn an_ipad_says_ipados_and_an_iphone_ios() {
        assert_eq!(ios_kind("iPad13,4", None), "ipados");
        assert_eq!(ios_kind("iPhone15,2", None), "ios");
        assert_eq!(ios_kind("arm64", Some("iPad14,1")), "ipados");
        assert_eq!(ios_kind("arm64", Some("iPhone16,1")), "ios");
        assert_eq!(ios_kind("arm64", Some("")), "ios");
        assert!(["macos", "linux", "windows", "android", "ios", "ipados"].contains(&platform()));
    }

    #[test]
    fn the_machine_guid_is_read_from_reg_query() {
        let answer = "\r\nHKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Cryptography\r\n    MachineGuid    REG_SZ    6f9c7a1e-2b44-4e0a-9d11-3c5b8e7f0a12\r\n\r\n";
        assert_eq!(
            machine_guid_from_reg(answer).as_deref(),
            Some("6f9c7a1e-2b44-4e0a-9d11-3c5b8e7f0a12")
        );
        assert_eq!(machine_guid_from_reg("ERROR: not found"), None);
    }
}
