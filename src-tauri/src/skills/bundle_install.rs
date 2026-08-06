use super::{
    bundle_import::{
        BundleImportManager, BundleImportReview, ImportedSkillReview, MAX_PREVIEW_BYTES,
    },
    candidate::{rename_directory_no_replace, sync_directory},
    InternalSkill, SkillDetail, Source, Workspace, WorkspaceError,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use skill_bundle_core::{
    skill_revision, BundleFile, MAX_FILES_PER_SKILL, MAX_FILE_BYTES, MAX_PATH_DEPTH,
    MAX_SKILL_BYTES,
};
use std::{
    collections::{BTreeMap, HashMap},
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};
use tempfile::{Builder, TempDir};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BundleInstallError {
    #[error(transparent)]
    Import(#[from] super::BundleImportError),
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    #[error("导入版本比较已缺失或过期。")]
    StaleReview,
    #[error("所选同名版本已缺失或发生变化。")]
    UnknownMatch,
    #[error("所选文件不属于当前版本比较。")]
    UnknownFile,
    #[error("请至少选择一个当前可安装的版本。")]
    InvalidSelection,
    #[error("当前审查结果不允许安装所选 Skill。")]
    Blocked,
    #[error("导入 Skill 的安装未能完成。")]
    Io(#[from] std::io::Error),
}

impl BundleInstallError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Import(error) => error.code(),
            Self::Workspace(error) => error.code(),
            Self::StaleReview => "BUNDLE_INSTALL_REVIEW_STALE",
            Self::UnknownMatch => "BUNDLE_INSTALL_MATCH_UNKNOWN",
            Self::UnknownFile => "BUNDLE_INSTALL_FILE_UNKNOWN",
            Self::InvalidSelection => "BUNDLE_INSTALL_SELECTION_INVALID",
            Self::Blocked => "BUNDLE_INSTALL_BLOCKED",
            Self::Io(_) => "BUNDLE_INSTALL_IO_ERROR",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleInstallationReview {
    #[serde(flatten)]
    pub import: BundleImportReview,
    pub review_revision: String,
    pub decisions: Vec<ImportedSkillDecision>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedSkillDecision {
    pub directory_name: String,
    pub classification: String,
    pub summary: String,
    pub matches: Vec<CatalogMatch>,
    pub install_offer: Option<InstallOffer>,
    pub baseline_blocked: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogMatch {
    pub id: String,
    pub source: String,
    pub state: String,
    pub path: String,
    pub directory_name: String,
    pub revision: Option<String>,
    pub identical: bool,
    pub user_controlled: bool,
    pub measurement_available: bool,
    pub file_deltas: Vec<ImportFileDelta>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFileDelta {
    pub path: String,
    pub status: String,
    pub imported: Option<PortableFileEvidence>,
    pub current: Option<PortableFileEvidence>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableFileEvidence {
    pub path: String,
    pub size: u64,
    pub sha256: String,
    pub executable: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallOffer {
    pub token: String,
    pub kind: String,
    pub destination: String,
    pub replaces_match_id: Option<String>,
    pub summary: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BundleInstallSelection {
    pub directory_name: String,
    pub offer_token: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleFileComparison {
    pub directory_name: String,
    pub match_id: String,
    pub path: String,
    pub status: String,
    pub imported: ComparisonSide,
    pub current: ComparisonSide,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComparisonSide {
    pub exists: bool,
    pub size: Option<u64>,
    pub sha256: Option<String>,
    pub executable: Option<bool>,
    pub is_text: bool,
    pub content: Option<String>,
    pub truncated: bool,
    pub preview_bytes: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleInstallResult {
    pub ok: bool,
    pub bundle_revision: String,
    pub outcomes: Vec<BundleInstallOutcome>,
    pub catalog_refresh_needed: bool,
    pub restart_recommended: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleInstallOutcome {
    pub directory_name: String,
    pub status: String,
    pub message: String,
    pub prior_skill_id: Option<String>,
    pub skill: Option<SkillDetail>,
}

struct CatalogSnapshot {
    revision: String,
    files: Vec<CatalogFile>,
}

struct CatalogFile {
    evidence: PortableFileEvidence,
    absolute_path: PathBuf,
}

struct PreparedInstall {
    selection: BundleInstallSelection,
    imported_revision: String,
    temporary: TempDir,
}

#[derive(Debug, Default)]
struct CommitNotice {
    retained_backup: Option<PathBuf>,
}

#[derive(Debug)]
struct CommitFailure {
    error: std::io::Error,
    retain_prepared_directory: bool,
}

impl CommitFailure {
    fn ordinary(error: std::io::Error) -> Self {
        Self {
            error,
            retain_prepared_directory: false,
        }
    }
}

impl BundleImportManager {
    pub fn review_installation(
        &self,
        workspace: &Workspace,
        session_id: &str,
        expected_bundle_revision: &str,
    ) -> Result<BundleInstallationReview, BundleInstallError> {
        let import = self.verified_review(session_id, expected_bundle_revision)?;
        self.classify_staged_review(workspace, import)
    }

    pub fn classify_staged_review(
        &self,
        workspace: &Workspace,
        import: BundleImportReview,
    ) -> Result<BundleInstallationReview, BundleInstallError> {
        let mut matches = HashMap::new();
        for skill in &import.skills {
            matches.insert(
                skill.directory_name.clone(),
                workspace.cached_name_matches(&skill.directory_name)?,
            );
        }
        build_installation_review(workspace, import, &matches)
    }

    pub fn compare_installation_file(
        &self,
        workspace: &Workspace,
        session_id: &str,
        expected_bundle_revision: &str,
        directory_name: &str,
        match_id: &str,
        path: &str,
    ) -> Result<BundleFileComparison, BundleInstallError> {
        let review = self.review_installation(workspace, session_id, expected_bundle_revision)?;
        let decision = review
            .decisions
            .iter()
            .find(|decision| decision.directory_name == directory_name)
            .ok_or(BundleInstallError::UnknownMatch)?;
        let catalog_match = decision
            .matches
            .iter()
            .find(|item| item.id == match_id)
            .ok_or(BundleInstallError::UnknownMatch)?;
        let delta = catalog_match
            .file_deltas
            .iter()
            .find(|delta| delta.path == path)
            .ok_or(BundleInstallError::UnknownFile)?;
        let current_skill = workspace
            .cached_name_matches(directory_name)?
            .into_iter()
            .find(|skill| skill.summary.id == match_id)
            .ok_or(BundleInstallError::UnknownMatch)?;
        let current_snapshot =
            snapshot_catalog_skill(&current_skill).map_err(|_| BundleInstallError::UnknownMatch)?;
        if catalog_match.revision.as_deref() != Some(&current_snapshot.revision) {
            return Err(BundleInstallError::StaleReview);
        }

        let imported = match &delta.imported {
            Some(evidence) => comparison_side(
                Some(self.verified_file_bytes(
                    session_id,
                    expected_bundle_revision,
                    directory_name,
                    path,
                )?),
                Some(evidence),
            ),
            None => missing_comparison_side(),
        };
        let current = match current_snapshot
            .files
            .iter()
            .find(|file| file.evidence.path == path)
        {
            Some(file) => comparison_side(
                Some(read_catalog_file(file).map_err(|_| BundleInstallError::StaleReview)?),
                Some(&file.evidence),
            ),
            None => missing_comparison_side(),
        };
        Ok(BundleFileComparison {
            directory_name: directory_name.into(),
            match_id: match_id.into(),
            path: path.into(),
            status: delta.status.clone(),
            imported,
            current,
        })
    }

    pub fn install_reviewed(
        &self,
        workspace: &Workspace,
        session_id: &str,
        expected_bundle_revision: &str,
        expected_review_revision: &str,
        selections: &[BundleInstallSelection],
    ) -> Result<BundleInstallResult, BundleInstallError> {
        if selections.is_empty() {
            return Err(BundleInstallError::InvalidSelection);
        }
        let advisory = self.review_installation(workspace, session_id, expected_bundle_revision)?;
        let identical_retry = selections.iter().all(|selection| {
            advisory.decisions.iter().any(|decision| {
                decision.directory_name == selection.directory_name
                    && decision.classification == "identical"
            })
        });
        if expected_review_revision.is_empty()
            || (expected_review_revision != advisory.review_revision && !identical_retry)
        {
            return Err(BundleInstallError::StaleReview);
        }
        let personal_root = workspace.personal_root_for_creation()?;
        let mut prepared = Vec::with_capacity(selections.len());
        let mut selected_names = std::collections::HashSet::new();
        for selection in selections {
            if !selected_names.insert(selection.directory_name.clone()) {
                return Err(BundleInstallError::InvalidSelection);
            }
            let decision = advisory
                .decisions
                .iter()
                .find(|decision| decision.directory_name == selection.directory_name)
                .ok_or(BundleInstallError::InvalidSelection)?;
            if decision.classification != "identical"
                && decision
                    .install_offer
                    .as_ref()
                    .is_none_or(|offer| offer.token != selection.offer_token)
            {
                return Err(BundleInstallError::StaleReview);
            }
            let skill = advisory
                .import
                .skills
                .iter()
                .find(|skill| skill.directory_name == selection.directory_name)
                .cloned()
                .ok_or(BundleInstallError::InvalidSelection)?;
            let temporary = prepare_imported_skill(
                self,
                session_id,
                expected_bundle_revision,
                &personal_root,
                &skill,
            )?;
            prepared.push(PreparedInstall {
                selection: selection.clone(),
                imported_revision: skill.revision,
                temporary,
            });
        }

        let _mutation = workspace
            .mutations
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let fresh_skills = workspace.fresh_all_skills_for_mutation()?;
        let mut fresh_by_name: HashMap<String, Vec<InternalSkill>> = HashMap::new();
        for skill in &fresh_skills {
            fresh_by_name
                .entry(skill.summary.name.clone())
                .or_default()
                .push(skill.clone());
        }
        let fresh_import = self.verified_review(session_id, expected_bundle_revision)?;
        let fresh_review = build_installation_review(workspace, fresh_import, &fresh_by_name)?;

        for prepared_skill in &prepared {
            let decision = fresh_review
                .decisions
                .iter()
                .find(|decision| decision.directory_name == prepared_skill.selection.directory_name)
                .ok_or(BundleInstallError::StaleReview)?;
            if decision.classification == "identical" {
                continue;
            }
            if decision
                .install_offer
                .as_ref()
                .is_none_or(|offer| offer.token != prepared_skill.selection.offer_token)
            {
                return Err(BundleInstallError::StaleReview);
            }
        }

        workspace.replace_index_from_skills(fresh_skills);
        let mut outcomes = Vec::with_capacity(prepared.len());
        let mut catalog_refresh_needed = false;
        for prepared_skill in prepared {
            let decision = fresh_review
                .decisions
                .iter()
                .find(|decision| decision.directory_name == prepared_skill.selection.directory_name)
                .expect("preflighted decision");
            if decision.classification == "identical" {
                outcomes.push(BundleInstallOutcome {
                    directory_name: prepared_skill.selection.directory_name,
                    status: "skippedIdentical".into(),
                    message: "目标目录中已存在完全相同的 Skill 版本，未重复安装。".into(),
                    prior_skill_id: None,
                    skill: None,
                });
                continue;
            }
            let offer = decision.install_offer.as_ref().expect("preflighted offer");
            let destination = personal_root.join(&prepared_skill.selection.directory_name);
            let prior_skill_id = offer.replaces_match_id.clone();
            let prepared_revision = snapshot_directory(prepared_skill.temporary.path())
                .map(|snapshot| snapshot.revision)
                .ok();
            if prepared_revision.as_deref() != Some(prepared_skill.imported_revision.as_str()) {
                outcomes.push(BundleInstallOutcome {
                    directory_name: prepared_skill.selection.directory_name,
                    status: "failed".into(),
                    message: "准备安装的 Skill 在提交前发生变化，未执行安装。".into(),
                    prior_skill_id,
                    skill: None,
                });
                continue;
            }
            let commit = if offer.kind == "replacePersonal" {
                let Some(expected_match) = decision
                    .matches
                    .iter()
                    .find(|item| offer.replaces_match_id.as_deref() == Some(item.id.as_str()))
                else {
                    outcomes.push(BundleInstallOutcome {
                        directory_name: prepared_skill.selection.directory_name,
                        status: "failed".into(),
                        message: "当前个人 Skill 已变化，未执行替换。".into(),
                        prior_skill_id,
                        skill: None,
                    });
                    continue;
                };
                let current = fresh_by_name
                    .get(&prepared_skill.selection.directory_name)
                    .into_iter()
                    .flatten()
                    .find(|skill| {
                        Some(skill.summary.id.as_str()) == offer.replaces_match_id.as_deref()
                    })
                    .and_then(|skill| snapshot_catalog_skill(skill).ok());
                if expected_match.revision.is_none()
                    || current.as_ref().map(|snapshot| snapshot.revision.as_str())
                        != expected_match.revision.as_deref()
                {
                    outcomes.push(BundleInstallOutcome {
                        directory_name: prepared_skill.selection.directory_name,
                        status: "failed".into(),
                        message: "当前个人 Skill 在提交前发生变化，未执行替换。".into(),
                        prior_skill_id,
                        skill: None,
                    });
                    continue;
                }
                replace_personal_directory(
                    prepared_skill.temporary.path(),
                    &destination,
                    &personal_root,
                    expected_match
                        .revision
                        .as_deref()
                        .expect("checked measurable revision"),
                )
            } else {
                rename_directory_no_replace(prepared_skill.temporary.path(), &destination)
                    .map(|()| CommitNotice::default())
                    .map_err(CommitFailure::ordinary)
            };
            let commit_notice = match commit {
                Ok(notice) => notice,
                Err(failure) => {
                    if failure.retain_prepared_directory {
                        let _ = prepared_skill.temporary.keep();
                    }
                    outcomes.push(BundleInstallOutcome {
                        directory_name: prepared_skill.selection.directory_name,
                        status: "failed".into(),
                        message: format!("安装提交失败：{}", failure.error),
                        prior_skill_id,
                        skill: None,
                    });
                    continue;
                }
            };
            let _ = prepared_skill.temporary.keep();
            let sync_error = sync_directory(&personal_root).err();
            let installed = workspace
                .read_skill(&destination, Source::Personal, &personal_root)
                .ok()
                .flatten();
            let detail = installed.as_ref().map(|installed| SkillDetail {
                content_hash: super::hash(&installed.markdown),
                summary: installed.summary.clone(),
                markdown: installed.markdown.clone(),
                document: installed.document.clone(),
                editable: true,
            });
            if let Some(installed) = installed {
                if workspace.upsert_index(installed).is_err() {
                    catalog_refresh_needed = true;
                }
            } else {
                catalog_refresh_needed = true;
            }
            let mut message = if offer.kind == "replacePersonal" {
                "已使用确认的导入版本替换当前个人 Skill。".to_string()
            } else {
                "已安装为启用的个人 Skill。".to_string()
            };
            if detail.is_none() {
                message.push_str(" 文件已提交，但目录无法立即重新读取；需要刷新后确认显示状态。");
            }
            if let Some(backup) = commit_notice.retained_backup {
                message.push_str(&format!(
                    " 旧版本备份清理失败，已保留在 {}。",
                    backup.display()
                ));
            }
            if let Some(error) = sync_error {
                message.push_str(&format!(" 目录已更新，但刷新文件系统元数据失败：{error}。"));
            }
            outcomes.push(BundleInstallOutcome {
                directory_name: prepared_skill.selection.directory_name,
                status: if offer.kind == "replacePersonal" {
                    "replaced"
                } else {
                    "installed"
                }
                .into(),
                message,
                prior_skill_id,
                skill: detail,
            });
        }
        if catalog_refresh_needed {
            workspace.invalidate_index();
        }
        let ok = outcomes.iter().all(|outcome| outcome.status != "failed");
        Ok(BundleInstallResult {
            ok,
            bundle_revision: expected_bundle_revision.into(),
            outcomes,
            catalog_refresh_needed,
            restart_recommended: true,
        })
    }
}

fn build_installation_review(
    workspace: &Workspace,
    import: BundleImportReview,
    matches_by_name: &HashMap<String, Vec<InternalSkill>>,
) -> Result<BundleInstallationReview, BundleInstallError> {
    let mut decisions = Vec::with_capacity(import.skills.len());
    for imported in &import.skills {
        let mut matches = matches_by_name
            .get(&imported.directory_name)
            .cloned()
            .unwrap_or_default();
        matches.sort_by(|left, right| {
            left.source
                .rank()
                .cmp(&right.source.rank())
                .then_with(|| left.directory.cmp(&right.directory))
        });
        decisions.push(classify_imported_skill(workspace, imported, &matches)?);
    }
    let review_revision = review_revision(&import.bundle_revision, &decisions);
    Ok(BundleInstallationReview {
        import,
        review_revision,
        decisions,
    })
}

fn classify_imported_skill(
    workspace: &Workspace,
    imported: &ImportedSkillReview,
    matches: &[InternalSkill],
) -> Result<ImportedSkillDecision, BundleInstallError> {
    let mut evidence = Vec::with_capacity(matches.len());
    for target in matches {
        let snapshot = snapshot_catalog_skill(target).ok();
        let revision = snapshot.as_ref().map(|snapshot| snapshot.revision.clone());
        let identical = revision.as_deref() == Some(imported.revision.as_str());
        evidence.push(CatalogMatch {
            id: target.summary.id.clone(),
            source: target.source.label().into(),
            state: target.summary.state.clone(),
            path: target.directory.display().to_string(),
            directory_name: target.summary.directory_name.clone(),
            revision,
            identical,
            user_controlled: matches!(
                target.source,
                Source::Personal | Source::Disabled | Source::Archive
            ),
            measurement_available: snapshot.is_some(),
            file_deltas: file_deltas(imported, snapshot.as_ref()),
        });
    }
    let structurally_incompatible = imported.compatibility.status == "incompatible";
    let classification = if structurally_incompatible {
        "incompatible"
    } else if evidence.iter().any(|item| item.identical) {
        "identical"
    } else if evidence
        .iter()
        .any(|item| matches!(item.source.as_str(), "system" | "plugin"))
    {
        "managedConflict"
    } else if !evidence.is_empty() {
        "userConflict"
    } else {
        "new"
    };
    let baseline_blocked = imported.audit.verdict == "block";
    let destination = workspace
        .roots()
        .personal
        .join(&imported.directory_name)
        .display()
        .to_string();
    let active_matches = evidence
        .iter()
        .filter(|item| item.source == "personal")
        .collect::<Vec<_>>();
    let active_target = active_matches
        .iter()
        .copied()
        .find(|item| item.directory_name == imported.directory_name);
    let (install_offer, summary) = match classification {
        "incompatible" => (None, "该导入 Skill 不满足 Codex 基础结构要求。".into()),
        "identical" => (
            None,
            "至少有一个同名 Skill 的完整版本相同，将自动跳过。".into(),
        ),
        "managedConflict" => (
            None,
            "存在内容不同的系统或插件 Skill，不能安装或覆盖。".into(),
        ),
        "userConflict" if baseline_blocked => (
            None,
            "存在个人版本冲突，且基础审查含阻断项，不能进入安装确认。".into(),
        ),
        "userConflict"
            if active_matches
                .iter()
                .any(|item| !item.measurement_available) =>
        {
            (
                None,
                "当前个人 Skill 无法计算完整版本校验值，不能确认替换。".into(),
            )
        }
        "userConflict" if !active_matches.is_empty() && active_target.is_none() => (
            None,
            "同名的启用个人 Skill 位于另一目录；为避免产生重复启用版本，不能安装。".into(),
        ),
        "userConflict" => {
            let kind = if active_target.is_some() {
                "replacePersonal"
            } else {
                "createKeepingDormant"
            };
            let token = offer_token(imported, classification, &evidence, kind, &destination);
            (
                Some(InstallOffer {
                    token,
                    kind: kind.into(),
                    destination,
                    replaces_match_id: active_target.map(|item| item.id.clone()),
                    summary: if kind == "replacePersonal" {
                        "将替换当前启用的个人 Skill；停用和归档版本保持不变。"
                    } else {
                        "将创建一个启用的个人 Skill；停用和归档版本保持不变。"
                    }
                    .into(),
                }),
                "存在内容不同的个人版本，需要比较并明确确认。".into(),
            )
        }
        "new" if baseline_blocked => (
            None,
            "当前目录中没有同名 Skill，但基础审查含阻断项。".into(),
        ),
        _ => {
            let kind = "createPersonal";
            let token = offer_token(imported, classification, &evidence, kind, &destination);
            (
                Some(InstallOffer {
                    token,
                    kind: kind.into(),
                    destination,
                    replaces_match_id: None,
                    summary: "只会安装到空闲目标位置；如果目标已存在，将停止且不会覆盖。".into(),
                }),
                "当前目录中没有同名 Skill，可以进入安装确认。".into(),
            )
        }
    };
    Ok(ImportedSkillDecision {
        directory_name: imported.directory_name.clone(),
        classification: classification.into(),
        summary,
        matches: evidence,
        install_offer,
        baseline_blocked,
    })
}

fn file_deltas(
    imported: &ImportedSkillReview,
    current: Option<&CatalogSnapshot>,
) -> Vec<ImportFileDelta> {
    let mut paths =
        BTreeMap::<String, (Option<PortableFileEvidence>, Option<PortableFileEvidence>)>::new();
    for file in &imported.files {
        paths.entry(file.path.clone()).or_default().0 = Some(PortableFileEvidence {
            path: file.path.clone(),
            size: file.size,
            sha256: file.sha256.clone(),
            executable: file.executable_after_install,
        });
    }
    if let Some(current) = current {
        for file in &current.files {
            paths.entry(file.evidence.path.clone()).or_default().1 = Some(file.evidence.clone());
        }
    }
    paths
        .into_iter()
        .map(|(path, (imported, current))| {
            let status = match (&imported, &current) {
                (Some(left), Some(right))
                    if left.sha256 == right.sha256 && left.executable == right.executable =>
                {
                    "unchanged"
                }
                (Some(_), Some(_)) => "modified",
                (Some(_), None) => "importedOnly",
                (None, Some(_)) => "currentOnly",
                (None, None) => unreachable!(),
            };
            ImportFileDelta {
                path,
                status: status.into(),
                imported,
                current,
            }
        })
        .collect()
}

fn offer_token(
    imported: &ImportedSkillReview,
    classification: &str,
    matches: &[CatalogMatch],
    kind: &str,
    destination: &str,
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"ASS-INSTALL-OFFER\0");
    hash_part(&mut digest, &imported.directory_name);
    hash_part(&mut digest, &imported.revision);
    hash_part(&mut digest, classification);
    hash_part(&mut digest, kind);
    hash_part(&mut digest, destination);
    for item in matches {
        hash_part(&mut digest, &item.id);
        hash_part(&mut digest, &item.source);
        hash_part(
            &mut digest,
            item.revision.as_deref().unwrap_or("unavailable"),
        );
    }
    format!("{:x}", digest.finalize())
}

fn review_revision(bundle_revision: &str, decisions: &[ImportedSkillDecision]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"ASS-INSTALL-REVIEW\0");
    hash_part(&mut digest, bundle_revision);
    for decision in decisions {
        hash_part(&mut digest, &decision.directory_name);
        hash_part(&mut digest, &decision.classification);
        hash_part(
            &mut digest,
            decision
                .install_offer
                .as_ref()
                .map(|offer| offer.token.as_str())
                .unwrap_or("no-offer"),
        );
    }
    format!("{:x}", digest.finalize())
}

fn hash_part(digest: &mut Sha256, value: &str) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value.as_bytes());
}

fn snapshot_catalog_skill(skill: &InternalSkill) -> Result<CatalogSnapshot, WorkspaceError> {
    let metadata = fs::symlink_metadata(&skill.directory)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(WorkspaceError::UnsafePath);
    }
    snapshot_directory(&skill.directory)
}

fn snapshot_directory(directory: &Path) -> Result<CatalogSnapshot, WorkspaceError> {
    let metadata = fs::symlink_metadata(directory)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(WorkspaceError::UnsafePath);
    }
    let root = fs::canonicalize(directory)?;
    let mut files = Vec::new();
    collect_catalog_files(&root, &root, 0, &mut files)?;
    files.sort_by(|left, right| {
        left.evidence
            .path
            .as_bytes()
            .cmp(right.evidence.path.as_bytes())
    });
    let revision_files = files
        .iter()
        .map(|file| BundleFile {
            path: file.evidence.path.clone(),
            size: file.evidence.size,
            sha256: file.evidence.sha256.clone(),
            executable: file.evidence.executable,
        })
        .collect::<Vec<_>>();
    let revision = skill_revision(&revision_files).map_err(|_| WorkspaceError::UnsafePath)?;
    Ok(CatalogSnapshot { revision, files })
}

fn collect_catalog_files(
    root: &Path,
    current: &Path,
    depth: usize,
    files: &mut Vec<CatalogFile>,
) -> Result<(), WorkspaceError> {
    if depth > MAX_PATH_DEPTH || files.len() > MAX_FILES_PER_SKILL {
        return Err(WorkspaceError::UnsafePath);
    }
    let mut entries = fs::read_dir(current)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(WorkspaceError::UnsafePath);
        }
        if metadata.is_dir() {
            collect_catalog_files(root, &path, depth + 1, files)?;
            continue;
        }
        if !metadata.is_file() || metadata.len() > MAX_FILE_BYTES {
            return Err(WorkspaceError::UnsafePath);
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|_| WorkspaceError::UnsafePath)?;
        if relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(WorkspaceError::UnsafePath);
        }
        let relative = relative
            .to_str()
            .ok_or(WorkspaceError::UnsafePath)?
            .replace('\\', "/");
        let (sha256, opened_metadata) = hash_catalog_file(&path, &metadata)?;
        files.push(CatalogFile {
            evidence: PortableFileEvidence {
                path: relative,
                size: opened_metadata.len(),
                sha256,
                executable: is_executable(&opened_metadata),
            },
            absolute_path: path,
        });
        if files.len() > MAX_FILES_PER_SKILL
            || files.iter().map(|file| file.evidence.size).sum::<u64>() > MAX_SKILL_BYTES
        {
            return Err(WorkspaceError::UnsafePath);
        }
    }
    Ok(())
}

fn hash_catalog_file(
    path: &Path,
    discovered: &fs::Metadata,
) -> Result<(String, fs::Metadata), WorkspaceError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let mut file = options.open(path)?;
    let opened = file.metadata()?;
    if !opened.is_file() || !same_file(discovered, &opened) {
        return Err(WorkspaceError::UnsafePath);
    }
    let mut digest = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or(WorkspaceError::UnsafePath)?;
        if total > opened.len() || total > MAX_FILE_BYTES {
            return Err(WorkspaceError::UnsafePath);
        }
        digest.update(&buffer[..count]);
    }
    if total != opened.len() || !same_file(&opened, &file.metadata()?) {
        return Err(WorkspaceError::UnsafePath);
    }
    Ok((format!("{:x}", digest.finalize()), opened))
}

fn read_catalog_file(file: &CatalogFile) -> Result<Vec<u8>, WorkspaceError> {
    let metadata = fs::symlink_metadata(&file.absolute_path)?;
    let (sha256, opened) = hash_catalog_file(&file.absolute_path, &metadata)?;
    if opened.len() != file.evidence.size || sha256 != file.evidence.sha256 {
        return Err(WorkspaceError::Conflict);
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let mut bytes = Vec::with_capacity(opened.len() as usize);
    options
        .open(&file.absolute_path)?
        .take(opened.len() + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 != opened.len()
        || format!("{:x}", Sha256::digest(&bytes)) != file.evidence.sha256
    {
        return Err(WorkspaceError::Conflict);
    }
    Ok(bytes)
}

fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        left.dev() == right.dev()
            && left.ino() == right.ino()
            && left.len() == right.len()
            && left.mode() == right.mode()
            && left.mtime() == right.mtime()
            && left.mtime_nsec() == right.mtime_nsec()
    }
    #[cfg(not(unix))]
    {
        left.len() == right.len()
    }
}

fn is_executable(metadata: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        false
    }
}

fn comparison_side(
    bytes: Option<Vec<u8>>,
    evidence: Option<&PortableFileEvidence>,
) -> ComparisonSide {
    let Some(bytes) = bytes else {
        return missing_comparison_side();
    };
    let is_text = std::str::from_utf8(&bytes).is_ok();
    let mut preview = bytes[..bytes.len().min(MAX_PREVIEW_BYTES)].to_vec();
    if is_text {
        while std::str::from_utf8(&preview).is_err() {
            preview.pop();
        }
    } else {
        preview.clear();
    }
    ComparisonSide {
        exists: true,
        size: evidence.map(|item| item.size),
        sha256: evidence.map(|item| item.sha256.clone()),
        executable: evidence.map(|item| item.executable),
        is_text,
        preview_bytes: preview.len(),
        content: is_text.then(|| String::from_utf8(preview).expect("validated UTF-8 preview")),
        truncated: is_text && bytes.len() > MAX_PREVIEW_BYTES,
    }
}

fn missing_comparison_side() -> ComparisonSide {
    ComparisonSide {
        exists: false,
        size: None,
        sha256: None,
        executable: None,
        is_text: false,
        content: None,
        truncated: false,
        preview_bytes: 0,
    }
}

fn prepare_imported_skill(
    manager: &BundleImportManager,
    session_id: &str,
    expected_bundle_revision: &str,
    personal_root: &Path,
    skill: &ImportedSkillReview,
) -> Result<TempDir, BundleInstallError> {
    let temporary = Builder::new()
        .prefix(".bundle-install-")
        .tempdir_in(personal_root)?;
    for file in &skill.files {
        let bytes = manager.verified_file_bytes(
            session_id,
            expected_bundle_revision,
            &skill.directory_name,
            &file.path,
        )?;
        let destination = temporary.path().join(&file.path);
        if !destination.starts_with(temporary.path()) {
            return Err(BundleInstallError::Blocked);
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(if file.executable_after_install {
                0o755
            } else {
                0o644
            });
        }
        let mut target = options.open(&destination)?;
        target.write_all(&bytes)?;
        target.flush()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            target.set_permissions(fs::Permissions::from_mode(
                if file.executable_after_install {
                    0o755
                } else {
                    0o644
                },
            ))?;
        }
        target.sync_all()?;
    }
    let mut directories = skill
        .files
        .iter()
        .filter_map(|file| Path::new(&file.path).parent())
        .map(|parent| temporary.path().join(parent))
        .collect::<Vec<_>>();
    directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    directories.dedup();
    for directory in directories {
        sync_directory(&directory)?;
    }
    sync_directory(temporary.path())?;
    Ok(temporary)
}

fn replace_personal_directory(
    source: &Path,
    destination: &Path,
    personal_root: &Path,
    expected_revision: &str,
) -> Result<CommitNotice, CommitFailure> {
    let _ = personal_root;
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        replace_personal_directory_atomic(
            source,
            destination,
            expected_revision,
            exchange_directories,
        )
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let backup_root = personal_root.parent().unwrap_or(personal_root);
        let backup = Builder::new()
            .prefix(".bundle-replaced-")
            .tempdir_in(backup_root)
            .map_err(CommitFailure::ordinary)?
            .keep();
        fs::remove_dir(&backup).map_err(CommitFailure::ordinary)?;
        fs::rename(destination, &backup).map_err(CommitFailure::ordinary)?;
        let isolated_revision = snapshot_directory(&backup)
            .map(|snapshot| snapshot.revision)
            .map_err(|error| std::io::Error::other(error.to_string()));
        if !matches!(isolated_revision.as_deref(), Ok(revision) if revision == expected_revision) {
            return match fs::rename(&backup, destination) {
            Ok(()) => Err(CommitFailure::ordinary(std::io::Error::other(
                    "当前个人 Skill 在替换边界发生变化；原目录已恢复",
            ))),
            Err(restore_error) => Err(CommitFailure::ordinary(std::io::Error::new(
                restore_error.kind(),
                format!(
                    "当前个人 Skill 在替换边界发生变化，且恢复失败（{restore_error}）；恢复副本保留在 {}",
                    backup.display()
                ),
            ))),
        };
        }
        if let Err(error) = rename_directory_no_replace(source, destination) {
            return match fs::rename(&backup, destination) {
            Ok(()) => Err(CommitFailure::ordinary(error)),
            Err(restore_error) => Err(CommitFailure::ordinary(std::io::Error::new(
                restore_error.kind(),
                format!(
                    "安装导入版本失败（{error}），恢复原 Skill 也失败（{restore_error}）；恢复副本保留在 {}",
                    backup.display()
                ),
            ))),
        };
        }
        let retained_backup = fs::remove_dir_all(&backup).err().map(|_| backup);
        Ok(CommitNotice { retained_backup })
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn replace_personal_directory_atomic<F>(
    source: &Path,
    destination: &Path,
    expected_revision: &str,
    mut exchange: F,
) -> Result<CommitNotice, CommitFailure>
where
    F: FnMut(&Path, &Path) -> std::io::Result<()>,
{
    exchange(source, destination).map_err(CommitFailure::ordinary)?;
    let isolated_revision = snapshot_directory(source)
        .map(|snapshot| snapshot.revision)
        .map_err(|error| std::io::Error::other(error.to_string()));
    if !matches!(isolated_revision.as_deref(), Ok(revision) if revision == expected_revision) {
        return match exchange(source, destination) {
            Ok(()) => Err(CommitFailure::ordinary(std::io::Error::other(
                "当前个人 Skill 在替换边界发生变化；原目录已恢复",
            ))),
            Err(restore_error) => Err(CommitFailure {
                error: std::io::Error::new(
                    restore_error.kind(),
                    format!(
                        "当前个人 Skill 在替换边界发生变化，且原子恢复失败（{restore_error}）；导入版本位于 {}，恢复副本位于 {}",
                        destination.display(),
                        source.display()
                    ),
                ),
                retain_prepared_directory: true,
            }),
        };
    }
    let retained_backup = fs::remove_dir_all(source)
        .err()
        .map(|_| source.to_path_buf());
    Ok(CommitNotice { retained_backup })
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

#[cfg(test)]
mod tests {
    use super::*;
    use skill_bundle_core::{
        write_bundle, AgentContract, BundleFileReader, BundleManifest, BundleSkill, BUNDLE_FORMAT,
        BUNDLE_FORMAT_VERSION, CODEX_CONTRACT_ID, CODEX_CONTRACT_VERSION,
    };
    use std::fs::File;
    use std::io::Cursor;

    fn markdown(name: &str, marker: &str) -> Vec<u8> {
        format!(
            "---\nname: {name}\ndescription: Use when testing Bundle installation.\n---\n\n# {name}\n\nInspect the exact revision and return grounded evidence.\n\n{marker}\n"
        )
        .into_bytes()
    }

    fn bundle(path: &Path, name: &str, marker: &str) {
        bundle_with_specs(path, &[(name, name, marker)]);
    }

    fn bundle_with_specs(path: &Path, specs: &[(&str, &str, &str)]) {
        let mut payloads = Vec::new();
        let mut skills = Vec::new();
        for (directory_name, document_name, marker) in specs {
            let document = markdown(document_name, marker);
            let helper = format!("echo {marker}\n").into_bytes();
            let files = vec![
                bundle_file("SKILL.md", &document, false),
                bundle_file("scripts/helper.sh", &helper, true),
            ];
            skills.push(BundleSkill {
                directory_name: (*directory_name).into(),
                revision: skill_revision(&files).unwrap(),
                files,
            });
            payloads.push(document);
            payloads.push(helper);
        }
        let manifest = BundleManifest {
            format: BUNDLE_FORMAT.into(),
            format_version: BUNDLE_FORMAT_VERSION,
            agent_contract: AgentContract {
                id: CODEX_CONTRACT_ID.into(),
                version: CODEX_CONTRACT_VERSION,
            },
            skills,
        };
        let mut cursors = payloads.into_iter().map(Cursor::new).collect::<Vec<_>>();
        let mut readers = cursors
            .iter_mut()
            .map(|reader| BundleFileReader { reader })
            .collect::<Vec<_>>();
        write_bundle(File::create(path).unwrap(), &manifest, &mut readers).unwrap();
    }

    fn bundle_file(path: &str, bytes: &[u8], executable: bool) -> BundleFile {
        BundleFile {
            path: path.into(),
            size: bytes.len() as u64,
            sha256: format!("{:x}", Sha256::digest(bytes)),
            executable,
        }
    }

    fn write_catalog_skill(root: &Path, relative: &str, name: &str, marker: &str) -> PathBuf {
        let directory = root.join(relative);
        fs::create_dir_all(directory.join("scripts")).unwrap();
        fs::write(directory.join("SKILL.md"), markdown(name, marker)).unwrap();
        let helper = directory.join("scripts/helper.sh");
        fs::write(&helper, format!("echo {marker}\n")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(helper, fs::Permissions::from_mode(0o755)).unwrap();
        }
        directory
    }

    fn setup() -> (tempfile::TempDir, Workspace, BundleImportManager) {
        let directory = tempfile::tempdir().unwrap();
        let workspace = Workspace::new(directory.path().join("codex"));
        let imports = BundleImportManager::new(directory.path().join("staging")).unwrap();
        (directory, workspace, imports)
    }

    fn stage_review(
        directory: &tempfile::TempDir,
        workspace: &Workspace,
        imports: &BundleImportManager,
        name: &str,
        marker: &str,
    ) -> BundleInstallationReview {
        let source = directory
            .path()
            .join(format!("{name}-{marker}.skillbundle"));
        bundle(&source, name, marker);
        let staged = imports.stage(&source).unwrap();
        imports.classify_staged_review(workspace, staged).unwrap()
    }

    #[test]
    fn classification_preserves_precedence_and_every_catalog_match() {
        let (directory, workspace, imports) = setup();
        let codex = directory.path().join("codex");
        write_catalog_skill(&codex, "skills/.system/demo", "demo", "managed-different");
        write_catalog_skill(&codex, "skills-disabled/demo", "demo", "imported");
        let review = stage_review(&directory, &workspace, &imports, "demo", "imported");
        let decision = &review.decisions[0];
        assert_eq!(decision.classification, "identical");
        assert_eq!(decision.matches.len(), 2);
        assert!(decision.matches.iter().any(|item| item.source == "system"));
        assert!(decision
            .matches
            .iter()
            .any(|item| item.source == "disabled" && item.identical));
        assert!(decision.install_offer.is_none());

        let (directory, workspace, imports) = setup();
        write_catalog_skill(
            &directory.path().join("codex"),
            "skills/.system/demo",
            "demo",
            "managed-different",
        );
        let review = stage_review(&directory, &workspace, &imports, "demo", "imported");
        assert_eq!(review.decisions[0].classification, "managedConflict");
        assert!(review.decisions[0].install_offer.is_none());
    }

    #[test]
    fn new_install_is_incremental_and_retry_is_an_identical_noop() {
        let (directory, workspace, imports) = setup();
        let review = stage_review(&directory, &workspace, &imports, "demo", "imported");
        let offer = review.decisions[0].install_offer.clone().unwrap();
        assert_eq!(review.decisions[0].classification, "new");
        let selection = BundleInstallSelection {
            directory_name: "demo".into(),
            offer_token: offer.token,
        };
        let first = imports
            .install_reviewed(
                &workspace,
                &review.import.session_id,
                &review.import.bundle_revision,
                &review.review_revision,
                std::slice::from_ref(&selection),
            )
            .unwrap();
        assert_eq!(first.outcomes[0].status, "installed");
        assert!(directory
            .path()
            .join("codex/skills/demo/scripts/helper.sh")
            .exists());
        let second = imports
            .install_reviewed(
                &workspace,
                &review.import.session_id,
                &review.import.bundle_revision,
                &review.review_revision,
                &[selection],
            )
            .unwrap();
        assert_eq!(second.outcomes[0].status, "skippedIdentical");
        assert_eq!(workspace.list_skills().unwrap().counts.personal, 1);
    }

    #[test]
    fn late_managed_conflict_stops_all_mutation() {
        let (directory, workspace, imports) = setup();
        let review = stage_review(&directory, &workspace, &imports, "demo", "imported");
        let offer = review.decisions[0].install_offer.clone().unwrap();
        write_catalog_skill(
            &directory.path().join("codex"),
            "skills/.system/demo",
            "demo",
            "late-managed",
        );
        assert!(matches!(
            imports.install_reviewed(
                &workspace,
                &review.import.session_id,
                &review.import.bundle_revision,
                &review.review_revision,
                &[BundleInstallSelection {
                    directory_name: "demo".into(),
                    offer_token: offer.token,
                }],
            ),
            Err(BundleInstallError::StaleReview)
        ));
        assert!(!directory.path().join("codex/skills/demo").exists());
    }

    #[test]
    fn personal_replacement_keeps_dormant_matches_and_supports_file_comparison() {
        let (directory, workspace, imports) = setup();
        let codex = directory.path().join("codex");
        write_catalog_skill(&codex, "skills/demo", "demo", "active-old");
        write_catalog_skill(&codex, "skill-archive/demo", "demo", "archived-old");
        let review = stage_review(&directory, &workspace, &imports, "demo", "imported");
        let decision = &review.decisions[0];
        assert_eq!(decision.classification, "userConflict");
        assert_eq!(decision.matches.len(), 2);
        let offer = decision.install_offer.clone().unwrap();
        assert_eq!(offer.kind, "replacePersonal");
        let active = decision
            .matches
            .iter()
            .find(|item| item.source == "personal")
            .unwrap();
        let comparison = imports
            .compare_installation_file(
                &workspace,
                &review.import.session_id,
                &review.import.bundle_revision,
                "demo",
                &active.id,
                "SKILL.md",
            )
            .unwrap();
        assert_eq!(comparison.status, "modified");
        assert!(comparison
            .imported
            .content
            .as_deref()
            .unwrap()
            .contains("imported"));
        assert!(comparison
            .current
            .content
            .as_deref()
            .unwrap()
            .contains("active-old"));

        let result = imports
            .install_reviewed(
                &workspace,
                &review.import.session_id,
                &review.import.bundle_revision,
                &review.review_revision,
                &[BundleInstallSelection {
                    directory_name: "demo".into(),
                    offer_token: offer.token,
                }],
            )
            .unwrap();
        assert_eq!(result.outcomes[0].status, "replaced");
        assert!(fs::read_to_string(codex.join("skills/demo/SKILL.md"))
            .unwrap()
            .contains("imported"));
        assert!(
            fs::read_to_string(codex.join("skill-archive/demo/SKILL.md"))
                .unwrap()
                .contains("archived-old")
        );
    }

    #[test]
    fn incompatible_and_baseline_blocked_imports_never_receive_offers() {
        let (directory, workspace, imports) = setup();
        let incompatible = directory.path().join("incompatible.skillbundle");
        bundle_with_specs(&incompatible, &[("demo", "different-name", "ordinary")]);
        let staged = imports.stage(&incompatible).unwrap();
        let review = imports.classify_staged_review(&workspace, staged).unwrap();
        assert_eq!(review.decisions[0].classification, "incompatible");
        assert!(review.decisions[0].install_offer.is_none());

        let blocked = directory.path().join("blocked.skillbundle");
        bundle(
            &blocked,
            "blocked-demo",
            "Run `rm -rf ~/Documents/archive` after export.",
        );
        let staged = imports.stage(&blocked).unwrap();
        let review = imports.classify_staged_review(&workspace, staged).unwrap();
        assert!(review.decisions[0].baseline_blocked);
        assert!(review.decisions[0].install_offer.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn unmeasurable_active_personal_target_is_not_replaceable() {
        use std::os::unix::fs::symlink;

        let (directory, workspace, imports) = setup();
        let codex = directory.path().join("codex");
        let active = write_catalog_skill(&codex, "skills/demo", "demo", "old");
        symlink(active.join("SKILL.md"), active.join("linked.md")).unwrap();
        let review = stage_review(&directory, &workspace, &imports, "demo", "imported");
        assert_eq!(review.decisions[0].classification, "userConflict");
        assert!(!review.decisions[0].matches[0].measurement_available);
        assert!(review.decisions[0].install_offer.is_none());
    }

    #[test]
    fn same_name_active_personal_skill_in_another_directory_is_not_duplicated() {
        let (directory, workspace, imports) = setup();
        write_catalog_skill(
            &directory.path().join("codex"),
            "skills/legacy-directory",
            "demo",
            "old",
        );
        let review = stage_review(&directory, &workspace, &imports, "demo", "imported");
        assert_eq!(review.decisions[0].classification, "userConflict");
        assert!(review.decisions[0].summary.contains("位于另一目录"));
        assert!(review.decisions[0].install_offer.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn strict_final_scan_blocks_when_catalog_evidence_is_incomplete() {
        use std::os::unix::fs::symlink;

        let (directory, workspace, imports) = setup();
        let review = stage_review(&directory, &workspace, &imports, "demo", "imported");
        let offer = review.decisions[0].install_offer.clone().unwrap();
        let system_root = directory.path().join("codex/skills/.system");
        fs::create_dir_all(&system_root).unwrap();
        symlink(directory.path(), system_root.join("unreadable-link")).unwrap();
        let result = imports.install_reviewed(
            &workspace,
            &review.import.session_id,
            &review.import.bundle_revision,
            &review.review_revision,
            &[BundleInstallSelection {
                directory_name: "demo".into(),
                offer_token: offer.token,
            }],
        );
        assert!(matches!(
            result,
            Err(BundleInstallError::Workspace(WorkspaceError::UnsafePath))
        ));
        assert!(!directory.path().join("codex/skills/demo").exists());
    }

    #[test]
    fn no_replace_failure_preserves_an_unindexed_obstruction() {
        let (directory, workspace, imports) = setup();
        let review = stage_review(&directory, &workspace, &imports, "demo", "imported");
        let offer = review.decisions[0].install_offer.clone().unwrap();
        let obstruction = directory.path().join("codex/skills/demo");
        fs::create_dir_all(obstruction.parent().unwrap()).unwrap();
        fs::write(&obstruction, b"do not overwrite").unwrap();
        let result = imports
            .install_reviewed(
                &workspace,
                &review.import.session_id,
                &review.import.bundle_revision,
                &review.review_revision,
                &[BundleInstallSelection {
                    directory_name: "demo".into(),
                    offer_token: offer.token,
                }],
            )
            .unwrap();
        assert!(!result.ok);
        assert_eq!(result.outcomes[0].status, "failed");
        assert_eq!(fs::read(&obstruction).unwrap(), b"do not overwrite");
    }

    #[test]
    fn replacement_boundary_mismatch_restores_the_prior_directory() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("skills");
        let source = write_catalog_skill(&root, ".bundle-install-test", "demo", "imported");
        let destination = write_catalog_skill(&root, "demo", "demo", "prior");
        let error =
            replace_personal_directory(&source, &destination, &root, "wrong-revision").unwrap_err();
        assert!(error.error.to_string().contains("已恢复"));
        assert!(fs::read_to_string(destination.join("SKILL.md"))
            .unwrap()
            .contains("prior"));
        assert!(fs::read_to_string(source.join("SKILL.md"))
            .unwrap()
            .contains("imported"));
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn failed_atomic_restore_marks_the_prior_directory_for_retention() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("skills");
        fs::create_dir_all(&root).unwrap();
        let temporary = Builder::new()
            .prefix(".bundle-install-test-")
            .tempdir_in(&root)
            .unwrap();
        write_catalog_skill(temporary.path(), "", "demo", "imported");
        let source = temporary.path().to_path_buf();
        let destination = write_catalog_skill(&root, "demo", "demo", "prior");
        let mut calls = 0;
        let failure = replace_personal_directory_atomic(
            &source,
            &destination,
            "wrong-revision",
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
        let retained = temporary.keep();
        assert_eq!(retained, source);
        assert!(fs::read_to_string(source.join("SKILL.md"))
            .unwrap()
            .contains("prior"));
        assert!(fs::read_to_string(destination.join("SKILL.md"))
            .unwrap()
            .contains("imported"));
    }

    #[test]
    fn multi_skill_receipt_preserves_success_before_a_late_no_replace_failure() {
        let (directory, workspace, imports) = setup();
        let source = directory.path().join("multiple.skillbundle");
        bundle_with_specs(
            &source,
            &[("alpha", "alpha", "one"), ("beta", "beta", "two")],
        );
        let staged = imports.stage(&source).unwrap();
        let review = imports.classify_staged_review(&workspace, staged).unwrap();
        let selections = review
            .decisions
            .iter()
            .map(|decision| BundleInstallSelection {
                directory_name: decision.directory_name.clone(),
                offer_token: decision.install_offer.as_ref().unwrap().token.clone(),
            })
            .collect::<Vec<_>>();
        let obstruction = directory.path().join("codex/skills/beta");
        fs::create_dir_all(obstruction.parent().unwrap()).unwrap();
        fs::write(&obstruction, b"existing file").unwrap();
        let result = imports
            .install_reviewed(
                &workspace,
                &review.import.session_id,
                &review.import.bundle_revision,
                &review.review_revision,
                &selections,
            )
            .unwrap();
        assert!(!result.ok);
        assert_eq!(result.outcomes[0].directory_name, "alpha");
        assert_eq!(result.outcomes[0].status, "installed");
        assert_eq!(result.outcomes[1].directory_name, "beta");
        assert_eq!(result.outcomes[1].status, "failed");
        assert!(directory
            .path()
            .join("codex/skills/alpha/SKILL.md")
            .exists());
        assert_eq!(fs::read(obstruction).unwrap(), b"existing file");
    }

    #[test]
    fn review_reuses_catalog_and_install_performs_one_strict_fresh_scan() {
        let (directory, workspace, imports) = setup();
        let review = stage_review(&directory, &workspace, &imports, "demo", "imported");
        assert_eq!(workspace.metrics_snapshot().full_scans, 1);
        let repeated = imports
            .review_installation(
                &workspace,
                &review.import.session_id,
                &review.import.bundle_revision,
            )
            .unwrap();
        assert_eq!(workspace.metrics_snapshot().full_scans, 1);
        let offer = repeated.decisions[0].install_offer.clone().unwrap();
        imports
            .install_reviewed(
                &workspace,
                &repeated.import.session_id,
                &repeated.import.bundle_revision,
                &repeated.review_revision,
                &[BundleInstallSelection {
                    directory_name: "demo".into(),
                    offer_token: offer.token,
                }],
            )
            .unwrap();
        assert_eq!(workspace.metrics_snapshot().full_scans, 2);
    }

    #[cfg(unix)]
    #[test]
    fn installation_preserves_declared_executable_modes() {
        use std::os::unix::fs::PermissionsExt;

        let (directory, workspace, imports) = setup();
        let review = stage_review(&directory, &workspace, &imports, "demo", "imported");
        let offer = review.decisions[0].install_offer.clone().unwrap();
        imports
            .install_reviewed(
                &workspace,
                &review.import.session_id,
                &review.import.bundle_revision,
                &review.review_revision,
                &[BundleInstallSelection {
                    directory_name: "demo".into(),
                    offer_token: offer.token,
                }],
            )
            .unwrap();
        let root = directory.path().join("codex/skills/demo");
        assert_eq!(
            fs::metadata(root.join("SKILL.md"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o644
        );
        assert_eq!(
            fs::metadata(root.join("scripts/helper.sh"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
    }
}
