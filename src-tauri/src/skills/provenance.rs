use super::{CandidateSource, Catalog, SkillDetail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tempfile::NamedTempFile;
use thiserror::Error;

const MAX_RECORDS: usize = 10_000;
const MAX_ID_BYTES: usize = 4096;
const MAX_VALUE_BYTES: usize = 4096;

#[derive(Debug, Error)]
pub enum ProvenanceError {
    #[error("The Skill provenance store is invalid or exceeds supported limits.")]
    InvalidStore,
    #[error("Unable to access Skill provenance: {0}")]
    Io(#[from] std::io::Error),
}

impl ProvenanceError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidStore => "INVALID_PROVENANCE_STORE",
            Self::Io(_) => "PROVENANCE_IO_ERROR",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcquisitionProvenance {
    pub kind: String,
    pub confidence: String,
    pub repository: Option<String>,
    pub requested_ref: Option<String>,
    pub resolved_sha: Option<String>,
    pub skill_path: Option<String>,
    pub selected_path: Option<String>,
    pub candidate_hash: Option<String>,
    pub recorded_at: Option<String>,
}

#[cfg(test)]
#[derive(Clone, Debug)]
struct GithubProvenanceConfirmation {
    pub directory_name: String,
    pub repository: String,
    pub skill_path: Option<String>,
}

#[cfg(test)]
#[derive(Clone, Debug, Default)]
struct ProvenanceBackfillResult {
    pub confirmed: usize,
    pub unchanged: usize,
    pub preserved_recorded: usize,
}

impl AcquisitionProvenance {
    pub fn unknown() -> Self {
        Self {
            kind: "unknown".into(),
            confidence: "unknown".into(),
            repository: None,
            requested_ref: None,
            resolved_sha: None,
            skill_path: None,
            selected_path: None,
            candidate_hash: None,
            recorded_at: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredRecord {
    acquisition: AcquisitionProvenance,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Store {
    version: u8,
    records: BTreeMap<String, StoredRecord>,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            version: 1,
            records: BTreeMap::new(),
        }
    }
}

#[derive(Clone)]
pub struct ProvenanceManager {
    path: PathBuf,
    mutation: Arc<Mutex<()>>,
}

impl ProvenanceManager {
    pub fn new(settings_directory: PathBuf) -> Self {
        Self {
            path: settings_directory.join("skill-provenance.json"),
            mutation: Arc::new(Mutex::new(())),
        }
    }

    pub fn attach_catalog(&self, catalog: &mut Catalog) -> Result<(), ProvenanceError> {
        let store = self.read()?;
        for skill in &mut catalog.skills {
            skill.acquisition = acquisition_for(&store, &skill.id);
        }
        Ok(())
    }

    pub fn attach_detail(&self, detail: &mut SkillDetail) -> Result<(), ProvenanceError> {
        let store = self.read()?;
        detail.summary.acquisition = acquisition_for(&store, &detail.summary.id);
        Ok(())
    }

    pub fn acquisition(&self, skill_id: &str) -> Result<AcquisitionProvenance, ProvenanceError> {
        validate_id(skill_id)?;
        Ok(acquisition_for(&self.read()?, skill_id))
    }

    pub fn record_candidate(
        &self,
        skill_id: &str,
        candidate_hash: &str,
        source: CandidateSource,
    ) -> Result<AcquisitionProvenance, ProvenanceError> {
        let acquisition = match source {
            CandidateSource::Github {
                repository,
                requested_ref,
                resolved_sha,
                skill_path,
            } => AcquisitionProvenance {
                kind: "github".into(),
                confidence: "recorded".into(),
                repository: Some(repository),
                requested_ref: Some(requested_ref),
                resolved_sha: Some(resolved_sha),
                skill_path: Some(skill_path),
                selected_path: None,
                candidate_hash: Some(candidate_hash.into()),
                recorded_at: Some(Utc::now().to_rfc3339()),
            },
            CandidateSource::Local { selected_path } => AcquisitionProvenance {
                kind: "local".into(),
                confidence: "recorded".into(),
                repository: None,
                requested_ref: None,
                resolved_sha: None,
                skill_path: None,
                selected_path: Some(selected_path),
                candidate_hash: Some(candidate_hash.into()),
                recorded_at: Some(Utc::now().to_rfc3339()),
            },
        };
        self.record(skill_id, acquisition.clone())?;
        Ok(acquisition)
    }

    #[cfg(test)]
    fn confirm_github_batch(
        &self,
        catalog: &Catalog,
        confirmations: &[GithubProvenanceConfirmation],
    ) -> Result<ProvenanceBackfillResult, ProvenanceError> {
        if confirmations.is_empty() {
            return Ok(ProvenanceBackfillResult::default());
        }

        let mut resolved = BTreeMap::new();
        for confirmation in confirmations {
            validate_repository(&confirmation.repository)?;
            validate_skill_path(confirmation.skill_path.as_deref())?;
            if confirmation.directory_name.is_empty()
                || confirmation.directory_name.len() > MAX_VALUE_BYTES
                || confirmation.directory_name.contains(['/', '\\', '\0'])
            {
                return Err(ProvenanceError::InvalidStore);
            }
            let matches = catalog
                .skills
                .iter()
                .filter(|skill| {
                    skill.source == "personal"
                        && skill.directory_name == confirmation.directory_name
                })
                .collect::<Vec<_>>();
            let [skill] = matches.as_slice() else {
                return Err(ProvenanceError::InvalidStore);
            };
            let acquisition = AcquisitionProvenance {
                kind: "github".into(),
                confidence: "confirmed".into(),
                repository: Some(confirmation.repository.clone()),
                requested_ref: None,
                resolved_sha: None,
                skill_path: confirmation.skill_path.clone(),
                selected_path: None,
                candidate_hash: None,
                recorded_at: Some(Utc::now().to_rfc3339()),
            };
            validate_acquisition(&acquisition)?;
            if resolved.insert(skill.id.clone(), acquisition).is_some() {
                return Err(ProvenanceError::InvalidStore);
            }
        }

        let _guard = self
            .mutation
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut store = self.read()?;
        let new_records = resolved
            .keys()
            .filter(|skill_id| !store.records.contains_key(*skill_id))
            .count();
        if store.records.len().saturating_add(new_records) > MAX_RECORDS {
            return Err(ProvenanceError::InvalidStore);
        }
        let mut result = ProvenanceBackfillResult::default();
        for (skill_id, acquisition) in &resolved {
            match store.records.get(skill_id) {
                Some(existing) if existing.acquisition.confidence == "recorded" => {
                    result.preserved_recorded += 1;
                }
                Some(existing) if same_confirmed_origin(&existing.acquisition, acquisition) => {
                    result.unchanged += 1;
                }
                Some(_) => return Err(ProvenanceError::InvalidStore),
                None => {
                    store.records.insert(
                        skill_id.clone(),
                        StoredRecord {
                            acquisition: acquisition.clone(),
                        },
                    );
                    result.confirmed += 1;
                }
            }
        }
        if result.confirmed > 0 {
            self.write(&store)?;
        }
        Ok(result)
    }

    pub fn replace_skill(&self, previous_id: &str, next_id: &str) -> Result<(), ProvenanceError> {
        validate_id(previous_id)?;
        validate_id(next_id)?;
        let _guard = self
            .mutation
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut store = self.read()?;
        if previous_id != next_id && store.records.contains_key(next_id) {
            return Err(ProvenanceError::InvalidStore);
        }
        let Some(record) = store.records.remove(previous_id) else {
            return Ok(());
        };
        store.records.insert(next_id.into(), record);
        self.write(&store)
    }

    pub fn remove_skill(&self, skill_id: &str) -> Result<(), ProvenanceError> {
        validate_id(skill_id)?;
        let _guard = self
            .mutation
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut store = self.read()?;
        if store.records.remove(skill_id).is_some() {
            self.write(&store)?;
        }
        Ok(())
    }

    pub(crate) fn recorded_acquisition(
        &self,
        skill_id: &str,
    ) -> Result<Option<AcquisitionProvenance>, ProvenanceError> {
        validate_id(skill_id)?;
        Ok(self
            .read()?
            .records
            .get(skill_id)
            .map(|record| record.acquisition.clone()))
    }

    pub(crate) fn restore_skill(
        &self,
        skill_id: &str,
        acquisition: &AcquisitionProvenance,
    ) -> Result<(), ProvenanceError> {
        self.record(skill_id, acquisition.clone())
    }

    fn record(
        &self,
        skill_id: &str,
        acquisition: AcquisitionProvenance,
    ) -> Result<(), ProvenanceError> {
        validate_id(skill_id)?;
        validate_acquisition(&acquisition)?;
        let _guard = self
            .mutation
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut store = self.read()?;
        if store.records.len() >= MAX_RECORDS && !store.records.contains_key(skill_id) {
            return Err(ProvenanceError::InvalidStore);
        }
        store
            .records
            .insert(skill_id.into(), StoredRecord { acquisition });
        self.write(&store)
    }

    fn read(&self) -> Result<Store, ProvenanceError> {
        let content = match fs::read_to_string(&self.path) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Store::default())
            }
            Err(error) => return Err(error.into()),
        };
        if content.len() > 8 * 1024 * 1024 {
            return Err(ProvenanceError::InvalidStore);
        }
        let store = serde_json::from_str(&content).map_err(|_| ProvenanceError::InvalidStore)?;
        validate_store(&store)?;
        Ok(store)
    }

    fn write(&self, store: &Store) -> Result<(), ProvenanceError> {
        validate_store(store)?;
        let parent = self.path.parent().ok_or(ProvenanceError::InvalidStore)?;
        fs::create_dir_all(parent)?;
        let bytes = serde_json::to_vec_pretty(store).map_err(|_| ProvenanceError::InvalidStore)?;
        let mut temporary = NamedTempFile::new_in(parent)?;
        temporary.write_all(&bytes)?;
        temporary.as_file_mut().sync_all()?;
        temporary
            .persist(&self.path)
            .map_err(|error| ProvenanceError::Io(error.error))?;
        set_private_permissions(parent, &self.path)?;
        Ok(())
    }
}

fn acquisition_for(store: &Store, skill_id: &str) -> AcquisitionProvenance {
    store
        .records
        .get(skill_id)
        .map(|record| record.acquisition.clone())
        .unwrap_or_else(AcquisitionProvenance::unknown)
}

fn validate_store(store: &Store) -> Result<(), ProvenanceError> {
    if store.version != 1 || store.records.len() > MAX_RECORDS {
        return Err(ProvenanceError::InvalidStore);
    }
    let mut ids = BTreeSet::new();
    for (id, record) in &store.records {
        validate_id(id)?;
        if !ids.insert(id) {
            return Err(ProvenanceError::InvalidStore);
        }
        validate_acquisition(&record.acquisition)?;
    }
    Ok(())
}

fn validate_id(value: &str) -> Result<(), ProvenanceError> {
    if value.is_empty() || value.len() > MAX_ID_BYTES || value.chars().any(char::is_control) {
        Err(ProvenanceError::InvalidStore)
    } else {
        Ok(())
    }
}

fn validate_acquisition(value: &AcquisitionProvenance) -> Result<(), ProvenanceError> {
    if !matches!(value.confidence.as_str(), "recorded" | "confirmed")
        || value
            .recorded_at
            .as_deref()
            .and_then(|timestamp| DateTime::parse_from_rfc3339(timestamp).ok())
            .is_none()
    {
        return Err(ProvenanceError::InvalidStore);
    }
    for item in [
        value.repository.as_deref(),
        value.requested_ref.as_deref(),
        value.resolved_sha.as_deref(),
        value.skill_path.as_deref(),
        value.selected_path.as_deref(),
        value.candidate_hash.as_deref(),
        value.recorded_at.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if item.len() > MAX_VALUE_BYTES || item.contains('\0') {
            return Err(ProvenanceError::InvalidStore);
        }
    }
    let shape_is_valid = match value.kind.as_str() {
        "github" if value.confidence == "recorded" => {
            value
                .repository
                .as_deref()
                .is_some_and(|repository| validate_repository(repository).is_ok())
                && value.requested_ref.is_some()
                && value.resolved_sha.is_some()
                && value
                    .skill_path
                    .as_deref()
                    .is_some_and(valid_recorded_skill_path)
                && value.selected_path.is_none()
                && value.candidate_hash.is_some()
        }
        "github" if value.confidence == "confirmed" => {
            value
                .repository
                .as_deref()
                .is_some_and(|repository| validate_repository(repository).is_ok())
                && value.requested_ref.is_none()
                && value.resolved_sha.is_none()
                && validate_skill_path(value.skill_path.as_deref()).is_ok()
                && value.selected_path.is_none()
                && value.candidate_hash.is_none()
        }
        "local" if value.confidence == "recorded" => {
            value.selected_path.is_some()
                && value.repository.is_none()
                && value.requested_ref.is_none()
                && value.resolved_sha.is_none()
                && value.skill_path.is_none()
                && value.candidate_hash.is_some()
        }
        _ => false,
    };
    if shape_is_valid {
        Ok(())
    } else {
        Err(ProvenanceError::InvalidStore)
    }
}

#[cfg(test)]
fn same_confirmed_origin(left: &AcquisitionProvenance, right: &AcquisitionProvenance) -> bool {
    left.kind == "github"
        && left.confidence == "confirmed"
        && left
            .repository
            .as_deref()
            .zip(right.repository.as_deref())
            .is_some_and(|(left, right)| left.eq_ignore_ascii_case(right))
        && left.skill_path == right.skill_path
}

fn validate_repository(value: &str) -> Result<(), ProvenanceError> {
    let components = value.split('/').collect::<Vec<_>>();
    if components.len() == 2 && components.into_iter().all(valid_github_atom) {
        Ok(())
    } else {
        Err(ProvenanceError::InvalidStore)
    }
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

fn validate_skill_path(value: Option<&str>) -> Result<(), ProvenanceError> {
    let Some(value) = value else {
        return Ok(());
    };
    let components = value.split('/').collect::<Vec<_>>();
    if value.is_empty()
        || value.len() > MAX_VALUE_BYTES
        || components.len() > 32
        || components.iter().any(|component| {
            component.is_empty()
                || *component == "."
                || *component == ".."
                || component.contains(['\\', '\0'])
        })
    {
        Err(ProvenanceError::InvalidStore)
    } else {
        Ok(())
    }
}

fn valid_recorded_skill_path(value: &str) -> bool {
    value.is_empty() || validate_skill_path(Some(value)).is_ok()
}

#[cfg(unix)]
fn set_private_permissions(directory: &Path, file: &Path) -> Result<(), ProvenanceError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;
    fs::set_permissions(file, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_permissions(_directory: &Path, _file: &Path) -> Result<(), ProvenanceError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::{Counts, Roots, SkillSummary};

    fn summary(id: &str) -> SkillSummary {
        SkillSummary {
            id: id.into(),
            name: "demo".into(),
            display_name: "Demo".into(),
            description: String::new(),
            summary: String::new(),
            source: "personal".into(),
            state: "active".into(),
            path: "/skills/demo".into(),
            directory_name: "demo".into(),
            modified_at: "1970-01-01T00:00:00Z".into(),
            file_count: 1,
            trigger_compliant: true,
            trigger_mode: "explicit".into(),
            has_blocking_findings: false,
            has_icon: false,
            brand_color: None,
            acquisition: AcquisitionProvenance::unknown(),
        }
    }

    #[test]
    fn records_exact_github_source_and_survives_restart() {
        let directory = tempfile::tempdir().unwrap();
        let manager = ProvenanceManager::new(directory.path().into());
        manager
            .record_candidate(
                "personal-id",
                &"b".repeat(64),
                CandidateSource::Github {
                    repository: "owner/repo".into(),
                    requested_ref: "main".into(),
                    resolved_sha: "a".repeat(40),
                    skill_path: "skills/demo".into(),
                },
            )
            .unwrap();
        let mut catalog = Catalog {
            codex_home: "/codex".into(),
            roots: Roots {
                personal_root: String::new(),
                system_root: String::new(),
                plugin_root: String::new(),
                disabled_root: String::new(),
                archive_root: String::new(),
            },
            skills: vec![summary("personal-id")],
            counts: Counts::default(),
        };
        ProvenanceManager::new(directory.path().into())
            .attach_catalog(&mut catalog)
            .unwrap();
        let source = &catalog.skills[0].acquisition;
        assert_eq!(source.kind, "github");
        assert_eq!(source.repository.as_deref(), Some("owner/repo"));
        assert_eq!(
            source.resolved_sha.as_deref(),
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
    }

    #[test]
    fn migrates_lifecycle_identity_and_removes_deleted_identity() {
        let directory = tempfile::tempdir().unwrap();
        let manager = ProvenanceManager::new(directory.path().into());
        manager
            .record_candidate(
                "personal-id",
                &"b".repeat(64),
                CandidateSource::Local {
                    selected_path: "/source/demo".into(),
                },
            )
            .unwrap();
        manager.replace_skill("personal-id", "archive-id").unwrap();
        let mut detail = SkillDetail {
            summary: summary("archive-id"),
            markdown: String::new(),
            document: crate::skills::SkillDocument {
                has_frontmatter: false,
                name: String::new(),
                description: String::new(),
                body: String::new(),
            },
            editable: false,
            content_hash: String::new(),
        };
        manager.attach_detail(&mut detail).unwrap();
        assert_eq!(detail.summary.acquisition.kind, "local");
        manager.remove_skill("archive-id").unwrap();
        manager.attach_detail(&mut detail).unwrap();
        assert_eq!(detail.summary.acquisition.kind, "unknown");
    }

    #[test]
    fn leaves_unrecorded_existing_skills_unknown() {
        let directory = tempfile::tempdir().unwrap();
        let manager = ProvenanceManager::new(directory.path().into());
        let mut detail = SkillDetail {
            summary: summary("existing"),
            markdown: String::new(),
            document: crate::skills::SkillDocument {
                has_frontmatter: false,
                name: String::new(),
                description: String::new(),
                body: String::new(),
            },
            editable: true,
            content_hash: String::new(),
        };
        manager.attach_detail(&mut detail).unwrap();
        assert_eq!(detail.summary.acquisition.kind, "unknown");
        assert_eq!(detail.summary.acquisition.confidence, "unknown");
    }

    #[test]
    fn confirms_a_legacy_repository_without_inventing_a_commit() {
        let directory = tempfile::tempdir().unwrap();
        let manager = ProvenanceManager::new(directory.path().into());
        let mut legacy = summary("legacy-id");
        legacy.directory_name = "grill-me".into();
        let result = manager
            .confirm_github_batch(
                &catalog(vec![legacy]),
                &[confirmation(
                    "grill-me",
                    "mattpocock/skills",
                    Some("skills/productivity/grill-me"),
                )],
            )
            .unwrap();
        assert_eq!(result.confirmed, 1);
        let mut detail = detail(summary("legacy-id"));
        manager.attach_detail(&mut detail).unwrap();
        let source = detail.summary.acquisition;
        assert_eq!(source.confidence, "confirmed");
        assert_eq!(source.repository.as_deref(), Some("mattpocock/skills"));
        assert_eq!(source.resolved_sha, None);
        assert_eq!(source.candidate_hash, None);
    }

    #[test]
    fn repeated_backfill_is_idempotent_and_preserves_exact_records() {
        let directory = tempfile::tempdir().unwrap();
        let manager = ProvenanceManager::new(directory.path().into());
        manager
            .record_candidate(
                "exact-id",
                &"b".repeat(64),
                CandidateSource::Github {
                    repository: "owner/exact".into(),
                    requested_ref: "main".into(),
                    resolved_sha: "a".repeat(40),
                    skill_path: "skills/exact".into(),
                },
            )
            .unwrap();
        let mut exact = summary("exact-id");
        exact.directory_name = "exact".into();
        let mut legacy = summary("legacy-id");
        legacy.directory_name = "legacy".into();
        let catalog = catalog(vec![exact, legacy]);
        let confirmations = vec![
            confirmation("exact", "other/repository", Some("skills/exact")),
            confirmation("legacy", "owner/legacy", Some("skills/legacy")),
        ];

        let first = manager
            .confirm_github_batch(&catalog, &confirmations)
            .unwrap();
        assert_eq!(first.confirmed, 1);
        assert_eq!(first.preserved_recorded, 1);
        let content = fs::read_to_string(&manager.path).unwrap();
        let second = manager
            .confirm_github_batch(&catalog, &confirmations)
            .unwrap();
        assert_eq!(second.unchanged, 1);
        assert_eq!(second.preserved_recorded, 1);
        assert_eq!(fs::read_to_string(&manager.path).unwrap(), content);

        let mut exact_detail = detail(summary("exact-id"));
        manager.attach_detail(&mut exact_detail).unwrap();
        assert_eq!(
            exact_detail.summary.acquisition.repository.as_deref(),
            Some("owner/exact")
        );
        assert_eq!(exact_detail.summary.acquisition.confidence, "recorded");
    }

    #[test]
    fn conflicting_or_invalid_backfill_is_atomic() {
        let directory = tempfile::tempdir().unwrap();
        let manager = ProvenanceManager::new(directory.path().into());
        let mut one = summary("one-id");
        one.directory_name = "one".into();
        let mut two = summary("two-id");
        two.directory_name = "two".into();
        let catalog = catalog(vec![one, two]);
        manager
            .confirm_github_batch(
                &catalog,
                &[confirmation("one", "owner/one", Some("skills/one"))],
            )
            .unwrap();
        let before = fs::read_to_string(&manager.path).unwrap();

        assert!(manager
            .confirm_github_batch(
                &catalog,
                &[
                    confirmation("one", "different/repo", Some("skills/one")),
                    confirmation("two", "owner/two", Some("../two")),
                ],
            )
            .is_err());
        assert_eq!(fs::read_to_string(&manager.path).unwrap(), before);
    }

    #[test]
    fn backfill_requires_one_current_personal_directory_match() {
        let directory = tempfile::tempdir().unwrap();
        let manager = ProvenanceManager::new(directory.path().into());
        let mut managed = summary("managed-id");
        managed.source = "system".into();
        managed.directory_name = "managed".into();
        assert!(manager
            .confirm_github_batch(
                &catalog(vec![managed]),
                &[confirmation("managed", "owner/repo", None)],
            )
            .is_err());
        assert!(!manager.path.exists());
    }

    fn confirmation(
        directory_name: &str,
        repository: &str,
        skill_path: Option<&str>,
    ) -> GithubProvenanceConfirmation {
        GithubProvenanceConfirmation {
            directory_name: directory_name.into(),
            repository: repository.into(),
            skill_path: skill_path.map(str::to_string),
        }
    }

    fn catalog(skills: Vec<SkillSummary>) -> Catalog {
        Catalog {
            codex_home: "/codex".into(),
            roots: Roots {
                personal_root: String::new(),
                system_root: String::new(),
                plugin_root: String::new(),
                disabled_root: String::new(),
                archive_root: String::new(),
            },
            skills,
            counts: Counts::default(),
        }
    }

    fn detail(summary: SkillSummary) -> SkillDetail {
        SkillDetail {
            summary,
            markdown: String::new(),
            document: crate::skills::SkillDocument {
                has_frontmatter: false,
                name: String::new(),
                description: String::new(),
                body: String::new(),
            },
            editable: true,
            content_hash: String::new(),
        }
    }
}
