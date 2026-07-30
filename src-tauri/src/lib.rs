mod skills;

use serde::Serialize;
use skills::{
    AuditResult, Catalog, CreateSkillResult, DeepAuditManager, DeepAuditPreview, DeepAuditResult,
    DeepAuditSettings, DeleteSkillResult, LifecyclePreview, LifecycleResult, NewSkillPreview,
    SkillDetail, Workspace, WorkspaceError,
};
use tauri::{Manager, State};

struct AppState {
    workspace: Workspace,
    deep_audit: DeepAuditManager,
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

#[tauri::command]
fn list_skills(state: State<'_, AppState>) -> Result<Catalog, CommandError> {
    state.workspace.list_skills().map_err(Into::into)
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
fn get_deep_audit_settings(state: State<'_, AppState>) -> Result<DeepAuditSettings, CommandError> {
    state.deep_audit.settings().map_err(Into::into)
}

#[tauri::command]
fn save_deep_audit_settings(
    endpoint: String,
    model: String,
    api_key: Option<String>,
    state: State<'_, AppState>,
) -> Result<DeepAuditSettings, CommandError> {
    state
        .deep_audit
        .save_settings(&endpoint, &model, api_key.as_deref())
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
    selected_paths: Vec<String>,
    expected_candidate_hash: String,
    state: State<'_, AppState>,
) -> Result<DeepAuditResult, CommandError> {
    let workspace = state.workspace.clone();
    let manager = state.deep_audit.clone();
    tauri::async_runtime::spawn_blocking(move || {
        manager.run(
            &workspace,
            id.as_deref(),
            &markdown,
            &selected_paths,
            &expected_candidate_hash,
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
        .setup(|app| {
            let settings_directory = app.path().app_config_dir()?;
            app.manage(AppState {
                workspace: Workspace::from_environment(),
                deep_audit: DeepAuditManager::new(settings_directory),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_skills,
            get_skill,
            audit_draft,
            save_draft,
            preview_new_skill,
            create_skill,
            preview_skill_lifecycle,
            apply_skill_lifecycle,
            delete_archived_skill,
            get_deep_audit_settings,
            save_deep_audit_settings,
            clear_deep_audit_settings,
            preview_deep_audit,
            run_deep_audit
        ])
        .run(tauri::generate_context!())
        .expect("error while running Agent Skill Studio");
}
