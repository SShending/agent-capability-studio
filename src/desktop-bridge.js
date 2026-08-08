import { invoke } from "@tauri-apps/api/core";

export const desktop = {
  setInterfaceLocale: (locale) => invoke("set_interface_locale", { locale }),
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
  previewBundleExport: (skillIds) => invoke("preview_bundle_export", { skillIds }),
  exportSkillBundle: (expectedPlanRevision, destination) =>
    invoke("export_skill_bundle", { expectedPlanRevision, destination }),
  stageSkillBundle: (selectedPath) => invoke("stage_skill_bundle", { selectedPath }),
  reviewImportedBundle: (sessionId, expectedBundleRevision) =>
    invoke("review_imported_bundle", { sessionId, expectedBundleRevision }),
  readImportedBundleFile: (sessionId, expectedBundleRevision, directoryName, path) =>
    invoke("read_imported_bundle_file", {
      sessionId,
      expectedBundleRevision,
      directoryName,
      path
    }),
  compareImportedBundleFile: (
    sessionId,
    expectedBundleRevision,
    directoryName,
    matchId,
    path
  ) => invoke("compare_imported_bundle_file", {
    sessionId,
    expectedBundleRevision,
    directoryName,
    matchId,
    path
  }),
  installImportedBundle: (
    sessionId,
    expectedBundleRevision,
    expectedReviewRevision,
    selections
  ) => invoke("install_imported_bundle", {
    sessionId,
    expectedBundleRevision,
    expectedReviewRevision,
    selections
  }),
  discardImportedBundle: (sessionId) => invoke("discard_imported_bundle", { sessionId }),
  stageGithubCandidate: (sourceUrl) => invoke("stage_github_candidate", { sourceUrl }),
  stageLocalCandidate: (selectedPath) => invoke("stage_local_candidate", { selectedPath }),
  getStagedCandidateReview: (sessionId, expectedCandidateHash) =>
    invoke("get_staged_candidate_review", { sessionId, expectedCandidateHash }),
  readStagedCandidateFile: (sessionId, expectedCandidateHash, path) =>
    invoke("read_staged_candidate_file", { sessionId, expectedCandidateHash, path }),
  previewStagedCandidateInstall: (sessionId, expectedCandidateHash) =>
    invoke("preview_staged_candidate_install", { sessionId, expectedCandidateHash }),
  installStagedCandidate: (sessionId, expectedCandidateHash, expectedInstallRevision) =>
    invoke("install_staged_candidate", { sessionId, expectedCandidateHash, expectedInstallRevision }),
  previewStagedCandidateDeepAudit: (sessionId, expectedCandidateHash) =>
    invoke("preview_staged_candidate_deep_audit", { sessionId, expectedCandidateHash }),
  runStagedCandidateDeepAudit: (
    sessionId,
    expectedStagedCandidateHash,
    selections,
    expectedCandidateHash,
    expectedProviderHash
  ) => invoke("run_staged_candidate_deep_audit", {
    sessionId,
    expectedStagedCandidateHash,
    selections,
    expectedCandidateHash,
    expectedProviderHash
  }),
  discardStagedCandidate: (sessionId) => invoke("discard_staged_candidate", { sessionId }),
  getDeepAuditSettings: () => invoke("get_deep_audit_settings"),
  saveDeepAuditSettings: (apiMode, endpoint, model, apiKey) =>
    invoke("save_deep_audit_settings", {
      apiMode,
      endpoint,
      model,
      apiKey
    }),
  testDeepAuditConnection: (apiMode, endpoint, model, apiKey) =>
    invoke("test_deep_audit_connection", {
      apiMode,
      endpoint,
      model,
      apiKey
    }),
  clearDeepAuditSettings: () => invoke("clear_deep_audit_settings"),
  previewDeepAudit: (id, markdown) => invoke("preview_deep_audit", { id, markdown }),
  runDeepAudit: (id, markdown, selections, expectedCandidateHash, expectedProviderHash) =>
    invoke("run_deep_audit", { id, markdown, selections, expectedCandidateHash, expectedProviderHash })
};
