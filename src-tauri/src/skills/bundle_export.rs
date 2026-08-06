use super::{InternalSkill, Source, Workspace};
use regex::Regex;
use serde::Serialize;
use sha2::{Digest, Sha256};
use skill_bundle_core::{
    bundle_revision, inspect_bundle, skill_revision, writable_bundle_size, write_bundle,
    AgentContract, BundleError, BundleFile, BundleFileReader, BundleManifest, BundleSkill,
    BUNDLE_FORMAT, BUNDLE_FORMAT_VERSION, CODEX_CONTRACT_ID, CODEX_CONTRACT_VERSION,
    MAX_ARCHIVE_BYTES, MAX_FILES_PER_SKILL, MAX_FILE_BYTES, MAX_PATH_DEPTH, MAX_SKILLS,
    MAX_SKILL_BYTES,
};
use std::{
    collections::HashSet,
    ffi::CString,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        LazyLock,
    },
};
#[cfg(not(unix))]
use tempfile::NamedTempFile;
use thiserror::Error;

const MAX_EXPORT_PLANS: usize = 16;
const MAX_SOURCE_ENTRIES_PER_SKILL: usize = MAX_FILES_PER_SKILL * 2;
const SECRET_SCAN_OVERLAP: usize = 512;
#[cfg(unix)]
static EXPORT_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

static PRIVATE_KEY_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)-----BEGIN (?:[A-Z0-9]+ )?PRIVATE KEY-----|-----BEGIN PGP PRIVATE KEY BLOCK-----",
    )
    .expect("private key pattern")
});
static KNOWN_TOKEN_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?:sk-[A-Za-z0-9_-]{20,}|gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|xox[baprs]-[A-Za-z0-9-]{20,}|AKIA[0-9A-Z]{16})",
    )
    .expect("known token pattern")
});
static CREDENTIAL_ASSIGNMENT_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)(?:api[_-]?key|api[_-]?token|access[_-]?token|client[_-]?secret|secret[_-]?key|password)\s*[:=]\s*["']?([A-Za-z0-9+/=_\-.]{20,})"#,
    )
    .expect("credential assignment pattern")
});

#[derive(Debug, Error)]
pub enum BundleExportError {
    #[error("Select at least one personal Skill to export.")]
    InvalidSelection,
    #[error("This export preview is missing or no longer current. Preview the export again.")]
    StalePlan,
    #[error("One or more selected Skills changed after preview. Preview the export again.")]
    SourceChanged,
    #[error("The selected destination must be a new .skillbundle file in an existing folder.")]
    InvalidDestination,
    #[error("A file or folder already exists at the selected destination.")]
    DestinationExists,
    #[error("The Skill Bundle could not be created: {0}")]
    Bundle(#[from] BundleError),
    #[error("The Skill Bundle could not be written to the selected destination.")]
    Io(#[from] std::io::Error),
}

impl BundleExportError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidSelection => "BUNDLE_EXPORT_SELECTION_INVALID",
            Self::StalePlan => "BUNDLE_EXPORT_PLAN_STALE",
            Self::SourceChanged => "BUNDLE_EXPORT_SOURCE_CHANGED",
            Self::InvalidDestination => "BUNDLE_EXPORT_DESTINATION_INVALID",
            Self::DestinationExists => "BUNDLE_EXPORT_DESTINATION_EXISTS",
            Self::Bundle(error) => error.code(),
            Self::Io(_) => "BUNDLE_EXPORT_IO_ERROR",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleExportPlan {
    pub plan_revision: String,
    pub bundle_revision: Option<String>,
    pub skills: Vec<ExportSkillPlan>,
    pub blocked: Vec<ExportBlock>,
    pub total_files: usize,
    pub total_bytes: u64,
    pub can_export: bool,
    pub unencrypted: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSkillPlan {
    pub id: String,
    pub name: String,
    pub directory_name: String,
    pub source: String,
    pub revision: Option<String>,
    pub file_count: usize,
    pub total_bytes: u64,
    pub executable_files: usize,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExportBlock {
    pub skill_id: String,
    pub skill_name: String,
    pub rule_id: String,
    pub relative_path: Option<String>,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleExportReceipt {
    pub ok: bool,
    pub destination: String,
    pub bundle_revision: String,
    pub skill_count: usize,
    pub file_count: usize,
    pub total_bytes: u64,
    pub archive_bytes: u64,
}

#[derive(Clone)]
pub(super) struct ExportPlanState {
    skills: Vec<PlannedSkill>,
}

#[derive(Clone)]
struct PlannedSkill {
    id: String,
    snapshot_revision: String,
}

struct SkillSnapshot {
    id: String,
    name: String,
    directory_name: String,
    snapshot_revision: String,
    bundle_skill: BundleSkill,
    files: Vec<SnapshotFile>,
    total_bytes: u64,
}

struct SnapshotFile {
    opened: File,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileIdentity {
    device: u64,
    inode: u64,
    len: u64,
    kind_and_execute: u32,
    modified_seconds: i64,
    modified_nanoseconds: i64,
}

#[derive(Debug)]
enum SnapshotError {
    Unsafe(Option<String>),
    Limit(Option<String>),
    Changed,
    Io,
    Bundle,
}

trait ExportObserver {
    fn after_discovery(&self, _skill_directory: &Path) {}
    fn after_first_file_chunk(&self, _skill_directory: &Path, _relative_path: &str) {}
    fn before_commit(&self, _destination: &Path) {}
    fn after_commit(&self, _destination: &Path) {}
}

struct NoopObserver;
impl ExportObserver for NoopObserver {}

impl Workspace {
    pub fn preview_bundle_export(
        &self,
        skill_ids: &[String],
    ) -> Result<BundleExportPlan, BundleExportError> {
        if skill_ids.is_empty() {
            return Err(BundleExportError::InvalidSelection);
        }
        let mut seen = HashSet::new();
        let mut snapshots = Vec::new();
        let mut skill_plans = Vec::new();
        let mut blocked = Vec::new();
        let mut scanned_bytes = 0_u64;
        let mut source_budget_exhausted = false;

        for id in skill_ids {
            if !seen.insert(id.clone()) {
                blocked.push(block(
                    id,
                    id,
                    "duplicate-selection",
                    None,
                    "This Skill was selected more than once.",
                ));
                continue;
            }
            let skill = match self.find_skill(id) {
                Ok(skill) => skill,
                Err(_) => {
                    blocked.push(block(
                        id,
                        id,
                        "skill-not-found",
                        None,
                        "This Skill is no longer in the current catalog.",
                    ));
                    continue;
                }
            };
            if skill.source != Source::Personal {
                skill_plans.push(empty_skill_plan(&skill));
                blocked.push(block(
                    id,
                    &skill.summary.name,
                    "source-not-exportable",
                    None,
                    "Only active personal Skills are exportable in Bundle v1.",
                ));
                continue;
            }
            if source_budget_exhausted || snapshots.len() >= MAX_SKILLS {
                skill_plans.push(empty_skill_plan(&skill));
                blocked.push(block(
                    id,
                    &skill.summary.name,
                    "bundle-size-limit",
                    None,
                    "The selected export exceeds the Bundle v1 archive limit.",
                ));
                continue;
            }

            match build_skill_snapshot(&skill, &NoopObserver) {
                Ok((snapshot, secret_blocks)) => {
                    if scanned_bytes
                        .checked_add(snapshot.total_bytes)
                        .is_none_or(|total| total > MAX_ARCHIVE_BYTES)
                    {
                        source_budget_exhausted = true;
                        skill_plans.push(empty_skill_plan(&skill));
                        blocked.push(block(
                            id,
                            &skill.summary.name,
                            "bundle-size-limit",
                            None,
                            "The selected export exceeds the Bundle v1 archive limit.",
                        ));
                        continue;
                    }
                    scanned_bytes += snapshot.total_bytes;
                    skill_plans.push(if secret_blocks.is_empty() {
                        snapshot_plan(&snapshot)
                    } else {
                        redacted_snapshot_plan(&snapshot)
                    });
                    blocked.extend(secret_blocks.into_iter().map(|finding| {
                        block(
                            &snapshot.id,
                            &snapshot.name,
                            finding.rule_id,
                            Some(finding.path),
                            finding.message,
                        )
                    }));
                    snapshots.push(snapshot);
                }
                Err(error) => {
                    skill_plans.push(empty_skill_plan(&skill));
                    blocked.push(snapshot_block(&skill, error));
                }
            }
        }

        skill_plans.sort_by(|left, right| {
            left.directory_name
                .as_bytes()
                .cmp(right.directory_name.as_bytes())
        });
        snapshots.sort_by(|left, right| {
            left.directory_name
                .as_bytes()
                .cmp(right.directory_name.as_bytes())
        });
        let total_files = snapshots.iter().map(|skill| skill.files.len()).sum();
        let total_bytes = snapshots.iter().map(|skill| skill.total_bytes).sum();
        if blocked.is_empty() && snapshots.len() == skill_ids.len() {
            let manifest = manifest_for_snapshots(&snapshots);
            if writable_bundle_size(&manifest).is_err() {
                blocked.push(block(
                    "bundle",
                    "Skill Bundle",
                    "bundle-size-limit",
                    None,
                    "The selected export exceeds the Bundle v1 archive limit.",
                ));
            }
        }
        let can_export = blocked.is_empty() && snapshots.len() == skill_ids.len();
        let plan_revision = if can_export {
            export_plan_revision(&snapshots, &blocked)
        } else {
            export_plan_revision(&[], &blocked)
        };
        let bundle_revision_value = if can_export {
            let manifest = manifest_for_snapshots(&snapshots);
            Some(bundle_revision(&manifest)?)
        } else {
            None
        };
        if can_export {
            let state = ExportPlanState {
                skills: snapshots
                    .iter()
                    .map(|snapshot| PlannedSkill {
                        id: snapshot.id.clone(),
                        snapshot_revision: snapshot.snapshot_revision.clone(),
                    })
                    .collect(),
            };
            let mut plans = self
                .export_plans
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if plans.len() >= MAX_EXPORT_PLANS {
                plans.clear();
            }
            plans.insert(plan_revision.clone(), state);
        }
        Ok(BundleExportPlan {
            plan_revision,
            bundle_revision: bundle_revision_value,
            skills: skill_plans,
            blocked,
            total_files,
            total_bytes,
            can_export,
            unencrypted: true,
        })
    }

    pub fn export_skill_bundle(
        &self,
        expected_plan_revision: &str,
        destination: &Path,
    ) -> Result<BundleExportReceipt, BundleExportError> {
        self.export_skill_bundle_observed(expected_plan_revision, destination, &NoopObserver)
    }

    fn export_skill_bundle_observed(
        &self,
        expected_plan_revision: &str,
        destination: &Path,
        observer: &dyn ExportObserver,
    ) -> Result<BundleExportReceipt, BundleExportError> {
        let plan = self
            .export_plans
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(expected_plan_revision)
            .cloned()
            .ok_or(BundleExportError::StalePlan)?;
        let destination = validated_destination(destination)?;
        let _mutation = self
            .mutations
            .lock()
            .unwrap_or_else(|error| error.into_inner());

        let mut snapshots = Vec::with_capacity(plan.skills.len());
        for planned in &plan.skills {
            let skill = self
                .find_skill(&planned.id)
                .map_err(|_| BundleExportError::SourceChanged)?;
            if skill.source != Source::Personal {
                return Err(BundleExportError::SourceChanged);
            }
            let (snapshot, secrets) = build_skill_snapshot(&skill, observer)
                .map_err(|_| BundleExportError::SourceChanged)?;
            if !secrets.is_empty() || snapshot.snapshot_revision != planned.snapshot_revision {
                return Err(BundleExportError::SourceChanged);
            }
            snapshots.push(snapshot);
        }
        snapshots.sort_by(|left, right| {
            left.directory_name
                .as_bytes()
                .cmp(right.directory_name.as_bytes())
        });
        let manifest = manifest_for_snapshots(&snapshots);
        let expected_bundle_revision = bundle_revision(&manifest)?;
        let parent = destination
            .parent()
            .ok_or(BundleExportError::InvalidDestination)?;
        let mut temporary = ExportTemporary::new(parent)?;
        let mut readers = snapshots
            .iter_mut()
            .flat_map(|snapshot| snapshot.files.iter_mut())
            .map(|file| {
                file.opened.seek(SeekFrom::Start(0))?;
                Ok(BundleFileReader {
                    reader: &mut file.opened,
                })
            })
            .collect::<Result<Vec<_>, std::io::Error>>()?;
        write_bundle(&mut temporary, &manifest, &mut readers).map_err(|error| match error {
            BundleError::HashMismatch | BundleError::SizeMismatch => {
                BundleExportError::SourceChanged
            }
            other => BundleExportError::Bundle(other),
        })?;
        drop(readers);
        temporary.flush()?;
        temporary.sync_all()?;
        temporary.seek(SeekFrom::Start(0))?;
        let inspection = inspect_bundle(&mut temporary)?;
        if inspection.manifest != manifest || inspection.bundle_revision != expected_bundle_revision
        {
            return Err(BundleExportError::SourceChanged);
        }

        for (planned, prior) in plan.skills.iter().zip(&snapshots) {
            let skill = self
                .find_skill(&planned.id)
                .map_err(|_| BundleExportError::SourceChanged)?;
            let (current, secrets) = build_skill_snapshot(&skill, observer)
                .map_err(|_| BundleExportError::SourceChanged)?;
            if !secrets.is_empty()
                || current.snapshot_revision != planned.snapshot_revision
                || current.snapshot_revision != prior.snapshot_revision
            {
                return Err(BundleExportError::SourceChanged);
            }
        }

        observer.before_commit(&destination);
        if fs::symlink_metadata(&destination).is_ok() {
            return Err(BundleExportError::DestinationExists);
        }
        let archive_bytes = temporary.metadata()?.len();
        let temporary_identity = file_identity(&temporary.metadata()?);
        let mut committed = temporary.commit_noclobber(&destination)?;
        observer.after_commit(&destination);
        if file_identity(&committed.metadata()?) != temporary_identity {
            return Err(BundleExportError::SourceChanged);
        }
        committed.seek(SeekFrom::Start(0))?;
        let committed_inspection = inspect_bundle(&mut committed)?;
        if committed_inspection.manifest != manifest
            || committed_inspection.bundle_revision != expected_bundle_revision
        {
            return Err(BundleExportError::SourceChanged);
        }
        let destination_metadata = fs::symlink_metadata(&destination)?;
        if destination_metadata.file_type().is_symlink()
            || file_identity(&destination_metadata) != temporary_identity
        {
            return Err(BundleExportError::SourceChanged);
        }
        temporary.verify_parent_path()?;
        temporary.keep_committed();
        temporary.sync_parent();
        self.export_plans
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(expected_plan_revision);
        Ok(BundleExportReceipt {
            ok: true,
            destination: destination.display().to_string(),
            bundle_revision: expected_bundle_revision,
            skill_count: manifest.skills.len(),
            file_count: inspection.total_files,
            total_bytes: inspection.total_bytes,
            archive_bytes,
        })
    }
}

#[cfg(unix)]
struct ExportTemporary {
    parent: File,
    parent_path: PathBuf,
    file: File,
    cleanup_name: Option<CString>,
    cleanup_identity: Option<FileIdentity>,
}

#[cfg(unix)]
impl ExportTemporary {
    fn new(parent_path: &Path) -> Result<Self, BundleExportError> {
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

        let path_metadata =
            fs::symlink_metadata(parent_path).map_err(|_| BundleExportError::InvalidDestination)?;
        if path_metadata.file_type().is_symlink() || !path_metadata.is_dir() {
            return Err(BundleExportError::InvalidDestination);
        }
        let mut options = OpenOptions::new();
        options.read(true);
        options.custom_flags(libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW);
        let parent = options
            .open(parent_path)
            .map_err(|_| BundleExportError::InvalidDestination)?;
        let opened_metadata = parent
            .metadata()
            .map_err(|_| BundleExportError::InvalidDestination)?;
        if (path_metadata.dev(), path_metadata.ino())
            != (opened_metadata.dev(), opened_metadata.ino())
        {
            return Err(BundleExportError::InvalidDestination);
        }

        let mut last_error = None;
        for _ in 0..128 {
            let sequence = EXPORT_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let name = CString::new(format!(
                ".agent-skill-studio-export-{}-{sequence}",
                std::process::id()
            ))
            .expect("generated export temporary name has no NUL");
            match create_export_file_at(&parent, &name) {
                Ok(file) => {
                    let identity = file_identity(&file.metadata()?);
                    let temporary = Self {
                        parent,
                        parent_path: parent_path.to_path_buf(),
                        file,
                        cleanup_name: Some(name),
                        cleanup_identity: Some(identity),
                    };
                    temporary.verify_parent_path()?;
                    return Ok(temporary);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    last_error = Some(error);
                }
                Err(error) => return Err(BundleExportError::Io(error)),
            }
        }
        Err(BundleExportError::Io(last_error.unwrap_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "could not reserve an export temporary file",
            )
        })))
    }

    fn sync_all(&self) -> std::io::Result<()> {
        self.file.sync_all()
    }

    fn metadata(&self) -> std::io::Result<fs::Metadata> {
        self.file.metadata()
    }

    fn verify_parent_path(&self) -> Result<(), BundleExportError> {
        use std::os::unix::fs::MetadataExt;

        let path_metadata = fs::symlink_metadata(&self.parent_path)
            .map_err(|_| BundleExportError::InvalidDestination)?;
        let opened_metadata = self
            .parent
            .metadata()
            .map_err(|_| BundleExportError::InvalidDestination)?;
        if path_metadata.file_type().is_symlink()
            || !path_metadata.is_dir()
            || (path_metadata.dev(), path_metadata.ino())
                != (opened_metadata.dev(), opened_metadata.ino())
        {
            return Err(BundleExportError::InvalidDestination);
        }
        Ok(())
    }

    fn commit_noclobber(&mut self, destination: &Path) -> Result<File, BundleExportError> {
        use std::os::{fd::AsRawFd, unix::io::FromRawFd, unix::prelude::OsStrExt};

        self.verify_parent_path()?;
        let temporary_name = self
            .cleanup_name
            .as_ref()
            .ok_or(BundleExportError::SourceChanged)?;
        let temporary_identity = self
            .cleanup_identity
            .as_ref()
            .ok_or(BundleExportError::SourceChanged)?;
        let destination_name = CString::new(
            destination
                .file_name()
                .ok_or(BundleExportError::InvalidDestination)?
                .as_bytes(),
        )
        .map_err(|_| BundleExportError::InvalidDestination)?;
        let result = unsafe {
            libc::linkat(
                self.parent.as_raw_fd(),
                temporary_name.as_ptr(),
                self.parent.as_raw_fd(),
                destination_name.as_ptr(),
                0,
            )
        };
        if result != 0 {
            let error = std::io::Error::last_os_error();
            return if error.kind() == std::io::ErrorKind::AlreadyExists {
                Err(BundleExportError::DestinationExists)
            } else {
                Err(BundleExportError::Io(error))
            };
        }
        unlink_export_file_at_if_identity(&self.parent, temporary_name, temporary_identity);
        self.cleanup_name = Some(destination_name.clone());
        let descriptor = unsafe {
            libc::openat(
                self.parent.as_raw_fd(),
                destination_name.as_ptr(),
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if descriptor < 0 {
            return Err(BundleExportError::Io(std::io::Error::last_os_error()));
        }
        Ok(unsafe { File::from_raw_fd(descriptor) })
    }

    fn keep_committed(&mut self) {
        self.cleanup_name = None;
        self.cleanup_identity = None;
    }

    fn sync_parent(&self) {
        let _ = self.parent.sync_all();
    }
}

#[cfg(unix)]
impl Drop for ExportTemporary {
    fn drop(&mut self) {
        if let Some(name) = &self.cleanup_name {
            if let Some(identity) = &self.cleanup_identity {
                unlink_export_file_at_if_identity(&self.parent, name, identity);
            }
        }
    }
}

#[cfg(unix)]
impl Write for ExportTemporary {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.file.write(buffer)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

#[cfg(unix)]
impl Read for ExportTemporary {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.file.read(buffer)
    }
}

#[cfg(unix)]
impl Seek for ExportTemporary {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.file.seek(position)
    }
}

#[cfg(unix)]
fn create_export_file_at(parent: &File, name: &CString) -> std::io::Result<File> {
    use std::os::{fd::AsRawFd, unix::io::FromRawFd};

    let descriptor = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDWR | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
    };
    if descriptor < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(unsafe { File::from_raw_fd(descriptor) })
    }
}

#[cfg(unix)]
fn unlink_export_file_at_if_identity(parent: &File, name: &CString, identity: &FileIdentity) {
    use std::os::fd::AsRawFd;

    let mut metadata = std::mem::MaybeUninit::<libc::stat>::uninit();
    let inspected = unsafe {
        libc::fstatat(
            parent.as_raw_fd(),
            name.as_ptr(),
            metadata.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if inspected != 0 {
        return;
    }
    let metadata = unsafe { metadata.assume_init() };
    if metadata.st_dev as u64 != identity.device || metadata.st_ino != identity.inode {
        return;
    }
    unsafe {
        libc::unlinkat(parent.as_raw_fd(), name.as_ptr(), 0);
    }
}

#[cfg(not(unix))]
struct ExportTemporary {
    inner: Option<NamedTempFile>,
    parent: PathBuf,
}

#[cfg(not(unix))]
impl ExportTemporary {
    fn new(parent: &Path) -> Result<Self, BundleExportError> {
        Ok(Self {
            inner: Some(NamedTempFile::new_in(parent)?),
            parent: parent.to_path_buf(),
        })
    }

    fn sync_all(&self) -> std::io::Result<()> {
        self.inner
            .as_ref()
            .expect("temporary exists")
            .as_file()
            .sync_all()
    }

    fn metadata(&self) -> std::io::Result<fs::Metadata> {
        self.inner
            .as_ref()
            .expect("temporary exists")
            .as_file()
            .metadata()
    }

    fn verify_parent_path(&self) -> Result<(), BundleExportError> {
        Ok(())
    }

    fn commit_noclobber(&mut self, destination: &Path) -> Result<File, BundleExportError> {
        self.inner
            .take()
            .expect("temporary exists")
            .persist_noclobber(destination)
            .map_err(|error| {
                if error.error.kind() == std::io::ErrorKind::AlreadyExists {
                    BundleExportError::DestinationExists
                } else {
                    BundleExportError::Io(error.error)
                }
            })
    }

    fn keep_committed(&mut self) {}

    fn sync_parent(&self) {
        if let Ok(directory) = File::open(&self.parent) {
            let _ = directory.sync_all();
        }
    }
}

#[cfg(not(unix))]
impl Write for ExportTemporary {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.inner.as_mut().expect("temporary exists").write(buffer)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.as_mut().expect("temporary exists").flush()
    }
}

#[cfg(not(unix))]
impl Read for ExportTemporary {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.inner.as_mut().expect("temporary exists").read(buffer)
    }
}

#[cfg(not(unix))]
impl Seek for ExportTemporary {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.inner
            .as_mut()
            .expect("temporary exists")
            .seek(position)
    }
}

struct SecretFinding {
    rule_id: &'static str,
    path: String,
    message: &'static str,
}

fn build_skill_snapshot(
    skill: &InternalSkill,
    observer: &dyn ExportObserver,
) -> Result<(SkillSnapshot, Vec<SecretFinding>), SnapshotError> {
    if skill.source != Source::Personal {
        return Err(SnapshotError::Unsafe(None));
    }
    let directory_name = skill.summary.directory_name.clone();
    if !matches!(
        Path::new(&directory_name)
            .components()
            .collect::<Vec<_>>()
            .as_slice(),
        [Component::Normal(_)]
    ) {
        return Err(SnapshotError::Unsafe(None));
    }
    let root_metadata = fs::symlink_metadata(&skill.root).map_err(|_| SnapshotError::Io)?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(SnapshotError::Unsafe(None));
    }
    let root = open_directory_nofollow(&skill.root)?;
    let root_identity = file_identity(&root.metadata().map_err(|_| SnapshotError::Io)?);
    if root_identity != file_identity(&root_metadata) {
        return Err(SnapshotError::Changed);
    }
    let directory = openat_directory(&root, &directory_name)?;
    let directory_identity = file_identity(&directory.metadata().map_err(|_| SnapshotError::Io)?);
    let path_metadata = fs::symlink_metadata(&skill.directory).map_err(|_| SnapshotError::Io)?;
    if path_metadata.file_type().is_symlink()
        || !path_metadata.is_dir()
        || directory_identity != file_identity(&path_metadata)
    {
        return Err(SnapshotError::Changed);
    }

    let mut directories = Vec::new();
    let mut paths = Vec::new();
    let mut entry_count = 0;
    let mut discovered_bytes = 0_u64;
    collect_paths(
        &skill.directory,
        &skill.directory,
        0,
        &mut directories,
        &mut paths,
        &mut entry_count,
        &mut discovered_bytes,
    )?;
    observer.after_discovery(&skill.directory);
    paths.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    directories.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    if !paths.iter().any(|path| path == "SKILL.md") {
        return Err(SnapshotError::Unsafe(Some("SKILL.md".into())));
    }

    let mut bundle_files = Vec::with_capacity(paths.len());
    let mut snapshot_files = Vec::with_capacity(paths.len());
    let mut secret_findings = Vec::new();
    let mut total_bytes = 0_u64;
    for relative in &paths {
        if is_credential_path(relative) {
            secret_findings.push(SecretFinding {
                rule_id: "credential-path",
                path: relative.clone(),
                message: "A filename or folder is reserved for credential material.",
            });
        }
        let mut opened = openat_file(&directory, relative)?;
        let before = opened.metadata().map_err(|_| SnapshotError::Io)?;
        if !before.is_file() || before.len() > MAX_FILE_BYTES {
            return Err(if before.len() > MAX_FILE_BYTES {
                SnapshotError::Limit(Some(relative.clone()))
            } else {
                SnapshotError::Unsafe(Some(relative.clone()))
            });
        }
        let identity = file_identity(&before);
        let executable = is_executable(&before);
        let (size, sha256, rules) =
            scan_opened_file(&mut opened, &skill.directory, relative, observer)?;
        if file_identity(&opened.metadata().map_err(|_| SnapshotError::Io)?) != identity {
            return Err(SnapshotError::Changed);
        }
        for rule_id in rules {
            let message = match rule_id {
                "private-key-material" => "A private-key block was found in this file.",
                "known-token-format" => {
                    "A value matching a known credential token format was found."
                }
                _ => "A high-confidence credential assignment was found.",
            };
            secret_findings.push(SecretFinding {
                rule_id,
                path: relative.clone(),
                message,
            });
        }
        total_bytes = total_bytes
            .checked_add(size)
            .ok_or_else(|| SnapshotError::Limit(Some(relative.clone())))?;
        bundle_files.push(BundleFile {
            path: relative.clone(),
            size,
            sha256,
            executable,
        });
        opened
            .seek(SeekFrom::Start(0))
            .map_err(|_| SnapshotError::Io)?;
        snapshot_files.push(SnapshotFile { opened });
    }

    let mut final_directories = Vec::new();
    let mut final_paths = Vec::new();
    let mut final_entry_count = 0;
    let mut final_discovered_bytes = 0_u64;
    collect_paths(
        &skill.directory,
        &skill.directory,
        0,
        &mut final_directories,
        &mut final_paths,
        &mut final_entry_count,
        &mut final_discovered_bytes,
    )?;
    final_directories.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    final_paths.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    let final_metadata = fs::symlink_metadata(&skill.directory).map_err(|_| SnapshotError::Io)?;
    if directories != final_directories
        || paths != final_paths
        || file_identity(&final_metadata) != directory_identity
    {
        return Err(SnapshotError::Changed);
    }
    secret_findings.sort_by(|left, right| {
        left.path
            .as_bytes()
            .cmp(right.path.as_bytes())
            .then_with(|| left.rule_id.cmp(right.rule_id))
    });
    secret_findings
        .dedup_by(|left, right| left.path == right.path && left.rule_id == right.rule_id);
    let revision = skill_revision(&bundle_files).map_err(|_| SnapshotError::Bundle)?;
    let snapshot_revision = complete_snapshot_revision(&directories, &bundle_files);
    Ok((
        SkillSnapshot {
            id: skill.summary.id.clone(),
            name: skill.summary.name.clone(),
            directory_name: directory_name.clone(),
            snapshot_revision,
            bundle_skill: BundleSkill {
                directory_name,
                revision,
                files: bundle_files,
            },
            files: snapshot_files,
            total_bytes,
        },
        secret_findings,
    ))
}

fn collect_paths(
    root: &Path,
    current: &Path,
    depth: usize,
    directories: &mut Vec<String>,
    files: &mut Vec<String>,
    entry_count: &mut usize,
    discovered_bytes: &mut u64,
) -> Result<(), SnapshotError> {
    if depth > MAX_PATH_DEPTH {
        return Err(SnapshotError::Limit(None));
    }
    for entry in fs::read_dir(current).map_err(|_| SnapshotError::Io)? {
        let entry = entry.map_err(|_| SnapshotError::Io)?;
        *entry_count = entry_count
            .checked_add(1)
            .ok_or(SnapshotError::Limit(None))?;
        if *entry_count > MAX_SOURCE_ENTRIES_PER_SKILL {
            return Err(SnapshotError::Limit(None));
        }
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .map_err(|_| SnapshotError::Unsafe(None))?
            .to_str()
            .ok_or(SnapshotError::Unsafe(None))?
            .replace('\\', "/");
        let metadata = fs::symlink_metadata(&path).map_err(|_| SnapshotError::Changed)?;
        if metadata.file_type().is_symlink() {
            return Err(SnapshotError::Unsafe(Some(relative)));
        }
        if metadata.is_dir() {
            directories.push(relative);
            collect_paths(
                root,
                &path,
                depth + 1,
                directories,
                files,
                entry_count,
                discovered_bytes,
            )?;
        } else if metadata.is_file() {
            if metadata.len() > MAX_FILE_BYTES || files.len() >= MAX_FILES_PER_SKILL {
                return Err(SnapshotError::Limit(Some(relative)));
            }
            *discovered_bytes = discovered_bytes
                .checked_add(metadata.len())
                .ok_or_else(|| SnapshotError::Limit(Some(relative.clone())))?;
            if *discovered_bytes > MAX_SKILL_BYTES {
                return Err(SnapshotError::Limit(Some(relative)));
            }
            files.push(relative);
        } else {
            return Err(SnapshotError::Unsafe(Some(relative)));
        }
    }
    Ok(())
}

fn scan_opened_file(
    file: &mut File,
    skill_directory: &Path,
    relative: &str,
    observer: &dyn ExportObserver,
) -> Result<(u64, String, Vec<&'static str>), SnapshotError> {
    file.seek(SeekFrom::Start(0))
        .map_err(|_| SnapshotError::Io)?;
    let mut digest = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 16 * 1024];
    let mut overlap = Vec::new();
    let mut rules = HashSet::new();
    let mut observed = false;
    loop {
        let count = file.read(&mut buffer).map_err(|_| SnapshotError::Changed)?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or_else(|| SnapshotError::Limit(Some(relative.into())))?;
        if total > MAX_FILE_BYTES {
            return Err(SnapshotError::Limit(Some(relative.into())));
        }
        digest.update(&buffer[..count]);
        let mut window = Vec::with_capacity(overlap.len() + count);
        window.extend_from_slice(&overlap);
        window.extend_from_slice(&buffer[..count]);
        scan_secret_window(&window, &mut rules);
        let keep = window.len().min(SECRET_SCAN_OVERLAP);
        overlap.clear();
        overlap.extend_from_slice(&window[window.len() - keep..]);
        if !observed {
            observed = true;
            observer.after_first_file_chunk(skill_directory, relative);
        }
    }
    let mut rules = rules.into_iter().collect::<Vec<_>>();
    rules.sort_unstable();
    Ok((total, format!("{:x}", digest.finalize()), rules))
}

fn scan_secret_window(bytes: &[u8], rules: &mut HashSet<&'static str>) {
    let text = String::from_utf8_lossy(bytes);
    if PRIVATE_KEY_PATTERN.is_match(&text) {
        rules.insert("private-key-material");
    }
    if KNOWN_TOKEN_PATTERN.is_match(&text) {
        rules.insert("known-token-format");
    }
    for captures in CREDENTIAL_ASSIGNMENT_PATTERN.captures_iter(&text) {
        if captures
            .get(1)
            .is_some_and(|value| high_confidence_credential(value.as_str()))
        {
            rules.insert("credential-assignment");
            break;
        }
    }
}

fn high_confidence_credential(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if [
        "placeholder",
        "example",
        "replace",
        "redacted",
        "changeme",
        "your_api",
        "your_token",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
    {
        return false;
    }
    value.bytes().collect::<HashSet<_>>().len() >= 6
}

fn is_credential_path(path: &str) -> bool {
    let components = path
        .split('/')
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    components.iter().any(|component| {
        matches!(
            component.as_str(),
            ".ssh"
                | ".aws"
                | ".azure"
                | ".gnupg"
                | ".npmrc"
                | ".pypirc"
                | ".netrc"
                | "credentials"
                | "credentials.json"
                | "secrets.json"
                | "service-account.json"
                | "id_rsa"
                | "id_ed25519"
                | "id_ecdsa"
                | "id_dsa"
        ) || component == ".env"
            || component.starts_with(".env.")
    })
}

fn open_directory_nofollow(path: &Path) -> Result<File, SnapshotError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW);
    }
    options.open(path).map_err(|_| SnapshotError::Unsafe(None))
}

#[cfg(unix)]
fn openat_directory(parent: &File, name: &str) -> Result<File, SnapshotError> {
    openat(
        parent,
        name,
        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW,
    )
}

#[cfg(not(unix))]
fn openat_directory(_parent: &File, _name: &str) -> Result<File, SnapshotError> {
    Err(SnapshotError::Unsafe(None))
}

#[cfg(unix)]
fn openat_file(root: &File, relative: &str) -> Result<File, SnapshotError> {
    let components = Path::new(relative).components().collect::<Vec<_>>();
    if components.is_empty()
        || components
            .iter()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(SnapshotError::Unsafe(Some(relative.into())));
    }
    let mut current: Option<File> = None;
    for (index, component) in components.iter().enumerate() {
        let name = match component {
            Component::Normal(value) => value
                .to_str()
                .ok_or_else(|| SnapshotError::Unsafe(Some(relative.into())))?,
            _ => return Err(SnapshotError::Unsafe(Some(relative.into()))),
        };
        let parent = current.as_ref().unwrap_or(root);
        let last = index + 1 == components.len();
        let opened = openat(
            parent,
            name,
            if last {
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK
            } else {
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW
            },
        )?;
        current = Some(opened);
    }
    current.ok_or_else(|| SnapshotError::Unsafe(Some(relative.into())))
}

#[cfg(not(unix))]
fn openat_file(_root: &File, relative: &str) -> Result<File, SnapshotError> {
    Err(SnapshotError::Unsafe(Some(relative.into())))
}

#[cfg(unix)]
fn openat(parent: &File, name: &str, flags: i32) -> Result<File, SnapshotError> {
    use std::os::{fd::AsRawFd, unix::io::FromRawFd};
    let name = CString::new(name.as_bytes()).map_err(|_| SnapshotError::Unsafe(None))?;
    let descriptor =
        unsafe { libc::openat(parent.as_raw_fd(), name.as_ptr(), flags | libc::O_CLOEXEC) };
    if descriptor < 0 {
        return Err(SnapshotError::Changed);
    }
    Ok(unsafe { File::from_raw_fd(descriptor) })
}

fn file_identity(metadata: &fs::Metadata) -> FileIdentity {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        FileIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
            len: metadata.len(),
            kind_and_execute: metadata.mode() & (u32::from(libc::S_IFMT) | 0o111),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
        }
    }
    #[cfg(not(unix))]
    {
        FileIdentity {
            device: 0,
            inode: 0,
            len: metadata.len(),
            kind_and_execute: 0,
            modified_seconds: 0,
            modified_nanoseconds: 0,
        }
    }
}

fn is_executable(metadata: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        metadata.mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        false
    }
}

fn complete_snapshot_revision(directories: &[String], files: &[BundleFile]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"ASS-EXPORT-SNAPSHOT\0");
    for directory in directories {
        hash_framed_string(&mut digest, directory);
        digest.update([b'd']);
    }
    for file in files {
        hash_framed_string(&mut digest, &file.path);
        digest.update([b'f']);
        digest.update(file.size.to_be_bytes());
        digest.update(file.sha256.as_bytes());
        digest.update([u8::from(file.executable)]);
    }
    format!("{:x}", digest.finalize())
}

fn export_plan_revision(snapshots: &[SkillSnapshot], blocked: &[ExportBlock]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"ASS-EXPORT-PLAN\0");
    for snapshot in snapshots {
        hash_framed_string(&mut digest, &snapshot.id);
        hash_framed_string(&mut digest, &snapshot.snapshot_revision);
    }
    for finding in blocked {
        hash_framed_string(&mut digest, &finding.skill_id);
        hash_framed_string(&mut digest, &finding.rule_id);
        hash_framed_string(
            &mut digest,
            finding.relative_path.as_deref().unwrap_or_default(),
        );
    }
    format!("{:x}", digest.finalize())
}

fn hash_framed_string(digest: &mut Sha256, value: &str) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value.as_bytes());
}

fn manifest_for_snapshots(snapshots: &[SkillSnapshot]) -> BundleManifest {
    BundleManifest {
        format: BUNDLE_FORMAT.into(),
        format_version: BUNDLE_FORMAT_VERSION,
        agent_contract: AgentContract {
            id: CODEX_CONTRACT_ID.into(),
            version: CODEX_CONTRACT_VERSION,
        },
        skills: snapshots
            .iter()
            .map(|snapshot| snapshot.bundle_skill.clone())
            .collect(),
    }
}

fn snapshot_plan(snapshot: &SkillSnapshot) -> ExportSkillPlan {
    ExportSkillPlan {
        id: snapshot.id.clone(),
        name: snapshot.name.clone(),
        directory_name: snapshot.directory_name.clone(),
        source: "personal".into(),
        revision: Some(snapshot.bundle_skill.revision.clone()),
        file_count: snapshot.files.len(),
        total_bytes: snapshot.total_bytes,
        executable_files: snapshot
            .bundle_skill
            .files
            .iter()
            .filter(|file| file.executable)
            .count(),
    }
}

fn redacted_snapshot_plan(snapshot: &SkillSnapshot) -> ExportSkillPlan {
    ExportSkillPlan {
        revision: None,
        ..snapshot_plan(snapshot)
    }
}

fn empty_skill_plan(skill: &InternalSkill) -> ExportSkillPlan {
    ExportSkillPlan {
        id: skill.summary.id.clone(),
        name: skill.summary.name.clone(),
        directory_name: skill.summary.directory_name.clone(),
        source: skill.source.label().into(),
        revision: None,
        file_count: 0,
        total_bytes: 0,
        executable_files: 0,
    }
}

fn snapshot_block(skill: &InternalSkill, error: SnapshotError) -> ExportBlock {
    let (rule, path, message) = match error {
        SnapshotError::Unsafe(path) => (
            "unsafe-entry",
            path,
            "A linked, special, escaped, or unsupported entry prevents export.",
        ),
        SnapshotError::Limit(path) => (
            "resource-limit",
            path,
            "This Skill exceeds a Bundle v1 file, size, or depth limit.",
        ),
        SnapshotError::Changed => (
            "source-changed",
            None,
            "The Skill changed while it was being inspected. Try previewing again.",
        ),
        SnapshotError::Io | SnapshotError::Bundle => (
            "source-unreadable",
            None,
            "The Skill could not be read as a valid Bundle v1 source.",
        ),
    };
    block(&skill.summary.id, &skill.summary.name, rule, path, message)
}

fn block(
    skill_id: &str,
    skill_name: &str,
    rule_id: &str,
    relative_path: Option<String>,
    message: &str,
) -> ExportBlock {
    ExportBlock {
        skill_id: skill_id.into(),
        skill_name: skill_name.into(),
        rule_id: rule_id.into(),
        relative_path,
        message: message.into(),
    }
}

fn validated_destination(destination: &Path) -> Result<PathBuf, BundleExportError> {
    if !destination.is_absolute()
        || destination.extension().and_then(|value| value.to_str()) != Some("skillbundle")
    {
        return Err(BundleExportError::InvalidDestination);
    }
    let file_name = destination
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or(BundleExportError::InvalidDestination)?;
    if destination
        .file_name()
        .and_then(|_| destination.components().next_back())
        .is_none_or(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(BundleExportError::InvalidDestination);
    }
    let parent = destination
        .parent()
        .ok_or(BundleExportError::InvalidDestination)?;
    let parent_metadata =
        fs::symlink_metadata(parent).map_err(|_| BundleExportError::InvalidDestination)?;
    if !parent_metadata.is_dir() {
        return Err(BundleExportError::InvalidDestination);
    }
    let parent = fs::canonicalize(parent).map_err(|_| BundleExportError::InvalidDestination)?;
    let destination = parent.join(file_name);
    if fs::symlink_metadata(&destination).is_ok() {
        return Err(BundleExportError::DestinationExists);
    }
    Ok(destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn workspace() -> (tempfile::TempDir, Workspace) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let workspace = Workspace::new(directory.path().to_path_buf());
        (directory, workspace)
    }

    fn markdown(name: &str) -> String {
        format!(
            "---\nname: {name}\ndescription: Use when testing Bundle export.\n---\n\n# {name}\n"
        )
    }

    fn write_skill(root: &Path, relative: &str, name: &str) -> PathBuf {
        let directory = root.join(relative);
        fs::create_dir_all(directory.join("scripts")).expect("skill directory");
        fs::write(directory.join("SKILL.md"), markdown(name)).expect("skill document");
        fs::write(directory.join("scripts/helper.sh"), "echo helper\n").expect("helper");
        directory
    }

    fn id_for(workspace: &Workspace, source: &str, name: &str) -> String {
        workspace
            .list_skills()
            .expect("catalog")
            .skills
            .into_iter()
            .find(|skill| skill.source == source && skill.name == name)
            .expect("skill")
            .id
    }

    #[test]
    fn preview_and_export_create_a_verified_bundle_without_touching_sources() {
        let (directory, workspace) = workspace();
        let skill_directory = write_skill(directory.path(), "skills/demo", "demo");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                skill_directory.join("scripts/helper.sh"),
                fs::Permissions::from_mode(0o755),
            )
            .expect("executable");
        }
        let id = id_for(&workspace, "personal", "demo");
        let plan = workspace.preview_bundle_export(&[id]).expect("preview");
        assert!(plan.can_export);
        assert!(plan.blocked.is_empty());
        assert_eq!(plan.total_files, 2);
        assert_eq!(plan.skills[0].executable_files, 1);
        let before = fs::read(skill_directory.join("SKILL.md")).unwrap();
        let destination = directory.path().join("demo.skillbundle");
        let receipt = workspace
            .export_skill_bundle(&plan.plan_revision, &destination)
            .expect("export");
        assert_eq!(receipt.skill_count, 1);
        assert_eq!(fs::read(skill_directory.join("SKILL.md")).unwrap(), before);
        let mut file = File::open(destination).expect("bundle");
        let inspection = inspect_bundle(&mut file).expect("verified bundle");
        assert_eq!(inspection.bundle_revision, receipt.bundle_revision);
        assert!(inspection.manifest.skills[0].files[1].executable);
        let metrics = workspace.metrics_snapshot();
        assert_eq!(metrics.full_scans, 1);
    }

    #[test]
    fn preview_blocks_non_personal_duplicate_unsafe_and_secret_sources() {
        let (directory, workspace) = workspace();
        write_skill(directory.path(), "skills/personal", "personal");
        write_skill(directory.path(), "skills/.system/managed", "managed");
        let personal = id_for(&workspace, "personal", "personal");
        let managed = id_for(&workspace, "system", "managed");
        let duplicate = workspace
            .preview_bundle_export(&[personal.clone(), personal])
            .expect("duplicate preview");
        assert!(!duplicate.can_export);
        assert!(duplicate
            .blocked
            .iter()
            .any(|finding| finding.rule_id == "duplicate-selection"));
        let managed = workspace
            .preview_bundle_export(&[managed])
            .expect("managed preview");
        assert!(managed
            .blocked
            .iter()
            .any(|finding| finding.rule_id == "source-not-exportable"));

        let secret_path = write_skill(directory.path(), "skills/secret-path", "secret-path");
        fs::write(secret_path.join(".env.local"), "SAFE=placeholder\n").unwrap();
        workspace.refresh_skills().unwrap();
        let id = id_for(&workspace, "personal", "secret-path");
        let plan = workspace.preview_bundle_export(&[id]).unwrap();
        assert!(plan
            .blocked
            .iter()
            .any(|finding| finding.rule_id == "credential-path"));
        assert!(plan.skills[0].revision.is_none());
        assert!(format!("{plan:?}").contains(".env.local"));
        assert!(!format!("{plan:?}").contains("SAFE=placeholder"));

        let secret_content =
            write_skill(directory.path(), "skills/secret-content", "secret-content");
        fs::write(
            secret_content.join("scripts/helper.sh"),
            "api_key = a9B8c7D6e5F4g3H2i1J0kLmNopQr\n",
        )
        .unwrap();
        workspace.refresh_skills().unwrap();
        let id = id_for(&workspace, "personal", "secret-content");
        let plan = workspace.preview_bundle_export(&[id]).unwrap();
        assert!(plan
            .blocked
            .iter()
            .any(|finding| finding.rule_id == "credential-assignment"));
        assert!(!format!("{plan:?}").contains("a9B8c7"));
    }

    #[test]
    fn private_key_material_is_detected_across_stream_chunks() {
        let (directory, workspace) = workspace();
        let skill = write_skill(directory.path(), "skills/keyed", "keyed");
        let mut bytes = vec![b'a'; 16 * 1024 - 10];
        bytes.extend_from_slice(b"-----BEGIN OPENSSH PRIVATE KEY-----\nsecret\n");
        fs::write(skill.join("scripts/helper.sh"), bytes).unwrap();
        let id = id_for(&workspace, "personal", "keyed");
        let plan = workspace.preview_bundle_export(&[id]).unwrap();
        assert!(plan
            .blocked
            .iter()
            .any(|finding| finding.rule_id == "private-key-material"));
    }

    #[test]
    fn discovery_stops_at_per_skill_file_limits_before_opening_every_file() {
        let (directory, workspace) = workspace();
        let skill = write_skill(directory.path(), "skills/crowded", "crowded");
        for index in 0..MAX_FILES_PER_SKILL {
            fs::write(skill.join(format!("file-{index:04}.txt")), b"x").unwrap();
        }
        let id = id_for(&workspace, "personal", "crowded");
        let plan = workspace.preview_bundle_export(&[id]).unwrap();
        assert!(!plan.can_export);
        assert!(plan
            .blocked
            .iter()
            .any(|finding| finding.rule_id == "resource-limit"));
    }

    #[test]
    fn placeholders_are_not_treated_as_secret_assignments() {
        let (directory, workspace) = workspace();
        let skill = write_skill(directory.path(), "skills/example", "example");
        fs::write(
            skill.join("scripts/helper.sh"),
            "api_key = YOUR_API_KEY_PLACEHOLDER\naccess_token=${TOKEN}\n",
        )
        .unwrap();
        let id = id_for(&workspace, "personal", "example");
        assert!(workspace.preview_bundle_export(&[id]).unwrap().can_export);
    }

    #[test]
    fn stale_sources_and_existing_destinations_never_produce_output() {
        let (directory, workspace) = workspace();
        let skill = write_skill(directory.path(), "skills/stale", "stale");
        let id = id_for(&workspace, "personal", "stale");
        let plan = workspace.preview_bundle_export(&[id]).unwrap();
        fs::write(skill.join("scripts/helper.sh"), "echo changed\n").unwrap();
        let destination = directory.path().join("stale.skillbundle");
        assert!(matches!(
            workspace.export_skill_bundle(&plan.plan_revision, &destination),
            Err(BundleExportError::SourceChanged)
        ));
        assert!(!destination.exists());

        workspace.refresh_skills().unwrap();
        let id = id_for(&workspace, "personal", "stale");
        let plan = workspace.preview_bundle_export(&[id]).unwrap();
        fs::write(&destination, "keep").unwrap();
        assert!(matches!(
            workspace.export_skill_bundle(&plan.plan_revision, &destination),
            Err(BundleExportError::DestinationExists)
        ));
        assert_eq!(fs::read_to_string(destination).unwrap(), "keep");
    }

    #[cfg(unix)]
    #[test]
    fn executable_mode_changes_make_the_preview_stale() {
        use std::os::unix::fs::PermissionsExt;

        let (directory, workspace) = workspace();
        let skill = write_skill(directory.path(), "skills/mode", "mode");
        let id = id_for(&workspace, "personal", "mode");
        let plan = workspace.preview_bundle_export(&[id]).unwrap();
        fs::set_permissions(
            skill.join("scripts/helper.sh"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        let destination = directory.path().join("mode.skillbundle");
        assert!(matches!(
            workspace.export_skill_bundle(&plan.plan_revision, &destination),
            Err(BundleExportError::SourceChanged)
        ));
        assert!(!destination.exists());
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_and_discovery_time_replacement_are_rejected() {
        use std::os::unix::fs::symlink;

        let (directory, workspace) = workspace();
        let skill = write_skill(directory.path(), "skills/linked", "linked");
        let outside = directory.path().join("outside");
        fs::write(&outside, "outside").unwrap();
        symlink(&outside, skill.join("scripts/link")).unwrap();
        let id = id_for(&workspace, "personal", "linked");
        let plan = workspace.preview_bundle_export(&[id]).unwrap();
        assert!(plan
            .blocked
            .iter()
            .any(|finding| finding.rule_id == "unsafe-entry"));

        fs::remove_file(skill.join("scripts/link")).unwrap();
        workspace.refresh_skills().unwrap();
        let internal = workspace
            .find_skill(&id_for(&workspace, "personal", "linked"))
            .unwrap();
        struct ReplaceAfterDiscovery;
        impl ExportObserver for ReplaceAfterDiscovery {
            fn after_discovery(&self, skill_directory: &Path) {
                let path = skill_directory.join("scripts/helper.sh");
                fs::remove_file(&path).unwrap();
                std::os::unix::fs::symlink("../../outside", path).unwrap();
            }
        }
        assert!(matches!(
            build_skill_snapshot(&internal, &ReplaceAfterDiscovery),
            Err(SnapshotError::Changed | SnapshotError::Unsafe(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn read_time_changes_and_special_file_replacements_are_rejected_without_blocking() {
        use std::os::unix::ffi::OsStrExt;

        let (directory, workspace) = workspace();
        let skill = write_skill(directory.path(), "skills/raced", "raced");
        fs::write(skill.join("scripts/helper.sh"), vec![b'a'; 64 * 1024]).unwrap();
        let id = id_for(&workspace, "personal", "raced");
        let internal = workspace.find_skill(&id).unwrap();
        struct ModifyDuringRead;
        impl ExportObserver for ModifyDuringRead {
            fn after_first_file_chunk(&self, skill_directory: &Path, relative_path: &str) {
                if relative_path == "scripts/helper.sh" {
                    fs::write(skill_directory.join(relative_path), vec![b'b'; 64 * 1024]).unwrap();
                }
            }
        }
        assert!(matches!(
            build_skill_snapshot(&internal, &ModifyDuringRead),
            Err(SnapshotError::Changed)
        ));

        fs::write(skill.join("scripts/helper.sh"), "regular again\n").unwrap();
        let internal = workspace.find_skill(&id).unwrap();
        struct ReplaceWithFifo;
        impl ExportObserver for ReplaceWithFifo {
            fn after_discovery(&self, skill_directory: &Path) {
                let path = skill_directory.join("scripts/helper.sh");
                fs::remove_file(&path).unwrap();
                let path = CString::new(path.as_os_str().as_bytes()).unwrap();
                assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
            }
        }
        assert!(matches!(
            build_skill_snapshot(&internal, &ReplaceWithFifo),
            Err(SnapshotError::Unsafe(_))
        ));
    }

    #[test]
    fn canonical_output_ignores_mtime_and_ordinary_permissions() {
        let (directory, workspace) = workspace();
        let skill = write_skill(directory.path(), "skills/stable", "stable");
        let id = id_for(&workspace, "personal", "stable");
        let first_plan = workspace
            .preview_bundle_export(std::slice::from_ref(&id))
            .unwrap();
        let first = directory.path().join("first.skillbundle");
        workspace
            .export_skill_bundle(&first_plan.plan_revision, &first)
            .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                skill.join("scripts/helper.sh"),
                fs::Permissions::from_mode(0o600),
            )
            .unwrap();
        }
        workspace.refresh_skills().unwrap();
        let second_plan = workspace.preview_bundle_export(&[id]).unwrap();
        let second = directory.path().join("second.skillbundle");
        workspace
            .export_skill_bundle(&second_plan.plan_revision, &second)
            .unwrap();
        assert_eq!(fs::read(first).unwrap(), fs::read(second).unwrap());
    }

    #[test]
    fn selected_skill_order_does_not_change_bundle_bytes() {
        let (directory, workspace) = workspace();
        write_skill(directory.path(), "skills/alpha", "alpha");
        write_skill(directory.path(), "skills/beta", "beta");
        let alpha = id_for(&workspace, "personal", "alpha");
        let beta = id_for(&workspace, "personal", "beta");
        let first_plan = workspace
            .preview_bundle_export(&[beta.clone(), alpha.clone()])
            .unwrap();
        let first = directory.path().join("order-one.skillbundle");
        workspace
            .export_skill_bundle(&first_plan.plan_revision, &first)
            .unwrap();
        let second_plan = workspace.preview_bundle_export(&[alpha, beta]).unwrap();
        let second = directory.path().join("order-two.skillbundle");
        workspace
            .export_skill_bundle(&second_plan.plan_revision, &second)
            .unwrap();
        assert_eq!(fs::read(first).unwrap(), fs::read(second).unwrap());
    }

    #[test]
    fn destination_created_at_commit_is_preserved_and_temp_is_cleaned() {
        let (directory, workspace) = workspace();
        write_skill(directory.path(), "skills/race", "race");
        let id = id_for(&workspace, "personal", "race");
        let plan = workspace.preview_bundle_export(&[id]).unwrap();
        let destination = directory.path().join("race.skillbundle");
        struct CreateAtCommit;
        impl ExportObserver for CreateAtCommit {
            fn before_commit(&self, destination: &Path) {
                fs::write(destination, "winner").unwrap();
            }
        }
        assert!(matches!(
            workspace.export_skill_bundle_observed(
                &plan.plan_revision,
                &destination,
                &CreateAtCommit
            ),
            Err(BundleExportError::DestinationExists)
        ));
        assert_eq!(fs::read_to_string(&destination).unwrap(), "winner");
        let leftovers = fs::read_dir(directory.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with(".tmp"))
            .count();
        assert_eq!(leftovers, 0);
    }

    #[cfg(unix)]
    #[test]
    fn replaced_destination_parent_cannot_redirect_the_commit() {
        let (directory, workspace) = workspace();
        write_skill(directory.path(), "skills/parent-race", "parent-race");
        let id = id_for(&workspace, "personal", "parent-race");
        let plan = workspace.preview_bundle_export(&[id]).unwrap();
        let export_parent = directory.path().join("exports");
        fs::create_dir(&export_parent).unwrap();
        let moved_parent = directory.path().join("exports-moved");
        let destination = export_parent.join("parent-race.skillbundle");

        struct ReplaceParent {
            moved_parent: PathBuf,
        }
        impl ExportObserver for ReplaceParent {
            fn before_commit(&self, destination: &Path) {
                let parent = destination.parent().unwrap();
                fs::rename(parent, &self.moved_parent).unwrap();
                fs::create_dir(parent).unwrap();
            }
        }

        assert!(matches!(
            workspace.export_skill_bundle_observed(
                &plan.plan_revision,
                &destination,
                &ReplaceParent {
                    moved_parent: moved_parent.clone(),
                },
            ),
            Err(BundleExportError::InvalidDestination)
        ));
        assert!(!destination.exists());
        assert!(!moved_parent.join("parent-race.skillbundle").exists());
        assert_eq!(
            fs::read_dir(&moved_parent)
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".agent-skill-studio-export-"))
                .count(),
            0
        );
    }

    #[cfg(unix)]
    #[test]
    fn failed_post_commit_verification_preserves_a_replacement_file() {
        let (directory, workspace) = workspace();
        write_skill(
            directory.path(),
            "skills/post-commit-race",
            "post-commit-race",
        );
        let id = id_for(&workspace, "personal", "post-commit-race");
        let plan = workspace.preview_bundle_export(&[id]).unwrap();
        let destination = directory.path().join("post-commit-race.skillbundle");

        struct ReplaceCommittedFile;
        impl ExportObserver for ReplaceCommittedFile {
            fn after_commit(&self, destination: &Path) {
                fs::remove_file(destination).unwrap();
                fs::write(destination, "replacement-wins").unwrap();
            }
        }

        assert!(matches!(
            workspace.export_skill_bundle_observed(
                &plan.plan_revision,
                &destination,
                &ReplaceCommittedFile,
            ),
            Err(BundleExportError::SourceChanged)
        ));
        assert_eq!(fs::read_to_string(destination).unwrap(), "replacement-wins");
    }
}
