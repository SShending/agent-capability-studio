use percent_encoding::percent_decode_str;
use reqwest::{
    blocking::{Client, Response},
    header::{HeaderMap, HeaderValue, ACCEPT},
    redirect::Policy,
    StatusCode, Url,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use tempfile::Builder;
use thiserror::Error;

const MAX_FILES: usize = 256;
const MAX_TOTAL_BYTES: usize = 25 * 1024 * 1024;
const MAX_FILE_BYTES: usize = 2 * 1024 * 1024;
const MAX_DEPTH: usize = 8;
const MAX_SOURCE_PATH_COMPONENTS: usize = 16;
const MAX_API_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum CandidateError {
    #[error("Enter a public https://github.com repository or Skill directory URL.")]
    InvalidGithubUrl,
    #[error("The selected local candidate must be a real directory, not a link.")]
    InvalidLocalDirectory,
    #[error("The candidate does not contain SKILL.md at its root.")]
    MissingSkillDocument,
    #[error("The candidate contains an unsafe or unsupported entry: {0}")]
    UnsafeEntry(String),
    #[error("The candidate contains too many files. The current limit is 256.")]
    TooManyFiles,
    #[error("A candidate file exceeds the 2 MiB limit: {0}")]
    FileTooLarge(String),
    #[error("The candidate exceeds the 25 MiB staging limit.")]
    CandidateTooLarge,
    #[error("The candidate directory exceeds the supported depth.")]
    TooDeep,
    #[error("The GitHub tree response was truncated or exceeded the response limit.")]
    TruncatedTree,
    #[error("The GitHub source changed or returned bytes inconsistent with its tree metadata.")]
    InconsistentGithubSource,
    #[error("GitHub rate limit reached. Try again after {0}.")]
    RateLimited(String),
    #[error("GitHub candidate acquisition failed: {0}")]
    Github(String),
    #[error("This staged candidate session is not available.")]
    UnknownSession,
    #[error("The staged candidate changed. Start the review again.")]
    ChangedSession,
    #[error("This file is not part of the staged candidate.")]
    UnknownFile,
    #[error("Unable to stage the candidate: {0}")]
    Io(#[from] std::io::Error),
}

impl CandidateError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidGithubUrl => "INVALID_GITHUB_CANDIDATE_URL",
            Self::InvalidLocalDirectory => "INVALID_LOCAL_CANDIDATE",
            Self::MissingSkillDocument => "MISSING_SKILL_DOCUMENT",
            Self::UnsafeEntry(_) => "UNSAFE_CANDIDATE_ENTRY",
            Self::TooManyFiles => "CANDIDATE_FILE_LIMIT",
            Self::FileTooLarge(_) => "CANDIDATE_FILE_SIZE_LIMIT",
            Self::CandidateTooLarge => "CANDIDATE_TOTAL_SIZE_LIMIT",
            Self::TooDeep => "CANDIDATE_DEPTH_LIMIT",
            Self::TruncatedTree => "GITHUB_TREE_LIMIT",
            Self::InconsistentGithubSource => "INCONSISTENT_GITHUB_SOURCE",
            Self::RateLimited(_) => "GITHUB_RATE_LIMIT",
            Self::Github(_) => "GITHUB_ACQUISITION_ERROR",
            Self::UnknownSession => "UNKNOWN_CANDIDATE_SESSION",
            Self::ChangedSession => "STALE_CANDIDATE",
            Self::UnknownFile => "UNKNOWN_CANDIDATE_FILE",
            Self::Io(_) => "CANDIDATE_IO_ERROR",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateManifest {
    pub session_id: String,
    pub source: CandidateSource,
    pub files: Vec<CandidateFile>,
    pub total_bytes: usize,
    pub candidate_hash: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CandidateSource {
    Local {
        selected_path: String,
    },
    Github {
        repository: String,
        requested_ref: String,
        resolved_sha: String,
        skill_path: String,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateFile {
    pub path: String,
    pub size: usize,
    pub sha256: String,
    pub executable: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateReview {
    pub manifest: CandidateManifest,
    pub compatibility: CandidateCompatibility,
    pub audit: super::AuditResult,
    pub skipped_entries: Vec<CandidateSkippedEntry>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateCompatibility {
    pub agent: String,
    pub status: String,
    pub summary: String,
    pub checks: Vec<CandidateCompatibilityCheck>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateCompatibilityCheck {
    pub id: String,
    pub label: String,
    pub status: String,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateSkippedEntry {
    pub path: String,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateFileContent {
    pub path: String,
    pub content: Option<String>,
    pub is_text: bool,
}

#[derive(Clone)]
pub struct CandidateStager {
    store: Arc<StagingStore>,
    github: Arc<dyn GithubTransport>,
}

struct StagingStore {
    root: PathBuf,
    sessions: Mutex<HashMap<String, CandidateManifest>>,
}

impl Drop for StagingStore {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl CandidateStager {
    pub fn new(staging_root: PathBuf) -> Result<Self, CandidateError> {
        Self::with_github(staging_root, Arc::new(HttpGithubTransport::new()))
    }

    fn with_github(
        staging_root: PathBuf,
        github: Arc<dyn GithubTransport>,
    ) -> Result<Self, CandidateError> {
        let root = prepare_staging_root(&staging_root)?;
        Ok(Self {
            store: Arc::new(StagingStore {
                root,
                sessions: Mutex::new(HashMap::new()),
            }),
            github,
        })
    }

    pub fn stage_local(
        &self,
        selected_directory: &Path,
    ) -> Result<CandidateManifest, CandidateError> {
        let metadata = fs::symlink_metadata(selected_directory)
            .map_err(|_| CandidateError::InvalidLocalDirectory)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(CandidateError::InvalidLocalDirectory);
        }
        let source_root = fs::canonicalize(selected_directory)
            .map_err(|_| CandidateError::InvalidLocalDirectory)?;
        if source_root.starts_with(&self.store.root) {
            return Err(CandidateError::InvalidLocalDirectory);
        }
        let mut inputs = Vec::new();
        collect_local_files(&source_root, &source_root, &mut inputs)?;
        require_skill_document(inputs.iter().map(|input| input.relative.as_str()))?;
        inputs.sort_by(|left, right| left.relative.cmp(&right.relative));

        let (session_id, session_path) = self.begin_session()?;
        let result = (|| {
            let mut files = Vec::with_capacity(inputs.len());
            let mut total_bytes = 0usize;
            for input in inputs {
                let bytes = read_local_file(&source_root, &input)?;
                total_bytes = checked_total(total_bytes, bytes.len())?;
                write_staged_file(&session_path, &input.relative, &bytes)?;
                files.push(CandidateFile {
                    path: input.relative,
                    size: bytes.len(),
                    sha256: sha256(&bytes),
                    executable: input.executable,
                });
            }
            Ok(self.complete_session(
                session_id.clone(),
                CandidateSource::Local {
                    selected_path: source_root.display().to_string(),
                },
                files,
                total_bytes,
            ))
        })();
        self.finish_or_cleanup(&session_id, &session_path, result)
    }

    pub fn stage_github(&self, source_url: &str) -> Result<CandidateManifest, CandidateError> {
        let request = parse_github_url(source_url)?;
        let requested_ref = match request.requested_ref.clone() {
            Some(reference) => reference,
            None => self
                .github
                .default_branch(&request.owner, &request.repository)?,
        };
        let commit =
            self.github
                .resolve_commit(&request.owner, &request.repository, &requested_ref)?;
        let target_tree_sha = self.resolve_target_tree(&request, &commit.root_tree_sha)?;
        let tree = self
            .github
            .tree(&request.owner, &request.repository, &target_tree_sha, true)?;
        if tree.truncated {
            return Err(CandidateError::TruncatedTree);
        }
        let inputs = github_inputs(tree.entries)?;
        require_skill_document(inputs.iter().map(|input| input.relative.as_str()))?;

        let (session_id, session_path) = self.begin_session()?;
        let result = (|| {
            let mut files = Vec::with_capacity(inputs.len());
            let mut total_bytes = 0usize;
            for input in inputs {
                let source_path = join_source_path(&request.skill_path, &input.relative);
                let bytes = self.github.download_blob(
                    &request.owner,
                    &request.repository,
                    &commit.sha,
                    &source_path,
                    input.declared_size,
                )?;
                if bytes.len() != input.declared_size {
                    return Err(CandidateError::InconsistentGithubSource);
                }
                total_bytes = checked_total(total_bytes, bytes.len())?;
                write_staged_file(&session_path, &input.relative, &bytes)?;
                files.push(CandidateFile {
                    path: input.relative,
                    size: bytes.len(),
                    sha256: sha256(&bytes),
                    executable: input.executable,
                });
            }
            Ok(self.complete_session(
                session_id.clone(),
                CandidateSource::Github {
                    repository: format!("{}/{}", request.owner, request.repository),
                    requested_ref,
                    resolved_sha: commit.sha,
                    skill_path: request.skill_path,
                },
                files,
                total_bytes,
            ))
        })();
        self.finish_or_cleanup(&session_id, &session_path, result)
    }

    pub fn review(
        &self,
        session_id: &str,
        expected_candidate_hash: &str,
    ) -> Result<CandidateReview, CandidateError> {
        let manifest = self.session_manifest(session_id, expected_candidate_hash)?;
        let directory = self.session_directory(session_id)?;
        let mut skill_markdown = None;
        for file in &manifest.files {
            let bytes = read_verified_staged_file(&directory, file)?;
            if file.path == "SKILL.md" {
                skill_markdown = Some(bytes);
            }
        }
        let skill_markdown = skill_markdown.ok_or(CandidateError::MissingSkillDocument)?;
        let audit = match String::from_utf8(skill_markdown) {
            Ok(markdown) => super::audit(&markdown, "", ""),
            Err(error) => non_text_skill_audit(error.as_bytes()),
        };
        Ok(CandidateReview {
            compatibility: compatibility_for(&manifest, &audit),
            manifest,
            audit,
            // v0.1 rejects unsupported entries during acquisition rather than omitting them.
            skipped_entries: Vec::new(),
        })
    }

    pub fn read_file(
        &self,
        session_id: &str,
        expected_candidate_hash: &str,
        path: &str,
    ) -> Result<CandidateFileContent, CandidateError> {
        let manifest = self.session_manifest(session_id, expected_candidate_hash)?;
        let path = validated_relative_string(path)?;
        let file = manifest
            .files
            .iter()
            .find(|file| file.path == path)
            .ok_or(CandidateError::UnknownFile)?;
        let bytes = read_verified_staged_file(&self.session_directory(session_id)?, file)?;
        match String::from_utf8(bytes) {
            Ok(content) => Ok(CandidateFileContent {
                path,
                content: Some(content),
                is_text: true,
            }),
            Err(_) => Ok(CandidateFileContent {
                path,
                content: None,
                is_text: false,
            }),
        }
    }

    pub fn discard(&self, session_id: &str) -> Result<(), CandidateError> {
        let sessions = self
            .store
            .sessions
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if !sessions.contains_key(session_id) {
            return Err(CandidateError::UnknownSession);
        }
        drop(sessions);
        remove_session_path(&self.session_directory(session_id)?)?;
        self.store
            .sessions
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(session_id);
        Ok(())
    }

    fn session_manifest(
        &self,
        session_id: &str,
        expected_candidate_hash: &str,
    ) -> Result<CandidateManifest, CandidateError> {
        let manifest = self
            .store
            .sessions
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(session_id)
            .cloned()
            .ok_or(CandidateError::UnknownSession)?;
        if expected_candidate_hash.is_empty() || expected_candidate_hash != manifest.candidate_hash
        {
            return Err(CandidateError::ChangedSession);
        }
        Ok(manifest)
    }

    fn session_directory(&self, session_id: &str) -> Result<PathBuf, CandidateError> {
        if session_id.is_empty() || session_id.contains(['/', '\\', '\0']) {
            return Err(CandidateError::UnknownSession);
        }
        let directory = self.store.root.join(session_id);
        let metadata =
            fs::symlink_metadata(&directory).map_err(|_| CandidateError::ChangedSession)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(CandidateError::ChangedSession);
        }
        let canonical = fs::canonicalize(directory).map_err(|_| CandidateError::ChangedSession)?;
        if !canonical.starts_with(&self.store.root) {
            return Err(CandidateError::ChangedSession);
        }
        Ok(canonical)
    }

    fn resolve_target_tree(
        &self,
        request: &GithubRequest,
        root_tree_sha: &str,
    ) -> Result<String, CandidateError> {
        let mut current_sha = root_tree_sha.to_owned();
        for component in source_components(&request.skill_path) {
            let tree =
                self.github
                    .tree(&request.owner, &request.repository, &current_sha, false)?;
            if tree.truncated {
                return Err(CandidateError::TruncatedTree);
            }
            let entry = tree
                .entries
                .into_iter()
                .find(|entry| entry.path == component)
                .ok_or_else(|| CandidateError::UnsafeEntry(request.skill_path.clone()))?;
            if entry.kind != "tree" || entry.mode != "040000" {
                return Err(CandidateError::UnsafeEntry(request.skill_path.clone()));
            }
            current_sha = entry.sha;
        }
        Ok(current_sha)
    }

    fn begin_session(&self) -> Result<(String, PathBuf), CandidateError> {
        let temporary = Builder::new()
            .prefix("candidate-")
            .tempdir_in(&self.store.root)?;
        let path = temporary.keep();
        let session_id = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| CandidateError::Io(std::io::Error::other("invalid staging path")))?
            .to_owned();
        Ok((session_id, path))
    }

    fn complete_session(
        &self,
        session_id: String,
        source: CandidateSource,
        files: Vec<CandidateFile>,
        total_bytes: usize,
    ) -> CandidateManifest {
        let fingerprint = files
            .iter()
            .map(|file| {
                format!(
                    "{}:{}:{}:{}",
                    file.path, file.size, file.sha256, file.executable
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        CandidateManifest {
            session_id,
            source,
            files,
            total_bytes,
            candidate_hash: sha256(fingerprint.as_bytes()),
        }
    }

    fn finish_or_cleanup(
        &self,
        session_id: &str,
        session_path: &Path,
        result: Result<CandidateManifest, CandidateError>,
    ) -> Result<CandidateManifest, CandidateError> {
        match result {
            Ok(manifest) => {
                self.store
                    .sessions
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .insert(session_id.to_owned(), manifest.clone());
                Ok(manifest)
            }
            Err(error) => {
                let _ = remove_session_path(session_path);
                Err(error)
            }
        }
    }
}

#[derive(Clone)]
struct LocalInput {
    relative: String,
    source_path: PathBuf,
    identity: LocalFileIdentity,
    executable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LocalFileIdentity {
    len: u64,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    mode: u32,
}

#[derive(Clone)]
struct GithubInput {
    relative: String,
    declared_size: usize,
    executable: bool,
}

fn prepare_staging_root(requested: &Path) -> Result<PathBuf, CandidateError> {
    match fs::symlink_metadata(requested) {
        Ok(metadata) if metadata.file_type().is_symlink() || metadata.is_file() => {
            fs::remove_file(requested)?;
        }
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(requested)?,
        Ok(_) => return Err(CandidateError::InvalidLocalDirectory),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    fs::create_dir_all(requested)?;
    Ok(fs::canonicalize(requested)?)
}

fn remove_session_path(path: &Path) -> Result<(), CandidateError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        fs::remove_file(path)?;
    } else {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn collect_local_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<LocalInput>,
) -> Result<(), CandidateError> {
    let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let relative = relative_string(root, &path)?;
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(CandidateError::UnsafeEntry(relative));
        }
        if metadata.is_dir() {
            if Path::new(&relative).components().count() >= MAX_DEPTH {
                return Err(CandidateError::TooDeep);
            }
            collect_local_files(root, &path, files)?;
        } else if metadata.is_file() {
            validate_file_slot(files.len(), metadata.len(), &relative)?;
            files.push(LocalInput {
                relative,
                source_path: path,
                identity: local_file_identity(&metadata),
                executable: is_executable(&metadata),
            });
        } else {
            return Err(CandidateError::UnsafeEntry(relative));
        }
    }
    validate_unique_paths(files.iter().map(|file| file.relative.as_str()))
}

fn read_local_file(root: &Path, input: &LocalInput) -> Result<Vec<u8>, CandidateError> {
    let canonical = fs::canonicalize(&input.source_path)?;
    if !canonical.starts_with(root) {
        return Err(CandidateError::UnsafeEntry(input.relative.clone()));
    }
    let metadata = fs::symlink_metadata(&input.source_path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(CandidateError::UnsafeEntry(input.relative.clone()));
    }
    let file = File::open(&input.source_path)?;
    let opened_metadata = file.metadata()?;
    if !opened_metadata.is_file() || local_file_identity(&opened_metadata) != input.identity {
        return Err(CandidateError::UnsafeEntry(input.relative.clone()));
    }
    read_limited(file, MAX_FILE_BYTES, || {
        CandidateError::FileTooLarge(input.relative.clone())
    })
}

fn github_inputs(entries: Vec<GithubTreeEntry>) -> Result<Vec<GithubInput>, CandidateError> {
    let mut files = Vec::new();
    let mut declared_total = 0usize;
    for entry in entries {
        if entry.mode == "120000" || entry.mode == "160000" || entry.kind == "commit" {
            return Err(CandidateError::UnsafeEntry(entry.path));
        }
        if entry.kind == "tree" && entry.mode == "040000" {
            continue;
        }
        if entry.kind != "blob" || !matches!(entry.mode.as_str(), "100644" | "100755") {
            return Err(CandidateError::UnsafeEntry(entry.path));
        }
        let relative = validated_relative_string(&entry.path)?;
        let declared_size = entry
            .size
            .and_then(|size| usize::try_from(size).ok())
            .ok_or_else(|| CandidateError::UnsafeEntry(relative.clone()))?;
        validate_file_slot(files.len(), declared_size as u64, &relative)?;
        declared_total = checked_total(declared_total, declared_size)?;
        files.push(GithubInput {
            relative,
            declared_size,
            executable: entry.mode == "100755",
        });
    }
    validate_unique_paths(files.iter().map(|file| file.relative.as_str()))?;
    files.sort_by(|left, right| left.relative.cmp(&right.relative));
    Ok(files)
}

fn validate_file_slot(count: usize, size: u64, path: &str) -> Result<(), CandidateError> {
    if count >= MAX_FILES {
        return Err(CandidateError::TooManyFiles);
    }
    if size > MAX_FILE_BYTES as u64 {
        return Err(CandidateError::FileTooLarge(path.into()));
    }
    Ok(())
}

fn checked_total(current: usize, added: usize) -> Result<usize, CandidateError> {
    let total = current
        .checked_add(added)
        .ok_or(CandidateError::CandidateTooLarge)?;
    if total > MAX_TOTAL_BYTES {
        Err(CandidateError::CandidateTooLarge)
    } else {
        Ok(total)
    }
}

fn require_skill_document<'a>(paths: impl Iterator<Item = &'a str>) -> Result<(), CandidateError> {
    if paths.into_iter().any(|path| path == "SKILL.md") {
        Ok(())
    } else {
        Err(CandidateError::MissingSkillDocument)
    }
}

fn validate_unique_paths<'a>(paths: impl Iterator<Item = &'a str>) -> Result<(), CandidateError> {
    let mut seen = HashSet::new();
    for path in paths {
        let key = path.to_lowercase();
        if !seen.insert(key) {
            return Err(CandidateError::UnsafeEntry(path.into()));
        }
    }
    Ok(())
}

fn relative_string(root: &Path, path: &Path) -> Result<String, CandidateError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| CandidateError::UnsafeEntry(path.display().to_string()))?;
    let value = relative
        .to_str()
        .ok_or_else(|| CandidateError::UnsafeEntry(relative.display().to_string()))?
        .replace('\\', "/");
    validated_relative_string(&value)
}

fn validated_relative_string(value: &str) -> Result<String, CandidateError> {
    if value.is_empty() || value.contains('\\') {
        return Err(CandidateError::UnsafeEntry(value.into()));
    }
    let path = Path::new(value);
    let components = path.components().collect::<Vec<_>>();
    if components.len() > MAX_DEPTH + 1
        || components
            .iter()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(CandidateError::UnsafeEntry(value.into()));
    }
    Ok(components
        .iter()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

fn write_staged_file(root: &Path, relative: &str, bytes: &[u8]) -> Result<(), CandidateError> {
    let relative = validated_relative_string(relative)?;
    let destination = root.join(&relative);
    if !destination.starts_with(root) {
        return Err(CandidateError::UnsafeEntry(relative));
    }
    let parent = destination
        .parent()
        .ok_or_else(|| CandidateError::UnsafeEntry(relative.clone()))?;
    fs::create_dir_all(parent)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)?;
    file.write_all(bytes)?;
    file.flush()?;
    let mut permissions = file.metadata()?.permissions();
    permissions.set_readonly(true);
    fs::set_permissions(destination, permissions)?;
    Ok(())
}

fn read_verified_staged_file(root: &Path, file: &CandidateFile) -> Result<Vec<u8>, CandidateError> {
    let relative = validated_relative_string(&file.path)?;
    let path = root.join(&relative);
    if !path.starts_with(root) {
        return Err(CandidateError::ChangedSession);
    }
    let metadata = fs::symlink_metadata(&path).map_err(|_| CandidateError::ChangedSession)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() != file.size as u64
    {
        return Err(CandidateError::ChangedSession);
    }
    let bytes = read_limited(File::open(path)?, MAX_FILE_BYTES, || {
        CandidateError::ChangedSession
    })?;
    if bytes.len() != file.size || sha256(&bytes) != file.sha256 {
        return Err(CandidateError::ChangedSession);
    }
    Ok(bytes)
}

fn non_text_skill_audit(bytes: &[u8]) -> super::AuditResult {
    super::AuditResult {
        verdict: "block".into(),
        findings: vec![super::Finding {
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
        content_hash: sha256(bytes),
        document: super::SkillDocument {
            has_frontmatter: false,
            name: String::new(),
            description: String::new(),
            body: String::new(),
        },
        diff: super::Diff {
            changed: false,
            start_line: 0,
            added_count: 0,
            removed_count: 0,
            before: Vec::new(),
            after: Vec::new(),
            truncated: false,
        },
    }
}

fn compatibility_for(
    manifest: &CandidateManifest,
    audit: &super::AuditResult,
) -> CandidateCompatibility {
    let non_text = audit
        .findings
        .iter()
        .any(|finding| finding.id == "non-text-skill-document");
    let document = &audit.document;
    let executable_count = manifest.files.iter().filter(|file| file.executable).count();
    let mut checks = vec![CandidateCompatibilityCheck {
        id: "staged-integrity".into(),
        label: "暂存文件完整性".into(),
        status: "pass".into(),
        detail: format!("{} 个文件与当前 SHA-256 清单一致。", manifest.files.len()),
    }];
    checks.push(CandidateCompatibilityCheck {
        id: "skill-document-text".into(),
        label: "SKILL.md 文本格式".into(),
        status: if non_text { "fail" } else { "pass" }.into(),
        detail: if non_text {
            "必须使用 UTF-8 文本，当前文件不能被 Codex 读取。".into()
        } else {
            "根说明文件可以作为 UTF-8 文本读取。".into()
        },
    });
    checks.push(CandidateCompatibilityCheck {
        id: "frontmatter".into(),
        label: "基本信息".into(),
        status: if !non_text && document.has_frontmatter {
            "pass"
        } else {
            "fail"
        }
        .into(),
        detail: if document.has_frontmatter && !non_text {
            "找到名称和用途的 frontmatter。".into()
        } else {
            "根文件开头需要以 --- 包围基本信息。".into()
        },
    });
    checks.push(CandidateCompatibilityCheck {
        id: "skill-name".into(),
        label: "Skill 名称".into(),
        status: if !non_text && super::valid_name(&document.name) {
            "pass"
        } else {
            "fail"
        }
        .into(),
        detail: if super::valid_name(&document.name) {
            format!("名称“{}”符合 Codex 命名规则。", document.name)
        } else {
            "名称只能使用小写字母、数字和单个连字符。".into()
        },
    });
    checks.push(CandidateCompatibilityCheck {
        id: "description".into(),
        label: "用途与触发条件".into(),
        status: if !non_text && !document.description.trim().is_empty() {
            "pass"
        } else {
            "fail"
        }
        .into(),
        detail: if document.description.trim().is_empty() {
            "缺少用途说明，Codex 无法判断何时使用。".into()
        } else if super::explicit_trigger(&document.name, &document.description) {
            "采用明确点名触发。".into()
        } else {
            "采用按意图触发，这也是受支持的策略。".into()
        },
    });
    checks.push(CandidateCompatibilityCheck {
        id: "executable-files".into(),
        label: "可执行支持文件".into(),
        status: if executable_count == 0 {
            "pass"
        } else {
            "review"
        }
        .into(),
        detail: if executable_count == 0 {
            "没有标记为可执行的支持文件。".into()
        } else {
            format!(
                "包含 {executable_count} 个可执行文件。审查过程不会运行它们，请在安装前确认用途。"
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
    let summary = match status {
        "incompatible" => "当前文件结构不满足 Codex Skill 的基本要求。".into(),
        "review" => "可以继续查看，但可执行支持文件需要在安装前人工确认。".into(),
        _ => "暂存结构符合 Codex Skill 的基础兼容性要求。".into(),
    };
    CandidateCompatibility {
        agent: "Codex".into(),
        status: status.into(),
        summary,
        checks,
    }
}

fn read_limited(
    reader: impl Read,
    limit: usize,
    error: impl FnOnce() -> CandidateError,
) -> Result<Vec<u8>, CandidateError> {
    let mut bytes = Vec::new();
    reader.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        Err(error())
    } else {
        Ok(bytes)
    }
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn local_file_identity(metadata: &fs::Metadata) -> LocalFileIdentity {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        LocalFileIdentity {
            len: metadata.len(),
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
        }
    }

    #[cfg(not(unix))]
    {
        LocalFileIdentity {
            len: metadata.len(),
        }
    }
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

#[derive(Clone, Debug)]
struct GithubRequest {
    owner: String,
    repository: String,
    requested_ref: Option<String>,
    skill_path: String,
}

fn parse_github_url(value: &str) -> Result<GithubRequest, CandidateError> {
    let url = Url::parse(value.trim()).map_err(|_| CandidateError::InvalidGithubUrl)?;
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(CandidateError::InvalidGithubUrl);
    }
    let segments = url
        .path_segments()
        .ok_or(CandidateError::InvalidGithubUrl)?
        .filter(|segment| !segment.is_empty())
        .map(decode_segment)
        .collect::<Result<Vec<_>, _>>()?;
    if segments.len() < 2 {
        return Err(CandidateError::InvalidGithubUrl);
    }
    let owner = segments[0].clone();
    let repository = segments[1].trim_end_matches(".git").to_owned();
    if !valid_github_atom(&owner) || !valid_github_atom(&repository) {
        return Err(CandidateError::InvalidGithubUrl);
    }
    let (requested_ref, skill_segments) = match segments.get(2).map(String::as_str) {
        None => (None, &segments[2..]),
        Some("tree") if segments.len() >= 4 => (Some(segments[3].clone()), &segments[4..]),
        Some("blob") if segments.len() >= 5 => {
            if segments.last().map(String::as_str) != Some("SKILL.md") {
                return Err(CandidateError::InvalidGithubUrl);
            }
            (Some(segments[3].clone()), &segments[4..segments.len() - 1])
        }
        _ => return Err(CandidateError::InvalidGithubUrl),
    };
    if requested_ref
        .as_deref()
        .is_some_and(|reference| reference.is_empty() || reference.len() > 255)
        || skill_segments.len() > MAX_SOURCE_PATH_COMPONENTS
        || skill_segments
            .iter()
            .any(|segment| !valid_path_atom(segment))
    {
        return Err(CandidateError::InvalidGithubUrl);
    }
    Ok(GithubRequest {
        owner,
        repository,
        requested_ref,
        skill_path: skill_segments.join("/"),
    })
}

fn decode_segment(value: &str) -> Result<String, CandidateError> {
    percent_decode_str(value)
        .decode_utf8()
        .map(|value| value.into_owned())
        .map_err(|_| CandidateError::InvalidGithubUrl)
}

fn valid_github_atom(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn valid_path_atom(value: &str) -> bool {
    !value.is_empty() && value != "." && value != ".." && !value.contains(['/', '\\', '\0'])
}

fn source_components(path: &str) -> impl Iterator<Item = &str> {
    path.split('/').filter(|component| !component.is_empty())
}

fn join_source_path(root: &str, relative: &str) -> String {
    if root.is_empty() {
        relative.into()
    } else {
        format!("{root}/{relative}")
    }
}

#[derive(Clone, Debug)]
struct ResolvedCommit {
    sha: String,
    root_tree_sha: String,
}

#[derive(Clone, Debug)]
struct GithubTree {
    entries: Vec<GithubTreeEntry>,
    truncated: bool,
}

#[derive(Clone, Debug, Deserialize)]
struct GithubTreeEntry {
    path: String,
    mode: String,
    #[serde(rename = "type")]
    kind: String,
    sha: String,
    size: Option<u64>,
}

trait GithubTransport: Send + Sync {
    fn default_branch(&self, owner: &str, repository: &str) -> Result<String, CandidateError>;
    fn resolve_commit(
        &self,
        owner: &str,
        repository: &str,
        reference: &str,
    ) -> Result<ResolvedCommit, CandidateError>;
    fn tree(
        &self,
        owner: &str,
        repository: &str,
        tree_sha: &str,
        recursive: bool,
    ) -> Result<GithubTree, CandidateError>;
    fn download_blob(
        &self,
        owner: &str,
        repository: &str,
        commit_sha: &str,
        path: &str,
        expected_size: usize,
    ) -> Result<Vec<u8>, CandidateError>;
}

struct HttpGithubTransport {
    client: Client,
}

impl HttpGithubTransport {
    fn new() -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github+json"),
        );
        headers.insert(
            "x-github-api-version",
            HeaderValue::from_static("2022-11-28"),
        );
        Self {
            client: Client::builder()
                .default_headers(headers)
                .user_agent("Agent-Skill-Studio/0.1")
                .connect_timeout(Duration::from_secs(15))
                .timeout(Duration::from_secs(60))
                .redirect(Policy::none())
                .build()
                .expect("valid GitHub acquisition client"),
        }
    }

    fn api_json<T: DeserializeOwned>(&self, url: Url) -> Result<T, CandidateError> {
        let response = self
            .client
            .get(url)
            .send()
            .map_err(|error| github_request_error(&error))?;
        let response = checked_github_response(response)?;
        let bytes = read_response_limited(response, MAX_API_RESPONSE_BYTES, || {
            CandidateError::TruncatedTree
        })?;
        serde_json::from_slice(&bytes)
            .map_err(|_| CandidateError::Github("GitHub returned malformed metadata".into()))
    }
}

impl GithubTransport for HttpGithubTransport {
    fn default_branch(&self, owner: &str, repository: &str) -> Result<String, CandidateError> {
        #[derive(Deserialize)]
        struct RepositoryResponse {
            default_branch: String,
        }
        let url = github_url("https://api.github.com", &["repos", owner, repository])?;
        let response: RepositoryResponse = self.api_json(url)?;
        if response.default_branch.is_empty() || response.default_branch.len() > 255 {
            Err(CandidateError::Github(
                "GitHub returned an invalid default branch".into(),
            ))
        } else {
            Ok(response.default_branch)
        }
    }

    fn resolve_commit(
        &self,
        owner: &str,
        repository: &str,
        reference: &str,
    ) -> Result<ResolvedCommit, CandidateError> {
        #[derive(Deserialize)]
        struct CommitResponse {
            sha: String,
            commit: CommitDetail,
        }
        #[derive(Deserialize)]
        struct CommitDetail {
            tree: CommitTree,
        }
        #[derive(Deserialize)]
        struct CommitTree {
            sha: String,
        }
        let url = github_url(
            "https://api.github.com",
            &["repos", owner, repository, "commits", reference],
        )?;
        let response: CommitResponse = self.api_json(url)?;
        if !valid_sha(&response.sha) || !valid_sha(&response.commit.tree.sha) {
            return Err(CandidateError::Github(
                "GitHub returned invalid commit metadata".into(),
            ));
        }
        Ok(ResolvedCommit {
            sha: response.sha,
            root_tree_sha: response.commit.tree.sha,
        })
    }

    fn tree(
        &self,
        owner: &str,
        repository: &str,
        tree_sha: &str,
        recursive: bool,
    ) -> Result<GithubTree, CandidateError> {
        #[derive(Deserialize)]
        struct TreeResponse {
            tree: Vec<GithubTreeEntry>,
            #[serde(default)]
            truncated: bool,
        }
        let mut url = github_url(
            "https://api.github.com",
            &["repos", owner, repository, "git", "trees", tree_sha],
        )?;
        if recursive {
            url.query_pairs_mut().append_pair("recursive", "1");
        }
        let response: TreeResponse = self.api_json(url)?;
        Ok(GithubTree {
            entries: response.tree,
            truncated: response.truncated,
        })
    }

    fn download_blob(
        &self,
        owner: &str,
        repository: &str,
        commit_sha: &str,
        path: &str,
        expected_size: usize,
    ) -> Result<Vec<u8>, CandidateError> {
        let mut parts = vec![owner, repository, commit_sha];
        parts.extend(source_components(path));
        let url = github_url("https://raw.githubusercontent.com", &parts)?;
        let response = self
            .client
            .get(url)
            .send()
            .map_err(|error| github_request_error(&error))?;
        let response = checked_github_response(response)?;
        if response
            .content_length()
            .is_some_and(|length| length != expected_size as u64)
        {
            return Err(CandidateError::InconsistentGithubSource);
        }
        read_response_limited(response, MAX_FILE_BYTES, || {
            CandidateError::FileTooLarge(path.into())
        })
    }
}

fn github_url(base: &str, parts: &[&str]) -> Result<Url, CandidateError> {
    let mut url = Url::parse(base).map_err(|_| CandidateError::InvalidGithubUrl)?;
    url.path_segments_mut()
        .map_err(|_| CandidateError::InvalidGithubUrl)?
        .extend(parts);
    Ok(url)
}

fn checked_github_response(response: Response) -> Result<Response, CandidateError> {
    if response.status().is_success() {
        return Ok(response);
    }
    if matches!(
        response.status(),
        StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS
    ) && response
        .headers()
        .get("x-ratelimit-remaining")
        .and_then(|value| value.to_str().ok())
        == Some("0")
    {
        let reset = response
            .headers()
            .get("x-ratelimit-reset")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("the GitHub reset time")
            .to_owned();
        return Err(CandidateError::RateLimited(reset));
    }
    Err(CandidateError::Github(format!(
        "GitHub returned HTTP {}",
        response.status().as_u16()
    )))
}

fn read_response_limited(
    response: Response,
    limit: usize,
    error: impl FnOnce() -> CandidateError,
) -> Result<Vec<u8>, CandidateError> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(error());
    }
    read_limited(response, limit, error)
}

fn github_request_error(error: &reqwest::Error) -> CandidateError {
    let message = if error.is_timeout() {
        "request timed out"
    } else if error.is_connect() {
        "connection failed"
    } else {
        "request could not be completed"
    };
    CandidateError::Github(message.into())
}

fn valid_sha(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    #[derive(Default)]
    struct FakeGithub {
        default_branch: String,
        commit: Option<ResolvedCommit>,
        trees: Mutex<HashMap<(String, bool), GithubTree>>,
        blobs: Mutex<HashMap<String, Vec<u8>>>,
        downloads: Mutex<Vec<(String, String)>>,
    }

    impl GithubTransport for FakeGithub {
        fn default_branch(
            &self,
            _owner: &str,
            _repository: &str,
        ) -> Result<String, CandidateError> {
            Ok(self.default_branch.clone())
        }

        fn resolve_commit(
            &self,
            _owner: &str,
            _repository: &str,
            _reference: &str,
        ) -> Result<ResolvedCommit, CandidateError> {
            self.commit
                .clone()
                .ok_or_else(|| CandidateError::Github("missing fake commit".into()))
        }

        fn tree(
            &self,
            _owner: &str,
            _repository: &str,
            tree_sha: &str,
            recursive: bool,
        ) -> Result<GithubTree, CandidateError> {
            self.trees
                .lock()
                .unwrap()
                .get(&(tree_sha.into(), recursive))
                .cloned()
                .ok_or_else(|| CandidateError::Github("missing fake tree".into()))
        }

        fn download_blob(
            &self,
            _owner: &str,
            _repository: &str,
            commit_sha: &str,
            path: &str,
            _expected_size: usize,
        ) -> Result<Vec<u8>, CandidateError> {
            self.downloads
                .lock()
                .unwrap()
                .push((commit_sha.into(), path.into()));
            self.blobs
                .lock()
                .unwrap()
                .get(path)
                .cloned()
                .ok_or_else(|| CandidateError::Github("missing fake blob".into()))
        }
    }

    fn stager(directory: &TempDir, github: Arc<dyn GithubTransport>) -> CandidateStager {
        CandidateStager::with_github(directory.path().join("staging"), github).unwrap()
    }

    fn tree_entry(
        path: &str,
        mode: &str,
        kind: &str,
        sha: &str,
        size: Option<u64>,
    ) -> GithubTreeEntry {
        GithubTreeEntry {
            path: path.into(),
            mode: mode.into(),
            kind: kind.into(),
            sha: sha.into(),
            size,
        }
    }

    #[test]
    fn stages_and_discards_a_local_candidate_without_changing_the_source() {
        let directory = TempDir::new().unwrap();
        let source = directory.path().join("source");
        fs::create_dir_all(source.join("scripts")).unwrap();
        fs::write(source.join("SKILL.md"), "---\nname: demo\n---\n").unwrap();
        fs::write(source.join("scripts/helper.sh"), "echo hello\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                source.join("scripts/helper.sh"),
                fs::Permissions::from_mode(0o755),
            )
            .unwrap();
        }
        let stager = stager(&directory, Arc::new(FakeGithub::default()));
        let manifest = stager.stage_local(&source).unwrap();
        assert_eq!(manifest.files.len(), 2);
        assert_eq!(manifest.total_bytes, 30);
        assert!(manifest
            .files
            .iter()
            .any(|file| file.path == "scripts/helper.sh" && file.executable));
        let staged = stager.store.root.join(&manifest.session_id);
        assert_eq!(
            fs::read_to_string(staged.join("SKILL.md")).unwrap(),
            "---\nname: demo\n---\n"
        );
        assert!(source.join("SKILL.md").exists());
        stager.discard(&manifest.session_id).unwrap();
        assert!(!staged.exists());
        assert!(source.join("SKILL.md").exists());
    }

    #[test]
    fn review_is_hash_bound_and_only_reads_manifest_files() {
        let directory = TempDir::new().unwrap();
        let source = directory.path().join("source");
        fs::create_dir_all(source.join("scripts")).unwrap();
        fs::write(
            source.join("SKILL.md"),
            "---\nname: demo\ndescription: Use this Skill when reviewing a demo.\n---\n\n# Demo\n\nRead the staged files and explain the evidence before any installation decision.\n",
        )
        .unwrap();
        fs::write(source.join("scripts/helper.txt"), "local helper\n").unwrap();
        let stager = stager(&directory, Arc::new(FakeGithub::default()));
        let manifest = stager.stage_local(&source).unwrap();

        let review = stager
            .review(&manifest.session_id, &manifest.candidate_hash)
            .unwrap();
        assert_eq!(review.compatibility.agent, "Codex");
        assert_eq!(review.compatibility.status, "compatible");
        assert!(review.skipped_entries.is_empty());
        assert_eq!(review.manifest.session_id, manifest.session_id);
        let content = stager
            .read_file(
                &manifest.session_id,
                &manifest.candidate_hash,
                "scripts/helper.txt",
            )
            .unwrap();
        assert_eq!(content.content.as_deref(), Some("local helper\n"));
        assert!(matches!(
            stager.read_file(&manifest.session_id, "wrong", "scripts/helper.txt"),
            Err(CandidateError::ChangedSession)
        ));
        assert!(matches!(
            stager.read_file(
                &manifest.session_id,
                &manifest.candidate_hash,
                "outside.txt"
            ),
            Err(CandidateError::UnknownFile)
        ));
    }

    #[test]
    fn review_detects_staging_changes_and_binary_file_previews() {
        let directory = TempDir::new().unwrap();
        let source = directory.path().join("source");
        fs::create_dir_all(&source).unwrap();
        fs::write(
            source.join("SKILL.md"),
            "---\nname: demo\ndescription: Use when viewing a demo candidate.\n---\n\n# Demo\n\nThese instructions are long enough for the basic structure check.\n",
        )
        .unwrap();
        fs::write(source.join("asset.bin"), [0xff, 0x00, 0x80]).unwrap();
        let stager = stager(&directory, Arc::new(FakeGithub::default()));
        let manifest = stager.stage_local(&source).unwrap();

        let binary = stager
            .read_file(&manifest.session_id, &manifest.candidate_hash, "asset.bin")
            .unwrap();
        assert!(!binary.is_text);
        assert!(binary.content.is_none());

        let staged_skill = stager
            .store
            .root
            .join(&manifest.session_id)
            .join("SKILL.md");
        fs::remove_file(&staged_skill).unwrap();
        fs::write(staged_skill, "changed").unwrap();
        assert!(matches!(
            stager.review(&manifest.session_id, &manifest.candidate_hash),
            Err(CandidateError::ChangedSession)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_local_symlinks_and_cleans_failed_staging() {
        use std::os::unix::fs::symlink;

        let directory = TempDir::new().unwrap();
        let source = directory.path().join("source");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("SKILL.md"), "skill").unwrap();
        symlink("SKILL.md", source.join("linked.md")).unwrap();
        let stager = stager(&directory, Arc::new(FakeGithub::default()));
        assert!(matches!(
            stager.stage_local(&source),
            Err(CandidateError::UnsafeEntry(_))
        ));
        assert_eq!(fs::read_dir(&stager.store.root).unwrap().count(), 0);
    }

    #[test]
    fn rejects_oversized_local_files() {
        let directory = TempDir::new().unwrap();
        let source = directory.path().join("source");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("SKILL.md"), vec![0u8; MAX_FILE_BYTES + 1]).unwrap();
        let stager = stager(&directory, Arc::new(FakeGithub::default()));
        assert!(matches!(
            stager.stage_local(&source),
            Err(CandidateError::FileTooLarge(_))
        ));
    }

    #[test]
    fn rejects_local_candidates_without_a_root_skill_document() {
        let directory = TempDir::new().unwrap();
        let source = directory.path().join("source");
        fs::create_dir_all(source.join("nested")).unwrap();
        fs::write(source.join("README.md"), "not a skill").unwrap();
        let stager = stager(&directory, Arc::new(FakeGithub::default()));
        assert!(matches!(
            stager.stage_local(&source),
            Err(CandidateError::MissingSkillDocument)
        ));
        assert_eq!(fs::read_dir(&stager.store.root).unwrap().count(), 0);
    }

    #[test]
    fn rejects_a_local_file_replaced_after_discovery() {
        let directory = TempDir::new().unwrap();
        let source = directory.path().join("source");
        fs::create_dir_all(&source).unwrap();
        let skill_path = source.join("SKILL.md");
        fs::write(&skill_path, "original").unwrap();

        let mut inputs = Vec::new();
        collect_local_files(&source, &source, &mut inputs).unwrap();
        let input = inputs
            .into_iter()
            .find(|input| input.relative == "SKILL.md")
            .unwrap();

        fs::rename(&skill_path, source.join("SKILL.original")).unwrap();
        fs::write(&skill_path, "replacement").unwrap();
        assert!(matches!(
            read_local_file(&source, &input),
            Err(CandidateError::UnsafeEntry(_))
        ));
    }

    #[test]
    fn rejects_local_candidates_beyond_the_depth_limit() {
        let directory = TempDir::new().unwrap();
        let source = directory.path().join("source");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("SKILL.md"), "skill").unwrap();
        let mut nested = source.clone();
        for index in 0..MAX_DEPTH {
            nested.push(format!("level-{index}"));
            fs::create_dir(&nested).unwrap();
        }
        let stager = stager(&directory, Arc::new(FakeGithub::default()));
        assert!(matches!(
            stager.stage_local(&source),
            Err(CandidateError::TooDeep)
        ));
    }

    #[test]
    fn candidate_hash_includes_the_executable_mode() {
        let directory = TempDir::new().unwrap();
        let stager = stager(&directory, Arc::new(FakeGithub::default()));
        let file = CandidateFile {
            path: "run.sh".into(),
            size: 4,
            sha256: sha256(b"echo"),
            executable: false,
        };
        let regular = stager.complete_session(
            "regular".into(),
            CandidateSource::Local {
                selected_path: "/candidate".into(),
            },
            vec![file.clone()],
            4,
        );
        let executable = stager.complete_session(
            "executable".into(),
            CandidateSource::Local {
                selected_path: "/candidate".into(),
            },
            vec![CandidateFile {
                executable: true,
                ..file
            }],
            4,
        );
        assert_ne!(regular.candidate_hash, executable.candidate_hash);
    }

    #[test]
    fn stages_github_files_at_the_resolved_commit() {
        let directory = TempDir::new().unwrap();
        let github = Arc::new(FakeGithub {
            default_branch: "main".into(),
            commit: Some(ResolvedCommit {
                sha: "a".repeat(40),
                root_tree_sha: "root".into(),
            }),
            ..FakeGithub::default()
        });
        github.trees.lock().unwrap().insert(
            ("root".into(), false),
            GithubTree {
                entries: vec![tree_entry("skills", "040000", "tree", "skills", None)],
                truncated: false,
            },
        );
        github.trees.lock().unwrap().insert(
            ("skills".into(), false),
            GithubTree {
                entries: vec![tree_entry("demo", "040000", "tree", "demo", None)],
                truncated: false,
            },
        );
        github.trees.lock().unwrap().insert(
            ("demo".into(), true),
            GithubTree {
                entries: vec![
                    tree_entry("SKILL.md", "100644", "blob", "one", Some(5)),
                    tree_entry("run.sh", "100755", "blob", "two", Some(4)),
                ],
                truncated: false,
            },
        );
        github
            .blobs
            .lock()
            .unwrap()
            .insert("skills/demo/SKILL.md".into(), b"skill".to_vec());
        github
            .blobs
            .lock()
            .unwrap()
            .insert("skills/demo/run.sh".into(), b"echo".to_vec());
        let stager = stager(&directory, github.clone());
        let manifest = stager
            .stage_github("https://github.com/owner/repo/tree/main/skills/demo")
            .unwrap();
        assert_eq!(manifest.files.len(), 2);
        assert_eq!(manifest.total_bytes, 9);
        assert!(manifest.files.iter().any(|file| file.executable));
        match manifest.source {
            CandidateSource::Github {
                repository,
                requested_ref,
                resolved_sha,
                skill_path,
            } => {
                assert_eq!(repository, "owner/repo");
                assert_eq!(requested_ref, "main");
                assert_eq!(resolved_sha, "a".repeat(40));
                assert_eq!(skill_path, "skills/demo");
            }
            CandidateSource::Local { .. } => panic!("expected GitHub source"),
        }
        assert!(github
            .downloads
            .lock()
            .unwrap()
            .iter()
            .all(|(sha, _)| sha == &"a".repeat(40)));
    }

    #[test]
    fn rejects_github_links_before_downloading_content() {
        let directory = TempDir::new().unwrap();
        let github = Arc::new(FakeGithub {
            default_branch: "main".into(),
            commit: Some(ResolvedCommit {
                sha: "b".repeat(40),
                root_tree_sha: "root".into(),
            }),
            ..FakeGithub::default()
        });
        github.trees.lock().unwrap().insert(
            ("root".into(), true),
            GithubTree {
                entries: vec![
                    tree_entry("SKILL.md", "100644", "blob", "one", Some(5)),
                    tree_entry("escape", "120000", "blob", "two", Some(10)),
                ],
                truncated: false,
            },
        );
        let stager = stager(&directory, github.clone());
        assert!(matches!(
            stager.stage_github("https://github.com/owner/repo"),
            Err(CandidateError::UnsafeEntry(_))
        ));
        assert!(github.downloads.lock().unwrap().is_empty());
        assert_eq!(fs::read_dir(&stager.store.root).unwrap().count(), 0);
    }

    #[test]
    fn rejects_case_colliding_github_paths() {
        let entries = vec![
            tree_entry("SKILL.md", "100644", "blob", "one", Some(5)),
            tree_entry("skill.md", "100644", "blob", "two", Some(5)),
        ];
        assert!(matches!(
            github_inputs(entries),
            Err(CandidateError::UnsafeEntry(_))
        ));
    }

    #[test]
    fn rejects_github_truncation_submodules_and_resource_limits_before_download() {
        let truncated = GithubTree {
            entries: vec![tree_entry("SKILL.md", "100644", "blob", "one", Some(5))],
            truncated: true,
        };
        let directory = TempDir::new().unwrap();
        let github = Arc::new(FakeGithub {
            commit: Some(ResolvedCommit {
                sha: "d".repeat(40),
                root_tree_sha: "root".into(),
            }),
            ..FakeGithub::default()
        });
        github
            .trees
            .lock()
            .unwrap()
            .insert(("root".into(), true), truncated);
        let stager = stager(&directory, github.clone());
        assert!(matches!(
            stager.stage_github("https://github.com/owner/repo"),
            Err(CandidateError::TruncatedTree)
        ));
        assert!(github.downloads.lock().unwrap().is_empty());

        let submodule = vec![
            tree_entry("SKILL.md", "100644", "blob", "one", Some(5)),
            tree_entry("dependency", "160000", "commit", "two", None),
        ];
        assert!(matches!(
            github_inputs(submodule),
            Err(CandidateError::UnsafeEntry(_))
        ));

        let traversal = vec![
            tree_entry("SKILL.md", "100644", "blob", "one", Some(5)),
            tree_entry("../escape", "100644", "blob", "two", Some(5)),
        ];
        assert!(matches!(
            github_inputs(traversal),
            Err(CandidateError::UnsafeEntry(_))
        ));

        let too_many = (0..=MAX_FILES)
            .map(|index| {
                let path = if index == 0 {
                    "SKILL.md".into()
                } else {
                    format!("file-{index}.txt")
                };
                tree_entry(&path, "100644", "blob", "sha", Some(1))
            })
            .collect();
        assert!(matches!(
            github_inputs(too_many),
            Err(CandidateError::TooManyFiles)
        ));

        let too_large = vec![tree_entry(
            "SKILL.md",
            "100644",
            "blob",
            "sha",
            Some((MAX_FILE_BYTES + 1) as u64),
        )];
        assert!(matches!(
            github_inputs(too_large),
            Err(CandidateError::FileTooLarge(_))
        ));

        let too_wide = (0..13)
            .map(|index| {
                let path = if index == 0 {
                    "SKILL.md".into()
                } else {
                    format!("file-{index}.txt")
                };
                tree_entry(&path, "100644", "blob", "sha", Some(MAX_FILE_BYTES as u64))
            })
            .collect();
        assert!(matches!(
            github_inputs(too_wide),
            Err(CandidateError::CandidateTooLarge)
        ));
    }

    #[test]
    fn github_download_size_mismatch_cleans_partial_session() {
        let directory = TempDir::new().unwrap();
        let github = Arc::new(FakeGithub {
            commit: Some(ResolvedCommit {
                sha: "c".repeat(40),
                root_tree_sha: "root".into(),
            }),
            ..FakeGithub::default()
        });
        github.trees.lock().unwrap().insert(
            ("root".into(), true),
            GithubTree {
                entries: vec![tree_entry("SKILL.md", "100644", "blob", "one", Some(5))],
                truncated: false,
            },
        );
        github
            .blobs
            .lock()
            .unwrap()
            .insert("SKILL.md".into(), b"shorter".to_vec());
        let stager = stager(&directory, github);
        assert!(matches!(
            stager.stage_github("https://github.com/owner/repo/tree/main"),
            Err(CandidateError::InconsistentGithubSource)
        ));
        assert_eq!(fs::read_dir(&stager.store.root).unwrap().count(), 0);
    }

    #[test]
    fn github_url_parser_rejects_credentials_queries_and_non_skill_blobs() {
        for url in [
            "http://github.com/owner/repo",
            "https://user@github.com/owner/repo",
            "https://github.com/owner/repo?token=secret",
            "https://github.com/owner/repo/blob/main/README.md",
            "https://example.com/owner/repo",
        ] {
            assert!(matches!(
                parse_github_url(url),
                Err(CandidateError::InvalidGithubUrl)
            ));
        }
    }

    #[test]
    fn startup_removes_stale_staging_and_unknown_sessions_cannot_delete_paths() {
        let directory = TempDir::new().unwrap();
        let root = directory.path().join("staging");
        fs::create_dir_all(root.join("stale")).unwrap();
        fs::write(root.join("stale/file"), "old").unwrap();
        let stager =
            CandidateStager::with_github(root.clone(), Arc::new(FakeGithub::default())).unwrap();
        assert_eq!(fs::read_dir(&stager.store.root).unwrap().count(), 0);
        assert!(matches!(
            stager.discard("../source"),
            Err(CandidateError::UnknownSession)
        ));
    }
}
