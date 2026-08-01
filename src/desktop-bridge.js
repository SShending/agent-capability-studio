import { invoke } from "@tauri-apps/api/core";

export const desktop = {
  listSkills: () => invoke("list_skills"),
  refreshSkills: () => invoke("refresh_skills"),
  getSkill: (id) => invoke("get_skill", { id }),
  auditDraft: (id, markdown) => invoke("audit_draft", { id, markdown }),
  saveDraft: (id, markdown, expectedHash) => invoke("save_draft", { id, markdown, expectedHash }),
  previewNewSkill: (markdown) => invoke("preview_new_skill", { markdown }),
  createSkill: (markdown, expectedDraftHash) => invoke("create_skill", { markdown, expectedDraftHash }),
  previewSkillLifecycle: (id, action) => invoke("preview_skill_lifecycle", { id, action }),
  applySkillLifecycle: (id, action, expectedDirectoryRevision) =>
    invoke("apply_skill_lifecycle", { id, action, expectedDirectoryRevision }),
  deleteArchivedSkill: (id, expectedDirectoryRevision, confirmationName) =>
    invoke("delete_archived_skill", { id, expectedDirectoryRevision, confirmationName }),
  stageGithubCandidate: (sourceUrl) => invoke("stage_github_candidate", { sourceUrl }),
  stageLocalCandidate: (selectedPath) => invoke("stage_local_candidate", { selectedPath }),
  discardStagedCandidate: (sessionId) => invoke("discard_staged_candidate", { sessionId }),
  getDeepAuditSettings: () => invoke("get_deep_audit_settings"),
  saveDeepAuditSettings: (apiMode, endpoint, model, apiKey) =>
    invoke("save_deep_audit_settings", { apiMode, endpoint, model, apiKey }),
  testDeepAuditConnection: (apiMode, endpoint, model, apiKey) =>
    invoke("test_deep_audit_connection", { apiMode, endpoint, model, apiKey }),
  clearDeepAuditSettings: () => invoke("clear_deep_audit_settings"),
  previewDeepAudit: (id, markdown) => invoke("preview_deep_audit", { id, markdown }),
  runDeepAudit: (id, markdown, selectedPaths, expectedCandidateHash, expectedProviderHash) =>
    invoke("run_deep_audit", { id, markdown, selectedPaths, expectedCandidateHash, expectedProviderHash })
};
