use base64::{engine::general_purpose::STANDARD, Engine as _};
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
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc, Arc, Mutex,
    },
    time::Duration,
};
use tempfile::Builder;
use thiserror::Error;

use super::{
    is_ignored_skill_metadata_name, is_ignored_skill_metadata_path,
    package::CandidateFileSyncAction,
};

const MAX_FILES: usize = 256;
const MAX_TOTAL_BYTES: usize = 25 * 1024 * 1024;
const MAX_FILE_BYTES: usize = 2 * 1024 * 1024;
const MAX_DEPTH: usize = 8;
const MAX_SOURCE_PATH_COMPONENTS: usize = 16;
const MAX_API_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const MAX_REPOSITORY_SKILLS: usize = 256;
const MAX_CONCURRENT_GITHUB_BLOBS: usize = 6;
const MAX_CACHED_REPOSITORY_REVISIONS: usize = 8;

#[derive(Debug, Error)]
pub enum CandidateError {
    #[error("Enter a public https://github.com repository or Skill directory URL.")]
    InvalidGithubUrl,
    #[error("The selected local candidate must be a real directory, not a link.")]
    InvalidLocalDirectory,
    #[error("The candidate does not contain SKILL.md at its root.")]
    MissingSkillDocument,
    #[error("The repository does not contain a Skill at a supported conventional path.")]
    NoRepositorySkills,
    #[error("The repository contains more than 256 discoverable Skills.")]
    TooManyRepositorySkills,
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
    #[error("This file cannot be synchronized with the requested action.")]
    InvalidFileSync,
    #[error("Unable to stage the candidate: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Error)]
pub enum CandidateInstallError {
    #[error(transparent)]
    Candidate(#[from] CandidateError),
    #[error(transparent)]
    Workspace(#[from] super::WorkspaceError),
    #[error("This candidate cannot be installed until blocking findings or compatibility problems are resolved.")]
    Blocked,
    #[error("The installation preview changed. Review the destination again.")]
    PreviewMismatch,
}

#[derive(Debug, Error)]
pub enum GithubUpdateError {
    #[error(transparent)]
    Candidate(#[from] CandidateError),
    #[error(transparent)]
    Workspace(#[from] super::WorkspaceError),
    #[error("This Skill does not have sufficient GitHub provenance for an update check.")]
    ProvenanceUnavailable,
    #[error("Only user-controlled Skills can check for GitHub updates.")]
    ReadOnly,
}

impl GithubUpdateError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Candidate(error) => error.code(),
            Self::Workspace(error) => error.code(),
            Self::ProvenanceUnavailable => "GITHUB_UPDATE_PROVENANCE_UNAVAILABLE",
            Self::ReadOnly => "GITHUB_UPDATE_READ_ONLY",
        }
    }
}

impl CandidateInstallError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Candidate(error) => error.code(),
            Self::Workspace(error) => error.code(),
            Self::Blocked => "CANDIDATE_INSTALL_BLOCKED",
            Self::PreviewMismatch => "STALE_INSTALL_PREVIEW",
        }
    }
}

impl CandidateError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidGithubUrl => "INVALID_GITHUB_CANDIDATE_URL",
            Self::InvalidLocalDirectory => "INVALID_LOCAL_CANDIDATE",
            Self::MissingSkillDocument => "MISSING_SKILL_DOCUMENT",
            Self::NoRepositorySkills => "NO_REPOSITORY_SKILLS",
            Self::TooManyRepositorySkills => "REPOSITORY_SKILL_LIMIT",
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
            Self::InvalidFileSync => "INVALID_CANDIDATE_FILE_SYNC",
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

/// Metadata-only discovery result for the existing GitHub candidate intake.
/// No candidate blob is downloaded while producing this value.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubRepositoryListing {
    pub repository: String,
    pub requested_ref: String,
    pub resolved_sha: String,
    pub candidates: Vec<GithubRepositoryCandidate>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubRepositoryCandidate {
    pub skill_path: String,
    pub directory_name: String,
    pub repository_root: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubUpdateCheck {
    pub status: String,
    pub manifest: CandidateManifest,
    pub local_files: Vec<CandidateFile>,
    pub local_revision: String,
    pub local_candidate_hash: String,
    pub installed_candidate_hash: Option<String>,
    pub installed_sha: Option<String>,
    pub remote_sha: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
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

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateInstallPreview {
    pub name: String,
    pub destination: String,
    pub file_count: usize,
    pub candidate_hash: String,
    pub install_revision: String,
    pub compatibility_status: String,
    pub audit_verdict: String,
    pub classification: String,
    pub conflict: Option<super::NameConflict>,
    pub can_install: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateInstallResult {
    pub status: String,
    pub installed_id: String,
    pub skill: Option<super::SkillDetail>,
    pub destination: String,
    pub installed_files: usize,
    pub candidate_hash: String,
    pub catalog_refresh_needed: bool,
    pub restart_recommended: bool,
    pub provenance_recorded: bool,
}

#[derive(Clone)]
pub struct CandidateStager {
    store: Arc<StagingStore>,
    github: Arc<dyn GithubTransport>,
    repository_revisions: Arc<Mutex<HashMap<RepositoryRevisionKey, RepositoryRevision>>>,
}

struct StagingStore {
    root: PathBuf,
    sessions: Mutex<HashMap<String, CandidateManifest>>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct RepositoryRevisionKey {
    owner: String,
    repository: String,
    requested_ref: String,
    resolved_sha: String,
}

#[derive(Clone)]
struct RepositoryRevision {
    commit: ResolvedCommit,
    entries: Vec<GithubTreeEntry>,
    candidates: Vec<GithubRepositoryCandidate>,
}

struct VerifiedCandidateSnapshot {
    review: CandidateReview,
    files: Vec<VerifiedCandidateFile>,
}

struct VerifiedCandidateFile {
    manifest: CandidateFile,
    bytes: Vec<u8>,
}

#[derive(Clone)]
pub(crate) struct CandidateAuditSnapshot {
    pub candidate_hash: String,
    pub files: Vec<CandidateAuditSnapshotFile>,
}

#[derive(Clone)]
pub(crate) struct CandidateAuditSnapshotFile {
    pub manifest: CandidateFile,
    pub bytes: Vec<u8>,
}

impl Drop for StagingStore {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl CandidateStager {
    pub fn source(
        &self,
        session_id: &str,
        expected_candidate_hash: &str,
    ) -> Result<CandidateSource, CandidateError> {
        Ok(self
            .session_manifest(session_id, expected_candidate_hash)?
            .source)
    }

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
            repository_revisions: Arc::new(Mutex::new(HashMap::new())),
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
        self.stage_github_request(request)
    }

    /// Lists the bounded set of conventional Skills in a repository root. This
    /// intentionally reads GitHub tree metadata only; content remains untrusted
    /// and unstaged until the owner opens one selected entry.
    pub fn list_github_repository(
        &self,
        source_url: &str,
    ) -> Result<GithubRepositoryListing, CandidateError> {
        let request = parse_github_url(source_url)?;
        if !request.skill_path.is_empty() {
            return Err(CandidateError::InvalidGithubUrl);
        }
        let requested_ref = match request.requested_ref.clone() {
            Some(reference) => reference,
            None => self
                .github
                .default_branch(&request.owner, &request.repository)?,
        };
        let commit =
            self.github
                .resolve_commit(&request.owner, &request.repository, &requested_ref)?;
        let tree = self.github.tree(
            &request.owner,
            &request.repository,
            &commit.root_tree_sha,
            true,
        )?;
        if tree.truncated {
            return Err(CandidateError::TruncatedTree);
        }
        let candidates = discover_repository_candidates(tree.entries.clone())?;
        self.remember_repository_revision(
            &request,
            &requested_ref,
            &commit,
            tree.entries,
            candidates.clone(),
        );
        Ok(GithubRepositoryListing {
            repository: format!("{}/{}", request.owner, request.repository),
            requested_ref,
            resolved_sha: commit.sha,
            candidates,
        })
    }

    /// Stages one path selected from a prior repository listing. The resolved
    /// commit is supplied by that listing. Discovery metadata is reused only
    /// within this process and every downloaded blob remains bound to the
    /// immutable commit and tree metadata.
    pub fn stage_github_repository_candidate(
        &self,
        source_url: &str,
        requested_ref: &str,
        resolved_sha: &str,
        skill_path: &str,
    ) -> Result<CandidateManifest, CandidateError> {
        let source = parse_github_url(source_url)?;
        if !source.skill_path.is_empty()
            || source
                .requested_ref
                .as_deref()
                .is_some_and(|source_ref| source_ref != requested_ref)
            || requested_ref.is_empty()
            || requested_ref.len() > 255
            || !valid_sha(resolved_sha)
            || !is_conventional_repository_skill_path(skill_path)
        {
            return Err(CandidateError::InvalidGithubUrl);
        }
        let request = GithubRequest {
            owner: source.owner,
            repository: source.repository,
            requested_ref: Some(requested_ref.into()),
            skill_path: skill_path.into(),
        };
        let revision = self.repository_revision(&request, requested_ref, resolved_sha)?;
        if !revision
            .candidates
            .iter()
            .any(|candidate| candidate.skill_path == skill_path)
        {
            return Err(CandidateError::UnsafeEntry(skill_path.into()));
        }
        let entries =
            repository_candidate_entries(&revision.entries, &revision.candidates, skill_path);
        let inputs = github_inputs(entries)?;
        require_skill_document(inputs.iter().map(|input| input.relative.as_str()))?;
        self.stage_github_inputs(request, requested_ref.into(), revision.commit, inputs)
    }

    pub fn check_github_update(
        &self,
        workspace: &super::Workspace,
        skill_id: &str,
        acquisition: &super::AcquisitionProvenance,
    ) -> Result<GithubUpdateCheck, GithubUpdateError> {
        if acquisition.kind != "github"
            || !matches!(acquisition.confidence.as_str(), "recorded" | "confirmed")
        {
            return Err(GithubUpdateError::ProvenanceUnavailable);
        }
        let repository = acquisition
            .repository
            .as_deref()
            .ok_or(GithubUpdateError::ProvenanceUnavailable)?;
        let (owner, repository) = repository
            .split_once('/')
            .filter(|(owner, repository)| valid_github_atom(owner) && valid_github_atom(repository))
            .ok_or(GithubUpdateError::ProvenanceUnavailable)?;
        let skill_path = acquisition.skill_path.clone().unwrap_or_default();
        if source_components(&skill_path).count() > MAX_SOURCE_PATH_COMPONENTS
            || source_components(&skill_path).any(|component| !valid_path_atom(component))
        {
            return Err(GithubUpdateError::ProvenanceUnavailable);
        }
        let skill = workspace.find_skill(skill_id)?;
        if !matches!(
            skill.source,
            super::Source::Personal | super::Source::Disabled | super::Source::Archive
        ) {
            return Err(GithubUpdateError::ReadOnly);
        }
        let local_files = candidate_files_for_directory(&skill.directory)?;
        let local_candidate_hash = candidate_hash(&local_files);
        let local_revision = super::lifecycle::directory_revision(&skill.directory)?;
        let manifest = self.stage_github_request(GithubRequest {
            owner: owner.into(),
            repository: repository.into(),
            requested_ref: acquisition.requested_ref.clone(),
            skill_path,
        })?;
        let CandidateSource::Github { resolved_sha, .. } = &manifest.source else {
            unreachable!("GitHub staging always records a GitHub source");
        };
        let status = classify_github_update(
            &local_candidate_hash,
            &manifest.candidate_hash,
            acquisition.candidate_hash.as_deref(),
            acquisition.resolved_sha.as_deref(),
            resolved_sha,
        );
        Ok(GithubUpdateCheck {
            status: status.into(),
            remote_sha: resolved_sha.clone(),
            manifest,
            local_files,
            local_revision,
            local_candidate_hash,
            installed_candidate_hash: acquisition.candidate_hash.clone(),
            installed_sha: acquisition.resolved_sha.clone(),
        })
    }

    fn stage_github_request(
        &self,
        request: GithubRequest,
    ) -> Result<CandidateManifest, CandidateError> {
        let requested_ref = match request.requested_ref.clone() {
            Some(reference) => reference,
            None => self
                .github
                .default_branch(&request.owner, &request.repository)?,
        };
        let commit =
            self.github
                .resolve_commit(&request.owner, &request.repository, &requested_ref)?;
        self.stage_github_at_commit(request, requested_ref, commit, false)
    }

    fn stage_github_at_commit(
        &self,
        request: GithubRequest,
        requested_ref: String,
        commit: ResolvedCommit,
        repository_root_intake: bool,
    ) -> Result<CandidateManifest, CandidateError> {
        let target_tree_sha = self.resolve_target_tree(&request, &commit.root_tree_sha)?;
        let tree = self
            .github
            .tree(&request.owner, &request.repository, &target_tree_sha, true)?;
        if tree.truncated {
            return Err(CandidateError::TruncatedTree);
        }
        let excluded_nested_skill_paths = if repository_root_intake && request.skill_path.is_empty()
        {
            discover_repository_candidates(tree.entries.clone())?
                .into_iter()
                .filter(|candidate| !candidate.repository_root)
                .map(|candidate| candidate.skill_path)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let inputs = github_inputs(
            tree.entries
                .into_iter()
                .filter(|entry| {
                    !excluded_nested_skill_paths.iter().any(|path| {
                        entry.path == *path || entry.path.starts_with(&format!("{path}/"))
                    })
                })
                .collect(),
        )?;
        require_skill_document(inputs.iter().map(|input| input.relative.as_str()))?;

        self.stage_github_inputs(request, requested_ref, commit, inputs)
    }

    fn stage_github_inputs(
        &self,
        request: GithubRequest,
        requested_ref: String,
        commit: ResolvedCommit,
        inputs: Vec<GithubInput>,
    ) -> Result<CandidateManifest, CandidateError> {
        let downloads = self.download_github_inputs(&request, &commit, &inputs)?;

        let (session_id, session_path) = self.begin_session()?;
        let result = (|| {
            let mut files = Vec::with_capacity(inputs.len());
            let mut total_bytes = 0usize;
            for (input, bytes) in inputs.into_iter().zip(downloads) {
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

    fn download_github_inputs(
        &self,
        request: &GithubRequest,
        commit: &ResolvedCommit,
        inputs: &[GithubInput],
    ) -> Result<Vec<Vec<u8>>, CandidateError> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }
        let next = AtomicUsize::new(0);
        let cancelled = AtomicBool::new(false);
        let worker_count = inputs.len().min(MAX_CONCURRENT_GITHUB_BLOBS);
        let (sender, receiver) = mpsc::channel();

        std::thread::scope(|scope| {
            for _ in 0..worker_count {
                let sender = sender.clone();
                let github = self.github.clone();
                let next = &next;
                let cancelled = &cancelled;
                scope.spawn(move || loop {
                    if cancelled.load(Ordering::Acquire) {
                        break;
                    }
                    let index = next.fetch_add(1, Ordering::AcqRel);
                    let Some(input) = inputs.get(index) else {
                        break;
                    };
                    let source_path = join_source_path(&request.skill_path, &input.relative);
                    let result = github
                        .download_blob(
                            &request.owner,
                            &request.repository,
                            &commit.sha,
                            &source_path,
                            &input.blob_sha,
                            input.declared_size,
                        )
                        .and_then(|bytes| {
                            if bytes.len() == input.declared_size {
                                Ok(bytes)
                            } else {
                                Err(CandidateError::InconsistentGithubSource)
                            }
                        });
                    if result.is_err() {
                        cancelled.store(true, Ordering::Release);
                    }
                    if sender.send((index, result)).is_err() {
                        break;
                    }
                });
            }
        });
        drop(sender);

        let mut downloads = (0..inputs.len()).map(|_| None).collect::<Vec<_>>();
        let mut error = None;
        for (index, result) in receiver {
            match result {
                Ok(bytes) => downloads[index] = Some(bytes),
                Err(candidate_error) if error.is_none() => error = Some(candidate_error),
                Err(_) => {}
            }
        }
        if let Some(error) = error {
            return Err(error);
        }
        downloads
            .into_iter()
            .map(|download| {
                download.ok_or_else(|| {
                    CandidateError::Github("a GitHub download worker stopped unexpectedly".into())
                })
            })
            .collect()
    }

    pub fn review(
        &self,
        session_id: &str,
        expected_candidate_hash: &str,
    ) -> Result<CandidateReview, CandidateError> {
        let manifest = self.session_manifest(session_id, expected_candidate_hash)?;
        let directory = self.session_directory(session_id)?;
        let skill = manifest
            .files
            .iter()
            .find(|file| file.path == "SKILL.md")
            .ok_or(CandidateError::MissingSkillDocument)?;
        let skill_bytes = read_verified_staged_file(&directory, skill)?;
        Ok(review_from_skill_bytes(manifest, skill_bytes))
    }

    pub fn preview_install(
        &self,
        workspace: &super::Workspace,
        session_id: &str,
        expected_candidate_hash: &str,
    ) -> Result<CandidateInstallPreview, CandidateInstallError> {
        let review = self.review(session_id, expected_candidate_hash)?;
        Ok(install_preview(workspace, &review)?)
    }

    pub fn install(
        &self,
        workspace: &super::Workspace,
        session_id: &str,
        expected_candidate_hash: &str,
        expected_install_revision: &str,
    ) -> Result<CandidateInstallResult, CandidateInstallError> {
        let snapshot = self.verified_snapshot(session_id, expected_candidate_hash)?;
        let advisory = install_preview(workspace, &snapshot.review)?;
        if expected_install_revision.is_empty()
            || expected_install_revision != advisory.install_revision
        {
            return Err(CandidateInstallError::PreviewMismatch);
        }
        if snapshot.review.compatibility.status == "incompatible"
            || snapshot.review.audit.verdict == "block"
        {
            return Err(CandidateInstallError::Blocked);
        }

        if advisory.classification == "identical" {
            let _mutation = workspace
                .mutations
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let refreshed = workspace.scan_catalog_index()?;
            *workspace
                .index
                .write()
                .unwrap_or_else(|error| error.into_inner()) = Some(refreshed);
            if let Some(skill) = exact_name_match(
                workspace,
                &advisory.name,
                &snapshot.review.manifest.candidate_hash,
            )? {
                return Ok(identical_install_result(&snapshot, skill));
            }
            return Err(CandidateInstallError::PreviewMismatch);
        }

        let personal_root = workspace.personal_root_for_creation()?;
        let destination = personal_root.join(&advisory.name);
        if !destination.starts_with(&personal_root)
            || destination.display().to_string() != advisory.destination
        {
            return Err(super::WorkspaceError::UnsafePath.into());
        }
        let temporary = Builder::new()
            .prefix(".candidate-install-")
            .tempdir_in(&personal_root)
            .map_err(super::WorkspaceError::Io)?;
        for file in &snapshot.files {
            write_install_file(temporary.path(), &file.manifest, &file.bytes)?;
        }
        sync_install_directories(temporary.path(), &snapshot.files)?;

        let _mutation = workspace
            .mutations
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let refreshed = workspace.scan_catalog_index()?;
        *workspace
            .index
            .write()
            .unwrap_or_else(|error| error.into_inner()) = Some(refreshed);
        if let Some(skill) = exact_name_match(
            workspace,
            &advisory.name,
            &snapshot.review.manifest.candidate_hash,
        )? {
            return Ok(identical_install_result(&snapshot, skill));
        }
        if let Some(conflict) = workspace.find_name_conflict(&advisory.name)? {
            return Err(super::WorkspaceError::NameConflict {
                name: advisory.name,
                source_label: conflict.source,
            }
            .into());
        }
        let current_personal_root = workspace.personal_root_for_creation()?;
        if current_personal_root != personal_root {
            return Err(super::WorkspaceError::UnsafePath.into());
        }
        rename_directory_no_replace(temporary.path(), &destination).map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                CandidateInstallError::Workspace(super::WorkspaceError::NameConflict {
                    name: advisory.name.clone(),
                    source_label: "personal".into(),
                })
            } else {
                CandidateInstallError::Workspace(super::WorkspaceError::Io(error))
            }
        })?;
        let _ = temporary.keep();
        // The rename is the commit boundary. Parent syncing is best-effort because a
        // post-commit durability error must not be reported as a failed installation.
        let _ = sync_directory(&personal_root);
        let installed_id = super::skill_id(super::Source::Personal, &destination);

        let installed =
            match workspace.read_skill(&destination, super::Source::Personal, &personal_root) {
                Ok(Some(skill)) => skill,
                Ok(None) | Err(_) => {
                    *workspace
                        .index
                        .write()
                        .unwrap_or_else(|error| error.into_inner()) = None;
                    return Ok(CandidateInstallResult {
                        status: "installed".into(),
                        installed_id,
                        skill: None,
                        destination: destination.display().to_string(),
                        installed_files: snapshot.files.len(),
                        candidate_hash: snapshot.review.manifest.candidate_hash,
                        catalog_refresh_needed: true,
                        restart_recommended: true,
                        provenance_recorded: false,
                    });
                }
            };
        let installed_id = installed.summary.id.clone();
        let skill = super::SkillDetail {
            content_hash: super::hash(&installed.markdown),
            summary: installed.summary.clone(),
            markdown: installed.markdown.clone(),
            document: installed.document.clone(),
            editable: true,
        };
        workspace.upsert_index(installed)?;
        debug_assert_eq!(skill.summary.id, installed_id);
        Ok(CandidateInstallResult {
            status: "installed".into(),
            installed_id,
            skill: Some(skill),
            destination: destination.display().to_string(),
            installed_files: snapshot.files.len(),
            candidate_hash: snapshot.review.manifest.candidate_hash,
            catalog_refresh_needed: false,
            restart_recommended: true,
            provenance_recorded: false,
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

    pub fn file_sync_data(
        &self,
        session_id: &str,
        expected_candidate_hash: &str,
        path: &str,
        action: CandidateFileSyncAction,
    ) -> Result<Option<(Vec<u8>, bool)>, CandidateError> {
        let snapshot = self.verified_snapshot(session_id, expected_candidate_hash)?;
        let path = validated_relative_string(path)?;
        let remote = snapshot
            .files
            .into_iter()
            .find(|file| file.manifest.path == path);
        match (action, remote) {
            (CandidateFileSyncAction::Add | CandidateFileSyncAction::Replace, Some(file)) => {
                Ok(Some((file.bytes, file.manifest.executable)))
            }
            (CandidateFileSyncAction::Delete, None) if path != "SKILL.md" => Ok(None),
            _ => Err(CandidateError::InvalidFileSync),
        }
    }

    pub fn directory_matches(
        &self,
        session_id: &str,
        expected_candidate_hash: &str,
        directory: &Path,
    ) -> Result<bool, CandidateError> {
        let manifest = self.session_manifest(session_id, expected_candidate_hash)?;
        Ok(candidate_hash(&candidate_files_for_directory(directory)?) == manifest.candidate_hash)
    }

    pub(crate) fn audit_snapshot(
        &self,
        session_id: &str,
        expected_candidate_hash: &str,
    ) -> Result<CandidateAuditSnapshot, CandidateError> {
        let snapshot = self.verified_snapshot(session_id, expected_candidate_hash)?;
        Ok(CandidateAuditSnapshot {
            candidate_hash: snapshot.review.manifest.candidate_hash,
            files: snapshot
                .files
                .into_iter()
                .map(|file| CandidateAuditSnapshotFile {
                    manifest: file.manifest,
                    bytes: file.bytes,
                })
                .collect(),
        })
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

    fn verified_snapshot(
        &self,
        session_id: &str,
        expected_candidate_hash: &str,
    ) -> Result<VerifiedCandidateSnapshot, CandidateError> {
        let manifest = self.session_manifest(session_id, expected_candidate_hash)?;
        let directory = self.session_directory(session_id)?;
        verify_staged_file_set(&directory, &manifest)?;
        let mut files = Vec::with_capacity(manifest.files.len());
        let mut skill_bytes = None;
        for file in &manifest.files {
            let bytes = read_verified_staged_file(&directory, file)?;
            if file.path == "SKILL.md" {
                skill_bytes = Some(bytes.clone());
            }
            files.push(VerifiedCandidateFile {
                manifest: file.clone(),
                bytes,
            });
        }
        let review = review_from_skill_bytes(
            manifest,
            skill_bytes.ok_or(CandidateError::MissingSkillDocument)?,
        );
        Ok(VerifiedCandidateSnapshot { review, files })
    }

    fn repository_revision(
        &self,
        request: &GithubRequest,
        requested_ref: &str,
        resolved_sha: &str,
    ) -> Result<RepositoryRevision, CandidateError> {
        let key = RepositoryRevisionKey {
            owner: request.owner.clone(),
            repository: request.repository.clone(),
            requested_ref: requested_ref.into(),
            resolved_sha: resolved_sha.into(),
        };
        if let Some(revision) = self
            .repository_revisions
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(&key)
            .cloned()
        {
            return Ok(revision);
        }

        let commit =
            self.github
                .resolve_commit(&request.owner, &request.repository, resolved_sha)?;
        if commit.sha != resolved_sha {
            return Err(CandidateError::InconsistentGithubSource);
        }
        let tree = self.github.tree(
            &request.owner,
            &request.repository,
            &commit.root_tree_sha,
            true,
        )?;
        if tree.truncated {
            return Err(CandidateError::TruncatedTree);
        }
        let candidates = discover_repository_candidates(tree.entries.clone())?;
        let revision = RepositoryRevision {
            commit,
            entries: tree.entries,
            candidates,
        };
        self.store_repository_revision(key, revision.clone());
        Ok(revision)
    }

    fn remember_repository_revision(
        &self,
        request: &GithubRequest,
        requested_ref: &str,
        commit: &ResolvedCommit,
        entries: Vec<GithubTreeEntry>,
        candidates: Vec<GithubRepositoryCandidate>,
    ) {
        self.store_repository_revision(
            RepositoryRevisionKey {
                owner: request.owner.clone(),
                repository: request.repository.clone(),
                requested_ref: requested_ref.into(),
                resolved_sha: commit.sha.clone(),
            },
            RepositoryRevision {
                commit: commit.clone(),
                entries,
                candidates,
            },
        );
    }

    fn store_repository_revision(&self, key: RepositoryRevisionKey, revision: RepositoryRevision) {
        let mut revisions = self
            .repository_revisions
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if !revisions.contains_key(&key) && revisions.len() >= MAX_CACHED_REPOSITORY_REVISIONS {
            revisions.clear();
        }
        revisions.insert(key, revision);
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

fn candidate_files_for_directory(directory: &Path) -> Result<Vec<CandidateFile>, CandidateError> {
    let metadata = fs::symlink_metadata(directory).map_err(CandidateError::Io)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(CandidateError::InvalidLocalDirectory);
    }
    let root = fs::canonicalize(directory).map_err(CandidateError::Io)?;
    let mut inputs = Vec::new();
    collect_local_files(&root, &root, &mut inputs)?;
    require_skill_document(inputs.iter().map(|input| input.relative.as_str()))?;
    inputs.sort_by(|left, right| left.relative.cmp(&right.relative));
    let mut files = Vec::with_capacity(inputs.len());
    let mut total_bytes = 0usize;
    for input in inputs {
        let bytes = read_local_file(&root, &input)?;
        total_bytes = checked_total(total_bytes, bytes.len())?;
        files.push(CandidateFile {
            path: input.relative,
            size: bytes.len(),
            sha256: sha256(&bytes),
            executable: input.executable,
        });
    }
    Ok(files)
}

fn candidate_hash(files: &[CandidateFile]) -> String {
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
    sha256(fingerprint.as_bytes())
}

fn classify_github_update(
    local_hash: &str,
    remote_hash: &str,
    installed_hash: Option<&str>,
    installed_sha: Option<&str>,
    remote_sha: &str,
) -> &'static str {
    if local_hash == remote_hash {
        return "identical";
    }
    match (installed_hash, installed_sha) {
        (Some(installed_hash), Some(installed_sha)) if local_hash == installed_hash => {
            if installed_sha == remote_sha {
                "localMismatch"
            } else {
                "remoteChanged"
            }
        }
        (Some(_), Some(installed_sha)) if installed_sha == remote_sha => "localChanged",
        (Some(_), Some(_)) => "diverged",
        _ => "differentUnknown",
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
    blob_sha: String,
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
        if is_ignored_skill_metadata_name(&entry.file_name()) {
            continue;
        }
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
        if entry.kind != "blob"
            || !matches!(entry.mode.as_str(), "100644" | "100755")
            || !valid_sha(&entry.sha)
        {
            return Err(CandidateError::UnsafeEntry(entry.path));
        }
        let relative = validated_relative_string(&entry.path)?;
        if is_ignored_skill_metadata_path(&relative) {
            continue;
        }
        let declared_size = entry
            .size
            .and_then(|size| usize::try_from(size).ok())
            .ok_or_else(|| CandidateError::UnsafeEntry(relative.clone()))?;
        validate_file_slot(files.len(), declared_size as u64, &relative)?;
        declared_total = checked_total(declared_total, declared_size)?;
        files.push(GithubInput {
            relative,
            blob_sha: entry.sha,
            declared_size,
            executable: entry.mode == "100755",
        });
    }
    validate_unique_paths(files.iter().map(|file| file.relative.as_str()))?;
    files.sort_by(|left, right| left.relative.cmp(&right.relative));
    Ok(files)
}

fn repository_candidate_entries(
    entries: &[GithubTreeEntry],
    candidates: &[GithubRepositoryCandidate],
    skill_path: &str,
) -> Vec<GithubTreeEntry> {
    if skill_path.is_empty() {
        let nested_paths = candidates
            .iter()
            .filter(|candidate| !candidate.repository_root)
            .map(|candidate| candidate.skill_path.as_str())
            .collect::<Vec<_>>();
        return entries
            .iter()
            .filter(|entry| {
                !nested_paths
                    .iter()
                    .any(|path| entry.path == *path || entry.path.starts_with(&format!("{path}/")))
            })
            .cloned()
            .collect();
    }

    let prefix = format!("{skill_path}/");
    entries
        .iter()
        .filter_map(|entry| {
            let relative = entry.path.strip_prefix(&prefix)?;
            if relative.is_empty() {
                return None;
            }
            let mut relative_entry = entry.clone();
            relative_entry.path = relative.into();
            Some(relative_entry)
        })
        .collect()
}

fn discover_repository_candidates(
    entries: Vec<GithubTreeEntry>,
) -> Result<Vec<GithubRepositoryCandidate>, CandidateError> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    for entry in entries {
        let Some((skill_path, directory_name, repository_root)) =
            conventional_repository_candidate_path(&entry.path)
        else {
            continue;
        };
        let path = validated_relative_string(&entry.path)?;
        // A case-variant conventional document is ambiguous on case-insensitive
        // filesystems. Reject it instead of silently treating it as unrelated.
        if path != candidate_document_path(&skill_path) {
            return Err(CandidateError::UnsafeEntry(entry.path));
        }
        if entry.kind != "blob"
            || !matches!(entry.mode.as_str(), "100644" | "100755")
            || !valid_sha(&entry.sha)
            || entry.size.is_none()
        {
            return Err(CandidateError::UnsafeEntry(entry.path));
        }
        let key = skill_path.to_lowercase();
        if !seen.insert(key) {
            return Err(CandidateError::UnsafeEntry(entry.path));
        }
        if candidates.len() >= MAX_REPOSITORY_SKILLS {
            return Err(CandidateError::TooManyRepositorySkills);
        }
        candidates.push(GithubRepositoryCandidate {
            skill_path,
            directory_name,
            repository_root,
        });
    }
    candidates.sort_by(|left, right| {
        right
            .repository_root
            .cmp(&left.repository_root)
            .then_with(|| left.skill_path.cmp(&right.skill_path))
    });
    if candidates.is_empty() {
        Err(CandidateError::NoRepositorySkills)
    } else {
        Ok(candidates)
    }
}

fn conventional_repository_candidate_path(path: &str) -> Option<(String, String, bool)> {
    let parts = path.split('/').collect::<Vec<_>>();
    match parts.as_slice() {
        [document] if document.eq_ignore_ascii_case("SKILL.md") => {
            Some((String::new(), "SKILL.md".into(), true))
        }
        [root, child, document]
            if root.eq_ignore_ascii_case("skills")
                && document.eq_ignore_ascii_case("SKILL.md")
                && valid_path_atom(child) =>
        {
            Some((format!("skills/{child}"), (*child).into(), false))
        }
        [root, category, child, document]
            if root.eq_ignore_ascii_case("skills")
                && document.eq_ignore_ascii_case("SKILL.md")
                && valid_path_atom(category)
                && valid_path_atom(child) =>
        {
            Some((format!("skills/{category}/{child}"), (*child).into(), false))
        }
        _ => None,
    }
}

fn candidate_document_path(skill_path: &str) -> String {
    join_source_path(skill_path, "SKILL.md")
}

fn is_conventional_repository_skill_path(skill_path: &str) -> bool {
    if skill_path.is_empty() {
        return true;
    }
    let parts = skill_path.split('/').collect::<Vec<_>>();
    matches!(
        parts.as_slice(),
        ["skills", child] if valid_path_atom(child)
    ) || matches!(
        parts.as_slice(),
        ["skills", category, child]
            if valid_path_atom(category) && valid_path_atom(child)
    )
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
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let opened = options
        .open(path)
        .map_err(|_| CandidateError::ChangedSession)?;
    let opened_metadata = opened
        .metadata()
        .map_err(|_| CandidateError::ChangedSession)?;
    if !opened_metadata.is_file() || opened_metadata.len() != file.size as u64 {
        return Err(CandidateError::ChangedSession);
    }
    let bytes = read_limited(opened, MAX_FILE_BYTES, || CandidateError::ChangedSession)?;
    if bytes.len() != file.size || sha256(&bytes) != file.sha256 {
        return Err(CandidateError::ChangedSession);
    }
    Ok(bytes)
}

fn review_from_skill_bytes(manifest: CandidateManifest, skill_bytes: Vec<u8>) -> CandidateReview {
    let audit = match String::from_utf8(skill_bytes) {
        Ok(markdown) => super::audit(&markdown, "", ""),
        Err(error) => non_text_skill_audit(error.as_bytes()),
    };
    CandidateReview {
        compatibility: compatibility_for(&manifest, &audit),
        manifest,
        audit,
        // v0.1 rejects unsupported entries during acquisition rather than omitting them.
        skipped_entries: Vec::new(),
    }
}

fn install_preview(
    workspace: &super::Workspace,
    review: &CandidateReview,
) -> Result<CandidateInstallPreview, super::WorkspaceError> {
    let name = review.audit.document.name.clone();
    let destination = canonical_intended_path(&workspace.roots().personal)?.join(&name);
    let exact_match = if super::valid_name(&name) {
        exact_name_match(workspace, &name, &review.manifest.candidate_hash)?
    } else {
        None
    };
    let conflict = if exact_match.is_none() && super::valid_name(&name) {
        workspace.find_name_conflict(&name)?
    } else {
        None
    };
    let structurally_blocked =
        review.compatibility.status == "incompatible" || review.audit.verdict == "block";
    let classification = if structurally_blocked {
        "blocked"
    } else if exact_match.is_some() {
        "identical"
    } else if conflict
        .as_ref()
        .is_some_and(|item| matches!(item.source.as_str(), "system" | "plugin"))
    {
        "managedConflict"
    } else if conflict.is_some() {
        "userConflict"
    } else {
        "new"
    };
    let can_install = matches!(classification, "new" | "identical");
    Ok(CandidateInstallPreview {
        install_revision: candidate_install_revision(
            &review.manifest,
            &name,
            &destination,
            &review.audit.content_hash,
        ),
        name,
        destination: destination.display().to_string(),
        file_count: review.manifest.files.len(),
        candidate_hash: review.manifest.candidate_hash.clone(),
        compatibility_status: review.compatibility.status.clone(),
        audit_verdict: review.audit.verdict.clone(),
        classification: classification.into(),
        conflict,
        can_install,
    })
}

fn exact_name_match(
    workspace: &super::Workspace,
    name: &str,
    candidate_hash_value: &str,
) -> Result<Option<super::InternalSkill>, super::WorkspaceError> {
    for skill in workspace.cached_name_matches(name)? {
        let Ok(files) = candidate_files_for_directory(&skill.directory) else {
            continue;
        };
        if candidate_hash(&files) == candidate_hash_value {
            return Ok(Some(skill));
        }
    }
    Ok(None)
}

fn identical_install_result(
    snapshot: &VerifiedCandidateSnapshot,
    skill: super::InternalSkill,
) -> CandidateInstallResult {
    let installed_id = skill.summary.id.clone();
    let destination = skill.directory.display().to_string();
    let detail = super::SkillDetail {
        content_hash: super::hash(&skill.markdown),
        summary: skill.summary,
        markdown: skill.markdown,
        document: skill.document,
        editable: skill.source == super::Source::Personal,
    };
    CandidateInstallResult {
        status: "skippedIdentical".into(),
        installed_id,
        skill: Some(detail),
        destination,
        installed_files: 0,
        candidate_hash: snapshot.review.manifest.candidate_hash.clone(),
        catalog_refresh_needed: false,
        restart_recommended: false,
        provenance_recorded: false,
    }
}

fn canonical_intended_path(path: &Path) -> Result<PathBuf, super::WorkspaceError> {
    let mut cursor = path;
    let mut missing = Vec::new();
    while fs::symlink_metadata(cursor).is_err() {
        let name = cursor
            .file_name()
            .ok_or(super::WorkspaceError::UnsafePath)?;
        missing.push(name.to_os_string());
        cursor = cursor.parent().ok_or(super::WorkspaceError::UnsafePath)?;
    }
    let mut canonical = fs::canonicalize(cursor)?;
    for name in missing.into_iter().rev() {
        canonical.push(name);
    }
    Ok(canonical)
}

fn candidate_install_revision(
    manifest: &CandidateManifest,
    name: &str,
    destination: &Path,
    audit_hash: &str,
) -> String {
    let source = match &manifest.source {
        CandidateSource::Local { selected_path } => format!("local\0{selected_path}"),
        CandidateSource::Github {
            repository,
            requested_ref,
            resolved_sha,
            skill_path,
        } => format!("github\0{repository}\0{requested_ref}\0{resolved_sha}\0{skill_path}"),
    };
    sha256(
        format!(
            "{}\0{}\0{}\0{}\0{}",
            manifest.candidate_hash,
            source,
            name,
            destination.display(),
            audit_hash
        )
        .as_bytes(),
    )
}

fn verify_staged_file_set(root: &Path, manifest: &CandidateManifest) -> Result<(), CandidateError> {
    let mut actual = Vec::new();
    collect_staged_file_paths(root, root, 0, &mut actual)?;
    actual.sort();
    let mut expected = manifest
        .files
        .iter()
        .map(|file| file.path.clone())
        .collect::<Vec<_>>();
    expected.sort();
    if actual != expected {
        return Err(CandidateError::ChangedSession);
    }
    Ok(())
}

fn collect_staged_file_paths(
    root: &Path,
    current: &Path,
    depth: usize,
    paths: &mut Vec<String>,
) -> Result<(), CandidateError> {
    if depth > MAX_DEPTH {
        return Err(CandidateError::ChangedSession);
    }
    let mut entries = fs::read_dir(current)
        .map_err(|_| CandidateError::ChangedSession)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| CandidateError::ChangedSession)?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        if is_ignored_skill_metadata_name(&entry.file_name()) {
            continue;
        }
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|_| CandidateError::ChangedSession)?;
        if metadata.file_type().is_symlink() {
            return Err(CandidateError::ChangedSession);
        }
        if metadata.is_dir() {
            collect_staged_file_paths(root, &path, depth + 1, paths)?;
        } else if metadata.is_file() {
            paths.push(relative_string(root, &path).map_err(|_| CandidateError::ChangedSession)?);
        } else {
            return Err(CandidateError::ChangedSession);
        }
    }
    Ok(())
}

fn write_install_file(
    root: &Path,
    manifest: &CandidateFile,
    bytes: &[u8],
) -> Result<(), super::WorkspaceError> {
    let relative =
        validated_relative_string(&manifest.path).map_err(|_| super::WorkspaceError::UnsafePath)?;
    let destination = root.join(&relative);
    if !destination.starts_with(root) {
        return Err(super::WorkspaceError::UnsafePath);
    }
    let parent = destination
        .parent()
        .ok_or(super::WorkspaceError::UnsafePath)?;
    fs::create_dir_all(parent)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)?;
    file.write_all(bytes)?;
    file.flush()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = if manifest.executable { 0o755 } else { 0o644 };
        file.set_permissions(fs::Permissions::from_mode(mode))?;
    }
    file.sync_all()?;
    Ok(())
}

pub(super) fn sync_directory(path: &Path) -> Result<(), super::WorkspaceError> {
    File::open(path)?.sync_all()?;
    Ok(())
}

fn sync_install_directories(
    root: &Path,
    files: &[VerifiedCandidateFile],
) -> Result<(), super::WorkspaceError> {
    let mut directories = files
        .iter()
        .filter_map(|file| Path::new(&file.manifest.path).parent())
        .map(|parent| root.join(parent))
        .collect::<Vec<_>>();
    directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    directories.dedup();
    for directory in directories {
        sync_directory(&directory)?;
    }
    sync_directory(root)
}

#[cfg(target_os = "macos")]
pub(super) fn rename_directory_no_replace(
    source: &Path,
    destination: &Path,
) -> std::io::Result<()> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};

    let source = CString::new(source.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    let destination = CString::new(destination.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    // RENAME_EXCL makes the same-filesystem directory commit atomic and non-overwriting.
    let result =
        unsafe { libc::renamex_np(source.as_ptr(), destination.as_ptr(), libc::RENAME_EXCL) };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(target_os = "linux")]
pub(super) fn rename_directory_no_replace(
    source: &Path,
    destination: &Path,
) -> std::io::Result<()> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};

    let source = CString::new(source.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    let destination = CString::new(destination.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            destination.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub(super) fn rename_directory_no_replace(
    _source: &Path,
    _destination: &Path,
) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "atomic no-replace installation is not supported on this platform",
    ))
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
        label: "暂存文件清单".into(),
        status: "pass".into(),
        detail: format!(
            "已记录 {} 个文件的大小、权限和 SHA-256；安装前会重新核对全部文件。",
            manifest.files.len()
        ),
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
        blob_sha: &str,
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
        _commit_sha: &str,
        path: &str,
        blob_sha: &str,
        expected_size: usize,
    ) -> Result<Vec<u8>, CandidateError> {
        let url = github_url(
            "https://api.github.com",
            &["repos", owner, repository, "git", "blobs", blob_sha],
        )?;
        let response: GithubBlobResponse = self.api_json(url)?;
        decode_github_blob(response, blob_sha, expected_size, path)
    }
}

#[derive(Deserialize)]
struct GithubBlobResponse {
    sha: String,
    size: usize,
    encoding: String,
    content: String,
}

fn decode_github_blob(
    response: GithubBlobResponse,
    expected_sha: &str,
    expected_size: usize,
    path: &str,
) -> Result<Vec<u8>, CandidateError> {
    if response.sha != expected_sha
        || response.size != expected_size
        || response.encoding != "base64"
    {
        return Err(CandidateError::InconsistentGithubSource);
    }
    let encoded = response
        .content
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect::<Vec<_>>();
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| CandidateError::InconsistentGithubSource)?;
    if bytes.len() != expected_size {
        return Err(CandidateError::InconsistentGithubSource);
    }
    if bytes.len() > MAX_FILE_BYTES {
        return Err(CandidateError::FileTooLarge(path.into()));
    }
    Ok(bytes)
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
        default_branch_calls: Mutex<usize>,
        resolved_references: Mutex<Vec<String>>,
        tree_calls: Mutex<Vec<(String, bool)>>,
        download_delay: Duration,
        active_downloads: AtomicUsize,
        max_concurrent_downloads: AtomicUsize,
    }

    impl GithubTransport for FakeGithub {
        fn default_branch(
            &self,
            _owner: &str,
            _repository: &str,
        ) -> Result<String, CandidateError> {
            *self.default_branch_calls.lock().unwrap() += 1;
            Ok(self.default_branch.clone())
        }

        fn resolve_commit(
            &self,
            _owner: &str,
            _repository: &str,
            reference: &str,
        ) -> Result<ResolvedCommit, CandidateError> {
            self.resolved_references
                .lock()
                .unwrap()
                .push(reference.into());
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
            self.tree_calls
                .lock()
                .unwrap()
                .push((tree_sha.into(), recursive));
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
            _blob_sha: &str,
            _expected_size: usize,
        ) -> Result<Vec<u8>, CandidateError> {
            let active = self.active_downloads.fetch_add(1, Ordering::AcqRel) + 1;
            self.max_concurrent_downloads
                .fetch_max(active, Ordering::AcqRel);
            self.downloads
                .lock()
                .unwrap()
                .push((commit_sha.into(), path.into()));
            if !self.download_delay.is_zero() {
                std::thread::sleep(self.download_delay);
            }
            let result = self
                .blobs
                .lock()
                .unwrap()
                .get(path)
                .cloned()
                .ok_or_else(|| CandidateError::Github("missing fake blob".into()));
            self.active_downloads.fetch_sub(1, Ordering::AcqRel);
            result
        }
    }

    fn stager(directory: &TempDir, github: Arc<dyn GithubTransport>) -> CandidateStager {
        CandidateStager::with_github(directory.path().join("staging"), github).unwrap()
    }

    fn installable_candidate(
        directory: &TempDir,
        name: &str,
        executable: bool,
    ) -> (CandidateStager, super::super::Workspace, CandidateManifest) {
        let source = directory.path().join(format!("source-{name}"));
        fs::create_dir_all(source.join("scripts")).unwrap();
        fs::write(
            source.join("SKILL.md"),
            format!(
                "---\nname: {name}\ndescription: Use when the user asks to inspect a staged candidate.\n---\n\n# Candidate\n\nReview the supplied evidence and report exact findings before taking any requested action.\n"
            ),
        )
        .unwrap();
        fs::write(source.join("scripts/helper.sh"), "echo staged\n").unwrap();
        #[cfg(unix)]
        if executable {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                source.join("scripts/helper.sh"),
                fs::Permissions::from_mode(0o755),
            )
            .unwrap();
        }
        let stager = stager(directory, Arc::new(FakeGithub::default()));
        let manifest = stager.stage_local(&source).unwrap();
        let workspace = super::super::Workspace::new(directory.path().join("codex"));
        (stager, workspace, manifest)
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
            sha: if kind == "blob" && !valid_sha(sha) {
                sha256(sha.as_bytes())[..40].into()
            } else {
                sha.into()
            },
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

    #[test]
    fn install_preview_does_not_write_and_install_preserves_files_and_modes() {
        let directory = TempDir::new().unwrap();
        let (stager, workspace, manifest) = installable_candidate(&directory, "demo-install", true);
        let personal_root = directory.path().join("codex/skills");
        let staged_root = stager.store.root.join(&manifest.session_id);
        let staged_before = fs::read(staged_root.join("SKILL.md")).unwrap();

        let preview = stager
            .preview_install(&workspace, &manifest.session_id, &manifest.candidate_hash)
            .unwrap();
        assert!(preview.can_install);
        assert!(!personal_root.exists());

        let result = stager
            .install(
                &workspace,
                &manifest.session_id,
                &manifest.candidate_hash,
                &preview.install_revision,
            )
            .unwrap();
        let installed_root = personal_root.join("demo-install");
        assert_eq!(result.installed_files, 2);
        assert_eq!(
            fs::read(installed_root.join("SKILL.md")).unwrap(),
            staged_before
        );
        assert_eq!(
            fs::read_to_string(installed_root.join("scripts/helper.sh")).unwrap(),
            "echo staged\n"
        );
        assert_eq!(
            fs::read(staged_root.join("SKILL.md")).unwrap(),
            staged_before
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_ne!(
                fs::metadata(installed_root.join("scripts/helper.sh"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o111,
                0
            );
        }
        assert!(fs::read_dir(&personal_root)
            .unwrap()
            .filter_map(Result::ok)
            .all(|entry| !entry
                .file_name()
                .to_string_lossy()
                .starts_with(".candidate-install-")));
    }

    #[test]
    fn install_rejects_stale_revisions_and_staged_changes() {
        let directory = TempDir::new().unwrap();
        let (stager, workspace, manifest) = installable_candidate(&directory, "demo-stale", false);
        let preview = stager
            .preview_install(&workspace, &manifest.session_id, &manifest.candidate_hash)
            .unwrap();
        assert!(matches!(
            stager.install(
                &workspace,
                &manifest.session_id,
                &manifest.candidate_hash,
                "wrong-revision"
            ),
            Err(CandidateInstallError::PreviewMismatch)
        ));
        assert!(!directory.path().join("codex/skills/demo-stale").exists());

        fs::write(
            stager
                .store
                .root
                .join(&manifest.session_id)
                .join("extra.txt"),
            "not in the manifest",
        )
        .unwrap();
        assert!(matches!(
            stager.install(
                &workspace,
                &manifest.session_id,
                &manifest.candidate_hash,
                &preview.install_revision
            ),
            Err(CandidateInstallError::Candidate(
                CandidateError::ChangedSession
            ))
        ));
        assert!(!directory.path().join("codex/skills/demo-stale").exists());
    }

    #[test]
    fn install_classifies_conflicts_and_rechecks_them_without_overwriting() {
        let directory = TempDir::new().unwrap();
        let (stager, workspace, manifest) =
            installable_candidate(&directory, "demo-conflict", false);
        let preview = stager
            .preview_install(&workspace, &manifest.session_id, &manifest.candidate_hash)
            .unwrap();
        let conflict = directory.path().join("codex/skills/demo-conflict");
        fs::create_dir_all(&conflict).unwrap();
        fs::write(conflict.join("keep.txt"), "existing content").unwrap();

        let conflict_preview = stager
            .preview_install(&workspace, &manifest.session_id, &manifest.candidate_hash)
            .unwrap();
        assert_eq!(conflict_preview.classification, "userConflict");
        assert!(!conflict_preview.can_install);

        assert!(matches!(
            stager.install(
                &workspace,
                &manifest.session_id,
                &manifest.candidate_hash,
                &preview.install_revision
            ),
            Err(CandidateInstallError::Workspace(
                super::super::WorkspaceError::NameConflict { .. }
            ))
        ));
        assert_eq!(
            fs::read_to_string(conflict.join("keep.txt")).unwrap(),
            "existing content"
        );
        assert!(!conflict.join("SKILL.md").exists());
    }

    #[test]
    fn install_rejects_blocked_documents_and_skips_identical_retries() {
        let directory = TempDir::new().unwrap();
        let blocked_source = directory.path().join("blocked-source");
        fs::create_dir_all(&blocked_source).unwrap();
        fs::write(blocked_source.join("SKILL.md"), [0xff, 0xfe]).unwrap();
        let stager = stager(&directory, Arc::new(FakeGithub::default()));
        let blocked = stager.stage_local(&blocked_source).unwrap();
        let workspace = super::super::Workspace::new(directory.path().join("codex"));
        let blocked_preview = stager
            .preview_install(&workspace, &blocked.session_id, &blocked.candidate_hash)
            .unwrap();
        assert!(!blocked_preview.can_install);
        assert!(matches!(
            stager.install(
                &workspace,
                &blocked.session_id,
                &blocked.candidate_hash,
                &blocked_preview.install_revision
            ),
            Err(CandidateInstallError::Blocked)
        ));

        let (stager, workspace, manifest) = installable_candidate(&directory, "demo-once", false);
        let preview = stager
            .preview_install(&workspace, &manifest.session_id, &manifest.candidate_hash)
            .unwrap();
        assert_eq!(preview.classification, "new");
        stager
            .install(
                &workspace,
                &manifest.session_id,
                &manifest.candidate_hash,
                &preview.install_revision,
            )
            .unwrap();
        let retry_preview = stager
            .preview_install(&workspace, &manifest.session_id, &manifest.candidate_hash)
            .unwrap();
        assert_eq!(retry_preview.classification, "identical");
        assert!(retry_preview.can_install);
        assert!(retry_preview.conflict.is_none());
        let retry = stager
            .install(
                &workspace,
                &manifest.session_id,
                &manifest.candidate_hash,
                &retry_preview.install_revision,
            )
            .unwrap();
        assert_eq!(retry.status, "skippedIdentical");
        assert_eq!(retry.installed_files, 0);
        assert_eq!(workspace.list_skills().unwrap().counts.personal, 1);
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
    fn candidate_source_uses_the_desktop_bridge_field_names() {
        let github = serde_json::to_value(CandidateSource::Github {
            repository: "owner/repo".into(),
            requested_ref: "main".into(),
            resolved_sha: "a".repeat(40),
            skill_path: "skills/demo".into(),
        })
        .unwrap();
        assert_eq!(github["kind"], "github");
        assert_eq!(github["requestedRef"], "main");
        assert_eq!(github["resolvedSha"], "a".repeat(40));
        assert_eq!(github["skillPath"], "skills/demo");
        assert!(github.get("resolved_sha").is_none());

        let local = serde_json::to_value(CandidateSource::Local {
            selected_path: "/tmp/demo".into(),
        })
        .unwrap();
        assert_eq!(local["selectedPath"], "/tmp/demo");
    }

    #[test]
    fn file_sync_data_allows_remote_adds_replacements_and_confirmed_absence() {
        let directory = TempDir::new().unwrap();
        let source = directory.path().join("source");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("SKILL.md"), "---\nname: demo\n---\n").unwrap();
        fs::write(source.join("new.txt"), "remote\n").unwrap();
        let stager = stager(&directory, Arc::new(FakeGithub::default()));
        let manifest = stager.stage_local(&source).unwrap();
        let added = stager
            .file_sync_data(
                &manifest.session_id,
                &manifest.candidate_hash,
                "new.txt",
                CandidateFileSyncAction::Add,
            )
            .unwrap()
            .unwrap();
        assert_eq!(added.0, b"remote\n");
        let replacement = stager
            .file_sync_data(
                &manifest.session_id,
                &manifest.candidate_hash,
                "SKILL.md",
                CandidateFileSyncAction::Replace,
            )
            .unwrap()
            .unwrap();
        assert_eq!(replacement.0, b"---\nname: demo\n---\n");
        assert!(matches!(
            stager.file_sync_data(
                &manifest.session_id,
                &manifest.candidate_hash,
                "new.txt",
                CandidateFileSyncAction::Delete,
            ),
            Err(CandidateError::InvalidFileSync)
        ));
        assert!(stager
            .directory_matches(&manifest.session_id, &manifest.candidate_hash, &source,)
            .unwrap());
        assert!(stager
            .file_sync_data(
                &manifest.session_id,
                &manifest.candidate_hash,
                "missing.txt",
                CandidateFileSyncAction::Delete,
            )
            .unwrap()
            .is_none());
        assert!(matches!(
            stager.file_sync_data(
                &manifest.session_id,
                &manifest.candidate_hash,
                "SKILL.md",
                CandidateFileSyncAction::Delete,
            ),
            Err(CandidateError::InvalidFileSync)
        ));
    }

    #[test]
    fn file_sync_rejects_tampering_in_a_nonselected_staged_file() {
        let directory = TempDir::new().unwrap();
        let source = directory.path().join("source");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("SKILL.md"), "---\nname: demo\n---\n").unwrap();
        fs::write(source.join("selected.txt"), "remote\n").unwrap();
        fs::write(source.join("other.txt"), "original\n").unwrap();
        let stager = stager(&directory, Arc::new(FakeGithub::default()));
        let manifest = stager.stage_local(&source).unwrap();
        let staged_other = stager
            .session_directory(&manifest.session_id)
            .unwrap()
            .join("other.txt");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&staged_other, fs::Permissions::from_mode(0o600)).unwrap();
        }
        #[cfg(not(unix))]
        {
            let mut permissions = fs::metadata(&staged_other).unwrap().permissions();
            permissions.set_readonly(false);
            fs::set_permissions(&staged_other, permissions).unwrap();
        }
        fs::write(staged_other, "tampered\n").unwrap();

        assert!(matches!(
            stager.file_sync_data(
                &manifest.session_id,
                &manifest.candidate_hash,
                "selected.txt",
                CandidateFileSyncAction::Replace,
            ),
            Err(CandidateError::ChangedSession)
        ));
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
    fn lists_conventional_repository_skills_without_downloading_blobs() {
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
            ("root".into(), true),
            GithubTree {
                entries: vec![
                    tree_entry("README.md", "100644", "blob", "readme", Some(1)),
                    tree_entry("SKILL.md", "100644", "blob", "root-skill", Some(5)),
                    tree_entry(
                        "skills/engineering/code-review/SKILL.md",
                        "100644",
                        "blob",
                        "categorized-skill",
                        Some(9),
                    ),
                    tree_entry(
                        "skills/research/SKILL.md",
                        "100644",
                        "blob",
                        "research-skill",
                        Some(8),
                    ),
                    tree_entry(
                        "skills/writing/SKILL.md",
                        "100644",
                        "blob",
                        "writing-skill",
                        Some(7),
                    ),
                    tree_entry(
                        "skills/too/deep/demo/SKILL.md",
                        "100644",
                        "blob",
                        "too-deep-skill",
                        Some(7),
                    ),
                    tree_entry(
                        "examples/demo/SKILL.md",
                        "100644",
                        "blob",
                        "ignored-skill",
                        Some(7),
                    ),
                    tree_entry(
                        "unrelated/a/b/c/d/e/f/g/h/i/j/README.md",
                        "100644",
                        "blob",
                        "deep-unrelated",
                        Some(7),
                    ),
                ],
                truncated: false,
            },
        );
        let stager = stager(&directory, github.clone());

        let listing = stager
            .list_github_repository("https://github.com/owner/repo")
            .unwrap();

        assert_eq!(listing.repository, "owner/repo");
        assert_eq!(listing.requested_ref, "main");
        assert_eq!(listing.resolved_sha, "a".repeat(40));
        assert_eq!(
            listing
                .candidates
                .iter()
                .map(|candidate| candidate.skill_path.as_str())
                .collect::<Vec<_>>(),
            vec![
                "",
                "skills/engineering/code-review",
                "skills/research",
                "skills/writing"
            ]
        );
        assert!(listing.candidates[0].repository_root);
        assert!(github.downloads.lock().unwrap().is_empty());
        assert_eq!(*github.default_branch_calls.lock().unwrap(), 1);
        assert_eq!(
            github.resolved_references.lock().unwrap().as_slice(),
            &["main"]
        );
        assert_eq!(
            github.tree_calls.lock().unwrap().as_slice(),
            &[("root".into(), true)]
        );
    }

    #[test]
    fn repository_listing_stages_only_the_selected_path_at_its_listing_sha() {
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
            ("root".into(), true),
            GithubTree {
                entries: vec![
                    tree_entry(
                        "skills/engineering/research/SKILL.md",
                        "100644",
                        "blob",
                        "skill",
                        Some(5),
                    ),
                    tree_entry(
                        "skills/engineering/other/SKILL.md",
                        "100644",
                        "blob",
                        "other",
                        Some(5),
                    ),
                ],
                truncated: false,
            },
        );
        github.blobs.lock().unwrap().insert(
            "skills/engineering/research/SKILL.md".into(),
            b"skill".to_vec(),
        );
        let stager = stager(&directory, github.clone());
        let listing = stager
            .list_github_repository("https://github.com/owner/repo")
            .unwrap();

        let manifest = stager
            .stage_github_repository_candidate(
                "https://github.com/owner/repo",
                &listing.requested_ref,
                &listing.resolved_sha,
                "skills/engineering/research",
            )
            .unwrap();

        let CandidateSource::Github {
            requested_ref,
            resolved_sha,
            skill_path,
            ..
        } = manifest.source
        else {
            panic!("expected GitHub provenance");
        };
        assert_eq!(requested_ref, "main");
        assert_eq!(resolved_sha, "a".repeat(40));
        assert_eq!(skill_path, "skills/engineering/research");
        assert_eq!(
            github.downloads.lock().unwrap().as_slice(),
            &[(
                "a".repeat(40),
                "skills/engineering/research/SKILL.md".into()
            )]
        );
        assert_eq!(
            github.resolved_references.lock().unwrap().as_slice(),
            &["main"]
        );
        assert_eq!(
            github.tree_calls.lock().unwrap().as_slice(),
            &[("root".into(), true)]
        );

        github.blobs.lock().unwrap().insert(
            "skills/engineering/other/SKILL.md".into(),
            b"other".to_vec(),
        );
        stager
            .stage_github_repository_candidate(
                "https://github.com/owner/repo",
                &listing.requested_ref,
                &listing.resolved_sha,
                "skills/engineering/other",
            )
            .unwrap();
        assert_eq!(
            github.resolved_references.lock().unwrap().as_slice(),
            &["main"]
        );
        assert_eq!(
            github.tree_calls.lock().unwrap().as_slice(),
            &[("root".into(), true)]
        );
        assert_eq!(github.downloads.lock().unwrap().len(), 2);
    }

    #[test]
    fn github_blob_downloads_are_bounded_and_manifest_order_is_deterministic() {
        let directory = TempDir::new().unwrap();
        let github = Arc::new(FakeGithub {
            commit: Some(ResolvedCommit {
                sha: "a".repeat(40),
                root_tree_sha: "root".into(),
            }),
            download_delay: Duration::from_millis(20),
            ..FakeGithub::default()
        });
        let mut entries = Vec::new();
        for index in (0..10).rev() {
            let path = if index == 0 {
                "SKILL.md".into()
            } else {
                format!("references/{index}.md")
            };
            entries.push(tree_entry(
                &path,
                "100644",
                "blob",
                &format!("blob-{index}"),
                Some(1),
            ));
            github.blobs.lock().unwrap().insert(path, vec![b'x']);
        }
        github.trees.lock().unwrap().insert(
            ("root".into(), true),
            GithubTree {
                entries,
                truncated: false,
            },
        );
        let stager = stager(&directory, github.clone());

        let manifest = stager
            .stage_github("https://github.com/owner/repo/tree/main")
            .unwrap();

        let paths = manifest
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>();
        let mut sorted = paths.clone();
        sorted.sort();
        assert_eq!(paths, sorted);
        assert_eq!(github.downloads.lock().unwrap().len(), 10);
        let max_concurrent = github.max_concurrent_downloads.load(Ordering::Acquire);
        assert!(max_concurrent > 1);
        assert!(max_concurrent <= MAX_CONCURRENT_GITHUB_BLOBS);
    }

    #[test]
    fn concurrent_github_download_failure_leaves_no_staging_session() {
        let directory = TempDir::new().unwrap();
        let github = Arc::new(FakeGithub {
            commit: Some(ResolvedCommit {
                sha: "a".repeat(40),
                root_tree_sha: "root".into(),
            }),
            download_delay: Duration::from_millis(10),
            ..FakeGithub::default()
        });
        github.trees.lock().unwrap().insert(
            ("root".into(), true),
            GithubTree {
                entries: vec![
                    tree_entry("SKILL.md", "100644", "blob", "skill", Some(5)),
                    tree_entry("references/good.md", "100644", "blob", "good", Some(4)),
                    tree_entry(
                        "references/missing.md",
                        "100644",
                        "blob",
                        "missing",
                        Some(7),
                    ),
                ],
                truncated: false,
            },
        );
        github
            .blobs
            .lock()
            .unwrap()
            .insert("SKILL.md".into(), b"skill".to_vec());
        github
            .blobs
            .lock()
            .unwrap()
            .insert("references/good.md".into(), b"good".to_vec());
        let stager = stager(&directory, github);

        assert!(matches!(
            stager.stage_github("https://github.com/owner/repo/tree/main"),
            Err(CandidateError::Github(_))
        ));
        assert!(stager.store.sessions.lock().unwrap().is_empty());
        assert_eq!(fs::read_dir(&stager.store.root).unwrap().count(), 0);
    }

    #[test]
    fn repository_root_staging_does_not_absorb_direct_nested_skills() {
        let directory = TempDir::new().unwrap();
        let github = Arc::new(FakeGithub {
            commit: Some(ResolvedCommit {
                sha: "a".repeat(40),
                root_tree_sha: "root".into(),
            }),
            ..FakeGithub::default()
        });
        github.trees.lock().unwrap().insert(
            ("root".into(), true),
            GithubTree {
                entries: vec![
                    tree_entry("SKILL.md", "100644", "blob", "root", Some(4)),
                    tree_entry(
                        "skills/research/SKILL.md",
                        "100644",
                        "blob",
                        "nested-skill",
                        Some(6),
                    ),
                    tree_entry(
                        "skills/research/run.sh",
                        "120000",
                        "blob",
                        "nested-link",
                        Some(3),
                    ),
                ],
                truncated: false,
            },
        );
        github
            .blobs
            .lock()
            .unwrap()
            .insert("SKILL.md".into(), b"root".to_vec());
        let stager = stager(&directory, github.clone());

        let manifest = stager
            .stage_github_repository_candidate(
                "https://github.com/owner/repo",
                "main",
                &"a".repeat(40),
                "",
            )
            .unwrap();

        assert_eq!(
            manifest
                .files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>(),
            vec!["SKILL.md"]
        );
        assert_eq!(
            github.downloads.lock().unwrap().as_slice(),
            &[("a".repeat(40), "SKILL.md".into())]
        );
    }

    #[test]
    fn repository_listing_rejects_case_collisions_and_truncation_before_download() {
        assert!(matches!(
            discover_repository_candidates(vec![
                tree_entry("skills/demo/SKILL.md", "100644", "blob", "one", Some(1)),
                tree_entry("skills/DEMO/SKILL.md", "100644", "blob", "two", Some(1)),
            ]),
            Err(CandidateError::UnsafeEntry(_))
        ));

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
            ("root".into(), true),
            GithubTree {
                entries: vec![],
                truncated: true,
            },
        );
        let stager = stager(&directory, github.clone());
        assert!(matches!(
            stager.list_github_repository("https://github.com/owner/repo"),
            Err(CandidateError::TruncatedTree)
        ));
        assert!(github.downloads.lock().unwrap().is_empty());
    }

    #[test]
    fn repository_listing_rejects_empty_and_oversized_results() {
        assert!(matches!(
            discover_repository_candidates(Vec::new()),
            Err(CandidateError::NoRepositorySkills)
        ));
        let entries = (0..=MAX_REPOSITORY_SKILLS)
            .map(|index| {
                tree_entry(
                    &format!("skills/skill-{index}/SKILL.md"),
                    "100644",
                    "blob",
                    &format!("skill-{index}"),
                    Some(1),
                )
            })
            .collect();
        assert!(matches!(
            discover_repository_candidates(entries),
            Err(CandidateError::TooManyRepositorySkills)
        ));
    }

    #[test]
    fn explicit_ref_repository_listing_skips_default_branch_lookup() {
        let directory = TempDir::new().unwrap();
        let github = Arc::new(FakeGithub {
            commit: Some(ResolvedCommit {
                sha: "a".repeat(40),
                root_tree_sha: "root".into(),
            }),
            ..FakeGithub::default()
        });
        github.trees.lock().unwrap().insert(
            ("root".into(), true),
            GithubTree {
                entries: vec![tree_entry(
                    "skills/demo/SKILL.md",
                    "100644",
                    "blob",
                    "skill",
                    Some(1),
                )],
                truncated: false,
            },
        );
        let stager = stager(&directory, github.clone());

        let listing = stager
            .list_github_repository("https://github.com/owner/repo/tree/main")
            .unwrap();

        assert_eq!(listing.requested_ref, "main");
        assert_eq!(*github.default_branch_calls.lock().unwrap(), 0);
        assert_eq!(
            github.resolved_references.lock().unwrap().as_slice(),
            &["main"]
        );
        assert!(github.downloads.lock().unwrap().is_empty());
    }

    #[test]
    fn local_candidate_fingerprint_matches_staged_candidate_semantics() {
        let directory = TempDir::new().unwrap();
        let source = directory.path().join("source");
        fs::create_dir_all(source.join("scripts")).unwrap();
        fs::write(source.join("SKILL.md"), "---\nname: demo\n---\n").unwrap();
        fs::write(source.join("scripts/run.sh"), "echo hello\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                source.join("scripts/run.sh"),
                fs::Permissions::from_mode(0o755),
            )
            .unwrap();
        }
        let stager = stager(&directory, Arc::new(FakeGithub::default()));
        let manifest = stager.stage_local(&source).unwrap();
        assert_eq!(
            candidate_hash(&candidate_files_for_directory(&source).unwrap()),
            manifest.candidate_hash
        );
    }

    #[test]
    fn local_candidate_fingerprint_ignores_finder_metadata() {
        let directory = TempDir::new().unwrap();
        let source = directory.path().join("source");
        fs::create_dir_all(source.join("scripts")).unwrap();
        fs::write(source.join("SKILL.md"), "---\nname: demo\n---\n").unwrap();
        fs::write(source.join("scripts/run.sh"), "echo hello\n").unwrap();
        let before = candidate_files_for_directory(&source).unwrap();
        fs::write(source.join(".DS_Store"), b"root finder metadata").unwrap();
        fs::write(source.join("scripts/.DS_Store"), b"nested finder metadata").unwrap();
        let after = candidate_files_for_directory(&source).unwrap();
        assert_eq!(candidate_hash(&after), candidate_hash(&before));
        assert_eq!(after.len(), before.len());
        assert!(after
            .iter()
            .zip(&before)
            .all(|(after, before)| after.path == before.path
                && after.size == before.size
                && after.sha256 == before.sha256
                && after.executable == before.executable));
        assert!(after.iter().all(|file| !file.path.contains(".DS_Store")));
    }

    #[test]
    fn github_candidate_tree_ignores_finder_metadata() {
        let files = github_inputs(vec![
            tree_entry("SKILL.md", "100644", "blob", "skill", Some(5)),
            tree_entry(".DS_Store", "100644", "blob", "finder", Some(9)),
            tree_entry(
                "references/.DS_Store",
                "100644",
                "blob",
                "nested-finder",
                Some(13),
            ),
        ])
        .unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].relative, "SKILL.md");
        assert!(matches!(
            github_inputs(vec![tree_entry(
                "../.DS_Store",
                "100644",
                "blob",
                "invalid-finder",
                Some(13),
            )]),
            Err(CandidateError::UnsafeEntry(_))
        ));
    }

    #[test]
    fn update_classification_separates_remote_changes_from_local_edits() {
        let installed_hash = "installed";
        let installed_sha = "a".repeat(40);
        let remote_sha = "b".repeat(40);
        assert_eq!(
            classify_github_update(
                installed_hash,
                "remote",
                Some(installed_hash),
                Some(&installed_sha),
                &remote_sha,
            ),
            "remoteChanged"
        );
        assert_eq!(
            classify_github_update(
                "locally-edited",
                installed_hash,
                Some(installed_hash),
                Some(&installed_sha),
                &installed_sha,
            ),
            "localChanged"
        );
        assert_eq!(
            classify_github_update("locally-edited", "remote", None, None, &remote_sha,),
            "differentUnknown"
        );
        assert_eq!(
            classify_github_update("same", "same", None, None, &remote_sha),
            "identical"
        );
    }

    #[test]
    fn checks_a_confirmed_github_source_without_mutating_the_local_skill() {
        let directory = TempDir::new().unwrap();
        let codex_home = directory.path().join("codex");
        let local = codex_home.join("skills/demo");
        fs::create_dir_all(&local).unwrap();
        let local_markdown = "---\nname: demo\ndescription: Use when checking updates.\n---\n";
        fs::write(local.join("SKILL.md"), local_markdown).unwrap();
        fs::write(local.join(".DS_Store"), b"finder metadata").unwrap();
        let workspace = super::super::Workspace::new(codex_home);
        let id = workspace
            .list_skills()
            .unwrap()
            .skills
            .into_iter()
            .find(|skill| skill.source == "personal")
            .unwrap()
            .id;
        let remote_markdown = local_markdown;
        let github = Arc::new(FakeGithub {
            default_branch: "main".into(),
            commit: Some(ResolvedCommit {
                sha: "b".repeat(40),
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
                entries: vec![tree_entry(
                    "SKILL.md",
                    "100644",
                    "blob",
                    "remote-skill",
                    Some(remote_markdown.len() as u64),
                )],
                truncated: false,
            },
        );
        github.blobs.lock().unwrap().insert(
            "skills/demo/SKILL.md".into(),
            remote_markdown.as_bytes().to_vec(),
        );
        let stager = stager(&directory, github);
        let result = stager
            .check_github_update(
                &workspace,
                &id,
                &super::super::AcquisitionProvenance {
                    kind: "github".into(),
                    confidence: "confirmed".into(),
                    repository: Some("owner/repo".into()),
                    requested_ref: None,
                    resolved_sha: None,
                    skill_path: Some("skills/demo".into()),
                    selected_path: None,
                    candidate_hash: None,
                    recorded_at: Some("2026-08-13T00:00:00Z".into()),
                },
            )
            .unwrap();
        assert_eq!(result.status, "identical");
        assert!(result
            .local_files
            .iter()
            .all(|file| !file.path.contains(".DS_Store")));
        assert_eq!(result.remote_sha, "b".repeat(40));
        assert_eq!(
            fs::read_to_string(local.join("SKILL.md")).unwrap(),
            local_markdown
        );
        assert!(stager
            .review(&result.manifest.session_id, &result.manifest.candidate_hash)
            .is_ok());
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

        let encoded_ref =
            parse_github_url("https://github.com/owner/repo/tree/feature%2Fintake").unwrap();
        assert_eq!(encoded_ref.requested_ref.as_deref(), Some("feature/intake"));
        assert!(encoded_ref.skill_path.is_empty());
    }

    #[test]
    fn github_blob_api_content_is_hash_and_size_bound() {
        let sha = "a".repeat(40);
        let decoded = decode_github_blob(
            GithubBlobResponse {
                sha: sha.clone(),
                size: 5,
                encoding: "base64".into(),
                content: "aGVs\nbG8=\n".into(),
            },
            &sha,
            5,
            "SKILL.md",
        )
        .unwrap();
        assert_eq!(decoded, b"hello");

        for response in [
            GithubBlobResponse {
                sha: "b".repeat(40),
                size: 5,
                encoding: "base64".into(),
                content: "aGVsbG8=".into(),
            },
            GithubBlobResponse {
                sha: sha.clone(),
                size: 4,
                encoding: "base64".into(),
                content: "aGVsbG8=".into(),
            },
            GithubBlobResponse {
                sha: sha.clone(),
                size: 5,
                encoding: "utf-8".into(),
                content: "hello".into(),
            },
            GithubBlobResponse {
                sha: sha.clone(),
                size: 5,
                encoding: "base64".into(),
                content: "not base64".into(),
            },
        ] {
            assert!(matches!(
                decode_github_blob(response, &sha, 5, "SKILL.md"),
                Err(CandidateError::InconsistentGithubSource)
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
