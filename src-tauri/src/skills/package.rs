use super::{
    audit, directory_replace::replace_directory_atomically_with_finalize,
    is_ignored_skill_metadata_name, lifecycle::directory_revision, parse_document, SkillDetail,
    Workspace, WorkspaceError,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};
use tempfile::{Builder, NamedTempFile, TempDir};

const MAX_FILES: usize = 256;
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_PACKAGE_BYTES: u64 = 25 * 1024 * 1024;
const MAX_DEPTH: usize = 8;
const MAX_TEXT_PREVIEW_BYTES: u64 = 512 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CandidateFileSyncAction {
    Add,
    Replace,
    Delete,
}

impl CandidateFileSyncAction {
    fn change_kind(self) -> &'static str {
        match self {
            Self::Add => "added",
            Self::Replace => "modified",
            Self::Delete => "deleted",
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct CandidateFileSyncOperation<'a> {
    action: CandidateFileSyncAction,
    remote: Option<(&'a [u8], bool)>,
}

impl<'a> CandidateFileSyncOperation<'a> {
    pub(crate) fn new(action: CandidateFileSyncAction, remote: Option<(&'a [u8], bool)>) -> Self {
        Self { action, remote }
    }
}

struct CandidateFileSyncGuards<F, C> {
    final_check: F,
    finalize: C,
}

#[cfg(test)]
fn no_candidate_sync_finalize(_path: &Path) -> Result<(), WorkspaceError> {
    Ok(())
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageEntry {
    pub path: String,
    pub kind: String,
    pub media_type: String,
    pub size: u64,
    pub content_hash: Option<String>,
    pub editable: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageValidation {
    pub code: String,
    pub severity: String,
    pub path: Option<String>,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageSnapshot {
    pub skill_id: String,
    pub skill_name: String,
    pub revision: String,
    pub editable: bool,
    pub entries: Vec<PackageEntry>,
    pub validations: Vec<PackageValidation>,
    pub total_files: usize,
    pub total_bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageFileContent {
    pub path: String,
    pub media_type: String,
    pub content: Option<String>,
    pub data_url: Option<String>,
    pub truncated: bool,
    pub editable: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageImportSource {
    pub source_path: String,
    pub file_name: String,
    pub size: u64,
    pub content_hash: String,
    pub media_type: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum PackageMutation {
    Write {
        path: String,
        content: String,
    },
    CopyFile {
        path: String,
        source_path: String,
        expected_hash: String,
        expected_size: u64,
    },
    Move {
        path: String,
        destination: String,
    },
    Delete {
        path: String,
    },
    CreateDirectory {
        path: String,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageChange {
    pub kind: String,
    pub path: String,
    pub destination: Option<String>,
    pub before_hash: Option<String>,
    pub after_hash: Option<String>,
    pub before_text: Option<String>,
    pub after_text: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackagePreview {
    pub expected_revision: String,
    pub proposed_revision: String,
    pub changes: Vec<PackageChange>,
    pub validations: Vec<PackageValidation>,
    pub can_apply: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageSaveResult {
    pub snapshot: PackageSnapshot,
    pub skill: SkillDetail,
    pub restart_recommended: bool,
}

pub(crate) struct PackageAuditSnapshot {
    directory: TempDir,
    pub revision: String,
}

impl PackageAuditSnapshot {
    pub fn path(&self) -> &Path {
        self.directory.path()
    }
}

impl Workspace {
    pub fn get_skill_package(&self, id: &str) -> Result<PackageSnapshot, WorkspaceError> {
        let skill = self.find_skill(id)?;
        package_snapshot(
            &skill.summary.id,
            &skill.summary.name,
            &skill.directory,
            skill.source == super::Source::Personal,
            self.measured_package_revision(&skill.directory)?,
        )
    }

    pub fn read_skill_package_file(
        &self,
        id: &str,
        expected_revision: &str,
        relative_path: &str,
    ) -> Result<PackageFileContent, WorkspaceError> {
        let skill = self.find_skill(id)?;
        self.ensure_package_revision(&skill.directory, expected_revision)?;
        let relative = valid_relative_path(relative_path)?;
        let path = contained_existing_path(&skill.directory, &relative)?;
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_file() || metadata.len() > MAX_FILE_BYTES {
            return Err(WorkspaceError::PackageTooLarge);
        }
        let bytes = fs::read(&path)?;
        let media_type = media_type(relative_path, &bytes);
        let (content, data_url, truncated) = if media_type == "text" {
            let limit = bytes.len().min(MAX_TEXT_PREVIEW_BYTES as usize);
            let text = std::str::from_utf8(&bytes[..limit])
                .map_err(|_| WorkspaceError::InvalidPackagePath)?;
            (Some(text.to_string()), None, limit < bytes.len())
        } else if media_type.starts_with("image/") {
            (
                None,
                Some(format!(
                    "data:{media_type};base64,{}",
                    STANDARD.encode(bytes)
                )),
                false,
            )
        } else {
            (None, None, false)
        };
        Ok(PackageFileContent {
            path: normalize_path(&relative),
            media_type,
            content,
            data_url,
            truncated,
            editable: skill.source == super::Source::Personal
                && metadata.len() <= MAX_TEXT_PREVIEW_BYTES
                && content_is_utf8(&path),
        })
    }

    pub fn inspect_package_import_source(
        &self,
        selected_path: &Path,
    ) -> Result<PackageImportSource, WorkspaceError> {
        let metadata = fs::symlink_metadata(selected_path)?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() > MAX_FILE_BYTES
        {
            return Err(WorkspaceError::PackageTooLarge);
        }
        let canonical = fs::canonicalize(selected_path)?;
        let prefix = read_prefix(&canonical, 8192)?;
        Ok(PackageImportSource {
            source_path: canonical.display().to_string(),
            file_name: canonical
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            size: metadata.len(),
            content_hash: hash_file(&canonical)?,
            media_type: media_type(&canonical.to_string_lossy(), &prefix),
        })
    }

    pub fn preview_skill_package(
        &self,
        id: &str,
        expected_revision: &str,
        mutations: &[PackageMutation],
    ) -> Result<PackagePreview, WorkspaceError> {
        let skill = self.editable_skill(id)?;
        self.ensure_package_revision(&skill.directory, expected_revision)?;
        let staged = stage_package(&skill.directory, mutations)?;
        let validations = validate_package(staged.path(), &skill.summary.name)?;
        let proposed_revision = self.measured_package_revision(staged.path())?;
        let changes = describe_changes(&skill.directory, staged.path(), mutations)?;
        Ok(PackagePreview {
            expected_revision: expected_revision.to_string(),
            proposed_revision,
            can_apply: !changes.is_empty()
                && !validations.iter().any(|item| item.severity == "blocker"),
            changes,
            validations,
        })
    }

    pub(crate) fn stage_skill_package_for_audit(
        &self,
        id: &str,
        expected_revision: &str,
        expected_proposed_revision: &str,
        mutations: &[PackageMutation],
    ) -> Result<PackageAuditSnapshot, WorkspaceError> {
        let skill = self.editable_skill(id)?;
        self.ensure_package_revision(&skill.directory, expected_revision)?;
        let staged = stage_package(&skill.directory, mutations)?;
        let validations = validate_package(staged.path(), &skill.summary.name)?;
        if validations.iter().any(is_structural_audit_blocker) {
            return Err(WorkspaceError::Blocked);
        }
        let revision = self.measured_package_revision(staged.path())?;
        if expected_proposed_revision.is_empty() || revision != expected_proposed_revision {
            return Err(WorkspaceError::PreviewMismatch);
        }
        Ok(PackageAuditSnapshot {
            directory: staged,
            revision,
        })
    }

    pub fn save_skill_package(
        &self,
        id: &str,
        expected_revision: &str,
        expected_proposed_revision: &str,
        mutations: &[PackageMutation],
    ) -> Result<PackageSaveResult, WorkspaceError> {
        let _mutation = self
            .mutations
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let skill = self.editable_skill(id)?;
        self.ensure_package_revision(&skill.directory, expected_revision)?;
        let staged = stage_package(&skill.directory, mutations)?;
        let validations = validate_package(staged.path(), &skill.summary.name)?;
        if validations.iter().any(|item| item.severity == "blocker") {
            return Err(WorkspaceError::Blocked);
        }
        let proposed_revision = self.measured_package_revision(staged.path())?;
        if proposed_revision != expected_proposed_revision {
            return Err(WorkspaceError::PreviewMismatch);
        }
        replace_directory(
            &skill.directory,
            &skill.root,
            expected_revision,
            staged.keep(),
            |isolated| self.measured_package_revision(isolated),
        )?;
        let updated = self
            .read_skill(&skill.directory, skill.source, &skill.root)?
            .ok_or(WorkspaceError::NotFound)?;
        self.upsert_index(updated.clone())?;
        let detail = self.get_skill(&updated.summary.id)?;
        let snapshot = package_snapshot(
            &updated.summary.id,
            &updated.summary.name,
            &updated.directory,
            true,
            self.measured_package_revision(&updated.directory)?,
        )?;
        Ok(PackageSaveResult {
            snapshot,
            skill: detail,
            restart_recommended: true,
        })
    }

    pub(crate) fn preview_candidate_file_sync(
        &self,
        id: &str,
        expected_revision: &str,
        path: &str,
        operation: CandidateFileSyncOperation<'_>,
    ) -> Result<PackagePreview, WorkspaceError> {
        let skill = self.editable_skill(id)?;
        self.ensure_package_revision(&skill.directory, expected_revision)?;
        let staged = stage_candidate_file_sync(&skill.directory, path, operation)?;
        let validations = validate_package(staged.path(), &skill.summary.name)?;
        let proposed_revision = self.measured_package_revision(staged.path())?;
        let relative = valid_relative_path(path)?;
        let before = skill.directory.join(&relative);
        let after = staged.path().join(&relative);
        let (before_text, after_text) = diff_text_pair(Some(&before), Some(&after))?;
        let changes = vec![PackageChange {
            kind: operation.action.change_kind().into(),
            path: normalize_path(&relative),
            destination: None,
            before_hash: before.is_file().then(|| hash_file(&before)).transpose()?,
            after_hash: after.is_file().then(|| hash_file(&after)).transpose()?,
            before_text,
            after_text,
        }];
        Ok(PackagePreview {
            expected_revision: expected_revision.into(),
            proposed_revision,
            can_apply: !validations.iter().any(|item| item.severity == "blocker"),
            changes,
            validations,
        })
    }

    #[cfg(test)]
    pub(crate) fn apply_candidate_file_sync(
        &self,
        id: &str,
        expected_revision: &str,
        expected_proposed_revision: &str,
        path: &str,
        operation: CandidateFileSyncOperation<'_>,
    ) -> Result<PackageSaveResult, WorkspaceError> {
        self.apply_candidate_file_sync_with_final_check(
            id,
            expected_revision,
            expected_proposed_revision,
            path,
            operation,
            || Ok(()),
        )
    }

    pub(crate) fn apply_candidate_file_sync_with_finalize<F>(
        &self,
        id: &str,
        expected_revision: &str,
        expected_proposed_revision: &str,
        path: &str,
        operation: CandidateFileSyncOperation<'_>,
        finalize: F,
    ) -> Result<PackageSaveResult, WorkspaceError>
    where
        F: FnOnce(&Path) -> Result<(), WorkspaceError>,
    {
        self.apply_candidate_file_sync_with_checks(
            id,
            expected_revision,
            expected_proposed_revision,
            path,
            operation,
            CandidateFileSyncGuards {
                final_check: || Ok(()),
                finalize,
            },
        )
    }

    #[cfg(test)]
    fn apply_candidate_file_sync_with_final_check<F>(
        &self,
        id: &str,
        expected_revision: &str,
        expected_proposed_revision: &str,
        path: &str,
        operation: CandidateFileSyncOperation<'_>,
        final_check: F,
    ) -> Result<PackageSaveResult, WorkspaceError>
    where
        F: FnOnce() -> Result<(), WorkspaceError>,
    {
        self.apply_candidate_file_sync_with_checks(
            id,
            expected_revision,
            expected_proposed_revision,
            path,
            operation,
            CandidateFileSyncGuards {
                final_check,
                finalize: no_candidate_sync_finalize,
            },
        )
    }

    fn apply_candidate_file_sync_with_checks<F, C>(
        &self,
        id: &str,
        expected_revision: &str,
        expected_proposed_revision: &str,
        path: &str,
        operation: CandidateFileSyncOperation<'_>,
        guards: CandidateFileSyncGuards<F, C>,
    ) -> Result<PackageSaveResult, WorkspaceError>
    where
        F: FnOnce() -> Result<(), WorkspaceError>,
        C: FnOnce(&Path) -> Result<(), WorkspaceError>,
    {
        let _mutation = self
            .mutations
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let skill = self.editable_skill(id)?;
        self.ensure_package_revision(&skill.directory, expected_revision)?;
        let staged = stage_candidate_file_sync(&skill.directory, path, operation)?;
        let validations = validate_package(staged.path(), &skill.summary.name)?;
        if validations.iter().any(|item| item.severity == "blocker") {
            return Err(WorkspaceError::Blocked);
        }
        if self.measured_package_revision(staged.path())? != expected_proposed_revision {
            return Err(WorkspaceError::PreviewMismatch);
        }
        (guards.final_check)()?;
        replace_directory_with_finalize(
            &skill.directory,
            &skill.root,
            expected_revision,
            staged.keep(),
            |isolated| self.measured_package_revision(isolated),
            guards.finalize,
        )?;
        let updated = self
            .read_skill(&skill.directory, skill.source, &skill.root)?
            .ok_or(WorkspaceError::NotFound)?;
        self.upsert_index(updated.clone())?;
        Ok(PackageSaveResult {
            snapshot: package_snapshot(
                &updated.summary.id,
                &updated.summary.name,
                &updated.directory,
                true,
                self.measured_package_revision(&updated.directory)?,
            )?,
            skill: self.get_skill(&updated.summary.id)?,
            restart_recommended: true,
        })
    }

    fn ensure_package_revision(
        &self,
        directory: &Path,
        expected: &str,
    ) -> Result<(), WorkspaceError> {
        if expected.is_empty() || self.measured_package_revision(directory)? != expected {
            Err(WorkspaceError::DirectoryChanged)
        } else {
            Ok(())
        }
    }

    fn measured_package_revision(&self, directory: &Path) -> Result<String, WorkspaceError> {
        let started = std::time::Instant::now();
        let result = directory_revision(directory);
        self.record_timing(
            &self.metrics.package_revisions,
            &self.metrics.package_revision_nanos,
            started,
        );
        result
    }
}

fn is_structural_audit_blocker(validation: &PackageValidation) -> bool {
    matches!(
        validation.code.as_str(),
        "missing-skill-document"
            | "skill-document-too-large"
            | "skill-document-not-text"
            | "skill-identity-change"
    )
}

fn stage_candidate_file_sync(
    source: &Path,
    path: &str,
    operation: CandidateFileSyncOperation<'_>,
) -> Result<TempDir, WorkspaceError> {
    let relative = valid_relative_path(path)?;
    if path == "SKILL.md" && operation.remote.is_none() {
        return Err(WorkspaceError::MissingSkillDocument);
    }
    let parent = source.parent().ok_or(WorkspaceError::UnsafePath)?;
    let temporary = Builder::new().prefix(".skill-update-").tempdir_in(parent)?;
    let mut budget = CopyBudget::default();
    copy_directory(source, temporary.path(), 0, &mut budget)?;
    let target = contained_target(temporary.path(), &relative)?;
    match (operation.action, operation.remote) {
        (CandidateFileSyncAction::Add, Some((bytes, executable)))
        | (CandidateFileSyncAction::Replace, Some((bytes, executable))) => {
            let replacing = operation.action == CandidateFileSyncAction::Replace;
            if replacing {
                if !target.exists() {
                    return Err(WorkspaceError::PackagePathConflict);
                }
                let existing = contained_existing_path(temporary.path(), &relative)?;
                if !existing.is_file() {
                    return Err(WorkspaceError::PackagePathConflict);
                }
            } else if target.exists() {
                return Err(WorkspaceError::PackagePathConflict);
            }
            if bytes.len() as u64 > MAX_FILE_BYTES {
                return Err(WorkspaceError::PackageTooLarge);
            }
            let target_parent = target.parent().ok_or(WorkspaceError::InvalidPackagePath)?;
            fs::create_dir_all(target_parent)?;
            let mut staged = NamedTempFile::new_in(target_parent)?;
            staged.write_all(bytes)?;
            staged.as_file_mut().sync_all()?;
            set_candidate_permissions(staged.as_file_mut(), executable)?;
            if replacing {
                fs::remove_file(&target)?;
            }
            staged
                .persist(target)
                .map_err(|error| WorkspaceError::Io(error.error))?;
        }
        (CandidateFileSyncAction::Delete, None) => {
            let existing = contained_existing_path(temporary.path(), &relative)?;
            if !existing.is_file() {
                return Err(WorkspaceError::PackagePathConflict);
            }
            fs::remove_file(existing)?;
        }
        _ => return Err(WorkspaceError::InvalidPackageOperation),
    }
    Ok(temporary)
}

#[cfg(unix)]
fn set_candidate_permissions(file: &mut File, executable: bool) -> Result<(), WorkspaceError> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(fs::Permissions::from_mode(if executable {
        0o755
    } else {
        0o644
    }))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_candidate_permissions(_file: &mut File, _executable: bool) -> Result<(), WorkspaceError> {
    Ok(())
}

fn package_snapshot(
    id: &str,
    name: &str,
    directory: &Path,
    editable: bool,
    revision: String,
) -> Result<PackageSnapshot, WorkspaceError> {
    let mut entries = Vec::new();
    let mut total_bytes = 0;
    collect_entries(directory, directory, 0, &mut entries, &mut total_bytes)?;
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    let validations = validate_package(directory, name)?;
    let total_files = entries.iter().filter(|entry| entry.kind == "file").count();
    Ok(PackageSnapshot {
        skill_id: id.to_string(),
        skill_name: name.to_string(),
        revision,
        editable,
        entries,
        validations,
        total_files,
        total_bytes,
    })
}

fn collect_entries(
    root: &Path,
    current: &Path,
    depth: usize,
    entries: &mut Vec<PackageEntry>,
    total_bytes: &mut u64,
) -> Result<(), WorkspaceError> {
    if depth > MAX_DEPTH {
        return Err(WorkspaceError::PackageTooLarge);
    }
    let mut children = fs::read_dir(current)?.collect::<Result<Vec<_>, _>>()?;
    children.sort_by_key(|entry| entry.file_name());
    for entry in children {
        if is_ignored_skill_metadata_name(&entry.file_name()) {
            continue;
        }
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(WorkspaceError::UnsafePath);
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|_| WorkspaceError::UnsafePath)?;
        if metadata.is_dir() {
            entries.push(PackageEntry {
                path: normalize_path(relative),
                kind: "directory".into(),
                media_type: "directory".into(),
                size: 0,
                content_hash: None,
                editable: true,
            });
            collect_entries(root, &path, depth + 1, entries, total_bytes)?;
        } else if metadata.is_file() {
            if metadata.len() > MAX_FILE_BYTES {
                return Err(WorkspaceError::PackageTooLarge);
            }
            *total_bytes = total_bytes
                .checked_add(metadata.len())
                .ok_or(WorkspaceError::PackageTooLarge)?;
            if *total_bytes > MAX_PACKAGE_BYTES
                || entries.iter().filter(|item| item.kind == "file").count() >= MAX_FILES
            {
                return Err(WorkspaceError::PackageTooLarge);
            }
            let prefix = read_prefix(&path, 8192)?;
            let media = media_type(&normalize_path(relative), &prefix);
            entries.push(PackageEntry {
                path: normalize_path(relative),
                kind: "file".into(),
                media_type: media.clone(),
                size: metadata.len(),
                content_hash: Some(hash_file(&path)?),
                editable: editable_text(&media, metadata.len()),
            });
        } else {
            return Err(WorkspaceError::UnsafePath);
        }
    }
    Ok(())
}

fn validate_package(
    root: &Path,
    expected_name: &str,
) -> Result<Vec<PackageValidation>, WorkspaceError> {
    let skill_path = root.join("SKILL.md");
    let metadata = match fs::symlink_metadata(&skill_path) {
        Ok(value) if value.is_file() && !value.file_type().is_symlink() => value,
        _ => {
            return Ok(vec![PackageValidation {
                code: "missing-skill-document".into(),
                severity: "blocker".into(),
                path: Some("SKILL.md".into()),
                message: "SKILL.md is required at the Package root.".into(),
            }])
        }
    };
    if metadata.len() > MAX_TEXT_PREVIEW_BYTES {
        return Ok(vec![PackageValidation {
            code: "skill-document-too-large".into(),
            severity: "blocker".into(),
            path: Some("SKILL.md".into()),
            message: "SKILL.md exceeds the editable text limit.".into(),
        }]);
    }
    let markdown = match fs::read_to_string(&skill_path) {
        Ok(value) => value,
        Err(_) => {
            return Ok(vec![PackageValidation {
                code: "skill-document-not-text".into(),
                severity: "blocker".into(),
                path: Some("SKILL.md".into()),
                message: "SKILL.md must be UTF-8 text.".into(),
            }])
        }
    };
    let document = parse_document(&markdown);
    let mut validations = Vec::new();
    if document.name != expected_name {
        validations.push(PackageValidation {
            code: "skill-identity-change".into(),
            severity: "blocker".into(),
            path: Some("SKILL.md".into()),
            message: "The Skill name must continue to match this Package identity.".into(),
        });
    }
    for finding in audit(&markdown, &markdown, expected_name).findings {
        if finding.severity == "blocker" {
            validations.push(PackageValidation {
                code: finding.id,
                severity: "blocker".into(),
                path: Some("SKILL.md".into()),
                message: finding.explanation,
            });
        }
    }
    validate_support_files(root, root, 0, &mut CopyBudget::default(), &mut validations)?;
    Ok(validations)
}

fn validate_support_files(
    root: &Path,
    current: &Path,
    depth: usize,
    budget: &mut CopyBudget,
    validations: &mut Vec<PackageValidation>,
) -> Result<(), WorkspaceError> {
    if depth > MAX_DEPTH {
        return Err(WorkspaceError::PackageTooLarge);
    }
    let mut entries = fs::read_dir(current)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        if is_ignored_skill_metadata_name(&entry.file_name()) {
            continue;
        }
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(WorkspaceError::UnsafePath);
        }
        if metadata.is_dir() {
            validate_support_files(root, &path, depth + 1, budget, validations)?;
            continue;
        }
        if !metadata.is_file() {
            return Err(WorkspaceError::UnsafePath);
        }
        budget.files += 1;
        budget.bytes = budget
            .bytes
            .checked_add(metadata.len())
            .ok_or(WorkspaceError::PackageTooLarge)?;
        if budget.files > MAX_FILES
            || metadata.len() > MAX_FILE_BYTES
            || budget.bytes > MAX_PACKAGE_BYTES
        {
            return Err(WorkspaceError::PackageTooLarge);
        }
        let relative = normalize_path(
            path.strip_prefix(root)
                .map_err(|_| WorkspaceError::UnsafePath)?,
        );
        if relative == "SKILL.md" {
            continue;
        }
        if is_executable(&metadata) {
            validations.push(PackageValidation {
                code: "executable-file".into(),
                severity: "warning".into(),
                path: Some(relative.clone()),
                message: "This Package contains an executable file. The Studio will never run it."
                    .into(),
            });
        }
        if metadata.len() > MAX_TEXT_PREVIEW_BYTES {
            continue;
        }
        let bytes = fs::read(&path)?;
        let Ok(text) = std::str::from_utf8(&bytes) else {
            continue;
        };
        for finding in super::audit::safety_findings(text) {
            if matches!(finding.severity.as_str(), "warning" | "blocker") {
                validations.push(PackageValidation {
                    code: finding.id,
                    severity: finding.severity,
                    path: Some(relative.clone()),
                    message: format!("{} {}", finding.explanation, finding.evidence),
                });
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
fn is_executable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &fs::Metadata) -> bool {
    false
}

fn stage_package(source: &Path, mutations: &[PackageMutation]) -> Result<TempDir, WorkspaceError> {
    let parent = source.parent().ok_or(WorkspaceError::UnsafePath)?;
    let temporary = Builder::new()
        .prefix(".skill-package-")
        .tempdir_in(parent)?;
    let mut budget = CopyBudget::default();
    copy_directory(source, temporary.path(), 0, &mut budget)?;
    apply_mutations(temporary.path(), mutations)?;
    if !temporary.path().join("SKILL.md").is_file() {
        return Err(WorkspaceError::MissingSkillDocument);
    }
    Ok(temporary)
}

#[derive(Default)]
struct CopyBudget {
    files: usize,
    bytes: u64,
}

fn copy_directory(
    source: &Path,
    destination: &Path,
    depth: usize,
    budget: &mut CopyBudget,
) -> Result<(), WorkspaceError> {
    if depth > MAX_DEPTH {
        return Err(WorkspaceError::PackageTooLarge);
    }
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        if is_ignored_skill_metadata_name(&entry.file_name()) {
            preserve_ignored_metadata_file(&entry, destination)?;
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink() {
            return Err(WorkspaceError::UnsafePath);
        }
        let target = destination.join(entry.file_name());
        if metadata.is_dir() {
            fs::create_dir(&target)?;
            fs::set_permissions(&target, metadata.permissions())?;
            copy_directory(&entry.path(), &target, depth + 1, budget)?;
        } else if metadata.is_file() {
            budget.files += 1;
            budget.bytes = budget
                .bytes
                .checked_add(metadata.len())
                .ok_or(WorkspaceError::PackageTooLarge)?;
            if budget.files > MAX_FILES
                || metadata.len() > MAX_FILE_BYTES
                || budget.bytes > MAX_PACKAGE_BYTES
            {
                return Err(WorkspaceError::PackageTooLarge);
            }
            fs::copy(entry.path(), &target)?;
            fs::set_permissions(&target, metadata.permissions())?;
        } else {
            return Err(WorkspaceError::UnsafePath);
        }
    }
    Ok(())
}

fn preserve_ignored_metadata_file(
    entry: &fs::DirEntry,
    destination: &Path,
) -> Result<(), WorkspaceError> {
    let metadata = fs::symlink_metadata(entry.path())?;
    if metadata.is_file() && !metadata.file_type().is_symlink() {
        let target = destination.join(entry.file_name());
        fs::copy(entry.path(), &target)?;
        fs::set_permissions(target, metadata.permissions())?;
    }
    Ok(())
}

fn apply_mutations(root: &Path, mutations: &[PackageMutation]) -> Result<(), WorkspaceError> {
    for mutation in mutations {
        match mutation {
            PackageMutation::Write { path, content } => {
                let relative = valid_relative_path(path)?;
                let target = contained_target(root, &relative)?;
                if content.len() as u64 > MAX_FILE_BYTES {
                    return Err(WorkspaceError::PackageTooLarge);
                }
                let parent = target.parent().ok_or(WorkspaceError::InvalidPackagePath)?;
                fs::create_dir_all(parent)?;
                let permissions = match fs::symlink_metadata(&target) {
                    Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
                        if !content_is_utf8(&target) {
                            return Err(WorkspaceError::BinaryPackageFile);
                        }
                        Some(metadata.permissions())
                    }
                    Ok(_) => return Err(WorkspaceError::PackagePathConflict),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                    Err(error) => return Err(error.into()),
                };
                let mut temporary = NamedTempFile::new_in(parent)?;
                if let Some(permissions) = permissions {
                    temporary.as_file_mut().set_permissions(permissions)?;
                }
                temporary.write_all(content.as_bytes())?;
                temporary.as_file_mut().sync_all()?;
                temporary
                    .persist(&target)
                    .map_err(|error| WorkspaceError::Io(error.error))?;
            }
            PackageMutation::CopyFile {
                path,
                source_path,
                expected_hash,
                expected_size,
            } => {
                let relative = valid_relative_path(path)?;
                let target = contained_target(root, &relative)?;
                if target.is_dir() {
                    return Err(WorkspaceError::PackagePathConflict);
                }
                let source = Path::new(source_path);
                let metadata = fs::symlink_metadata(source)?;
                if metadata.file_type().is_symlink()
                    || !metadata.is_file()
                    || metadata.len() > MAX_FILE_BYTES
                    || metadata.len() != *expected_size
                    || hash_file(source)? != *expected_hash
                {
                    return Err(WorkspaceError::DirectoryChanged);
                }
                let parent = target.parent().ok_or(WorkspaceError::InvalidPackagePath)?;
                fs::create_dir_all(parent)?;
                let mut input = File::open(source)?;
                let opened = input.metadata()?;
                if !opened.is_file() || opened.len() != *expected_size {
                    return Err(WorkspaceError::DirectoryChanged);
                }
                let permissions = fs::symlink_metadata(&target)
                    .ok()
                    .map(|value| value.permissions());
                let mut temporary = NamedTempFile::new_in(parent)?;
                if let Some(permissions) = permissions {
                    temporary.as_file_mut().set_permissions(permissions)?;
                }
                let copied = std::io::copy(&mut input, temporary.as_file_mut())?;
                if copied != *expected_size || hash_file(temporary.path())? != *expected_hash {
                    return Err(WorkspaceError::DirectoryChanged);
                }
                temporary.as_file_mut().sync_all()?;
                temporary
                    .persist(&target)
                    .map_err(|error| WorkspaceError::Io(error.error))?;
            }
            PackageMutation::Move { path, destination } => {
                if path == "SKILL.md" || destination == "SKILL.md" {
                    return Err(WorkspaceError::MissingSkillDocument);
                }
                let source = contained_existing_path(root, &valid_relative_path(path)?)?;
                let destination = contained_target(root, &valid_relative_path(destination)?)?;
                if destination.exists() {
                    return Err(WorkspaceError::PackagePathConflict);
                }
                fs::create_dir_all(
                    destination
                        .parent()
                        .ok_or(WorkspaceError::InvalidPackagePath)?,
                )?;
                fs::rename(source, destination)?;
            }
            PackageMutation::Delete { path } => {
                if path == "SKILL.md" {
                    return Err(WorkspaceError::MissingSkillDocument);
                }
                let target = contained_existing_path(root, &valid_relative_path(path)?)?;
                if target.is_dir() {
                    fs::remove_dir_all(target)?;
                } else {
                    fs::remove_file(target)?;
                }
            }
            PackageMutation::CreateDirectory { path } => {
                let target = contained_target(root, &valid_relative_path(path)?)?;
                if target.exists() {
                    return Err(WorkspaceError::PackagePathConflict);
                }
                fs::create_dir_all(target)?;
            }
        }
    }
    Ok(())
}

fn describe_changes(
    before: &Path,
    after: &Path,
    mutations: &[PackageMutation],
) -> Result<Vec<PackageChange>, WorkspaceError> {
    let mut changes = Vec::new();
    for mutation in mutations {
        match mutation {
            PackageMutation::Write { path, .. } | PackageMutation::CopyFile { path, .. } => {
                let before_path = before.join(valid_relative_path(path)?);
                let after_path = after.join(valid_relative_path(path)?);
                let before_hash = before_path
                    .is_file()
                    .then(|| hash_file(&before_path))
                    .transpose()?;
                let after_hash = Some(hash_file(&after_path)?);
                if before_hash != after_hash {
                    let (before_text, after_text) =
                        diff_text_pair(Some(&before_path), Some(&after_path))?;
                    changes.push(PackageChange {
                        kind: if before_hash.is_some() {
                            "modified"
                        } else {
                            "added"
                        }
                        .into(),
                        path: path.clone(),
                        destination: None,
                        before_hash,
                        after_hash,
                        before_text,
                        after_text,
                    });
                }
            }
            PackageMutation::Move { path, destination } => {
                let before_path = before.join(valid_relative_path(path)?);
                let after_path = after.join(valid_relative_path(destination)?);
                let (before_text, after_text) =
                    diff_text_pair(Some(&before_path), Some(&after_path))?;
                changes.push(PackageChange {
                    kind: "moved".into(),
                    path: path.clone(),
                    destination: Some(destination.clone()),
                    before_hash: file_hash_if_file(&before_path)?,
                    after_hash: file_hash_if_file(&after_path)?,
                    before_text,
                    after_text,
                });
            }
            PackageMutation::Delete { path } => {
                let before_path = before.join(valid_relative_path(path)?);
                let (before_text, after_text) = diff_text_pair(Some(&before_path), None)?;
                changes.push(PackageChange {
                    kind: "deleted".into(),
                    path: path.clone(),
                    destination: None,
                    before_hash: file_hash_if_file(&before_path)?,
                    after_hash: None,
                    before_text,
                    after_text,
                });
            }
            PackageMutation::CreateDirectory { path } => changes.push(PackageChange {
                kind: "added".into(),
                path: path.clone(),
                destination: None,
                before_hash: None,
                after_hash: None,
                before_text: None,
                after_text: None,
            }),
        }
    }
    Ok(changes)
}

fn diff_text_pair(
    before: Option<&Path>,
    after: Option<&Path>,
) -> Result<(Option<String>, Option<String>), WorkspaceError> {
    let before_text = before.map(diff_text).transpose()?.flatten();
    let after_text = after.map(diff_text).transpose()?.flatten();
    if before.is_some_and(Path::is_file) && before_text.is_none()
        || after.is_some_and(Path::is_file) && after_text.is_none()
        || before_text.is_none() && after_text.is_none()
    {
        return Ok((None, None));
    }
    Ok((
        Some(before_text.unwrap_or_default()),
        Some(after_text.unwrap_or_default()),
    ))
}

fn diff_text(path: &Path) -> Result<Option<String>, WorkspaceError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(value) if value.is_file() && !value.file_type().is_symlink() => value,
        Ok(_) => return Ok(None),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if metadata.len() > MAX_TEXT_PREVIEW_BYTES {
        return Ok(None);
    }
    let bytes = fs::read(path)?;
    if bytes.contains(&0) {
        return Ok(None);
    }
    Ok(std::str::from_utf8(&bytes).ok().map(ToOwned::to_owned))
}

fn replace_directory<F>(
    destination: &Path,
    root: &Path,
    expected_revision: &str,
    staged: PathBuf,
    revision: F,
) -> Result<(), WorkspaceError>
where
    F: Fn(&Path) -> Result<String, WorkspaceError>,
{
    replace_directory_with_finalize(
        destination,
        root,
        expected_revision,
        staged,
        revision,
        |_| Ok(()),
    )
}

fn replace_directory_with_finalize<F, C>(
    destination: &Path,
    root: &Path,
    expected_revision: &str,
    staged: PathBuf,
    revision: F,
    finalize: C,
) -> Result<(), WorkspaceError>
where
    F: Fn(&Path) -> Result<String, WorkspaceError>,
    C: FnOnce(&Path) -> Result<(), WorkspaceError>,
{
    let canonical_root = fs::canonicalize(root)?;
    let canonical_destination = fs::canonicalize(destination)?;
    let metadata = fs::symlink_metadata(destination)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || !canonical_destination.starts_with(&canonical_root)
    {
        let _ = fs::remove_dir_all(staged);
        return Err(WorkspaceError::UnsafePath);
    }
    match replace_directory_atomically_with_finalize(
        &staged,
        destination,
        expected_revision,
        |isolated| revision(isolated).map_err(|error| std::io::Error::other(error.to_string())),
        || finalize(destination).map_err(|error| std::io::Error::other(error.to_string())),
    ) {
        Ok(_) => Ok(()),
        Err(failure) => {
            if !failure.retain_prepared_directory {
                let _ = fs::remove_dir_all(&staged);
            }
            if failure.boundary_changed {
                Err(WorkspaceError::DirectoryChanged)
            } else {
                Err(WorkspaceError::Io(failure.error))
            }
        }
    }
}

fn valid_relative_path(value: &str) -> Result<PathBuf, WorkspaceError> {
    if value.is_empty()
        || value.contains('\0')
        || value.contains('\\')
        || super::is_ignored_skill_metadata_path(value)
    {
        return Err(WorkspaceError::InvalidPackagePath);
    }
    let path = Path::new(value);
    if path.is_absolute()
        || path.components().count() > MAX_DEPTH
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(WorkspaceError::InvalidPackagePath);
    }
    Ok(path.to_path_buf())
}

fn contained_existing_path(root: &Path, relative: &Path) -> Result<PathBuf, WorkspaceError> {
    let canonical_root = fs::canonicalize(root)?;
    let target = fs::canonicalize(root.join(relative))?;
    if !target.starts_with(&canonical_root) {
        return Err(WorkspaceError::UnsafePath);
    }
    let metadata = fs::symlink_metadata(root.join(relative))?;
    if metadata.file_type().is_symlink() {
        return Err(WorkspaceError::UnsafePath);
    }
    Ok(target)
}

fn contained_target(root: &Path, relative: &Path) -> Result<PathBuf, WorkspaceError> {
    let target = root.join(relative);
    let mut ancestor = target.parent().ok_or(WorkspaceError::InvalidPackagePath)?;
    while !ancestor.exists() {
        ancestor = ancestor
            .parent()
            .ok_or(WorkspaceError::InvalidPackagePath)?;
    }
    let canonical_root = fs::canonicalize(root)?;
    let canonical_ancestor = fs::canonicalize(ancestor)?;
    if !canonical_ancestor.starts_with(&canonical_root) {
        return Err(WorkspaceError::UnsafePath);
    }
    Ok(target)
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn media_type(path: &str, bytes: &[u8]) -> String {
    let extension = Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let image = match extension.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        _ => None,
    };
    if let Some(value) = image {
        return value.into();
    }
    if std::str::from_utf8(bytes).is_ok() && !bytes.contains(&0) {
        "text".into()
    } else {
        "binary".into()
    }
}

fn editable_text(media: &str, size: u64) -> bool {
    media == "text" && size <= MAX_TEXT_PREVIEW_BYTES
}

fn content_is_utf8(path: &Path) -> bool {
    fs::read(path).is_ok_and(|bytes| !bytes.contains(&0) && std::str::from_utf8(&bytes).is_ok())
}

fn read_prefix(path: &Path, limit: usize) -> Result<Vec<u8>, WorkspaceError> {
    let mut file = File::open(path)?;
    let mut bytes = vec![0; limit];
    let count = file.read(&mut bytes)?;
    bytes.truncate(count);
    Ok(bytes)
}

fn hash_file(path: &Path) -> Result<String, WorkspaceError> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn file_hash_if_file(path: &Path) -> Result<Option<String>, WorkspaceError> {
    if path.is_file() {
        Ok(Some(hash_file(path)?))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate_operation<'a>(
        action: CandidateFileSyncAction,
        remote: Option<(&'a [u8], bool)>,
    ) -> CandidateFileSyncOperation<'a> {
        CandidateFileSyncOperation::new(action, remote)
    }

    fn fixture() -> (tempfile::TempDir, Workspace, String) {
        let directory = tempfile::tempdir().unwrap();
        let skill = directory.path().join("skills/demo");
        fs::create_dir_all(skill.join("references")).unwrap();
        fs::write(
            skill.join("SKILL.md"),
            "---\nname: demo\ndescription: Use when demo is requested.\n---\n\n# Demo\n\n1. Work carefully.\n",
        )
        .unwrap();
        fs::write(skill.join("references/guide.md"), "old").unwrap();
        let workspace = Workspace::new(directory.path().to_path_buf());
        let id = workspace.list_skills().unwrap().skills[0].id.clone();
        (directory, workspace, id)
    }

    #[test]
    fn snapshots_and_reads_complete_package() {
        let (_directory, workspace, id) = fixture();
        let snapshot = workspace.get_skill_package(&id).unwrap();
        assert_eq!(snapshot.total_files, 2);
        assert!(snapshot
            .entries
            .iter()
            .any(|entry| entry.path == "references/guide.md"));
        let content = workspace
            .read_skill_package_file(&id, &snapshot.revision, "references/guide.md")
            .unwrap();
        assert_eq!(content.content.as_deref(), Some("old"));
        assert!(content.editable);
    }

    #[test]
    fn snapshots_and_revisions_ignore_finder_metadata() {
        let (directory, workspace, id) = fixture();
        let before = workspace.get_skill_package(&id).unwrap();
        let skill = directory.path().join("skills/demo");
        fs::write(skill.join(".DS_Store"), b"root metadata").unwrap();
        fs::write(skill.join("references/.DS_Store"), b"nested metadata").unwrap();
        let after = workspace.get_skill_package(&id).unwrap();
        assert_eq!(after.revision, before.revision);
        assert_eq!(after.total_files, before.total_files);
        assert!(after
            .entries
            .iter()
            .all(|entry| !entry.path.contains(".DS_Store")));
        assert!(matches!(
            workspace.preview_skill_package(
                &id,
                &after.revision,
                &[PackageMutation::Write {
                    path: "references/.DS_Store".into(),
                    content: "hidden".into(),
                }],
            ),
            Err(WorkspaceError::InvalidPackagePath)
        ));

        let mutations = [PackageMutation::Write {
            path: "references/guide.md".into(),
            content: "updated".into(),
        }];
        let preview = workspace
            .preview_skill_package(&id, &after.revision, &mutations)
            .unwrap();
        workspace
            .save_skill_package(&id, &after.revision, &preview.proposed_revision, &mutations)
            .unwrap();
        assert_eq!(fs::read(skill.join(".DS_Store")).unwrap(), b"root metadata");
        assert_eq!(
            fs::read(skill.join("references/.DS_Store")).unwrap(),
            b"nested metadata"
        );
    }

    #[test]
    fn previews_and_applies_text_and_tree_changes() {
        let (_directory, workspace, id) = fixture();
        let snapshot = workspace.get_skill_package(&id).unwrap();
        let mutations = vec![
            PackageMutation::Write {
                path: "references/guide.md".into(),
                content: "new".into(),
            },
            PackageMutation::Write {
                path: "assets/note.txt".into(),
                content: "asset".into(),
            },
        ];
        let preview = workspace
            .preview_skill_package(&id, &snapshot.revision, &mutations)
            .unwrap();
        assert!(preview.can_apply);
        assert_eq!(preview.changes.len(), 2);
        let modified = preview
            .changes
            .iter()
            .find(|change| change.path == "references/guide.md")
            .unwrap();
        assert_eq!(modified.before_text.as_deref(), Some("old"));
        assert_eq!(modified.after_text.as_deref(), Some("new"));
        let saved = workspace
            .save_skill_package(
                &id,
                &snapshot.revision,
                &preview.proposed_revision,
                &mutations,
            )
            .unwrap();
        assert_eq!(saved.snapshot.total_files, 3);
        assert_eq!(
            fs::read_to_string(_directory.path().join("skills/demo/references/guide.md")).unwrap(),
            "new"
        );
    }

    #[test]
    fn rejects_traversal_and_removing_skill_document() {
        let (_directory, workspace, id) = fixture();
        let snapshot = workspace.get_skill_package(&id).unwrap();
        let traversal = vec![PackageMutation::Write {
            path: "../escape".into(),
            content: "bad".into(),
        }];
        assert!(matches!(
            workspace.preview_skill_package(&id, &snapshot.revision, &traversal),
            Err(WorkspaceError::InvalidPackagePath)
        ));
        let removal = vec![PackageMutation::Delete {
            path: "SKILL.md".into(),
        }];
        assert!(matches!(
            workspace.preview_skill_package(&id, &snapshot.revision, &removal),
            Err(WorkspaceError::MissingSkillDocument)
        ));
    }

    #[test]
    fn preview_rejects_mutations_that_push_the_package_over_file_limits() {
        let (directory, workspace, id) = fixture();
        let snapshot = workspace.get_skill_package(&id).unwrap();
        let mutations = (0..MAX_FILES)
            .map(|index| PackageMutation::Write {
                path: format!("references/overflow-{index:03}.md"),
                content: "bounded".into(),
            })
            .collect::<Vec<_>>();

        let result = workspace.preview_skill_package(&id, &snapshot.revision, &mutations);

        assert!(matches!(result, Err(WorkspaceError::PackageTooLarge)));
        assert!(!directory
            .path()
            .join("skills/demo/references/overflow-000.md")
            .exists());
    }

    #[test]
    fn typed_write_rejects_replacing_an_existing_binary_file_with_text() {
        let (directory, workspace, id) = fixture();
        let binary = directory.path().join("skills/demo/assets/data.bin");
        fs::create_dir_all(binary.parent().unwrap()).unwrap();
        fs::write(&binary, [b'a', 0x00, b'b']).unwrap();
        let snapshot = workspace.get_skill_package(&id).unwrap();
        let mutations = vec![PackageMutation::Write {
            path: "assets/data.bin".into(),
            content: "replacement".into(),
        }];

        let result = workspace.preview_skill_package(&id, &snapshot.revision, &mutations);

        assert!(result.is_err());
        assert_eq!(fs::read(&binary).unwrap(), [b'a', 0x00, b'b']);
    }

    #[test]
    fn stale_revision_never_overwrites_external_changes() {
        let (directory, workspace, id) = fixture();
        let snapshot = workspace.get_skill_package(&id).unwrap();
        fs::write(
            directory.path().join("skills/demo/references/guide.md"),
            "external",
        )
        .unwrap();
        let mutations = vec![PackageMutation::Write {
            path: "references/guide.md".into(),
            content: "ours".into(),
        }];
        assert!(matches!(
            workspace.preview_skill_package(&id, &snapshot.revision, &mutations),
            Err(WorkspaceError::DirectoryChanged)
        ));
        assert_eq!(
            fs::read_to_string(directory.path().join("skills/demo/references/guide.md")).unwrap(),
            "external"
        );
    }

    #[test]
    fn imports_binary_files_and_binds_the_selected_source_hash() {
        let (directory, workspace, id) = fixture();
        let source = directory.path().join("selected.png");
        fs::write(&source, [0x89, b'P', b'N', b'G', 0, 1, 2]).unwrap();
        let inspected = workspace.inspect_package_import_source(&source).unwrap();
        let snapshot = workspace.get_skill_package(&id).unwrap();
        let mutations = vec![PackageMutation::CopyFile {
            path: "assets/selected.png".into(),
            source_path: inspected.source_path.clone(),
            expected_hash: inspected.content_hash.clone(),
            expected_size: inspected.size,
        }];
        let preview = workspace
            .preview_skill_package(&id, &snapshot.revision, &mutations)
            .unwrap();
        assert!(preview.can_apply);
        workspace
            .save_skill_package(
                &id,
                &snapshot.revision,
                &preview.proposed_revision,
                &mutations,
            )
            .unwrap();
        assert_eq!(
            fs::read(directory.path().join("skills/demo/assets/selected.png")).unwrap(),
            [0x89, b'P', b'N', b'G', 0, 1, 2]
        );

        let next_snapshot = workspace.get_skill_package(&id).unwrap();
        fs::write(&source, b"replaced").unwrap();
        assert!(matches!(
            workspace.preview_skill_package(&id, &next_snapshot.revision, &mutations),
            Err(WorkspaceError::DirectoryChanged)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn package_validation_reports_executables_outside_the_scripts_directory() {
        use std::os::unix::fs::PermissionsExt;

        let (directory, workspace, id) = fixture();
        let executable = directory.path().join("skills/demo/assets/tool");
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::write(&executable, "#!/bin/sh\necho demo\n").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();

        let snapshot = workspace.get_skill_package(&id).unwrap();
        assert!(snapshot.validations.iter().any(|validation| {
            validation.code == "executable-file"
                && validation.path.as_deref() == Some("assets/tool")
                && validation.severity == "warning"
        }));
    }

    #[test]
    fn package_validation_maps_support_file_safety_evidence_to_its_path() {
        let (directory, workspace, id) = fixture();
        let support = directory.path().join("skills/demo/references/unsafe.md");
        fs::write(&support, "Run `rm -rf ~/Documents/archive` after export.\n").unwrap();

        let snapshot = workspace.get_skill_package(&id).unwrap();
        assert!(snapshot.validations.iter().any(|validation| {
            validation.code == "destructive-filesystem"
                && validation.path.as_deref() == Some("references/unsafe.md")
                && validation.severity == "blocker"
        }));
    }

    #[test]
    fn candidate_file_sync_adds_replaces_and_deletes_after_revision_bound_preview() {
        let directory = tempfile::tempdir().unwrap();
        let workspace = Workspace::new(directory.path().join("codex"));
        let skill_root = directory.path().join("codex/skills/demo");
        fs::create_dir_all(skill_root.join("references")).unwrap();
        fs::write(
            skill_root.join("SKILL.md"),
            "---\nname: demo\ndescription: Use when testing sync.\n---\n",
        )
        .unwrap();
        fs::write(skill_root.join("references/old.md"), "old\n").unwrap();
        fs::write(skill_root.join(".DS_Store"), b"finder metadata").unwrap();
        let id = workspace
            .list_skills()
            .unwrap()
            .skills
            .into_iter()
            .find(|skill| skill.source == "personal")
            .unwrap()
            .id;
        let snapshot = workspace.get_skill_package(&id).unwrap();
        let preview = workspace
            .preview_candidate_file_sync(
                &id,
                &snapshot.revision,
                "references/new.md",
                candidate_operation(CandidateFileSyncAction::Add, Some((b"new\n", false))),
            )
            .unwrap();
        assert_eq!(preview.changes[0].kind, "added");
        assert!(!skill_root.join("references/new.md").exists());
        let added = workspace
            .apply_candidate_file_sync(
                &id,
                &snapshot.revision,
                &preview.proposed_revision,
                "references/new.md",
                candidate_operation(CandidateFileSyncAction::Add, Some((b"new\n", false))),
            )
            .unwrap();
        assert_eq!(
            fs::read_to_string(skill_root.join("references/new.md")).unwrap(),
            "new\n"
        );
        let replace_preview = workspace
            .preview_candidate_file_sync(
                &id,
                &added.snapshot.revision,
                "references/new.md",
                candidate_operation(
                    CandidateFileSyncAction::Replace,
                    Some((b"remote replacement\n", true)),
                ),
            )
            .unwrap();
        assert_eq!(replace_preview.changes[0].kind, "modified");
        assert_eq!(
            fs::read_to_string(skill_root.join("references/new.md")).unwrap(),
            "new\n"
        );
        let replaced = workspace
            .apply_candidate_file_sync(
                &id,
                &added.snapshot.revision,
                &replace_preview.proposed_revision,
                "references/new.md",
                candidate_operation(
                    CandidateFileSyncAction::Replace,
                    Some((b"remote replacement\n", true)),
                ),
            )
            .unwrap();
        assert_eq!(
            fs::read_to_string(skill_root.join("references/new.md")).unwrap(),
            "remote replacement\n"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_ne!(
                fs::metadata(skill_root.join("references/new.md"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o111,
                0
            );
        }
        let delete_preview = workspace
            .preview_candidate_file_sync(
                &id,
                &replaced.snapshot.revision,
                "references/old.md",
                candidate_operation(CandidateFileSyncAction::Delete, None),
            )
            .unwrap();
        assert_eq!(delete_preview.changes[0].kind, "deleted");
        workspace
            .apply_candidate_file_sync(
                &id,
                &replaced.snapshot.revision,
                &delete_preview.proposed_revision,
                "references/old.md",
                candidate_operation(CandidateFileSyncAction::Delete, None),
            )
            .unwrap();
        assert!(!skill_root.join("references/old.md").exists());
        assert_eq!(
            fs::read(skill_root.join(".DS_Store")).unwrap(),
            b"finder metadata"
        );
    }

    #[test]
    fn candidate_file_sync_rejects_overwrite_skill_deletion_and_stale_revision() {
        let directory = tempfile::tempdir().unwrap();
        let workspace = Workspace::new(directory.path().join("codex"));
        let skill_root = directory.path().join("codex/skills/demo");
        fs::create_dir_all(&skill_root).unwrap();
        fs::write(
            skill_root.join("SKILL.md"),
            "---\nname: demo\ndescription: Use when testing sync guards.\n---\n",
        )
        .unwrap();
        fs::write(skill_root.join("existing.md"), "local\n").unwrap();
        let id = workspace
            .list_skills()
            .unwrap()
            .skills
            .into_iter()
            .find(|skill| skill.source == "personal")
            .unwrap()
            .id;
        let revision = workspace.get_skill_package(&id).unwrap().revision;
        assert!(matches!(
            workspace.preview_candidate_file_sync(
                &id,
                &revision,
                "missing.md",
                candidate_operation(CandidateFileSyncAction::Replace, Some((b"remote\n", false)),),
            ),
            Err(WorkspaceError::PackagePathConflict)
        ));
        assert!(matches!(
            workspace.preview_candidate_file_sync(
                &id,
                &revision,
                "existing.md",
                candidate_operation(CandidateFileSyncAction::Add, Some((b"remote\n", false))),
            ),
            Err(WorkspaceError::PackagePathConflict)
        ));
        assert!(matches!(
            workspace.preview_candidate_file_sync(
                &id,
                &revision,
                "SKILL.md",
                candidate_operation(CandidateFileSyncAction::Delete, None),
            ),
            Err(WorkspaceError::MissingSkillDocument)
        ));
        let invalid_skill = workspace
            .preview_candidate_file_sync(
                &id,
                &revision,
                "SKILL.md",
                candidate_operation(
                    CandidateFileSyncAction::Replace,
                    Some((
                        b"---\nname: another\ndescription: Wrong identity.\n---\n",
                        false,
                    )),
                ),
            )
            .unwrap();
        assert!(!invalid_skill.can_apply);
        let preview = workspace
            .preview_candidate_file_sync(
                &id,
                &revision,
                "new.md",
                candidate_operation(CandidateFileSyncAction::Add, Some((b"new\n", false))),
            )
            .unwrap();
        fs::write(skill_root.join("external.md"), "changed\n").unwrap();
        assert!(matches!(
            workspace.apply_candidate_file_sync(
                &id,
                &revision,
                &preview.proposed_revision,
                "new.md",
                candidate_operation(CandidateFileSyncAction::Add, Some((b"new\n", false))),
            ),
            Err(WorkspaceError::DirectoryChanged)
        ));
        assert!(!skill_root.join("new.md").exists());
    }

    #[test]
    fn candidate_file_sync_restores_external_edits_detected_at_the_commit_boundary() {
        let directory = tempfile::tempdir().unwrap();
        let workspace = Workspace::new(directory.path().join("codex"));
        let skill_root = directory.path().join("codex/skills/demo");
        fs::create_dir_all(&skill_root).unwrap();
        fs::write(
            skill_root.join("SKILL.md"),
            "---\nname: demo\ndescription: Use when testing final sync checks.\n---\n",
        )
        .unwrap();
        fs::write(skill_root.join("local.md"), "opened\n").unwrap();
        let id = workspace.list_skills().unwrap().skills[0].id.clone();
        let revision = workspace.get_skill_package(&id).unwrap().revision;
        let preview = workspace
            .preview_candidate_file_sync(
                &id,
                &revision,
                "remote.md",
                candidate_operation(CandidateFileSyncAction::Add, Some((b"remote\n", false))),
            )
            .unwrap();

        let result = workspace.apply_candidate_file_sync_with_final_check(
            &id,
            &revision,
            &preview.proposed_revision,
            "remote.md",
            candidate_operation(CandidateFileSyncAction::Add, Some((b"remote\n", false))),
            || {
                fs::write(skill_root.join("local.md"), "external\n")?;
                Ok(())
            },
        );

        assert!(matches!(result, Err(WorkspaceError::DirectoryChanged)));
        assert_eq!(
            fs::read_to_string(skill_root.join("local.md")).unwrap(),
            "external\n"
        );
        assert!(!skill_root.join("remote.md").exists());
    }

    #[test]
    fn large_catalog_package_preview_and_save_have_bounded_io_invocations() {
        let directory = tempfile::tempdir().unwrap();
        let workspace = Workspace::new(directory.path().to_path_buf());
        for index in 0..121 {
            let name = format!("skill-{index:03}");
            let root = directory.path().join("skills").join(&name);
            fs::create_dir_all(&root).unwrap();
            fs::write(
                root.join("SKILL.md"),
                format!(
                    "---\nname: {name}\ndescription: Use when testing Package performance.\n---\n\n# Test\n\nInspect and return grounded evidence.\n"
                ),
            )
            .unwrap();
        }
        let skill = workspace.list_skills().unwrap().skills[60].clone();
        let snapshot = workspace.get_skill_package(&skill.id).unwrap();
        let mutations = [PackageMutation::Write {
            path: "references/guide.md".into(),
            content: "bounded\n".into(),
        }];
        let preview = workspace
            .preview_skill_package(&skill.id, &snapshot.revision, &mutations)
            .unwrap();
        workspace
            .save_skill_package(
                &skill.id,
                &snapshot.revision,
                &preview.proposed_revision,
                &mutations,
            )
            .unwrap();

        let metrics = workspace.metrics_snapshot();
        assert_eq!(metrics.full_scans, 1);
        assert_eq!(metrics.skill_reads, 122);
        assert_eq!(metrics.baseline_audits, metrics.skill_reads);
        assert_eq!(metrics.package_revisions, 7);
        assert!(metrics.package_revision_nanos > 0);
    }

    #[test]
    #[ignore = "read-only benchmark for the project owner's real Codex catalog"]
    fn owner_catalog_package_open_timing() {
        let codex_home = std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex")))
            .expect("CODEX_HOME or HOME");
        let workspace = Workspace::new(codex_home);
        let started = std::time::Instant::now();
        let catalog = workspace.list_skills().unwrap();
        let indexed = started.elapsed();
        let personal = catalog
            .skills
            .iter()
            .find(|skill| skill.source == "personal")
            .expect("at least one personal Skill");
        let opened = std::time::Instant::now();
        let snapshot = workspace.get_skill_package(&personal.id).unwrap();
        let package_open = opened.elapsed();
        let metrics = workspace.metrics_snapshot();

        eprintln!(
            "owner_catalog package_performance indexed_skills={} catalog_ms={} package_files={} package_open_ms={} full_scans={} package_revisions={}",
            catalog.skills.len(),
            indexed.as_millis(),
            snapshot.total_files,
            package_open.as_millis(),
            metrics.full_scans,
            metrics.package_revisions,
        );
        assert_eq!(metrics.full_scans, 1);
        assert_eq!(metrics.package_revisions, 1);
    }
}
