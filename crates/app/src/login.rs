//! Opening the app at login, quietly: it starts with `--hidden` and shows only its tray icon.

use anyhow::{Context as _, Result};
use auto_launch::{AutoLaunch, AutoLaunchBuilder};
use std::path::Path;

/// Passed by the login item: start without a window.
pub const HIDDEN: &str = "--hidden";

pub fn started_hidden() -> bool {
    std::env::args_os().any(|a| a == HIDDEN)
}

fn launcher() -> Result<AutoLaunch> {
    // A debug build lives in `target/`; registering it would leave a stale login item.
    if cfg!(debug_assertions) && std::env::var_os("CC_SAME_DEV_LOGIN").is_none() {
        anyhow::bail!("login items are off in debug builds (set CC_SAME_DEV_LOGIN=1)");
    }
    let exe = std::env::current_exe().context("locating the app")?;
    let exe = exe.to_str().context("the app's path is not valid Unicode")?;
    AutoLaunchBuilder::new()
        .set_app_name(crate::APP_NAME)
        .set_app_path(exe)
        .set_args(&[HIDDEN])
        .set_bundle_identifiers(&[crate::APP_ID])
        .build()
        .context("setting up the login item")
}

pub fn enabled() -> bool {
    launcher().and_then(|l| l.is_enabled().map_err(Into::into)).unwrap_or(false)
}

pub fn set(on: bool) -> Result<()> {
    let launcher = launcher()?;
    if on { launcher.enable() } else { launcher.disable() }.context("changing the login item")
}

/// Whether `path` is somewhere an app cannot stay: a disk image, or the temporary read-only
/// place macOS runs a downloaded app from until it is moved (App Translocation).
#[cfg(target_os = "macos")]
pub fn temporary_place(path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt as _;
    if path.components().any(|c| c.as_os_str() == "AppTranslocation") {
        return true;
    }
    let Ok(c_path) = std::ffi::CString::new(path.as_os_str().as_bytes()) else { return false };
    let mut volume: libc::statfs = unsafe { std::mem::zeroed() };
    let found = unsafe { libc::statfs(c_path.as_ptr(), &mut volume) } == 0;
    found && volume.f_flags & libc::MNT_RDONLY as u32 != 0
}

#[cfg(not(target_os = "macos"))]
pub fn temporary_place(_path: &Path) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn a_disk_image_or_a_translocated_app_is_a_temporary_place() {
        let translocated = "/private/var/folders/x/T/AppTranslocation/0A1B/d/CC Same.app/Contents/MacOS/CC Same";
        assert!(temporary_place(Path::new(translocated)));
        let dir = tempfile::tempdir().unwrap();
        assert!(!temporary_place(dir.path()));
    }
}
