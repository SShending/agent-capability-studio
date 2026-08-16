mod skills;

use serde::{Deserialize, Serialize};
use skills::{
    AuditResult, BundleExportError, BundleExportPlan, BundleExportReceipt, BundleFileComparison,
    BundleImportError, BundleImportFileContent, BundleImportManager, BundleInstallError,
    BundleInstallResult, BundleInstallSelection, BundleInstallationReview, CandidateFileContent,
    CandidateFileSyncAction, CandidateFileSyncOperation, CandidateInstallPreview,
    CandidateInstallResult, CandidateManifest, CandidateReview, CandidateStager, Catalog,
    CollectionManager, CollectionSnapshot, CreateSkillResult, DeepAuditApiMode,
    DeepAuditConnectionResult, DeepAuditManager, DeepAuditPreview, DeepAuditResult,
    DeepAuditSelection, DeepAuditSettings, DeleteSkillResult, GithubRepositoryListing,
    GithubUpdateCheck, GithubUpdateError, LifecyclePreview, LifecycleResult, NewSkillPreview,
    PackageFileContent, PackageImportSource, PackageMutation, PackagePreview, PackageSaveResult,
    PackageSnapshot, ProvenanceManager, SkillDetail, Workspace, WorkspaceError,
};
use tauri::{
    menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder, SubmenuBuilder},
    Emitter, Manager, State,
};

const MENU_SETTINGS: &str = "studio-settings";
const MENU_LANGUAGE_ZH_CN: &str = "studio-language-zh-cn";
const MENU_LANGUAGE_EN: &str = "studio-language-en";

struct MenuLabels {
    settings: &'static str,
    language: &'static str,
    edit: &'static str,
    window: &'static str,
    about: &'static str,
    services: &'static str,
    hide: &'static str,
    hide_others: &'static str,
    show_all: &'static str,
    quit: &'static str,
    undo: &'static str,
    redo: &'static str,
    cut: &'static str,
    copy: &'static str,
    paste: &'static str,
    select_all: &'static str,
    minimize: &'static str,
    zoom: &'static str,
    fullscreen: &'static str,
    close: &'static str,
}

fn menu_labels(locale: &str) -> MenuLabels {
    if locale == "en" {
        MenuLabels {
            settings: "Settings…",
            language: "Interface Language",
            edit: "Edit",
            window: "Window",
            about: "About Agent Skill Studio",
            services: "Services",
            hide: "Hide Agent Skill Studio",
            hide_others: "Hide Others",
            show_all: "Show All",
            quit: "Quit Agent Skill Studio",
            undo: "Undo",
            redo: "Redo",
            cut: "Cut",
            copy: "Copy",
            paste: "Paste",
            select_all: "Select All",
            minimize: "Minimize",
            zoom: "Zoom",
            fullscreen: "Enter Full Screen",
            close: "Close Window",
        }
    } else {
        MenuLabels {
            settings: "设置…",
            language: "界面语言",
            edit: "编辑",
            window: "窗口",
            about: "关于 Agent Skill Studio",
            services: "服务",
            hide: "隐藏 Agent Skill Studio",
            hide_others: "隐藏其他",
            show_all: "全部显示",
            quit: "退出 Agent Skill Studio",
            undo: "撤销",
            redo: "重做",
            cut: "剪切",
            copy: "复制",
            paste: "粘贴",
            select_all: "全选",
            minimize: "最小化",
            zoom: "缩放",
            fullscreen: "进入全屏幕",
            close: "关闭窗口",
        }
    }
}

fn install_interface_menu(app: &tauri::AppHandle, locale: &str) -> tauri::Result<()> {
    let labels = menu_labels(locale);
    let settings = MenuItemBuilder::with_id(MENU_SETTINGS, labels.settings)
        .accelerator("CmdOrCtrl+,")
        .build(app)?;
    let chinese = CheckMenuItemBuilder::with_id(MENU_LANGUAGE_ZH_CN, "简体中文")
        .checked(locale != "en")
        .build(app)?;
    let english = CheckMenuItemBuilder::with_id(MENU_LANGUAGE_EN, "English")
        .checked(locale == "en")
        .build(app)?;
    let language = SubmenuBuilder::new(app, labels.language)
        .items(&[&chinese, &english])
        .build()?;
    let application = SubmenuBuilder::new(app, "Agent Skill Studio")
        .about_with_text(labels.about, None)
        .separator()
        .item(&settings)
        .item(&language)
        .separator()
        .services_with_text(labels.services)
        .separator()
        .hide_with_text(labels.hide)
        .hide_others_with_text(labels.hide_others)
        .show_all_with_text(labels.show_all)
        .separator()
        .quit_with_text(labels.quit)
        .build()?;
    let edit = SubmenuBuilder::new(app, labels.edit)
        .undo_with_text(labels.undo)
        .redo_with_text(labels.redo)
        .separator()
        .cut_with_text(labels.cut)
        .copy_with_text(labels.copy)
        .paste_with_text(labels.paste)
        .select_all_with_text(labels.select_all)
        .build()?;
    let window = SubmenuBuilder::new(app, labels.window)
        .minimize_with_text(labels.minimize)
        .maximize_with_text(labels.zoom)
        .fullscreen_with_text(labels.fullscreen)
        .separator()
        .close_window_with_text(labels.close)
        .build()?;
    let menu = MenuBuilder::new(app)
        .items(&[&application, &edit, &window])
        .build()?;
    app.set_menu(menu)?;
    Ok(())
}

struct AppState {
    workspace: Workspace,
    collections: CollectionManager,
    provenance: ProvenanceManager,
    deep_audit: DeepAuditManager,
    candidates: CandidateStager,
    bundle_imports: BundleImportManager,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LifecycleCommandResult {
    #[serde(flatten)]
    result: LifecycleResult,
    collections: CollectionSnapshot,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeleteLifecycleCommandResult {
    #[serde(flatten)]
    result: DeleteSkillResult,
    collections: CollectionSnapshot,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandError {
    code: String,
    message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CandidateFileSyncApplyRequest {
    id: String,
    session_id: String,
    expected_candidate_hash: String,
    expected_local_revision: String,
    expected_proposed_revision: String,
    path: String,
    action: CandidateFileSyncAction,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkillPackageDeepAuditRequest {
    id: String,
    expected_revision: String,
    expected_proposed_revision: String,
    mutations: Vec<PackageMutation>,
    selections: Vec<DeepAuditSelection>,
    expected_candidate_hash: String,
    expected_provider_hash: String,
}

impl From<WorkspaceError> for CommandError {
    fn from(error: WorkspaceError) -> Self {
        Self {
            code: error.code().to_string(),
            message: error.to_string(),
        }
    }
}

impl From<skills::CollectionsError> for CommandError {
    fn from(error: skills::CollectionsError) -> Self {
        Self {
            code: error.code().to_string(),
            message: error.to_string(),
        }
    }
}

impl From<skills::ProvenanceError> for CommandError {
    fn from(error: skills::ProvenanceError) -> Self {
        Self {
            code: error.code().to_string(),
            message: error.to_string(),
        }
    }
}

impl From<skills::DeepAuditError> for CommandError {
    fn from(error: skills::DeepAuditError) -> Self {
        Self {
            code: error.code().to_string(),
            message: error.to_string(),
        }
    }
}

impl From<skills::CandidateError> for CommandError {
    fn from(error: skills::CandidateError) -> Self {
        Self {
            code: error.code().to_string(),
            message: error.to_string(),
        }
    }
}

impl From<skills::CandidateInstallError> for CommandError {
    fn from(error: skills::CandidateInstallError) -> Self {
        Self {
            code: error.code().to_string(),
            message: error.to_string(),
        }
    }
}

impl From<GithubUpdateError> for CommandError {
    fn from(error: GithubUpdateError) -> Self {
        Self {
            code: error.code().to_string(),
            message: error.to_string(),
        }
    }
}

impl From<BundleExportError> for CommandError {
    fn from(error: BundleExportError) -> Self {
        Self {
            code: error.code().to_string(),
            message: error.to_string(),
        }
    }
}

impl From<BundleImportError> for CommandError {
    fn from(error: BundleImportError) -> Self {
        Self {
            code: error.code().to_string(),
            message: error.to_string(),
        }
    }
}

impl From<BundleInstallError> for CommandError {
    fn from(error: BundleInstallError) -> Self {
        Self {
            code: error.code().to_string(),
            message: error.to_string(),
        }
    }
}

#[tauri::command]
fn set_interface_locale(locale: String, app: tauri::AppHandle) -> Result<(), String> {
    if locale != "zh-CN" && locale != "en" {
        return Err("unsupported interface locale".into());
    }
    install_interface_menu(&app, &locale).map_err(|error| error.to_string())
}

#[tauri::command]
fn list_skills(state: State<'_, AppState>) -> Result<Catalog, CommandError> {
    let mut catalog = state.workspace.list_skills()?;
    let _ = state.provenance.attach_catalog(&mut catalog);
    Ok(catalog)
}

#[tauri::command]
fn refresh_skills(state: State<'_, AppState>) -> Result<Catalog, CommandError> {
    let mut catalog = state.workspace.refresh_skills()?;
    let _ = state.provenance.attach_catalog(&mut catalog);
    Ok(catalog)
}

#[tauri::command]
fn get_skill(id: String, state: State<'_, AppState>) -> Result<SkillDetail, CommandError> {
    let mut detail = state.workspace.get_skill(&id)?;
    let _ = state.provenance.attach_detail(&mut detail);
    Ok(detail)
}

#[tauri::command]
fn get_skill_package(
    id: String,
    state: State<'_, AppState>,
) -> Result<PackageSnapshot, CommandError> {
    state.workspace.get_skill_package(&id).map_err(Into::into)
}

#[tauri::command]
fn read_skill_package_file(
    id: String,
    expected_revision: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<PackageFileContent, CommandError> {
    state
        .workspace
        .read_skill_package_file(&id, &expected_revision, &path)
        .map_err(Into::into)
}

#[tauri::command]
fn inspect_package_import_source(
    selected_path: String,
    state: State<'_, AppState>,
) -> Result<PackageImportSource, CommandError> {
    state
        .workspace
        .inspect_package_import_source(std::path::Path::new(&selected_path))
        .map_err(Into::into)
}

#[tauri::command]
fn preview_skill_package(
    id: String,
    expected_revision: String,
    mutations: Vec<PackageMutation>,
    state: State<'_, AppState>,
) -> Result<PackagePreview, CommandError> {
    state
        .workspace
        .preview_skill_package(&id, &expected_revision, &mutations)
        .map_err(Into::into)
}

#[tauri::command]
fn save_skill_package(
    id: String,
    expected_revision: String,
    expected_proposed_revision: String,
    mutations: Vec<PackageMutation>,
    state: State<'_, AppState>,
) -> Result<PackageSaveResult, CommandError> {
    let mut result = state.workspace.save_skill_package(
        &id,
        &expected_revision,
        &expected_proposed_revision,
        &mutations,
    )?;
    let _ = state.provenance.attach_detail(&mut result.skill);
    Ok(result)
}

#[tauri::command]
fn preview_skill_package_deep_audit(
    id: String,
    expected_revision: String,
    expected_proposed_revision: String,
    mutations: Vec<PackageMutation>,
    state: State<'_, AppState>,
) -> Result<DeepAuditPreview, CommandError> {
    state
        .deep_audit
        .preview_skill_package(
            &state.workspace,
            &id,
            &expected_revision,
            &expected_proposed_revision,
            &mutations,
        )
        .map_err(Into::into)
}

#[tauri::command]
async fn run_skill_package_deep_audit(
    request: SkillPackageDeepAuditRequest,
    state: State<'_, AppState>,
) -> Result<DeepAuditResult, CommandError> {
    let workspace = state.workspace.clone();
    let manager = state.deep_audit.clone();
    tauri::async_runtime::spawn_blocking(move || {
        manager.run_skill_package(
            &workspace,
            &request.id,
            &request.expected_revision,
            &request.expected_proposed_revision,
            &request.mutations,
            &request.selections,
            &request.expected_candidate_hash,
            &request.expected_provider_hash,
        )
    })
    .await
    .map_err(|_| CommandError {
        code: "DEEP_AUDIT_TASK_ERROR".into(),
        message: "The Package Deep Audit task stopped before completion.".into(),
    })?
    .map_err(Into::into)
}

#[tauri::command]
fn list_collections(state: State<'_, AppState>) -> Result<CollectionSnapshot, CommandError> {
    let known_skill_ids = state
        .workspace
        .list_skills()?
        .skills
        .into_iter()
        .map(|skill| skill.id)
        .collect::<Vec<_>>();
    state.collections.list(&known_skill_ids).map_err(Into::into)
}

#[tauri::command]
fn create_collection(
    name: String,
    state: State<'_, AppState>,
) -> Result<CollectionSnapshot, CommandError> {
    state.collections.create(&name).map_err(Into::into)
}

#[tauri::command]
fn rename_collection(
    id: String,
    name: String,
    state: State<'_, AppState>,
) -> Result<CollectionSnapshot, CommandError> {
    state.collections.rename(&id, &name).map_err(Into::into)
}

#[tauri::command]
fn delete_collection(
    id: String,
    state: State<'_, AppState>,
) -> Result<CollectionSnapshot, CommandError> {
    state.collections.delete(&id).map_err(Into::into)
}

#[tauri::command]
fn set_skill_collection_memberships(
    skill_id: String,
    collection_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<CollectionSnapshot, CommandError> {
    state
        .collections
        .set_skill_memberships(&skill_id, &collection_ids)
        .map_err(Into::into)
}

#[tauri::command]
fn audit_draft(
    id: String,
    markdown: String,
    state: State<'_, AppState>,
) -> Result<AuditResult, CommandError> {
    state
        .workspace
        .audit_draft(&id, &markdown)
        .map_err(Into::into)
}

#[tauri::command]
fn save_draft(
    id: String,
    markdown: String,
    expected_hash: String,
    state: State<'_, AppState>,
) -> Result<skills::SaveResult, CommandError> {
    state
        .workspace
        .save_draft(&id, &markdown, &expected_hash)
        .map_err(Into::into)
}

#[tauri::command]
fn preview_new_skill(
    markdown: String,
    state: State<'_, AppState>,
) -> Result<NewSkillPreview, CommandError> {
    state
        .workspace
        .preview_new_skill(&markdown)
        .map_err(Into::into)
}

#[tauri::command]
fn create_skill(
    markdown: String,
    expected_draft_hash: String,
    state: State<'_, AppState>,
) -> Result<CreateSkillResult, CommandError> {
    state
        .workspace
        .create_skill(&markdown, &expected_draft_hash)
        .map_err(Into::into)
}

#[tauri::command]
fn preview_skill_lifecycle(
    id: String,
    action: String,
    state: State<'_, AppState>,
) -> Result<LifecyclePreview, CommandError> {
    state
        .workspace
        .preview_skill_lifecycle(&id, &action)
        .map_err(Into::into)
}

#[tauri::command]
fn apply_skill_lifecycle(
    id: String,
    action: String,
    expected_directory_revision: String,
    state: State<'_, AppState>,
) -> Result<LifecycleCommandResult, CommandError> {
    apply_lifecycle_workflow(
        &state.workspace,
        &state.provenance,
        &state.collections,
        &id,
        &action,
        &expected_directory_revision,
    )
    .map_err(Into::into)
}

fn apply_lifecycle_workflow(
    workspace: &Workspace,
    provenance: &ProvenanceManager,
    collections: &CollectionManager,
    id: &str,
    action: &str,
    expected_directory_revision: &str,
) -> Result<LifecycleCommandResult, WorkspaceError> {
    let mut collection_snapshot = None;
    let result = workspace.apply_skill_lifecycle_with_finalize(
        id,
        action,
        expected_directory_revision,
        |previous_id, next_id, detail| {
            provenance
                .replace_skill(previous_id, next_id)
                .map_err(|error| WorkspaceError::LifecycleMetadata(error.to_string()))?;
            let snapshot = match collections.replace_member(previous_id, Some(next_id)) {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    return match provenance.replace_skill(next_id, previous_id) {
                        Ok(()) => Err(WorkspaceError::LifecycleMetadata(error.to_string())),
                        Err(rollback) => Err(WorkspaceError::LifecycleRecoveryFailed(format!(
                            "{error}; provenance restore failed: {rollback}"
                        ))),
                    };
                }
            };
            if let Err(error) = provenance.attach_detail(detail) {
                let collection_rollback = collections.replace_member(next_id, Some(previous_id));
                let provenance_rollback = provenance.replace_skill(next_id, previous_id);
                if collection_rollback.is_err() || provenance_rollback.is_err() {
                    return Err(WorkspaceError::LifecycleRecoveryFailed(format!(
                        "{error}; Studio metadata restore was incomplete"
                    )));
                }
                return Err(WorkspaceError::LifecycleMetadata(error.to_string()));
            }
            collection_snapshot = Some(snapshot);
            Ok(())
        },
    )?;
    Ok(LifecycleCommandResult {
        result,
        collections: collection_snapshot.unwrap_or(CollectionSnapshot {
            collections: Vec::new(),
        }),
    })
}

#[tauri::command]
fn delete_archived_skill(
    id: String,
    expected_directory_revision: String,
    confirmation_name: String,
    state: State<'_, AppState>,
) -> Result<DeleteLifecycleCommandResult, CommandError> {
    delete_lifecycle_workflow(
        &state.workspace,
        &state.provenance,
        &state.collections,
        &id,
        &expected_directory_revision,
        &confirmation_name,
    )
    .map_err(Into::into)
}

fn delete_lifecycle_workflow(
    workspace: &Workspace,
    provenance: &ProvenanceManager,
    collections: &CollectionManager,
    id: &str,
    expected_directory_revision: &str,
    confirmation_name: &str,
) -> Result<DeleteLifecycleCommandResult, WorkspaceError> {
    let acquisition = provenance
        .recorded_acquisition(id)
        .map_err(|error| WorkspaceError::LifecycleMetadata(error.to_string()))?;
    let memberships = collections
        .memberships_for(id)
        .map_err(|error| WorkspaceError::LifecycleMetadata(error.to_string()))?;
    let collection_snapshot = std::cell::RefCell::new(None);
    let result = workspace.delete_archived_skill_with_finalize(
        id,
        expected_directory_revision,
        confirmation_name,
        || {
            provenance
                .remove_skill(id)
                .map_err(|error| WorkspaceError::LifecycleMetadata(error.to_string()))?;
            match collections.replace_member(id, None) {
                Ok(snapshot) => {
                    *collection_snapshot.borrow_mut() = Some(snapshot);
                    Ok(())
                }
                Err(error) => {
                    if let Some(acquisition) = &acquisition {
                        provenance
                            .restore_skill(id, acquisition)
                            .map_err(|rollback| {
                                WorkspaceError::LifecycleRecoveryFailed(format!(
                                    "{error}; provenance restore failed: {rollback}"
                                ))
                            })?;
                    }
                    Err(WorkspaceError::LifecycleMetadata(error.to_string()))
                }
            }
        },
        || {
            if let Some(acquisition) = &acquisition {
                provenance
                    .restore_skill(id, acquisition)
                    .map_err(|error| WorkspaceError::LifecycleMetadata(error.to_string()))?;
            }
            collections
                .set_skill_memberships(id, &memberships)
                .map_err(|error| WorkspaceError::LifecycleMetadata(error.to_string()))?;
            Ok(())
        },
    )?;
    Ok(DeleteLifecycleCommandResult {
        result,
        collections: collection_snapshot
            .into_inner()
            .unwrap_or(CollectionSnapshot {
                collections: Vec::new(),
            }),
    })
}

#[tauri::command]
async fn preview_bundle_export(
    skill_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<BundleExportPlan, CommandError> {
    let workspace = state.workspace.clone();
    tauri::async_runtime::spawn_blocking(move || workspace.preview_bundle_export(&skill_ids))
        .await
        .map_err(|_| CommandError {
            code: "BUNDLE_EXPORT_TASK_ERROR".into(),
            message: "The Bundle export preview stopped before completion.".into(),
        })?
        .map_err(Into::into)
}

#[tauri::command]
async fn export_skill_bundle(
    expected_plan_revision: String,
    destination: String,
    state: State<'_, AppState>,
) -> Result<BundleExportReceipt, CommandError> {
    let workspace = state.workspace.clone();
    tauri::async_runtime::spawn_blocking(move || {
        workspace.export_skill_bundle(&expected_plan_revision, std::path::Path::new(&destination))
    })
    .await
    .map_err(|_| CommandError {
        code: "BUNDLE_EXPORT_TASK_ERROR".into(),
        message: "The Bundle export task stopped before completion.".into(),
    })?
    .map_err(Into::into)
}

#[tauri::command]
async fn stage_skill_bundle(
    selected_path: String,
    state: State<'_, AppState>,
) -> Result<BundleInstallationReview, CommandError> {
    let imports = state.bundle_imports.clone();
    let workspace = state.workspace.clone();
    tauri::async_runtime::spawn_blocking(
        move || -> Result<BundleInstallationReview, BundleInstallError> {
            let review = imports.stage(std::path::Path::new(&selected_path))?;
            imports.classify_staged_review(&workspace, review)
        },
    )
    .await
    .map_err(|_| CommandError {
        code: "BUNDLE_IMPORT_TASK_ERROR".into(),
        message: "The Bundle import task stopped before completion.".into(),
    })?
    .map_err(Into::into)
}

#[tauri::command]
async fn review_imported_bundle(
    session_id: String,
    expected_bundle_revision: String,
    state: State<'_, AppState>,
) -> Result<BundleInstallationReview, CommandError> {
    let imports = state.bundle_imports.clone();
    let workspace = state.workspace.clone();
    tauri::async_runtime::spawn_blocking(move || {
        imports.review_installation(&workspace, &session_id, &expected_bundle_revision)
    })
    .await
    .map_err(|_| CommandError {
        code: "BUNDLE_INSTALL_TASK_ERROR".into(),
        message: "The Bundle installation review stopped before completion.".into(),
    })?
    .map_err(Into::into)
}

#[tauri::command]
async fn compare_imported_bundle_file(
    session_id: String,
    expected_bundle_revision: String,
    directory_name: String,
    match_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<BundleFileComparison, CommandError> {
    let imports = state.bundle_imports.clone();
    let workspace = state.workspace.clone();
    tauri::async_runtime::spawn_blocking(move || {
        imports.compare_installation_file(
            &workspace,
            &session_id,
            &expected_bundle_revision,
            &directory_name,
            &match_id,
            &path,
        )
    })
    .await
    .map_err(|_| CommandError {
        code: "BUNDLE_INSTALL_TASK_ERROR".into(),
        message: "The Bundle file comparison stopped before completion.".into(),
    })?
    .map_err(Into::into)
}

#[tauri::command]
async fn install_imported_bundle(
    session_id: String,
    expected_bundle_revision: String,
    expected_review_revision: String,
    selections: Vec<BundleInstallSelection>,
    state: State<'_, AppState>,
) -> Result<BundleInstallResult, CommandError> {
    let imports = state.bundle_imports.clone();
    let workspace = state.workspace.clone();
    tauri::async_runtime::spawn_blocking(move || {
        imports.install_reviewed(
            &workspace,
            &session_id,
            &expected_bundle_revision,
            &expected_review_revision,
            &selections,
        )
    })
    .await
    .map_err(|_| CommandError {
        code: "BUNDLE_INSTALL_TASK_ERROR".into(),
        message: "The Bundle installation task stopped before completion.".into(),
    })?
    .map_err(Into::into)
}

#[tauri::command]
async fn read_imported_bundle_file(
    session_id: String,
    expected_bundle_revision: String,
    directory_name: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<BundleImportFileContent, CommandError> {
    let imports = state.bundle_imports.clone();
    tauri::async_runtime::spawn_blocking(move || {
        imports.read_file(
            &session_id,
            &expected_bundle_revision,
            &directory_name,
            &path,
        )
    })
    .await
    .map_err(|_| CommandError {
        code: "BUNDLE_IMPORT_TASK_ERROR".into(),
        message: "The staged Bundle file read stopped before completion.".into(),
    })?
    .map_err(Into::into)
}

#[tauri::command]
fn discard_imported_bundle(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    state
        .bundle_imports
        .discard(&session_id)
        .map_err(Into::into)
}

#[tauri::command]
async fn stage_github_candidate(
    source_url: String,
    state: State<'_, AppState>,
) -> Result<CandidateManifest, CommandError> {
    let stager = state.candidates.clone();
    tauri::async_runtime::spawn_blocking(move || stager.stage_github(&source_url))
        .await
        .map_err(|_| CommandError {
            code: "CANDIDATE_TASK_ERROR".into(),
            message: "The candidate acquisition task stopped before completion.".into(),
        })?
        .map_err(Into::into)
}

#[tauri::command]
async fn list_github_repository_candidates(
    source_url: String,
    state: State<'_, AppState>,
) -> Result<GithubRepositoryListing, CommandError> {
    let stager = state.candidates.clone();
    tauri::async_runtime::spawn_blocking(move || stager.list_github_repository(&source_url))
        .await
        .map_err(|_| CommandError {
            code: "CANDIDATE_TASK_ERROR".into(),
            message: "The repository discovery task stopped before completion.".into(),
        })?
        .map_err(Into::into)
}

#[tauri::command]
async fn stage_github_repository_candidate(
    source_url: String,
    requested_ref: String,
    resolved_sha: String,
    skill_path: String,
    state: State<'_, AppState>,
) -> Result<CandidateManifest, CommandError> {
    let stager = state.candidates.clone();
    tauri::async_runtime::spawn_blocking(move || {
        stager.stage_github_repository_candidate(
            &source_url,
            &requested_ref,
            &resolved_sha,
            &skill_path,
        )
    })
    .await
    .map_err(|_| CommandError {
        code: "CANDIDATE_TASK_ERROR".into(),
        message: "The repository candidate acquisition task stopped before completion.".into(),
    })?
    .map_err(Into::into)
}

#[tauri::command]
async fn check_github_skill_update(
    id: String,
    state: State<'_, AppState>,
) -> Result<GithubUpdateCheck, CommandError> {
    let workspace = state.workspace.clone();
    let stager = state.candidates.clone();
    let acquisition = state.provenance.acquisition(&id)?;
    tauri::async_runtime::spawn_blocking(move || {
        stager.check_github_update(&workspace, &id, &acquisition)
    })
    .await
    .map_err(|_| CommandError {
        code: "CANDIDATE_TASK_ERROR".into(),
        message: "The GitHub update check stopped before completion.".into(),
    })?
    .map_err(Into::into)
}

#[tauri::command]
async fn stage_local_candidate(
    selected_path: String,
    state: State<'_, AppState>,
) -> Result<CandidateManifest, CommandError> {
    let stager = state.candidates.clone();
    tauri::async_runtime::spawn_blocking(move || {
        stager.stage_local(std::path::Path::new(&selected_path))
    })
    .await
    .map_err(|_| CommandError {
        code: "CANDIDATE_TASK_ERROR".into(),
        message: "The candidate acquisition task stopped before completion.".into(),
    })?
    .map_err(Into::into)
}

#[tauri::command]
fn discard_staged_candidate(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    state.candidates.discard(&session_id).map_err(Into::into)
}

#[tauri::command]
fn get_deep_audit_settings(state: State<'_, AppState>) -> Result<DeepAuditSettings, CommandError> {
    state.deep_audit.settings().map_err(Into::into)
}

#[tauri::command]
fn save_deep_audit_settings(
    api_mode: DeepAuditApiMode,
    endpoint: String,
    model: String,
    api_key: Option<String>,
    state: State<'_, AppState>,
) -> Result<DeepAuditSettings, CommandError> {
    state
        .deep_audit
        .save_settings(api_mode, &endpoint, &model, api_key.as_deref())
        .map_err(Into::into)
}

#[tauri::command]
async fn get_staged_candidate_review(
    session_id: String,
    expected_candidate_hash: String,
    state: State<'_, AppState>,
) -> Result<CandidateReview, CommandError> {
    let stager = state.candidates.clone();
    tauri::async_runtime::spawn_blocking(move || {
        stager.review(&session_id, &expected_candidate_hash)
    })
    .await
    .map_err(|_| CommandError {
        code: "CANDIDATE_TASK_ERROR".into(),
        message: "The candidate review task stopped before completion.".into(),
    })?
    .map_err(Into::into)
}

#[tauri::command]
async fn read_staged_candidate_file(
    session_id: String,
    expected_candidate_hash: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<CandidateFileContent, CommandError> {
    let stager = state.candidates.clone();
    tauri::async_runtime::spawn_blocking(move || {
        stager.read_file(&session_id, &expected_candidate_hash, &path)
    })
    .await
    .map_err(|_| CommandError {
        code: "CANDIDATE_TASK_ERROR".into(),
        message: "The candidate file preview task stopped before completion.".into(),
    })?
    .map_err(Into::into)
}

#[tauri::command]
async fn preview_staged_candidate_file_sync(
    id: String,
    session_id: String,
    expected_candidate_hash: String,
    expected_local_revision: String,
    path: String,
    action: CandidateFileSyncAction,
    state: State<'_, AppState>,
) -> Result<PackagePreview, CommandError> {
    let workspace = state.workspace.clone();
    let stager = state.candidates.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let remote = stager.file_sync_data(&session_id, &expected_candidate_hash, &path, action)?;
        workspace
            .preview_candidate_file_sync(
                &id,
                &expected_local_revision,
                &path,
                CandidateFileSyncOperation::new(
                    action,
                    remote
                        .as_ref()
                        .map(|(bytes, executable)| (bytes.as_slice(), *executable)),
                ),
            )
            .map_err(CommandError::from)
    })
    .await
    .map_err(|_| CommandError {
        code: "CANDIDATE_TASK_ERROR".into(),
        message: "The candidate file synchronization preview stopped before completion.".into(),
    })?
}

#[tauri::command]
async fn apply_staged_candidate_file_sync(
    request: CandidateFileSyncApplyRequest,
    state: State<'_, AppState>,
) -> Result<PackageSaveResult, CommandError> {
    let workspace = state.workspace.clone();
    let stager = state.candidates.clone();
    let provenance = state.provenance.clone();
    tauri::async_runtime::spawn_blocking(move || {
        apply_candidate_file_sync_workflow(&workspace, &stager, &provenance, request)
    })
    .await
    .map_err(|_| CommandError {
        code: "CANDIDATE_TASK_ERROR".into(),
        message: "The candidate file synchronization stopped before completion.".into(),
    })?
}

fn apply_candidate_file_sync_workflow(
    workspace: &Workspace,
    stager: &CandidateStager,
    provenance: &ProvenanceManager,
    request: CandidateFileSyncApplyRequest,
) -> Result<PackageSaveResult, CommandError> {
    let source = stager.source(&request.session_id, &request.expected_candidate_hash)?;
    let remote = stager.file_sync_data(
        &request.session_id,
        &request.expected_candidate_hash,
        &request.path,
        request.action,
    )?;
    let mut result = workspace
        .apply_candidate_file_sync_with_finalize(
            &request.id,
            &request.expected_local_revision,
            &request.expected_proposed_revision,
            &request.path,
            CandidateFileSyncOperation::new(
                request.action,
                remote
                    .as_ref()
                    .map(|(bytes, executable)| (bytes.as_slice(), *executable)),
            ),
            |destination| {
                let complete_match = stager
                    .directory_matches(
                        &request.session_id,
                        &request.expected_candidate_hash,
                        destination,
                    )
                    .map_err(|error| {
                        WorkspaceError::Io(std::io::Error::other(error.to_string()))
                    })?;
                if complete_match {
                    provenance
                        .record_candidate(&request.id, &request.expected_candidate_hash, source)
                        .map_err(|error| {
                            WorkspaceError::Io(std::io::Error::other(error.to_string()))
                        })?;
                }
                Ok(())
            },
        )
        .map_err(CommandError::from)?;
    let _ = provenance.attach_detail(&mut result.skill);
    Ok(result)
}

#[tauri::command]
async fn preview_staged_candidate_install(
    session_id: String,
    expected_candidate_hash: String,
    state: State<'_, AppState>,
) -> Result<CandidateInstallPreview, CommandError> {
    let workspace = state.workspace.clone();
    let stager = state.candidates.clone();
    tauri::async_runtime::spawn_blocking(move || {
        stager.preview_install(&workspace, &session_id, &expected_candidate_hash)
    })
    .await
    .map_err(|_| CommandError {
        code: "CANDIDATE_TASK_ERROR".into(),
        message: "The candidate installation preview stopped before completion.".into(),
    })?
    .map_err(Into::into)
}

#[tauri::command]
async fn install_staged_candidate(
    session_id: String,
    expected_candidate_hash: String,
    expected_install_revision: String,
    state: State<'_, AppState>,
) -> Result<CandidateInstallResult, CommandError> {
    let workspace = state.workspace.clone();
    let stager = state.candidates.clone();
    let provenance = state.provenance.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let source = stager.source(&session_id, &expected_candidate_hash)?;
        let mut result = stager.install(
            &workspace,
            &session_id,
            &expected_candidate_hash,
            &expected_install_revision,
        )?;
        if result.status == "installed" {
            if let Ok(acquisition) =
                provenance.record_candidate(&result.installed_id, &result.candidate_hash, source)
            {
                result.provenance_recorded = true;
                if let Some(skill) = &mut result.skill {
                    skill.summary.acquisition = acquisition;
                }
            }
        }
        Ok::<_, skills::CandidateInstallError>(result)
    })
    .await
    .map_err(|_| CommandError {
        code: "CANDIDATE_TASK_ERROR".into(),
        message: "The candidate installation task stopped before completion.".into(),
    })?
    .map_err(Into::into)
}

#[tauri::command]
async fn preview_staged_candidate_deep_audit(
    session_id: String,
    expected_candidate_hash: String,
    state: State<'_, AppState>,
) -> Result<DeepAuditPreview, CommandError> {
    let stager = state.candidates.clone();
    let manager = state.deep_audit.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let snapshot = stager
            .audit_snapshot(&session_id, &expected_candidate_hash)
            .map_err(CommandError::from)?;
        if snapshot.candidate_hash != expected_candidate_hash {
            return Err(CommandError::from(skills::CandidateError::ChangedSession));
        }
        manager
            .preview_staged_candidate(&snapshot)
            .map_err(CommandError::from)
    })
    .await
    .map_err(|_| CommandError {
        code: "DEEP_AUDIT_TASK_ERROR".into(),
        message: "The staged candidate Deep Audit preview stopped before completion.".into(),
    })?
}

#[tauri::command]
async fn run_staged_candidate_deep_audit(
    session_id: String,
    expected_staged_candidate_hash: String,
    selections: Vec<DeepAuditSelection>,
    expected_candidate_hash: String,
    expected_provider_hash: String,
    state: State<'_, AppState>,
) -> Result<DeepAuditResult, CommandError> {
    let stager = state.candidates.clone();
    let manager = state.deep_audit.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let snapshot = stager
            .audit_snapshot(&session_id, &expected_staged_candidate_hash)
            .map_err(CommandError::from)?;
        if snapshot.candidate_hash != expected_staged_candidate_hash {
            return Err(CommandError::from(skills::CandidateError::ChangedSession));
        }
        manager
            .run_staged_candidate(
                &snapshot,
                &selections,
                &expected_candidate_hash,
                &expected_provider_hash,
            )
            .map_err(CommandError::from)
    })
    .await
    .map_err(|_| CommandError {
        code: "DEEP_AUDIT_TASK_ERROR".into(),
        message: "The staged candidate Deep Audit task stopped before completion.".into(),
    })?
}

#[tauri::command]
async fn test_deep_audit_connection(
    api_mode: DeepAuditApiMode,
    endpoint: String,
    model: String,
    api_key: Option<String>,
    state: State<'_, AppState>,
) -> Result<DeepAuditConnectionResult, CommandError> {
    let manager = state.deep_audit.clone();
    tauri::async_runtime::spawn_blocking(move || {
        manager.test_connection(api_mode, &endpoint, &model, api_key.as_deref())
    })
    .await
    .map_err(|_| CommandError {
        code: "DEEP_AUDIT_TASK_ERROR".into(),
        message: "The connection test stopped before completion.".into(),
    })?
    .map_err(Into::into)
}

#[tauri::command]
fn clear_deep_audit_settings(
    state: State<'_, AppState>,
) -> Result<DeepAuditSettings, CommandError> {
    state.deep_audit.clear_settings().map_err(Into::into)
}

#[tauri::command]
fn preview_deep_audit(
    id: Option<String>,
    markdown: String,
    state: State<'_, AppState>,
) -> Result<DeepAuditPreview, CommandError> {
    state
        .deep_audit
        .preview(&state.workspace, id.as_deref(), &markdown)
        .map_err(Into::into)
}

#[tauri::command]
async fn run_deep_audit(
    id: Option<String>,
    markdown: String,
    selections: Vec<DeepAuditSelection>,
    expected_candidate_hash: String,
    expected_provider_hash: String,
    state: State<'_, AppState>,
) -> Result<DeepAuditResult, CommandError> {
    let workspace = state.workspace.clone();
    let manager = state.deep_audit.clone();
    tauri::async_runtime::spawn_blocking(move || {
        manager.run(
            &workspace,
            id.as_deref(),
            &markdown,
            &selections,
            &expected_candidate_hash,
            &expected_provider_hash,
        )
    })
    .await
    .map_err(|_| CommandError {
        code: "DEEP_AUDIT_TASK_ERROR".into(),
        message: "The Deep Audit task stopped before completion.".into(),
    })?
    .map_err(Into::into)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            install_interface_menu(app.handle(), "zh-CN")?;
            app.on_menu_event(|app_handle, event| match event.id().as_ref() {
                MENU_SETTINGS => {
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                    let _ = app_handle.emit("studio-menu-action", "open-settings");
                }
                MENU_LANGUAGE_ZH_CN => {
                    let _ = install_interface_menu(app_handle, "zh-CN");
                    let _ = app_handle.emit("studio-menu-action", "locale:zh-CN");
                }
                MENU_LANGUAGE_EN => {
                    let _ = install_interface_menu(app_handle, "en");
                    let _ = app_handle.emit("studio-menu-action", "locale:en");
                }
                _ => {}
            });
            let settings_directory = app.path().app_config_dir()?;
            let candidate_staging_directory =
                app.path().app_cache_dir()?.join("candidate-staging-v1");
            let bundle_import_directory = app.path().app_cache_dir()?.join("bundle-import-v1");
            app.manage(AppState {
                workspace: Workspace::from_environment(),
                collections: CollectionManager::new(settings_directory.clone()),
                provenance: ProvenanceManager::new(settings_directory.clone()),
                deep_audit: DeepAuditManager::new(settings_directory),
                candidates: CandidateStager::new(candidate_staging_directory)?,
                bundle_imports: BundleImportManager::new(bundle_import_directory)?,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            set_interface_locale,
            list_skills,
            refresh_skills,
            get_skill,
            get_skill_package,
            read_skill_package_file,
            inspect_package_import_source,
            preview_skill_package,
            save_skill_package,
            preview_skill_package_deep_audit,
            run_skill_package_deep_audit,
            list_collections,
            create_collection,
            rename_collection,
            delete_collection,
            set_skill_collection_memberships,
            audit_draft,
            save_draft,
            preview_new_skill,
            create_skill,
            preview_skill_lifecycle,
            apply_skill_lifecycle,
            delete_archived_skill,
            preview_bundle_export,
            export_skill_bundle,
            stage_skill_bundle,
            review_imported_bundle,
            read_imported_bundle_file,
            compare_imported_bundle_file,
            install_imported_bundle,
            discard_imported_bundle,
            stage_github_candidate,
            list_github_repository_candidates,
            stage_github_repository_candidate,
            check_github_skill_update,
            stage_local_candidate,
            get_staged_candidate_review,
            read_staged_candidate_file,
            preview_staged_candidate_file_sync,
            apply_staged_candidate_file_sync,
            preview_staged_candidate_install,
            install_staged_candidate,
            preview_staged_candidate_deep_audit,
            run_staged_candidate_deep_audit,
            discard_staged_candidate,
            get_deep_audit_settings,
            save_deep_audit_settings,
            test_deep_audit_connection,
            clear_deep_audit_settings,
            preview_deep_audit,
            run_deep_audit
        ])
        .run(tauri::generate_context!())
        .expect("error while running Agent Skill Studio");
}

#[cfg(test)]
mod lifecycle_workflow_tests {
    use super::*;
    use std::fs;

    fn write_skill(codex_home: &std::path::Path, name: &str) {
        let directory = codex_home.join("skills").join(name);
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Use when testing lifecycle metadata.\n---\n"),
        )
        .unwrap();
    }

    #[test]
    fn lifecycle_workflow_moves_skill_provenance_and_collection_together() {
        let directory = tempfile::tempdir().unwrap();
        let codex_home = directory.path().join("codex");
        let settings = directory.path().join("settings");
        write_skill(&codex_home, "demo");
        let workspace = Workspace::new(codex_home.clone());
        let provenance = ProvenanceManager::new(settings.clone());
        let collections = CollectionManager::new(settings);
        let skill = workspace.list_skills().unwrap().skills[0].clone();
        let collection_id = collections.create("Research").unwrap().collections[0]
            .id
            .clone();
        collections
            .set_skill_memberships(&skill.id, &[collection_id])
            .unwrap();
        let preview = workspace
            .preview_skill_lifecycle(&skill.id, "disable")
            .unwrap();

        let result = apply_lifecycle_workflow(
            &workspace,
            &provenance,
            &collections,
            &skill.id,
            "disable",
            &preview.directory_revision,
        )
        .unwrap();

        assert!(codex_home.join("skills-disabled/demo").exists());
        assert_eq!(
            result.collections.collections[0].member_ids,
            vec![result.result.id.clone()]
        );
    }

    #[test]
    fn lifecycle_workflow_restores_directory_when_collection_metadata_is_invalid() {
        let directory = tempfile::tempdir().unwrap();
        let codex_home = directory.path().join("codex");
        let settings = directory.path().join("settings");
        write_skill(&codex_home, "demo");
        let workspace = Workspace::new(codex_home.clone());
        let provenance = ProvenanceManager::new(settings.clone());
        let collections = CollectionManager::new(settings.clone());
        let skill = workspace.list_skills().unwrap().skills[0].clone();
        fs::create_dir_all(&settings).unwrap();
        fs::write(settings.join("collections.json"), "not json").unwrap();
        let preview = workspace
            .preview_skill_lifecycle(&skill.id, "disable")
            .unwrap();

        let result = apply_lifecycle_workflow(
            &workspace,
            &provenance,
            &collections,
            &skill.id,
            "disable",
            &preview.directory_revision,
        );

        assert!(matches!(result, Err(WorkspaceError::LifecycleMetadata(_))));
        assert!(codex_home.join("skills/demo").exists());
        assert!(!codex_home.join("skills-disabled/demo").exists());
        assert_eq!(
            workspace.get_skill(&skill.id).unwrap().summary.source,
            "personal"
        );
    }

    #[test]
    fn delete_workflow_removes_archived_skill_provenance_and_collection_together() {
        let directory = tempfile::tempdir().unwrap();
        let codex_home = directory.path().join("codex");
        let settings = directory.path().join("settings");
        write_skill(&codex_home, "demo");
        let workspace = Workspace::new(codex_home.clone());
        let provenance = ProvenanceManager::new(settings.clone());
        let collections = CollectionManager::new(settings);
        let skill = workspace.list_skills().unwrap().skills[0].clone();
        provenance
            .record_candidate(
                &skill.id,
                "candidate-revision",
                skills::CandidateSource::Local {
                    selected_path: "/tmp/demo".into(),
                },
            )
            .unwrap();
        let collection_id = collections.create("Research").unwrap().collections[0]
            .id
            .clone();
        collections
            .set_skill_memberships(&skill.id, std::slice::from_ref(&collection_id))
            .unwrap();
        let archive = workspace
            .preview_skill_lifecycle(&skill.id, "archive")
            .unwrap();
        let archived = apply_lifecycle_workflow(
            &workspace,
            &provenance,
            &collections,
            &skill.id,
            "archive",
            &archive.directory_revision,
        )
        .unwrap()
        .result;
        let delete = workspace
            .preview_skill_lifecycle(&archived.id, "delete")
            .unwrap();

        let result = delete_lifecycle_workflow(
            &workspace,
            &provenance,
            &collections,
            &archived.id,
            &delete.directory_revision,
            "demo",
        )
        .unwrap();

        assert!(!codex_home.join("skill-archive/demo").exists());
        assert_eq!(result.result.deleted_name, "demo");
        assert!(result.collections.collections[0].member_ids.is_empty());
        assert_eq!(
            provenance.acquisition(&archived.id).unwrap().confidence,
            "unknown"
        );
    }

    #[cfg(unix)]
    #[test]
    fn delete_workflow_keeps_directory_and_provenance_when_collection_update_fails() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let codex_home = directory.path().join("codex");
        let provenance_settings = directory.path().join("provenance-settings");
        let collection_settings = directory.path().join("collection-settings");
        write_skill(&codex_home, "demo");
        let workspace = Workspace::new(codex_home.clone());
        let provenance = ProvenanceManager::new(provenance_settings);
        let collections = CollectionManager::new(collection_settings.clone());
        let skill = workspace.list_skills().unwrap().skills[0].clone();
        provenance
            .record_candidate(
                &skill.id,
                "candidate-revision",
                skills::CandidateSource::Local {
                    selected_path: "/tmp/demo".into(),
                },
            )
            .unwrap();
        let collection_id = collections.create("Research").unwrap().collections[0]
            .id
            .clone();
        collections
            .set_skill_memberships(&skill.id, &[collection_id])
            .unwrap();
        let archive = workspace
            .preview_skill_lifecycle(&skill.id, "archive")
            .unwrap();
        let archived = apply_lifecycle_workflow(
            &workspace,
            &provenance,
            &collections,
            &skill.id,
            "archive",
            &archive.directory_revision,
        )
        .unwrap()
        .result;
        let delete = workspace
            .preview_skill_lifecycle(&archived.id, "delete")
            .unwrap();
        fs::set_permissions(&collection_settings, fs::Permissions::from_mode(0o500)).unwrap();

        let result = delete_lifecycle_workflow(
            &workspace,
            &provenance,
            &collections,
            &archived.id,
            &delete.directory_revision,
            "demo",
        );

        fs::set_permissions(&collection_settings, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(matches!(result, Err(WorkspaceError::LifecycleMetadata(_))));
        assert!(codex_home.join("skill-archive/demo").exists());
        assert_eq!(
            provenance.acquisition(&archived.id).unwrap().confidence,
            "recorded"
        );
        assert_eq!(
            collections
                .list(std::slice::from_ref(&archived.id))
                .unwrap()
                .collections[0]
                .member_ids,
            vec![archived.id]
        );
    }
}

#[cfg(test)]
mod candidate_file_sync_workflow_tests {
    use super::*;
    use std::fs;

    #[test]
    fn exact_candidate_sync_restores_package_when_provenance_cannot_advance() {
        let directory = tempfile::tempdir().unwrap();
        let codex_home = directory.path().join("codex");
        let installed = codex_home.join("skills/demo");
        let candidate = directory.path().join("candidate");
        let staging = directory.path().join("staging");
        let settings = directory.path().join("settings");
        fs::create_dir_all(&installed).unwrap();
        fs::create_dir_all(&candidate).unwrap();
        let skill =
            "---\nname: demo\ndescription: Use when testing exact update provenance.\n---\n";
        fs::write(installed.join("SKILL.md"), skill).unwrap();
        fs::write(installed.join("guide.md"), "local\n").unwrap();
        fs::write(candidate.join("SKILL.md"), skill).unwrap();
        fs::write(candidate.join("guide.md"), "remote\n").unwrap();

        let workspace = Workspace::new(codex_home);
        let stager = CandidateStager::new(staging).unwrap();
        let provenance = ProvenanceManager::new(settings.clone());
        let manifest = stager.stage_local(&candidate).unwrap();
        let id = workspace.list_skills().unwrap().skills[0].id.clone();
        let local_revision = workspace.get_skill_package(&id).unwrap().revision;
        let remote = stager
            .file_sync_data(
                &manifest.session_id,
                &manifest.candidate_hash,
                "guide.md",
                CandidateFileSyncAction::Replace,
            )
            .unwrap();
        let preview = workspace
            .preview_candidate_file_sync(
                &id,
                &local_revision,
                "guide.md",
                CandidateFileSyncOperation::new(
                    CandidateFileSyncAction::Replace,
                    remote
                        .as_ref()
                        .map(|(bytes, executable)| (bytes.as_slice(), *executable)),
                ),
            )
            .unwrap();
        fs::create_dir_all(&settings).unwrap();
        fs::write(settings.join("skill-provenance.json"), "not json").unwrap();

        let result = apply_candidate_file_sync_workflow(
            &workspace,
            &stager,
            &provenance,
            CandidateFileSyncApplyRequest {
                id,
                session_id: manifest.session_id,
                expected_candidate_hash: manifest.candidate_hash,
                expected_local_revision: local_revision,
                expected_proposed_revision: preview.proposed_revision,
                path: "guide.md".into(),
                action: CandidateFileSyncAction::Replace,
            },
        );

        assert!(result.is_err());
        assert_eq!(
            fs::read_to_string(installed.join("guide.md")).unwrap(),
            "local\n"
        );
    }
}
