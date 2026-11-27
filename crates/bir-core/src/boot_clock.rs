//! The clocks the PIN wait is measured on (accounts-and-lock PLAN §B.2, L4 and X3).
//!
//! A wait is timed on **time since boot** while the device has not restarted, so moving the wall clock changes
//! nothing; after a restart only the wall clock is left. "The same boot" is decided by a **boot id** that does
//! not move with the clock, never by comparing uptimes (a reboot with a longer uptime would read as the same
//! boot), and never by "now minus uptime" (derived from the wall clock):
//!
//! | System | Boot id | Time since boot (counts sleep) |
//! |---|---|---|
//! | macOS, iOS | `kern.bootsessionuuid` (`sysctl`); not `kern.boottime`, which the kernel moves when the clock is set | `mach_continuous_time` |
//! | Linux, Android | `/proc/sys/kernel/random/boot_id` | `clock_gettime(CLOCK_BOOTTIME)` |
//! | Windows | the `BootId` counter (`HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management\PrefetchParameters`) | `GetTickCount64` |
//!
//! Where either half cannot be read, the reading has no boot ([`ClockReading::boot`] is `None`): a wait it starts
//! is then timed on the wall clock alone, which a clock set back can never shorten (the wait is capped at its own
//! length).

/// The clocks, read once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClockReading {
    /// The wall clock: seconds since the Unix epoch.
    pub unix_secs: i64,
    /// This boot: its id and the seconds since it began. `None` where either cannot be read.
    pub boot: Option<Boot>,
}

/// One boot of the device, and how far into it a reading was taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Boot {
    /// Never empty.
    pub id: String,
    /// Seconds since this boot began, sleep included.
    pub secs: i64,
}

/// Read the clocks now.
pub fn read() -> ClockReading {
    let unix_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0);
    let boot = match (boot_id(), secs_since_boot()) {
        (Some(id), Some(secs)) if !id.is_empty() => Some(Boot { id, secs }),
        _ => None,
    };
    ClockReading { unix_secs, boot }
}

/// This boot's id ([`read`]'s table), or `None` where it cannot be read.
pub fn boot_id() -> Option<String> {
    os::boot_id()
        .map(|id| id.trim().to_owned())
        .filter(|id| !id.is_empty())
}

/// Seconds since this boot began, sleep included, or `None` where it cannot be read.
pub fn secs_since_boot() -> Option<i64> {
    os::secs_since_boot()
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod os {
    /// `mach_timebase_info_data_t` (`<mach/mach_time.h>`): ticks to nanoseconds is `numer / denom`.
    #[repr(C)]
    struct Timebase {
        numer: u32,
        denom: u32,
    }

    // libSystem. Declared here: `mach_continuous_time` (macOS 10.12, iOS 10: mach ticks since boot, sleep
    // included) is not in the libc crate, and libc's `mach_timebase_info` is deprecated there in favour of a crate
    // this tree does not have.
    unsafe extern "C" {
        fn mach_continuous_time() -> u64;
        fn mach_timebase_info(info: *mut Timebase) -> libc::c_int;
    }

    pub fn boot_id() -> Option<String> {
        let name = c"kern.bootsessionuuid";
        let mut buffer = [0u8; 64];
        let mut length = buffer.len();
        // SAFETY: `name` is NUL-terminated; `buffer` and `length` describe a writable buffer of that size.
        let status = unsafe {
            libc::sysctlbyname(
                name.as_ptr(),
                buffer.as_mut_ptr().cast(),
                &mut length,
                std::ptr::null_mut(),
                0,
            )
        };
        if status != 0 {
            return None;
        }
        let text = &buffer[..length.min(buffer.len())];
        let text = text.split(|byte| *byte == 0).next().unwrap_or_default();
        String::from_utf8(text.to_vec()).ok()
    }

    pub fn secs_since_boot() -> Option<i64> {
        let mut timebase = Timebase { numer: 0, denom: 0 };
        // SAFETY: `timebase` is a valid out pointer of the C layout.
        if unsafe { mach_timebase_info(&mut timebase) } != 0 || timebase.denom == 0 {
            return None;
        }
        // SAFETY: no arguments; reads a kernel counter.
        let ticks = unsafe { mach_continuous_time() };
        let nanos = u128::from(ticks) * u128::from(timebase.numer) / u128::from(timebase.denom);
        i64::try_from(nanos / 1_000_000_000).ok()
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
mod os {
    pub fn boot_id() -> Option<String> {
        std::fs::read_to_string("/proc/sys/kernel/random/boot_id").ok()
    }

    pub fn secs_since_boot() -> Option<i64> {
        let mut now = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // SAFETY: `now` is a valid out pointer.
        if unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut now) } != 0 {
            return None;
        }
        // `time_t` is 32 bits on 32-bit Android (armv7).
        #[allow(clippy::unnecessary_cast)]
        Some(now.tv_sec as i64)
    }
}

#[cfg(target_os = "windows")]
mod os {
    use windows_sys::Win32::System::Registry::{
        HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RegGetValueW,
    };
    use windows_sys::Win32::System::SystemInformation::GetTickCount64;

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn boot_id() -> Option<String> {
        let key = wide(
            r"SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management\PrefetchParameters",
        );
        let value = wide("BootId");
        let mut data: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        // SAFETY: `key` and `value` are NUL-terminated UTF-16; `data` and `size` describe a writable DWORD.
        let status = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                key.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_DWORD,
                std::ptr::null_mut(),
                (&mut data as *mut u32).cast(),
                &mut size,
            )
        };
        (status == 0).then(|| format!("bootid:{data}"))
    }

    pub fn secs_since_boot() -> Option<i64> {
        // SAFETY: no arguments.
        let millis = unsafe { GetTickCount64() };
        i64::try_from(millis / 1000).ok()
    }
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "linux",
    target_os = "android",
    target_os = "windows"
)))]
mod os {
    pub fn boot_id() -> Option<String> {
        None
    }

    pub fn secs_since_boot() -> Option<i64> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// On the systems the app ships on, the boot id and time since boot are read for real, and two readings a
    /// moment apart are the same boot with time moving forward. Run here on macOS; Linux and Windows run it on the
    /// Devin machines (the clock-forward proof of X3 is a separate, manual step there).
    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    #[test]
    fn the_boot_id_and_time_since_boot_are_read_here() {
        let first = read();
        let boot = first.boot.clone().expect("a boot id and time since boot");
        assert!(!boot.id.is_empty());
        assert!(boot.secs > 0, "{boot:?}");
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let second = read();
        let later = second.boot.expect("read again");
        assert_eq!(later.id, boot.id, "the same boot");
        assert!(later.secs >= boot.secs + 1, "{later:?} after {boot:?}");
        println!(
            "boot id on {}: {} ({} s since boot)",
            std::env::consts::OS,
            boot.id,
            boot.secs
        );
    }
}
