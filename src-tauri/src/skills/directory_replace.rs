use std::{
    fs,
    path::{Path, PathBuf},
};
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
use tempfile::Builder;

#[derive(Debug, Default)]
pub(super) struct ReplacementNotice {
    pub retained_backup: Option<PathBuf>,
}

#[derive(Debug)]
pub(super) struct ReplacementFailure {
    pub error: std::io::Error,
    pub retain_prepared_directory: bool,
    pub boundary_changed: bool,
}

impl ReplacementFailure {
    pub(super) fn ordinary(error: std::io::Error) -> Self {
        Self {
            error,
            retain_prepared_directory: false,
            boundary_changed: false,
        }
    }
}

pub(super) fn replace_directory_atomically<F>(
    prepared: &Path,
    destination: &Path,
    expected_revision: &str,
    revision: F,
) -> Result<ReplacementNotice, ReplacementFailure>
where
    F: Fn(&Path) -> std::io::Result<String>,
{
    replace_directory_atomically_with_finalize(
        prepared,
        destination,
        expected_revision,
        revision,
        || Ok(()),
    )
}

pub(super) fn replace_directory_atomically_with_finalize<F, C>(
    prepared: &Path,
    destination: &Path,
    expected_revision: &str,
    revision: F,
    finalize: C,
) -> Result<ReplacementNotice, ReplacementFailure>
where
    F: Fn(&Path) -> std::io::Result<String>,
    C: FnOnce() -> std::io::Result<()>,
{
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        replace_with_exchange(
            prepared,
            destination,
            expected_revision,
            revision,
            finalize,
            exchange_directories,
        )
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        replace_with_rollback(prepared, destination, expected_revision, revision, finalize)
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn replace_with_exchange<F, C, X>(
    prepared: &Path,
    destination: &Path,
    expected_revision: &str,
    revision: F,
    finalize: C,
    mut exchange: X,
) -> Result<ReplacementNotice, ReplacementFailure>
where
    F: Fn(&Path) -> std::io::Result<String>,
    C: FnOnce() -> std::io::Result<()>,
    X: FnMut(&Path, &Path) -> std::io::Result<()>,
{
    exchange(prepared, destination).map_err(ReplacementFailure::ordinary)?;
    let isolated_revision = revision(prepared);
    if !matches!(isolated_revision.as_deref(), Ok(actual) if actual == expected_revision) {
        return match exchange(prepared, destination) {
            Ok(()) => Err(ReplacementFailure {
                error: std::io::Error::other(
                    "The live Skill changed at the replacement boundary; the prior directory was restored.",
                ),
                retain_prepared_directory: false,
                boundary_changed: true,
            }),
            Err(restore_error) => Err(ReplacementFailure {
                error: std::io::Error::new(
                    restore_error.kind(),
                    format!(
                        "The live Skill changed at the replacement boundary and atomic restore failed ({restore_error}); the new version is at {} and the recovery copy is at {}.",
                        destination.display(),
                        prepared.display()
                    ),
                ),
                retain_prepared_directory: true,
                boundary_changed: true,
            }),
        };
    }
    if let Err(error) = finalize() {
        return match exchange(prepared, destination) {
            Ok(()) => Err(ReplacementFailure::ordinary(error)),
            Err(restore_error) => Err(ReplacementFailure {
                error: std::io::Error::new(
                    restore_error.kind(),
                    format!(
                        "replacement finalization failed ({error}) and atomic restore failed ({restore_error}); the new version is at {} and the recovery copy is at {}.",
                        destination.display(),
                        prepared.display()
                    ),
                ),
                retain_prepared_directory: true,
                boundary_changed: false,
            }),
        };
    }
    let retained_backup = fs::remove_dir_all(prepared)
        .err()
        .map(|_| prepared.to_path_buf());
    Ok(ReplacementNotice { retained_backup })
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn replace_with_rollback<F, C>(
    prepared: &Path,
    destination: &Path,
    expected_revision: &str,
    revision: F,
    finalize: C,
) -> Result<ReplacementNotice, ReplacementFailure>
where
    F: Fn(&Path) -> std::io::Result<String>,
    C: FnOnce() -> std::io::Result<()>,
{
    let parent = destination
        .parent()
        .ok_or_else(|| ReplacementFailure::ordinary(std::io::Error::other("missing parent")))?;
    let backup = Builder::new()
        .prefix(".skill-replaced-")
        .tempdir_in(parent)
        .map_err(ReplacementFailure::ordinary)?
        .keep();
    fs::remove_dir(&backup).map_err(ReplacementFailure::ordinary)?;
    fs::rename(destination, &backup).map_err(ReplacementFailure::ordinary)?;
    if !matches!(revision(&backup).as_deref(), Ok(actual) if actual == expected_revision) {
        return match fs::rename(&backup, destination) {
            Ok(()) => Err(ReplacementFailure {
                error: std::io::Error::other(
                    "The live Skill changed at the replacement boundary; the prior directory was restored.",
                ),
                retain_prepared_directory: false,
                boundary_changed: true,
            }),
            Err(restore_error) => Err(ReplacementFailure {
                error: restore_error,
                retain_prepared_directory: true,
                boundary_changed: true,
            }),
        };
    }
    if let Err(error) = fs::rename(prepared, destination) {
        return match fs::rename(&backup, destination) {
            Ok(()) => Err(ReplacementFailure::ordinary(error)),
            Err(restore_error) => Err(ReplacementFailure {
                error: std::io::Error::new(
                    restore_error.kind(),
                    format!("replacement failed ({error}) and restore failed ({restore_error})"),
                ),
                retain_prepared_directory: true,
                boundary_changed: false,
            }),
        };
    }
    if let Err(error) = finalize() {
        if let Err(restore_error) = fs::rename(destination, prepared) {
            return Err(ReplacementFailure {
                error: std::io::Error::new(
                    restore_error.kind(),
                    format!(
                        "replacement finalization failed ({error}) and moving the new directory aside failed ({restore_error}); the recovery copy is at {}.",
                        backup.display()
                    ),
                ),
                retain_prepared_directory: true,
                boundary_changed: false,
            });
        }
        return match fs::rename(&backup, destination) {
            Ok(()) => Err(ReplacementFailure::ordinary(error)),
            Err(restore_error) => Err(ReplacementFailure {
                error: std::io::Error::new(
                    restore_error.kind(),
                    format!(
                        "replacement finalization failed ({error}) and restoring the prior directory failed ({restore_error}); the new version is at {} and the recovery copy is at {}.",
                        prepared.display(),
                        backup.display()
                    ),
                ),
                retain_prepared_directory: true,
                boundary_changed: false,
            }),
        };
    }
    let retained_backup = fs::remove_dir_all(&backup).err().map(|_| backup);
    Ok(ReplacementNotice { retained_backup })
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn exchange_directories(left: &Path, right: &Path) -> std::io::Result<()> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};

    let left = CString::new(left.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::other("directory path contains a NUL byte"))?;
    let right = CString::new(right.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::other("directory path contains a NUL byte"))?;
    #[cfg(target_os = "macos")]
    let result = unsafe {
        libc::renameatx_np(
            libc::AT_FDCWD,
            left.as_ptr(),
            libc::AT_FDCWD,
            right.as_ptr(),
            libc::RENAME_SWAP,
        )
    };
    #[cfg(target_os = "linux")]
    let result = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            libc::AT_FDCWD,
            left.as_ptr(),
            libc::AT_FDCWD,
            right.as_ptr(),
            libc::RENAME_EXCHANGE,
        ) as libc::c_int
    };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
mod tests {
    use super::*;

    #[test]
    fn failed_restore_marks_the_isolated_prior_directory_for_recovery() {
        let directory = tempfile::tempdir().unwrap();
        let prepared = directory.path().join("prepared");
        let destination = directory.path().join("live");
        fs::create_dir(&prepared).unwrap();
        fs::create_dir(&destination).unwrap();
        fs::write(prepared.join("value"), "new").unwrap();
        fs::write(destination.join("value"), "old").unwrap();
        let mut calls = 0;

        let failure = replace_with_exchange(
            &prepared,
            &destination,
            "expected",
            |_| Ok("changed".into()),
            || Ok(()),
            |left, right| {
                calls += 1;
                if calls == 1 {
                    exchange_directories(left, right)
                } else {
                    Err(std::io::Error::other("injected restore failure"))
                }
            },
        )
        .unwrap_err();

        assert!(failure.retain_prepared_directory);
        assert_eq!(fs::read_to_string(prepared.join("value")).unwrap(), "old");
        assert_eq!(
            fs::read_to_string(destination.join("value")).unwrap(),
            "new"
        );
    }
}
