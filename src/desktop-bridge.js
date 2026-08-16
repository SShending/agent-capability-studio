import { invoke } from "@tauri-apps/api/core";

export const desktop = {
  setInterfaceLocale: (locale) => invoke("set_interface_locale", { locale }),
  listSkills: () => invoke("list_skills"),
  refreshSkills: () => invoke("refresh_skills"),
  getSkill: (id) => invoke("get_skill", { id }),
  getSkillPackage: (id) => invoke("get_skill_package", { id }),
  readSkillPackageFile: (id, expectedRevision, path) =>
    invoke("read_skill_package_file", { id, expectedRevision, path }),
  inspectPackageImportSource: (selectedPath) =>
    invoke("inspect_package_import_source", { selectedPath }),
  previewSkillPackage: (id, expectedRevision, mutations) =>
    invoke("preview_skill_package", { id, expectedRevision, mutations }),
  saveSkillPackage: (id, expectedRevision, expectedProposedRevision, mutations) =>
    invoke("save_skill_package", {
      id,
      expectedRevision,
      expectedProposedRevision,
      mutations
    }),
  previewSkillPackageDeepAudit: (id, expectedRevision, expectedProposedRevision, mutations) =>
    invoke("preview_skill_package_deep_audit", {
      id,
      expectedRevision,
      expectedProposedRevision,
      mutations
    }),
  runSkillPackageDeepAudit: (
    id,
    expectedRevision,
    expectedProposedRevision,
    mutations,
    selections,
    expectedCandidateHash,
    expectedProviderHash
  ) => invoke("run_skill_package_deep_audit", {
    request: {
      id,
      expectedRevision,
      expectedProposedRevision,
      mutations,
      selections,
      expectedCandidateHash,
      expectedProviderHash
    }
  }),
  listCollections: () => invoke("list_collections"),
  createCollection: (name) => invoke("create_collection", { name }),
  renameCollection: (id, name) => invoke("rename_collection", { id, name }),
  deleteCollection: (id) => invoke("delete_collection", { id }),
  setSkillCollectionMemberships: (skillId, collectionIds) =>
    invoke("set_skill_collection_memberships", { skillId, collectionIds }),
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
  listGithubRepositoryCandidates: (sourceUrl) =>
    invoke("list_github_repository_candidates", { sourceUrl }),
  stageGithubRepositoryCandidate: (sourceUrl, requestedRef, resolvedSha, skillPath) =>
    invoke("stage_github_repository_candidate", {
      sourceUrl,
      requestedRef,
      resolvedSha,
      skillPath
    }),
  checkGithubSkillUpdate: (id) => invoke("check_github_skill_update", { id }),
  stageLocalCandidate: (selectedPath) => invoke("stage_local_candidate", { selectedPath }),
  getStagedCandidateReview: (sessionId, expectedCandidateHash) =>
    invoke("get_staged_candidate_review", { sessionId, expectedCandidateHash }),
  readStagedCandidateFile: (sessionId, expectedCandidateHash, path) =>
    invoke("read_staged_candidate_file", { sessionId, expectedCandidateHash, path }),
  previewStagedCandidateFileSync: (
    id,
    sessionId,
    expectedCandidateHash,
    expectedLocalRevision,
    path,
    action
  ) => invoke("preview_staged_candidate_file_sync", {
    id,
    sessionId,
    expectedCandidateHash,
    expectedLocalRevision,
    path,
    action
  }),
  applyStagedCandidateFileSync: (
    id,
    sessionId,
    expectedCandidateHash,
    expectedLocalRevision,
    expectedProposedRevision,
    path,
    action
  ) => invoke("apply_staged_candidate_file_sync", {
    request: {
      id,
      sessionId,
      expectedCandidateHash,
      expectedLocalRevision,
      expectedProposedRevision,
      path,
      action
    }
  }),
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
