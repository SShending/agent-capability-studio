use super::{is_ignored_skill_metadata_path, AuditResult, Diff, Finding, SkillDocument};
use serde::Serialize;
use sha2::{Digest, Sha256};
use skill_bundle_core::{
    skill_revision, visit_bundle_files, BundleError, BundleFile, BundleManifest, BundleSkill,
    MAX_ARCHIVE_BYTES,
};
use std::{
    collections::HashMap,
    ffi::CString,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
};
use tempfile::Builder;
use thiserror::Error;

pub(super) const MAX_PREVIEW_BYTES: usize = 512 * 1024;

#[derive(Debug, Error)]
pub enum BundleImportError {
    #[error("Select a regular .skillbundle file.")]
    InvalidSource,
    #[error("The selected Bundle changed while it was being copied. Select it again.")]
    SourceChanged,
    #[error("This Bundle import session is missing or no longer current.")]
    UnknownSession,
    #[error("The staged Bundle content changed after verification. Import it again.")]
    ChangedSession,
    #[error("The selected Bundle exceeds the accepted resource limits.")]
    LimitExceeded,
    #[error("The Skill Bundle could not be verified: {0}")]
    Bundle(#[from] BundleError),
    #[error("The Skill Bundle could not be staged.")]
    Io(#[from] std::io::Error),
}

impl BundleImportError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidSource => "BUNDLE_IMPORT_SOURCE_INVALID",
            Self::SourceChanged => "BUNDLE_IMPORT_SOURCE_CHANGED",
            Self::UnknownSession => "BUNDLE_IMPORT_SESSION_UNKNOWN",
            Self::ChangedSession => "BUNDLE_IMPORT_SESSION_CHANGED",
            Self::LimitExceeded => "BUNDLE_LIMIT_EXCEEDED",
            Self::Bundle(error) => error.code(),
            Self::Io(_) => "BUNDLE_IMPORT_IO_ERROR",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleImportReview {
    pub session_id: String,
    pub source_file_name: String,
    pub source_revision: String,
    pub bundle_revision: String,
    pub skills: Vec<ImportedSkillReview>,
    pub total_files: usize,
    pub total_bytes: u64,
    pub installed: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedSkillReview {
    pub directory_name: String,
    pub revision: String,
    pub files: Vec<ImportedFile>,
    pub compatibility: ImportCompatibility,
    pub audit: AuditResult,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
    pub executable_after_install: bool,
    pub staged_executable: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCompatibility {
    pub agent: String,
    pub status: String,
    pub summary: String,
    pub checks: Vec<ImportCompatibilityCheck>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCompatibilityCheck {
    pub id: String,
    pub label: String,
    pub status: String,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleImportFileContent {
    pub directory_name: String,
    pub path: String,
    pub content: Option<String>,
    pub is_text: bool,
    pub truncated: bool,
    pub preview_bytes: usize,
}

#[derive(Clone)]
pub struct BundleImportManager {
    store: Arc<ImportStore>,
}

struct ImportStore {
    root: PathBuf,
    sessions: Mutex<HashMap<String, ImportSession>>,
}

#[derive(Clone)]
struct ImportSession {
    verified_manifest: BundleManifest,
    manifest: BundleManifest,
    bundle_revision: String,
    review: BundleImportReview,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceIdentity {
    len: u64,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    mode: u32,
    #[cfg(unix)]
    modified_seconds: i64,
    #[cfg(unix)]
    modified_nanoseconds: i64,
}

impl Drop for ImportStore {
    fn drop(&mut self) {
        let _ = remove_staging_path(&self.root);
    }
}

impl BundleImportManager {
    pub fn new(root: PathBuf) -> Result<Self, BundleImportError> {
        let root = prepare_staging_root(&root)?;
        Ok(Self {
            store: Arc::new(ImportStore {
                root,
                sessions: Mutex::new(HashMap::new()),
            }),
        })
    }

    pub fn stage(&self, source: &Path) -> Result<BundleImportReview, BundleImportError> {
        if source.extension().and_then(|value| value.to_str()) != Some("skillbundle") {
            return Err(BundleImportError::InvalidSource);
        }
        let temporary = Builder::new()
            .prefix("bundle-import-")
            .tempdir_in(&self.store.root)?;
        let session_path = temporary.keep();
        let session_id = session_path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or(BundleImportError::InvalidSource)?
            .to_owned();
        let result = self.stage_into_session(source, &session_id, &session_path);
        match result {
            Ok((review, session)) => {
                self.store
                    .sessions
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .insert(session_id, session);
                Ok(review)
            }
            Err(error) => {
                let _ = remove_staging_path(&session_path);
                Err(error)
            }
        }
    }

    pub(super) fn verified_review(
        &self,
        session_id: &str,
        expected_bundle_revision: &str,
    ) -> Result<BundleImportReview, BundleImportError> {
        let session = self.session(session_id, expected_bundle_revision)?;
        let content_root = self.session_path(session_id)?.join("content");
        for skill in &session.verified_manifest.skills {
            for file in &skill.files {
                read_staged_file(&content_root, &skill.directory_name, file)?;
            }
        }
        Ok(session.review)
    }

    pub(super) fn verified_file_bytes(
        &self,
        session_id: &str,
        expected_bundle_revision: &str,
        directory_name: &str,
        path: &str,
    ) -> Result<Vec<u8>, BundleImportError> {
        let session = self.session(session_id, expected_bundle_revision)?;
        let skill = session
            .manifest
            .skills
            .iter()
            .find(|skill| skill.directory_name == directory_name)
            .ok_or(BundleImportError::ChangedSession)?;
        let file = skill
            .files
            .iter()
            .find(|file| file.path == path)
            .ok_or(BundleImportError::ChangedSession)?;
        read_staged_file(
            &self.session_path(session_id)?.join("content"),
            directory_name,
            file,
        )
    }

    pub fn read_file(
        &self,
        session_id: &str,
        expected_bundle_revision: &str,
        directory_name: &str,
        path: &str,
    ) -> Result<BundleImportFileContent, BundleImportError> {
        let session = self.session(session_id, expected_bundle_revision)?;
        let skill = session
            .manifest
            .skills
            .iter()
            .find(|skill| skill.directory_name == directory_name)
            .ok_or(BundleImportError::ChangedSession)?;
        let file = skill
            .files
            .iter()
            .find(|file| file.path == path)
            .ok_or(BundleImportError::ChangedSession)?;
        let preview = read_staged_file_preview(
            &self.session_path(session_id)?.join("content"),
            directory_name,
            file,
        )?;
        Ok(BundleImportFileContent {
            directory_name: directory_name.into(),
            path: path.into(),
            preview_bytes: preview.content.len(),
            content: preview.is_text.then(|| {
                String::from_utf8(preview.content)
                    .expect("stream validation keeps the preview on a UTF-8 boundary")
            }),
            is_text: preview.is_text,
            truncated: preview.truncated,
        })
    }

    pub fn discard(&self, session_id: &str) -> Result<(), BundleImportError> {
        let removed = self
            .store
            .sessions
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(session_id)
            .ok_or(BundleImportError::UnknownSession)?;
        drop(removed);
        remove_staging_path(&self.session_path(session_id)?)?;
        Ok(())
    }

    fn stage_into_session(
        &self,
        source: &Path,
        session_id: &str,
        session_path: &Path,
    ) -> Result<(BundleImportReview, ImportSession), BundleImportError> {
        let staged_archive = session_path.join("source.skillbundle");
        let source_revision = copy_selected_bundle(source, &staged_archive)?;
        let content_root = session_path.join("content");
        fs::create_dir(&content_root)?;
        let mut archive = File::open(&staged_archive)?;
        let inspection = visit_bundle_files(&mut archive, |skill, file, reader| {
            write_staged_file(&content_root, skill, file, reader)
        })?;
        let manifest = logical_manifest(&inspection.manifest)?;
        let mut skills = Vec::with_capacity(manifest.skills.len());
        for skill in &manifest.skills {
            let root_file = skill
                .files
                .iter()
                .find(|file| file.path == "SKILL.md")
                .ok_or(BundleImportError::ChangedSession)?;
            let skill_bytes = read_staged_file(&content_root, &skill.directory_name, root_file)?;
            let audit = audit_skill_bytes(&skill_bytes, &skill.directory_name);
            let compatibility = compatibility_for(skill, &audit);
            skills.push(ImportedSkillReview {
                directory_name: skill.directory_name.clone(),
                revision: skill.revision.clone(),
                files: skill.files.iter().map(imported_file).collect(),
                compatibility,
                audit,
            });
        }
        let review = BundleImportReview {
            session_id: session_id.into(),
            source_file_name: source
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("selected.skillbundle")
                .into(),
            source_revision,
            bundle_revision: inspection.bundle_revision.clone(),
            skills,
            total_files: manifest.skills.iter().map(|skill| skill.files.len()).sum(),
            total_bytes: manifest
                .skills
                .iter()
                .flat_map(|skill| &skill.files)
                .map(|file| file.size)
                .sum(),
            installed: false,
        };
        let session = ImportSession {
            verified_manifest: inspection.manifest,
            manifest,
            bundle_revision: inspection.bundle_revision,
            review: review.clone(),
        };
        Ok((review, session))
    }

    fn session(
        &self,
        session_id: &str,
        expected_bundle_revision: &str,
    ) -> Result<ImportSession, BundleImportError> {
        let session = self
            .store
            .sessions
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(session_id)
            .cloned()
            .ok_or(BundleImportError::UnknownSession)?;
        if expected_bundle_revision.is_empty()
            || session.bundle_revision != expected_bundle_revision
        {
            return Err(BundleImportError::ChangedSession);
        }
        Ok(session)
    }

    fn session_path(&self, session_id: &str) -> Result<PathBuf, BundleImportError> {
        if session_id.is_empty()
            || session_id.contains(['/', '\\'])
            || Path::new(session_id)
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(BundleImportError::UnknownSession);
        }
        let path = self.store.root.join(session_id);
        if path.parent() != Some(self.store.root.as_path()) {
            return Err(BundleImportError::UnknownSession);
        }
        Ok(path)
    }
}

fn logical_manifest(verified: &BundleManifest) -> Result<BundleManifest, BundleImportError> {
    let mut manifest = verified.clone();
    for skill in &mut manifest.skills {
        skill
            .files
            .retain(|file| !is_ignored_skill_metadata_path(&file.path));
        skill.revision = skill_revision(&skill.files)?;
    }
    Ok(manifest)
}

fn copy_selected_bundle(source: &Path, destination: &Path) -> Result<String, BundleImportError> {
    let metadata = fs::symlink_metadata(source).map_err(|_| BundleImportError::InvalidSource)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(BundleImportError::InvalidSource);
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let mut source = options
        .open(source)
        .map_err(|_| BundleImportError::InvalidSource)?;
    let opened_metadata = source
        .metadata()
        .map_err(|_| BundleImportError::InvalidSource)?;
    if !opened_metadata.is_file() || source_identity(&metadata) != source_identity(&opened_metadata)
    {
        return Err(BundleImportError::SourceChanged);
    }
    let identity = source_identity(&opened_metadata);
    let mut target = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)?;
    let mut digest = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let count = source.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or(BundleImportError::LimitExceeded)?;
        if total > MAX_ARCHIVE_BYTES {
            return Err(BundleImportError::LimitExceeded);
        }
        digest.update(&buffer[..count]);
        target.write_all(&buffer[..count])?;
    }
    target.flush()?;
    target.sync_all()?;
    if source_identity(&source.metadata()?) != identity {
        return Err(BundleImportError::SourceChanged);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn write_staged_file(
    content_root: &Path,
    skill: &BundleSkill,
    expected: &BundleFile,
    reader: &mut dyn Read,
) -> std::io::Result<()> {
    let mut target = open_staged_target(content_root, &skill.directory_name, &expected.path)?;
    let mut digest = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or_else(|| std::io::Error::other("staged size overflow"))?;
        if total > expected.size {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "staged size mismatch",
            ));
        }
        digest.update(&buffer[..count]);
        target.write_all(&buffer[..count])?;
    }
    if total != expected.size || format!("{:x}", digest.finalize()) != expected.sha256 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "staged evidence mismatch",
        ));
    }
    target.flush()?;
    target.sync_all()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        target.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn create_contained_directories(root: &Path, destination: &Path) -> std::io::Result<()> {
    let relative = destination
        .strip_prefix(root)
        .map_err(|_| std::io::Error::other("escaped staging root"))?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err(std::io::Error::other("invalid staging component"));
        };
        current.push(name);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(std::io::Error::other("unsafe staging directory"));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&current)?;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn read_staged_file(
    content_root: &Path,
    directory_name: &str,
    expected: &BundleFile,
) -> Result<Vec<u8>, BundleImportError> {
    let file = open_staged_existing(content_root, directory_name, &expected.path)
        .map_err(|_| BundleImportError::ChangedSession)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() != expected.size {
        return Err(BundleImportError::ChangedSession);
    }
    let mut bytes = Vec::with_capacity(expected.size as usize);
    file.take(expected.size + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 != expected.size
        || format!("{:x}", Sha256::digest(&bytes)) != expected.sha256
    {
        return Err(BundleImportError::ChangedSession);
    }
    Ok(bytes)
}

struct StagedFilePreview {
    content: Vec<u8>,
    is_text: bool,
    truncated: bool,
}

#[derive(Default)]
struct Utf8StreamValidator {
    pending: Vec<u8>,
    valid: bool,
}

impl Utf8StreamValidator {
    fn new() -> Self {
        Self {
            pending: Vec::new(),
            valid: true,
        }
    }

    fn update(&mut self, bytes: &[u8]) {
        if !self.valid {
            return;
        }
        let mut combined = std::mem::take(&mut self.pending);
        combined.extend_from_slice(bytes);
        if let Err(error) = std::str::from_utf8(&combined) {
            if error.error_len().is_some() {
                self.valid = false;
            } else {
                self.pending
                    .extend_from_slice(&combined[error.valid_up_to()..]);
                if self.pending.len() > 3 {
                    self.valid = false;
                }
            }
        }
    }

    fn finish(&self) -> bool {
        self.valid && self.pending.is_empty()
    }
}

fn read_staged_file_preview(
    content_root: &Path,
    directory_name: &str,
    expected: &BundleFile,
) -> Result<StagedFilePreview, BundleImportError> {
    let mut file = open_staged_existing(content_root, directory_name, &expected.path)
        .map_err(|_| BundleImportError::ChangedSession)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() != expected.size {
        return Err(BundleImportError::ChangedSession);
    }

    let mut digest = Sha256::new();
    let mut total = 0_u64;
    let mut preview = Vec::with_capacity((expected.size as usize).min(MAX_PREVIEW_BYTES));
    let mut utf8 = Utf8StreamValidator::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or(BundleImportError::ChangedSession)?;
        if total > expected.size {
            return Err(BundleImportError::ChangedSession);
        }
        digest.update(&buffer[..count]);
        utf8.update(&buffer[..count]);
        if preview.len() < MAX_PREVIEW_BYTES {
            let available = MAX_PREVIEW_BYTES - preview.len();
            preview.extend_from_slice(&buffer[..count.min(available)]);
        }
    }
    if total != expected.size || format!("{:x}", digest.finalize()) != expected.sha256 {
        return Err(BundleImportError::ChangedSession);
    }

    let is_text = utf8.finish();
    if is_text {
        while std::str::from_utf8(&preview).is_err() {
            preview.pop();
        }
    } else {
        preview.clear();
    }
    Ok(StagedFilePreview {
        truncated: is_text && total as usize > preview.len(),
        content: preview,
        is_text,
    })
}

fn staged_components(directory_name: &str, relative: &str) -> std::io::Result<Vec<String>> {
    let mut result = vec!["skills".to_string(), directory_name.to_string()];
    for component in Path::new(relative).components() {
        let Component::Normal(value) = component else {
            return Err(std::io::Error::other("invalid staged path"));
        };
        result.push(
            value
                .to_str()
                .ok_or_else(|| std::io::Error::other("non-UTF-8 staged path"))?
                .to_string(),
        );
    }
    if result.len() < 3 {
        return Err(std::io::Error::other("empty staged path"));
    }
    Ok(result)
}

#[cfg(unix)]
fn open_staged_target(
    content_root: &Path,
    directory_name: &str,
    relative: &str,
) -> std::io::Result<File> {
    let components = staged_components(directory_name, relative)?;
    let mut directory = open_root_directory(content_root)?;
    for component in &components[..components.len() - 1] {
        directory = open_or_create_directory_at(&directory, component)?;
    }
    openat_file(
        &directory,
        components.last().expect("non-empty staged components"),
        libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW,
        0o600,
    )
}

#[cfg(not(unix))]
fn open_staged_target(
    content_root: &Path,
    directory_name: &str,
    relative: &str,
) -> std::io::Result<File> {
    let destination = content_root
        .join("skills")
        .join(directory_name)
        .join(relative);
    if !destination.starts_with(content_root) {
        return Err(std::io::Error::other("escaped staged path"));
    }
    create_contained_directories(
        content_root,
        destination
            .parent()
            .ok_or_else(|| std::io::Error::other("missing staged parent"))?,
    )?;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
}

#[cfg(unix)]
fn open_staged_existing(
    content_root: &Path,
    directory_name: &str,
    relative: &str,
) -> std::io::Result<File> {
    let components = staged_components(directory_name, relative)?;
    let mut directory = open_root_directory(content_root)?;
    for component in &components[..components.len() - 1] {
        directory = openat_file(
            &directory,
            component,
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW,
            0,
        )?;
    }
    openat_file(
        &directory,
        components.last().expect("non-empty staged components"),
        libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK,
        0,
    )
}

#[cfg(not(unix))]
fn open_staged_existing(
    content_root: &Path,
    directory_name: &str,
    relative: &str,
) -> std::io::Result<File> {
    let path = content_root
        .join("skills")
        .join(directory_name)
        .join(relative);
    if !path.starts_with(content_root) {
        return Err(std::io::Error::other("escaped staged path"));
    }
    OpenOptions::new().read(true).open(path)
}

#[cfg(unix)]
fn open_root_directory(path: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
}

#[cfg(unix)]
fn open_or_create_directory_at(parent: &File, name: &str) -> std::io::Result<File> {
    match openat_file(
        parent,
        name,
        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW,
        0,
    ) {
        Ok(directory) => Ok(directory),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            use std::os::fd::AsRawFd;
            let name = CString::new(name)
                .map_err(|_| std::io::Error::other("invalid staged component"))?;
            let result = unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) };
            if result != 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() != std::io::ErrorKind::AlreadyExists {
                    return Err(error);
                }
            }
            openat_file(
                parent,
                name.to_str()
                    .map_err(|_| std::io::Error::other("invalid staged component"))?,
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW,
                0,
            )
        }
        Err(error) => Err(error),
    }
}

#[cfg(unix)]
fn openat_file(parent: &File, name: &str, flags: i32, mode: u32) -> std::io::Result<File> {
    use std::os::{fd::AsRawFd, unix::io::FromRawFd};
    let name = CString::new(name).map_err(|_| std::io::Error::other("invalid staged name"))?;
    let descriptor = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_CLOEXEC,
            mode,
        )
    };
    if descriptor < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(unsafe { File::from_raw_fd(descriptor) })
    }
}

fn imported_file(file: &BundleFile) -> ImportedFile {
    ImportedFile {
        path: file.path.clone(),
        size: file.size,
        sha256: file.sha256.clone(),
        executable_after_install: file.executable,
        staged_executable: false,
    }
}

fn audit_skill_bytes(bytes: &[u8], expected_name: &str) -> AuditResult {
    match std::str::from_utf8(bytes) {
        Ok(markdown) => super::audit(markdown, "", expected_name),
        Err(_) => AuditResult {
            verdict: "block".into(),
            findings: vec![Finding {
                id: "non-text-skill-document".into(),
                severity: "blocker".into(),
                title: "SKILL.md 不是可读取的文本".into(),
                explanation: "Codex Skill 的根说明文件必须是 UTF-8 文本。".into(),
                evidence: "SKILL.md 无法按 UTF-8 文本读取。".into(),
                confidence: "high".into(),
                source: "baseline".into(),
                file_path: Some("SKILL.md".into()),
                line_start: None,
                line_end: None,
                disposition: "confirmed".into(),
                review_note: None,
            }],
            content_hash: format!("{:x}", Sha256::digest(bytes)),
            document: SkillDocument {
                has_frontmatter: false,
                name: String::new(),
                description: String::new(),
                body: String::new(),
            },
            diff: Diff {
                changed: false,
                start_line: 0,
                added_count: 0,
                removed_count: 0,
                before: Vec::new(),
                after: Vec::new(),
                truncated: false,
            },
        },
    }
}

fn compatibility_for(skill: &BundleSkill, audit: &AuditResult) -> ImportCompatibility {
    let non_text = audit
        .findings
        .iter()
        .any(|finding| finding.id == "non-text-skill-document");
    let mut checks = vec![ImportCompatibilityCheck {
        id: "bundle-integrity".into(),
        label: "Bundle 完整性".into(),
        status: "pass".into(),
        detail: "文件路径、大小、SHA-256 与 Bundle revision 已全部验证。".into(),
    }];
    checks.push(ImportCompatibilityCheck {
        id: "skill-document".into(),
        label: "SKILL.md".into(),
        status: if !non_text && audit.document.has_frontmatter {
            "pass"
        } else {
            "fail"
        }
        .into(),
        detail: if !non_text && audit.document.has_frontmatter {
            "根说明文件是 UTF-8 文本并包含 frontmatter。".into()
        } else {
            "根说明文件必须是 UTF-8 文本并包含 frontmatter。".into()
        },
    });
    checks.push(ImportCompatibilityCheck {
        id: "directory-identity".into(),
        label: "目录与文档身份".into(),
        status: if !non_text && audit.document.name == skill.directory_name {
            "pass"
        } else {
            "fail"
        }
        .into(),
        detail: if !non_text && audit.document.name == skill.directory_name {
            format!("目录名与 Skill 名称均为“{}”。", skill.directory_name)
        } else {
            format!(
                "Bundle 目录名“{}”必须与 SKILL.md 名称“{}”一致。",
                skill.directory_name, audit.document.name
            )
        },
    });
    checks.push(ImportCompatibilityCheck {
        id: "description".into(),
        label: "用途与触发条件".into(),
        status: if !non_text && !audit.document.description.trim().is_empty() {
            "pass"
        } else {
            "fail"
        }
        .into(),
        detail: if audit.document.description.trim().is_empty() {
            "缺少用途说明，Codex 无法判断何时使用。".into()
        } else if super::explicit_trigger(&audit.document.name, &audit.document.description) {
            "采用明确点名触发。".into()
        } else {
            "采用按意图触发，这也是受支持的策略。".into()
        },
    });
    checks.push(ImportCompatibilityCheck {
        id: "skill-name".into(),
        label: "Skill 名称".into(),
        status: if !non_text && super::valid_name(&audit.document.name) {
            "pass"
        } else {
            "fail"
        }
        .into(),
        detail: if super::valid_name(&audit.document.name) {
            format!("名称“{}”符合 Codex 命名规则。", audit.document.name)
        } else {
            "名称只能使用小写字母、数字和单个连字符。".into()
        },
    });
    let executable_count = skill.files.iter().filter(|file| file.executable).count();
    checks.push(ImportCompatibilityCheck {
        id: "executable-files".into(),
        label: "可执行支持文件".into(),
        status: if executable_count == 0 {
            "pass"
        } else {
            "review"
        }
        .into(),
        detail: if executable_count == 0 {
            "没有声明可执行支持文件。".into()
        } else {
            format!(
                "声明了 {executable_count} 个可执行文件；暂存期间不会运行，也不会保留执行权限。"
            )
        },
    });
    let status = if checks.iter().any(|check| check.status == "fail") {
        "incompatible"
    } else if checks.iter().any(|check| check.status == "review") {
        "review"
    } else {
        "compatible"
    };
    ImportCompatibility {
        agent: "Codex".into(),
        status: status.into(),
        summary: match status {
            "incompatible" => "该 Skill 不满足 Codex 的基本文档要求。",
            "review" => "结构可读取，但可执行支持文件需要安装前复核。",
            _ => "该 Skill 满足 Codex 的基础结构要求。",
        }
        .into(),
        checks,
    }
}

fn source_identity(metadata: &fs::Metadata) -> SourceIdentity {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        SourceIdentity {
            len: metadata.len(),
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
        }
    }
    #[cfg(not(unix))]
    {
        SourceIdentity {
            len: metadata.len(),
        }
    }
}

fn prepare_staging_root(root: &Path) -> Result<PathBuf, BundleImportError> {
    match fs::symlink_metadata(root) {
        Ok(metadata) if metadata.file_type().is_symlink() || metadata.is_file() => {
            fs::remove_file(root)?;
        }
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(root)?,
        Ok(_) => return Err(BundleImportError::InvalidSource),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    fs::create_dir_all(root)?;
    Ok(fs::canonicalize(root)?)
}

fn remove_staging_path(path: &Path) -> Result<(), BundleImportError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || metadata.is_file() => {
            fs::remove_file(path)?;
        }
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path)?,
        Ok(_) => return Err(BundleImportError::ChangedSession),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use skill_bundle_core::{
        skill_revision, write_bundle, AgentContract, BundleFileReader, BUNDLE_FORMAT,
        BUNDLE_FORMAT_VERSION, CODEX_CONTRACT_ID, CODEX_CONTRACT_VERSION,
    };
    use std::io::Cursor;

    fn valid_bundle(path: &Path, script: &[u8], executable: bool) {
        let markdown = b"---\nname: demo\ndescription: Use when importing a Bundle.\n---\n# Demo\n";
        let files = vec![
            file("SKILL.md", markdown, false),
            file("run.sh", script, executable),
        ];
        let manifest = BundleManifest {
            format: BUNDLE_FORMAT.into(),
            format_version: BUNDLE_FORMAT_VERSION,
            agent_contract: AgentContract {
                id: CODEX_CONTRACT_ID.into(),
                version: CODEX_CONTRACT_VERSION,
            },
            skills: vec![BundleSkill {
                directory_name: "demo".into(),
                revision: skill_revision(&files).unwrap(),
                files,
            }],
        };
        let mut markdown_reader = Cursor::new(markdown.as_slice());
        let mut script_reader = Cursor::new(script);
        let mut readers = [
            BundleFileReader {
                reader: &mut markdown_reader,
            },
            BundleFileReader {
                reader: &mut script_reader,
            },
        ];
        let file = File::create(path).unwrap();
        write_bundle(file, &manifest, &mut readers).unwrap();
    }

    fn skill_document_bundle(path: &Path, directory_name: &str, markdown: &[u8]) {
        let files = vec![file("SKILL.md", markdown, false)];
        let manifest = BundleManifest {
            format: BUNDLE_FORMAT.into(),
            format_version: BUNDLE_FORMAT_VERSION,
            agent_contract: AgentContract {
                id: CODEX_CONTRACT_ID.into(),
                version: CODEX_CONTRACT_VERSION,
            },
            skills: vec![BundleSkill {
                directory_name: directory_name.into(),
                revision: skill_revision(&files).unwrap(),
                files,
            }],
        };
        let mut markdown_reader = Cursor::new(markdown);
        let mut readers = [BundleFileReader {
            reader: &mut markdown_reader,
        }];
        write_bundle(File::create(path).unwrap(), &manifest, &mut readers).unwrap();
    }

    fn bundle_with_finder_metadata(path: &Path) {
        let finder = b"finder metadata";
        let markdown = b"---\nname: demo\ndescription: Use when importing a Bundle.\n---\n# Demo\n";
        let nested_finder = b"nested finder metadata";
        let files = vec![
            file(".DS_Store", finder, false),
            file("SKILL.md", markdown, false),
            file("references/.DS_Store", nested_finder, false),
        ];
        let manifest = BundleManifest {
            format: BUNDLE_FORMAT.into(),
            format_version: BUNDLE_FORMAT_VERSION,
            agent_contract: AgentContract {
                id: CODEX_CONTRACT_ID.into(),
                version: CODEX_CONTRACT_VERSION,
            },
            skills: vec![BundleSkill {
                directory_name: "demo".into(),
                revision: skill_revision(&files).unwrap(),
                files,
            }],
        };
        let mut finder_reader = Cursor::new(finder.as_slice());
        let mut markdown_reader = Cursor::new(markdown.as_slice());
        let mut nested_finder_reader = Cursor::new(nested_finder.as_slice());
        let mut readers = [
            BundleFileReader {
                reader: &mut finder_reader,
            },
            BundleFileReader {
                reader: &mut markdown_reader,
            },
            BundleFileReader {
                reader: &mut nested_finder_reader,
            },
        ];
        write_bundle(File::create(path).unwrap(), &manifest, &mut readers).unwrap();
    }

    fn file(path: &str, bytes: &[u8], executable: bool) -> BundleFile {
        BundleFile {
            path: path.into(),
            size: bytes.len() as u64,
            sha256: format!("{:x}", Sha256::digest(bytes)),
            executable,
        }
    }

    #[test]
    fn verifies_stages_audits_and_discards_without_installing_or_executing() {
        let directory = tempfile::tempdir().unwrap();
        let marker = directory.path().join("executed");
        let script = format!("#!/bin/sh\ntouch {}\n", marker.display());
        let source = directory.path().join("source.skillbundle");
        valid_bundle(&source, script.as_bytes(), true);
        let manager = BundleImportManager::new(directory.path().join("staging")).unwrap();
        let review = manager.stage(&source).unwrap();
        assert_eq!(review.skills.len(), 1);
        assert_eq!(review.total_files, 2);
        assert!(!review.installed);
        assert!(!marker.exists());
        assert_eq!(review.skills[0].compatibility.status, "review");
        assert!(review.skills[0]
            .files
            .iter()
            .all(|file| !file.staged_executable));
        let content = manager
            .read_file(
                &review.session_id,
                &review.bundle_revision,
                "demo",
                "SKILL.md",
            )
            .unwrap();
        assert!(content.is_text);
        assert!(!content.truncated);
        assert_eq!(content.preview_bytes, markdown_len());
        assert!(content.content.unwrap().contains("name: demo"));
        let session = manager.session_path(&review.session_id).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(session.join("content/skills/demo/run.sh"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o111, 0);
        }
        manager.discard(&review.session_id).unwrap();
        assert!(!session.exists());
    }

    #[test]
    fn verifies_finder_metadata_before_excluding_it_from_the_import_model() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("finder.skillbundle");
        bundle_with_finder_metadata(&source);
        let manager = BundleImportManager::new(directory.path().join("staging")).unwrap();
        let review = manager.stage(&source).unwrap();
        assert_eq!(review.total_files, 1);
        assert_eq!(review.total_bytes, markdown_len() as u64);
        assert_eq!(review.skills[0].files.len(), 1);
        assert_eq!(review.skills[0].files[0].path, "SKILL.md");
        assert_eq!(
            review.skills[0].revision,
            skill_revision(&[file(
                "SKILL.md",
                b"---\nname: demo\ndescription: Use when importing a Bundle.\n---\n# Demo\n",
                false,
            )])
            .unwrap()
        );
        let session = manager
            .session(&review.session_id, &review.bundle_revision)
            .unwrap();
        assert_eq!(session.verified_manifest.skills[0].files.len(), 3);
        assert_eq!(session.manifest.skills[0].files.len(), 1);
        assert!(manager
            .verified_review(&review.session_id, &review.bundle_revision)
            .is_ok());
        assert!(manager
            .session_path(&review.session_id)
            .unwrap()
            .join("content/skills/demo/.DS_Store")
            .is_file());
        fs::write(
            manager
                .session_path(&review.session_id)
                .unwrap()
                .join("content/skills/demo/.DS_Store"),
            b"tampered finder metadata",
        )
        .unwrap();
        assert!(matches!(
            manager.verified_review(&review.session_id, &review.bundle_revision),
            Err(BundleImportError::ChangedSession)
        ));
    }

    fn markdown_len() -> usize {
        b"---\nname: demo\ndescription: Use when importing a Bundle.\n---\n# Demo\n".len()
    }

    #[test]
    fn file_preview_is_bounded_after_full_hash_verification() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("large.skillbundle");
        let large = vec![b'a'; MAX_PREVIEW_BYTES + 37];
        valid_bundle(&source, &large, false);
        let manager = BundleImportManager::new(directory.path().join("staging")).unwrap();
        let review = manager.stage(&source).unwrap();
        let preview = manager
            .read_file(
                &review.session_id,
                &review.bundle_revision,
                "demo",
                "run.sh",
            )
            .unwrap();
        assert!(preview.is_text);
        assert!(preview.truncated);
        assert_eq!(preview.preview_bytes, MAX_PREVIEW_BYTES);
        assert_eq!(preview.content.unwrap().len(), MAX_PREVIEW_BYTES);

        let binary_source = directory.path().join("binary.skillbundle");
        valid_bundle(&binary_source, &[0xff, 0xfe, 0xfd], false);
        let binary_review = manager.stage(&binary_source).unwrap();
        let binary = manager
            .read_file(
                &binary_review.session_id,
                &binary_review.bundle_revision,
                "demo",
                "run.sh",
            )
            .unwrap();
        assert!(!binary.is_text);
        assert!(!binary.truncated);
        assert_eq!(binary.preview_bytes, 0);
        assert!(binary.content.is_none());
    }

    #[test]
    fn compatibility_requires_description_and_matching_directory_identity() {
        let directory = tempfile::tempdir().unwrap();
        let manager = BundleImportManager::new(directory.path().join("staging")).unwrap();

        let missing_description = directory.path().join("missing-description.skillbundle");
        skill_document_bundle(
            &missing_description,
            "demo",
            b"---\nname: demo\ndescription:\n---\n\n# Demo\n\nThese instructions are long enough for compatibility review.\n",
        );
        let review = manager.stage(&missing_description).unwrap();
        assert_eq!(review.skills[0].compatibility.status, "incompatible");
        assert!(review.skills[0]
            .compatibility
            .checks
            .iter()
            .any(|check| check.id == "description" && check.status == "fail"));
        assert_eq!(review.skills[0].audit.verdict, "block");

        let mismatched_name = directory.path().join("mismatched-name.skillbundle");
        skill_document_bundle(
            &mismatched_name,
            "demo",
            b"---\nname: other\ndescription: Use when reviewing identity.\n---\n\n# Other\n\nThese instructions are long enough for compatibility review.\n",
        );
        let review = manager.stage(&mismatched_name).unwrap();
        assert_eq!(review.skills[0].compatibility.status, "incompatible");
        assert!(review.skills[0]
            .compatibility
            .checks
            .iter()
            .any(|check| check.id == "directory-identity" && check.status == "fail"));
        assert!(review.skills[0]
            .audit
            .findings
            .iter()
            .any(|finding| finding.id == "identity-change"));
    }

    #[test]
    fn malformed_and_tampered_sessions_are_rejected_and_cleaned() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("staging");
        let manager = BundleImportManager::new(root.clone()).unwrap();
        let malformed = directory.path().join("bad.skillbundle");
        fs::write(&malformed, b"not a zip").unwrap();
        assert!(manager.stage(&malformed).is_err());
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);

        let source = directory.path().join("valid.skillbundle");
        valid_bundle(&source, b"echo test\n", false);
        let review = manager.stage(&source).unwrap();
        let staged = manager
            .session_path(&review.session_id)
            .unwrap()
            .join("content/skills/demo/SKILL.md");
        fs::write(staged, b"changed").unwrap();
        assert!(matches!(
            manager.read_file(
                &review.session_id,
                &review.bundle_revision,
                "demo",
                "SKILL.md"
            ),
            Err(BundleImportError::ChangedSession)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn source_symlinks_are_rejected_and_startup_removes_abandoned_sessions() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("actual.skillbundle");
        valid_bundle(&source, b"echo test\n", false);
        let linked = directory.path().join("linked.skillbundle");
        symlink(&source, &linked).unwrap();
        let root = directory.path().join("staging");
        fs::create_dir_all(root.join("abandoned")).unwrap();
        let manager = BundleImportManager::new(root.clone()).unwrap();
        assert!(!root.join("abandoned").exists());
        assert!(matches!(
            manager.stage(&linked),
            Err(BundleImportError::InvalidSource)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn staged_parent_symlink_replacement_cannot_escape_the_session() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("valid.skillbundle");
        valid_bundle(&source, b"echo test\n", false);
        let manager = BundleImportManager::new(directory.path().join("staging")).unwrap();
        let review = manager.stage(&source).unwrap();
        let session = manager.session_path(&review.session_id).unwrap();
        let skill_directory = session.join("content/skills/demo");
        fs::remove_dir_all(&skill_directory).unwrap();
        let outside = directory.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("SKILL.md"), b"outside").unwrap();
        symlink(&outside, &skill_directory).unwrap();
        assert!(matches!(
            manager.read_file(
                &review.session_id,
                &review.bundle_revision,
                "demo",
                "SKILL.md"
            ),
            Err(BundleImportError::ChangedSession)
        ));
    }
}
