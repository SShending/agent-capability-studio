mod skills;

use serde::Serialize;
use skills::{
    AuditResult, BundleExportError, BundleExportPlan, BundleExportReceipt, BundleFileComparison,
    BundleImportError, BundleImportFileContent, BundleImportManager, BundleInstallError,
    BundleInstallResult, BundleInstallSelection, BundleInstallationReview, CandidateFileContent,
    CandidateInstallPreview, CandidateInstallResult, CandidateManifest, CandidateReview,
    CandidateStager, Catalog, CreateSkillResult, DeepAuditApiMode, DeepAuditConnectionResult,
    DeepAuditManager, DeepAuditPreview, DeepAuditResult, DeepAuditSelection, DeepAuditSettings,
    DeleteSkillResult, LifecyclePreview, LifecycleResult, NewSkillPreview, SkillDetail, Workspace,
    WorkspaceError,
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
    deep_audit: DeepAuditManager,
    candidates: CandidateStager,
    bundle_imports: BundleImportManager,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandError {
    code: String,
    message: String,
}

impl From<WorkspaceError> for CommandError {
    fn from(error: WorkspaceError) -> Self {
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
    state.workspace.list_skills().map_err(Into::into)
}

#[tauri::command]
fn refresh_skills(state: State<'_, AppState>) -> Result<Catalog, CommandError> {
    state.workspace.refresh_skills().map_err(Into::into)
}

#[tauri::command]
fn get_skill(id: String, state: State<'_, AppState>) -> Result<SkillDetail, CommandError> {
    state.workspace.get_skill(&id).map_err(Into::into)
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
) -> Result<LifecycleResult, CommandError> {
    state
        .workspace
        .apply_skill_lifecycle(&id, &action, &expected_directory_revision)
        .map_err(Into::into)
}

#[tauri::command]
fn delete_archived_skill(
    id: String,
    expected_directory_revision: String,
    confirmation_name: String,
    state: State<'_, AppState>,
) -> Result<DeleteSkillResult, CommandError> {
    state
        .workspace
        .delete_archived_skill(&id, &expected_directory_revision, &confirmation_name)
        .map_err(Into::into)
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
    tauri::async_runtime::spawn_blocking(move || {
        stager.install(
            &workspace,
            &session_id,
            &expected_candidate_hash,
            &expected_install_revision,
        )
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
            stage_local_candidate,
            get_staged_candidate_review,
            read_staged_candidate_file,
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
