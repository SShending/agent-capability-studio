mod skills;

use serde::Serialize;
use skills::{
    AuditResult, Catalog, CreateSkillResult, DeleteSkillResult, LifecyclePreview, LifecycleResult,
    NewSkillPreview, SkillDetail, Workspace, WorkspaceError,
};
use tauri::State;

struct AppState(Workspace);

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

#[tauri::command]
fn list_skills(state: State<'_, AppState>) -> Result<Catalog, CommandError> {
    state.0.list_skills().map_err(Into::into)
}

#[tauri::command]
fn get_skill(id: String, state: State<'_, AppState>) -> Result<SkillDetail, CommandError> {
    state.0.get_skill(&id).map_err(Into::into)
}

#[tauri::command]
fn audit_draft(
    id: String,
    markdown: String,
    state: State<'_, AppState>,
) -> Result<AuditResult, CommandError> {
    state.0.audit_draft(&id, &markdown).map_err(Into::into)
}

#[tauri::command]
fn save_draft(
    id: String,
    markdown: String,
    expected_hash: String,
    state: State<'_, AppState>,
) -> Result<skills::SaveResult, CommandError> {
    state
        .0
        .save_draft(&id, &markdown, &expected_hash)
        .map_err(Into::into)
}

#[tauri::command]
fn preview_new_skill(
    markdown: String,
    state: State<'_, AppState>,
) -> Result<NewSkillPreview, CommandError> {
    state.0.preview_new_skill(&markdown).map_err(Into::into)
}

#[tauri::command]
fn create_skill(
    markdown: String,
    expected_draft_hash: String,
    state: State<'_, AppState>,
) -> Result<CreateSkillResult, CommandError> {
    state
        .0
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
        .0
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
        .0
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
        .0
        .delete_archived_skill(&id, &expected_directory_revision, &confirmation_name)
        .map_err(Into::into)
}

pub fn run() {
    tauri::Builder::default()
        .manage(AppState(Workspace::from_environment()))
        .invoke_handler(tauri::generate_handler![
            list_skills,
            get_skill,
            audit_draft,
            save_draft,
            preview_new_skill,
            create_skill,
            preview_skill_lifecycle,
            apply_skill_lifecycle,
            delete_archived_skill
        ])
        .run(tauri::generate_context!())
        .expect("error while running Agent Skill Studio");
}
