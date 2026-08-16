import { createIcons, icons } from "lucide";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { desktop } from "./desktop-bridge.js";
import {
  addCatalogSkill,
  applyInstallOutcome,
  personalSkillsNeedingAttention,
  removeCatalogSkill,
  replaceCatalogSkill
} from "./catalog-state.js";
import { parseSkillDocument, updateSkillDocument } from "./skill-document.js";
import { serverInstallPrompt } from "./server-install-prompt.js";
import {
  beginExportOperation,
  beginImportPreview,
  clearImportReview,
  createBundleWorkflowState,
  finishExportCommit,
  invalidateExportOperations,
  isCurrentExportOperation,
  isCurrentImportPreview,
  setImportReview
} from "./bundle-workflow-state.js";
import { createLocalization } from "./localization.js";
import { createLineDiff } from "./line-diff.js";
import {
  buildPackageChangePresentation,
  deletePackageMutations,
  renamePackageMutations,
  singleSkillExportIssue
} from "./package-workflow-state.js";
import { groupSkillsByProvenance } from "./provenance-groups.js";
import { buildUpdateComparison } from "./update-comparison.js";
import {
  adjacentRepositoryQueuePosition,
  clearRepositoryReviewQueue,
  createRepositoryReviewQueue,
  createRepositorySessionCache,
  currentRepositoryQueuePath,
  drainRepositorySessions,
  filterUninstalledRepositoryCandidates,
  getOrStageRepositorySession,
  persistRepositoryReviewQueue,
  removeCurrentRepositoryQueuePath,
  removeRepositorySession,
  restoreRepositoryReviewQueue,
  setRepositoryQueuePosition
} from "./repository-intake-state.js";

window.lucide = {
  createIcons: (options = {}) => createIcons({ icons, ...options })
};

const localization = createLocalization({ storage: window.localStorage, document });
const t = (key, values) => localization.t(key, values);
localization.apply();

const state = {
  skills: [],
  counts: {},
  roots: {},
  source: "all",
  collectionId: null,
  collections: [],
  query: "",
  sort: "source",
  selectedId: null,
  detail: null,
  editor: null,
  auditSequence: 0,
  deepAuditSettings: null,
  deepAuditPreview: null,
  deepAuditContext: null,
  confirmAction: null,
  confirmRequiredName: null,
  confirmBusy: false,
  confirmLabel: t("common.confirm"),
  providerTestSequence: 0,
  candidate: null,
  candidateSourceMode: "github",
  candidateLocalPath: null,
  repositoryListing: null,
  repositoryQueue: restoreRepositoryReviewQueue(window.localStorage),
  repositorySessions: createRepositorySessionCache(),
  repositoryNavigationBusy: false,
  bundleExportPlan: null,
  bundleExportBusy: false,
  singleSkillExportBusy: false,
  bundleInstallBusy: false,
  bundleInstallReviewStale: false,
  bundleInstallResult: null,
  bundleWorkflow: createBundleWorkflowState(),
  packageWorkspace: null,
  collectionEditId: null,
  toastTimer: null
};

const elements = {
  list: document.querySelector("#skill-list"),
  empty: document.querySelector("#empty-state"),
  resultSummary: document.querySelector("#result-summary"),
  search: document.querySelector("#search-input"),
  sort: document.querySelector("#sort-select"),
  refresh: document.querySelector("#refresh-button"),
  create: document.querySelector("#create-skill-button"),
  exportSkills: document.querySelector("#export-skills-button"),
  reviewCandidate: document.querySelector("#review-candidate-button"),
  auditStatus: document.querySelector("#audit-status"),
  auditLabel: document.querySelector("#audit-label"),
  auditIssueList: document.querySelector("#audit-issue-list"),
  confirmDialog: document.querySelector("#confirm-dialog"),
  confirmForm: document.querySelector("#confirm-form"),
  confirmTitle: document.querySelector("#confirm-title"),
  confirmMessage: document.querySelector("#confirm-message"),
  confirmSubmit: document.querySelector("#confirm-submit"),
  confirmCancel: document.querySelector("#cancel-confirm-button"),
  confirmNameField: document.querySelector("#confirm-name-field"),
  confirmName: document.querySelector("#confirm-name"),
  detailPanel: document.querySelector("#detail-panel"),
  detailEmpty: document.querySelector("#detail-empty"),
  detailContent: document.querySelector("#detail-content"),
  closeDetail: document.querySelector("#close-detail-button"),
  collectionNav: document.querySelector("#collection-nav"),
  addCollection: document.querySelector("#add-collection-button"),
  collectionDialog: document.querySelector("#collection-dialog"),
  collectionForm: document.querySelector("#collection-form"),
  collectionDialogTitle: document.querySelector("#collection-dialog-title"),
  collectionName: document.querySelector("#collection-name"),
  cancelCollection: document.querySelector("#cancel-collection"),
  deleteCollection: document.querySelector("#delete-collection-button"),
  collectionMembershipDialog: document.querySelector("#collection-membership-dialog"),
  collectionMembershipForm: document.querySelector("#collection-membership-form"),
  collectionMembershipList: document.querySelector("#collection-membership-list"),
  manageCollections: document.querySelector("#manage-collections-button"),
  packageDialog: document.querySelector("#package-dialog"),
  packageTitle: document.querySelector("#package-title"),
  packageStatus: document.querySelector("#package-status"),
  closePackage: document.querySelector("#close-package-button"),
  packageExport: document.querySelector("#package-export-button"),
  packageDeepAudit: document.querySelector("#package-deep-audit-button"),
  packagePreview: document.querySelector("#package-preview-button"),
  packageSave: document.querySelector("#package-save-button"),
  packageTree: document.querySelector("#package-tree"),
  packageSummary: document.querySelector("#package-summary"),
  packageNewFile: document.querySelector("#package-new-file"),
  packageImportFile: document.querySelector("#package-import-file"),
  packageNewFolder: document.querySelector("#package-new-folder"),
  packageFileName: document.querySelector("#package-file-name"),
  packageFileMeta: document.querySelector("#package-file-meta"),
  packageFileActions: document.querySelector("#package-file-actions"),
  packageRenameFile: document.querySelector("#package-rename-file"),
  packageDeleteFile: document.querySelector("#package-delete-file"),
  packageEmptyPreview: document.querySelector("#package-empty-preview"),
  packageTextEditor: document.querySelector("#package-text-editor"),
  packageImagePreview: document.querySelector("#package-image-preview"),
  packageImage: document.querySelector("#package-image"),
  packageBinaryPreview: document.querySelector("#package-binary-preview"),
  packageAuditVerdict: document.querySelector("#package-audit-verdict"),
  packageAuditBadge: document.querySelector("#package-audit-badge"),
  packageAuditSummary: document.querySelector("#package-audit-summary"),
  packageFindingCount: document.querySelector("#package-finding-count"),
  packageFindingList: document.querySelector("#package-finding-list"),
  packageDeepResultSection: document.querySelector("#package-deep-result-section"),
  packageDeepResultVerdict: document.querySelector("#package-deep-result-verdict"),
  packageDeepResultBadge: document.querySelector("#package-deep-result-badge"),
  packageDeepResultSummary: document.querySelector("#package-deep-result-summary"),
  packageDeepResultMeta: document.querySelector("#package-deep-result-meta"),
  packageDeepFindingList: document.querySelector("#package-deep-finding-list"),
  packageValidationCount: document.querySelector("#package-validation-count"),
  packageValidationList: document.querySelector("#package-validation-list"),
  packageChangeCount: document.querySelector("#package-change-count"),
  packageChangeList: document.querySelector("#package-change-list"),
  editorDialog: document.querySelector("#editor-dialog"),
  closeEditor: document.querySelector("#close-editor-button"),
  editorTitle: document.querySelector("#editor-title"),
  draftStatus: document.querySelector("#draft-status"),
  guidedMode: document.querySelector("#guided-mode-button"),
  sourceMode: document.querySelector("#source-mode-button"),
  guidedEditor: document.querySelector("#guided-editor"),
  sourceEditor: document.querySelector("#source-editor"),
  draftName: document.querySelector("#draft-name"),
  draftDescription: document.querySelector("#draft-description"),
  draftSections: document.querySelector("#draft-sections"),
  draftSectionCount: document.querySelector("#draft-section-count"),
  draftBodyFallbackField: document.querySelector("#draft-body-fallback-field"),
  draftBodyFallback: document.querySelector("#draft-body-fallback"),
  draftSource: document.querySelector("#draft-source"),
  auditDraft: document.querySelector("#audit-draft-button"),
  deepAudit: document.querySelector("#deep-audit-button"),
  saveDraft: document.querySelector("#save-draft-button"),
  auditVerdict: document.querySelector("#audit-verdict"),
  auditVerdictBadge: document.querySelector("#audit-verdict-badge"),
  auditSummary: document.querySelector("#audit-summary"),
  findingCount: document.querySelector("#finding-count"),
  findingList: document.querySelector("#finding-list"),
  diffCount: document.querySelector("#diff-count"),
  diffEmpty: document.querySelector("#diff-empty"),
  diffView: document.querySelector("#diff-view"),
  diffBefore: document.querySelector("#diff-before"),
  diffAfter: document.querySelector("#diff-after"),
  creationPreview: document.querySelector("#creation-preview"),
  creationDestination: document.querySelector("#creation-destination"),
  creationState: document.querySelector("#creation-state"),
  deepResultSection: document.querySelector("#deep-result-section"),
  deepResultVerdict: document.querySelector("#deep-result-verdict"),
  deepResultBadge: document.querySelector("#deep-result-badge"),
  deepResultSummary: document.querySelector("#deep-result-summary"),
  deepResultMeta: document.querySelector("#deep-result-meta"),
  deepFindingList: document.querySelector("#deep-finding-list"),
  candidateIntakeDialog: document.querySelector("#candidate-intake-dialog"),
  candidateIntakeForm: document.querySelector("#candidate-intake-form"),
  candidateGithubMode: document.querySelector("#candidate-github-mode"),
  candidateLocalMode: document.querySelector("#candidate-local-mode"),
  candidateSourceControl: document.querySelector("#candidate-source-control"),
  candidateGithubField: document.querySelector("#candidate-github-field"),
  candidateLocalField: document.querySelector("#candidate-local-field"),
  candidateGithubUrl: document.querySelector("#candidate-github-url"),
  candidateRepositoryListing: document.querySelector("#candidate-repository-listing"),
  candidateRepositoryTitle: document.querySelector("#candidate-repository-title"),
  candidateRepositoryVersion: document.querySelector("#candidate-repository-version"),
  candidateRepositoryNote: document.querySelector("#candidate-repository-note"),
  candidateRepositoryList: document.querySelector("#candidate-repository-list"),
  candidateResetRepository: document.querySelector("#candidate-reset-repository"),
  candidateSelectAllRepository: document.querySelector("#candidate-select-all-repository"),
  candidateLocalPath: document.querySelector("#candidate-local-path"),
  chooseCandidateFolder: document.querySelector("#choose-candidate-folder"),
  stageCandidate: document.querySelector("#stage-candidate-button"),
  candidateReviewDialog: document.querySelector("#candidate-review-dialog"),
  candidateReviewKicker: document.querySelector("#candidate-review-kicker"),
  candidateMutationState: document.querySelector("#candidate-mutation-state"),
  candidatePreviousQueueItem: document.querySelector("#candidate-previous-queue-item"),
  candidateNextQueueItem: document.querySelector("#candidate-next-queue-item"),
  candidateUpdateSummary: document.querySelector("#candidate-update-summary"),
  candidateUpdateTitle: document.querySelector("#candidate-update-title"),
  candidateUpdateDescription: document.querySelector("#candidate-update-description"),
  candidateReviewTitle: document.querySelector("#candidate-review-title"),
  candidateDeepAudit: document.querySelector("#candidate-deep-audit-button"),
  installCandidate: document.querySelector("#install-candidate-button"),
  candidateSourceList: document.querySelector("#candidate-source-list"),
  candidateSourceSection: document.querySelector("#candidate-source-section"),
  candidateCompatibilitySection: document.querySelector("#candidate-compatibility-section"),
  candidateCompatibilityStatus: document.querySelector("#candidate-compatibility-status"),
  candidateCompatibilitySummary: document.querySelector("#candidate-compatibility-summary"),
  candidateCompatibilityList: document.querySelector("#candidate-compatibility-list"),
  candidateFileCount: document.querySelector("#candidate-file-count"),
  candidateFilesTitle: document.querySelector("#candidate-files-title"),
  candidateFiles: document.querySelector("#candidate-files"),
  candidatePreviewTitle: document.querySelector("#candidate-preview-title"),
  candidatePreviewSection: document.querySelector("#candidate-preview-section"),
  candidatePreviewMeta: document.querySelector("#candidate-preview-meta"),
  candidatePreviewEmpty: document.querySelector("#candidate-preview-empty"),
  candidateFilePreview: document.querySelector("#candidate-file-preview"),
  candidateReviewPane: document.querySelector("#candidate-review-pane"),
  candidateUpdateComparison: document.querySelector("#candidate-update-comparison"),
  candidateComparisonPath: document.querySelector("#candidate-comparison-path"),
  candidateUpdateAttribution: document.querySelector("#candidate-update-attribution"),
  candidateSyncFile: document.querySelector("#candidate-sync-file"),
  candidateToggleAudit: document.querySelector("#candidate-toggle-audit"),
  candidateUnifiedDiff: document.querySelector("#candidate-unified-diff"),
  candidateLocalFileMeta: document.querySelector("#candidate-local-file-meta"),
  candidateLocalPreviewEmpty: document.querySelector("#candidate-local-preview-empty"),
  candidateLocalFilePreview: document.querySelector("#candidate-local-file-preview"),
  candidateRemoteFileMeta: document.querySelector("#candidate-remote-file-meta"),
  candidateRemotePreviewEmpty: document.querySelector("#candidate-remote-preview-empty"),
  candidateRemoteFilePreview: document.querySelector("#candidate-remote-file-preview"),
  candidateAuditVerdict: document.querySelector("#candidate-audit-verdict"),
  candidateAuditBadge: document.querySelector("#candidate-audit-badge"),
  candidateAuditSummary: document.querySelector("#candidate-audit-summary"),
  candidateFindingCount: document.querySelector("#candidate-finding-count"),
  candidateFindingList: document.querySelector("#candidate-finding-list"),
  candidateDeepResultSection: document.querySelector("#candidate-deep-result-section"),
  candidateDeepResultVerdict: document.querySelector("#candidate-deep-result-verdict"),
  candidateDeepResultBadge: document.querySelector("#candidate-deep-result-badge"),
  candidateDeepResultSummary: document.querySelector("#candidate-deep-result-summary"),
  candidateDeepResultMeta: document.querySelector("#candidate-deep-result-meta"),
  candidateDeepFindingList: document.querySelector("#candidate-deep-finding-list"),
  candidateSkippedCount: document.querySelector("#candidate-skipped-count"),
  candidateSkippedSummary: document.querySelector("#candidate-skipped-summary"),
  candidateSkippedList: document.querySelector("#candidate-skipped-list"),
  updateResultDialog: document.querySelector("#update-result-dialog"),
  updateResultDescription: document.querySelector("#update-result-description"),
  updateResultRepository: document.querySelector("#update-result-repository"),
  updateResultCommit: document.querySelector("#update-result-commit"),
  closeUpdateResult: document.querySelector("#close-update-result"),
  bundleExportDialog: document.querySelector("#bundle-export-dialog"),
  bundleExportForm: document.querySelector("#bundle-export-form"),
  bundleSelectAll: document.querySelector("#bundle-select-all"),
  bundleSelectionCount: document.querySelector("#bundle-selection-count"),
  bundleSkillList: document.querySelector("#bundle-skill-list"),
  bundleExportReview: document.querySelector("#bundle-export-review"),
  bundleExportSkillCount: document.querySelector("#bundle-export-skill-count"),
  bundleExportFileCount: document.querySelector("#bundle-export-file-count"),
  bundleExportByteCount: document.querySelector("#bundle-export-byte-count"),
  bundleExportBlocks: document.querySelector("#bundle-export-blocks"),
  bundleExportReceipt: document.querySelector("#bundle-export-receipt"),
  bundleReceiptDestination: document.querySelector("#bundle-receipt-destination"),
  bundleReceiptRevision: document.querySelector("#bundle-receipt-revision"),
  bundleReceiptSkillCount: document.querySelector("#bundle-receipt-skill-count"),
  bundleReceiptFileCount: document.querySelector("#bundle-receipt-file-count"),
  bundleReceiptByteCount: document.querySelector("#bundle-receipt-byte-count"),
  serverInstallPrompt: document.querySelector("#server-install-prompt"),
  copyServerInstallPrompt: document.querySelector("#copy-server-install-prompt"),
  previewBundleExport: document.querySelector("#preview-bundle-export"),
  applyBundleExport: document.querySelector("#apply-bundle-export"),
  closeBundleExport: document.querySelector("#close-bundle-export"),
  cancelBundleExport: document.querySelector("#cancel-bundle-export"),
  importBundleButton: document.querySelector("#import-skill-bundle-button"),
  bundleImportDialog: document.querySelector("#bundle-import-dialog"),
  bundleImportTitle: document.querySelector("#bundle-import-title"),
  bundleImportPhase: document.querySelector("#bundle-import-phase"),
  bundleImportMutationState: document.querySelector("#bundle-import-mutation-state"),
  bundleImportSource: document.querySelector("#bundle-import-source"),
  bundleImportSkillCount: document.querySelector("#bundle-import-skill-count"),
  bundleImportSkillList: document.querySelector("#bundle-import-skill-list"),
  bundleInstallResults: document.querySelector("#bundle-install-results"),
  bundleImportPreviewTitle: document.querySelector("#bundle-import-preview-title"),
  bundleImportPreviewEmpty: document.querySelector("#bundle-import-preview-empty"),
  bundleImportFilePreview: document.querySelector("#bundle-import-file-preview"),
  bundleImportCurrentFilePreview: document.querySelector("#bundle-import-current-file-preview"),
  bundleImportStatus: document.querySelector("#bundle-import-status"),
  installBundleSkills: document.querySelector("#install-bundle-skills"),
  discardBundleImport: document.querySelector("#discard-bundle-import"),
  closeBundleImport: document.querySelector("#close-bundle-import"),
  settingsDialog: document.querySelector("#settings-dialog"),
  settingsForm: document.querySelector("#settings-form"),
  deepApiMode: document.querySelector("#deep-audit-api-mode"),
  deepEndpoint: document.querySelector("#deep-audit-endpoint"),
  deepModel: document.querySelector("#deep-audit-model"),
  deepApiKey: document.querySelector("#deep-audit-api-key"),
  deepCredentialState: document.querySelector("#deep-audit-credential-state"),
  deepConnectionStatus: document.querySelector("#deep-audit-connection-status"),
  testDeepConnection: document.querySelector("#test-deep-audit-connection"),
  clearDeepSettings: document.querySelector("#clear-deep-audit-settings"),
  saveDeepSettings: document.querySelector("#save-deep-audit-settings"),
  deepConsentDialog: document.querySelector("#deep-audit-consent-dialog"),
  deepConsentForm: document.querySelector("#deep-audit-consent-form"),
  deepConsentApiMode: document.querySelector("#deep-consent-api-mode"),
  deepConsentEndpoint: document.querySelector("#deep-consent-endpoint"),
  deepConsentModel: document.querySelector("#deep-consent-model"),
  deepConsentRequests: document.querySelector("#deep-consent-requests"),
  deepConsentFiles: document.querySelector("#deep-consent-files"),
  deepSkippedFiles: document.querySelector("#deep-skipped-files"),
  deepSkippedSummary: document.querySelector("#deep-skipped-summary"),
  deepSkippedList: document.querySelector("#deep-skipped-list"),
  runDeepAudit: document.querySelector("#run-deep-audit"),
  toast: document.querySelector("#toast"),
  toastMessage: document.querySelector("#toast-message")
};

function sourceLabel(source) {
  const key = `source.${source}`;
  const label = t(key);
  return label === key ? source : label;
}

function skillStateLabel(stateValue) {
  return {
    active: t("detail.active"),
    disabled: t("detail.disabled"),
    archived: t("detail.archived")
  }[stateValue] || stateValue;
}

const deepAuditApiModeLabels = {
  chatCompletions: "Chat Completions",
  responses: "Responses"
};

function localizedError(error) {
  const key = error?.code ? `error.code.${error.code}` : null;
  if (key) {
    const message = t(key);
    if (message !== key) return message;
  }
  return error?.message || t("error.generic");
}

function refreshIcons() {
  window.lucide?.createIcons({ attrs: { "aria-hidden": "true" } });
}

async function api(url, options = {}) {
  if (url === "/api/skills" && !options.method) return desktop.listSkills();
  const detail = url.match(/^\/api\/skills\/([^/]+)$/);
  if (detail && !options.method) return desktop.getSkill(decodeURIComponent(detail[1]));
  const audit = url.match(/^\/api\/skills\/([^/]+)\/audit$/);
  const body = options.body ? JSON.parse(options.body) : {};
  if (audit && options.method === "POST") return desktop.auditDraft(decodeURIComponent(audit[1]), body.markdown);
  if (detail && options.method === "PUT") {
    return desktop.saveDraft(decodeURIComponent(detail[1]), body.markdown, body.expectedHash);
  }
  throw new Error(t("error.operationUnavailable"));
}

function initials(name) {
  return name
    .split(/[-_\s]+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase())
    .join("") || "S";
}

function formatDate(value) {
  return new Intl.DateTimeFormat(localization.locale, {
    year: "numeric",
    month: "short",
    day: "numeric"
  }).format(new Date(value));
}

function skillIcon(skill, className = "skill-icon") {
  const container = document.createElement("div");
  container.className = className;
  if (skill.brandColor) container.style.backgroundColor = skill.brandColor;
  if (skill.hasIcon) {
    const image = document.createElement("img");
    image.src = `/api/skills/${encodeURIComponent(skill.id)}/icon`;
    image.alt = "";
    image.loading = "lazy";
    image.addEventListener("error", () => {
      image.remove();
      container.textContent = initials(skill.displayName);
    });
    container.append(image);
  } else {
    container.textContent = initials(skill.displayName);
  }
  return container;
}

function visibleSkills() {
  const query = state.query.trim().toLocaleLowerCase(localization.locale);
  const filtered = state.skills.filter((skill) => {
    const matchesSource = state.source === "all" || skill.source === state.source;
    const collection = state.collections.find((item) => item.id === state.collectionId);
    const matchesCollection = !collection || collection.memberIds.includes(skill.id);
    const haystack = `${skill.name} ${skill.displayName} ${skill.summary} ${skill.description} ${skill.acquisition?.repository || ""}`.toLocaleLowerCase(localization.locale);
    return matchesSource && matchesCollection && (!query || haystack.includes(query));
  });

  return filtered.sort((left, right) => {
    if (state.sort === "name") return left.displayName.localeCompare(right.displayName);
    if (state.sort === "updated") return right.modifiedAt.localeCompare(left.modifiedAt);
    const order = { personal: 0, disabled: 1, system: 2, plugin: 3, archive: 4 };
    return order[left.source] - order[right.source] || left.displayName.localeCompare(right.displayName);
  });
}

function setSourceFilter(source) {
  state.source = source;
  state.collectionId = null;
  document.querySelectorAll("[data-source]").forEach((item) => {
    const active = item.dataset.source === source;
    item.classList.toggle("is-active", active);
    item.setAttribute("aria-pressed", String(active));
  });
  renderList();
  renderCollections();
}

async function inspectHealthIssue(skill) {
  state.query = "";
  elements.search.value = "";
  setSourceFilter("personal");
  await selectSkill(skill.id);
  if (state.detail?.id !== skill.id) return;
  try {
    await openPackageWorkspace(skill);
  } catch (error) {
    showToast(localizedError(error), true);
  }
}

function renderHealthIssues() {
  const issues = personalSkillsNeedingAttention(state.skills);
  const rows = issues.map((skill) => {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "audit-issue";
    button.title = t("health.issueTitle", { name: skill.displayName });
    button.setAttribute("aria-label", button.title);
    button.innerHTML = '<i data-lucide="circle-alert"></i><span></span><i data-lucide="chevron-right"></i>';
    button.querySelector("span").textContent = skill.displayName;
    button.addEventListener("click", () => inspectHealthIssue(skill));
    return button;
  });
  elements.auditIssueList.replaceChildren(...rows);
  elements.auditIssueList.hidden = rows.length === 0;
}

function updateCounts() {
  const counts = state.counts;
  document.querySelector("#count-all").textContent = counts.total || 0;
  for (const source of ["personal", "disabled", "system", "plugin", "archive"]) {
    document.querySelector(`#count-${source}`).textContent = counts[source] || 0;
  }
  document.querySelector("#codex-home").textContent = state.codexHome || "~/.codex";

  elements.auditStatus.classList.toggle("is-good", !counts.needsAttention);
  elements.auditStatus.classList.toggle("has-issues", Boolean(counts.needsAttention));
  elements.auditLabel.textContent = counts.needsAttention
    ? t("health.needsAttention", { count: counts.needsAttention })
    : t("health.normal");
  renderHealthIssues();
}

function applyCatalogState(catalog) {
  state.skills = catalog.skills;
  state.counts = catalog.counts;
  updateCounts();
  renderList();
  refreshIcons();
}

function renderList() {
  const skills = visibleSkills();
  elements.list.replaceChildren();
  elements.empty.hidden = skills.length > 0;
  elements.resultSummary.textContent = t("list.results", {
    visible: skills.length,
    total: state.counts.total || 0
  });

  const appendSkill = (skill) => {
    const row = document.createElement("button");
    row.type = "button";
    row.className = `skill-row${skill.id === state.selectedId ? " is-selected" : ""}`;
    row.dataset.id = skill.id;
    row.setAttribute("aria-label", t("list.viewSkill", { name: skill.displayName }));
    row.setAttribute("aria-pressed", String(skill.id === state.selectedId));

    const identity = document.createElement("span");
    identity.className = "skill-identity";
    identity.append(skillIcon(skill));
    const copy = document.createElement("span");
    copy.className = "skill-copy";
    const name = document.createElement("span");
    name.className = "skill-name";
    name.textContent = skill.displayName;
    const description = document.createElement("span");
    description.className = "skill-description";
    description.textContent = skill.summary;
    copy.append(name, description);
    identity.append(copy);

    const source = document.createElement("span");
    source.className = `source-badge ${skill.source}`;
    source.textContent = sourceLabel(skill.source);

    const trigger = document.createElement("span");
    const readonlySource = ["system", "plugin"].includes(skill.source);
    trigger.className = `trigger-badge ${readonlySource ? "source-managed" : "good"}`;
    trigger.textContent = readonlySource
      ? t("trigger.sourceManaged")
      : skill.triggerMode === "explicit"
        ? t("trigger.explicitShort")
        : t("trigger.contextualShort");

    const files = document.createElement("span");
    files.className = "file-count";
    files.textContent = String(skill.fileCount);

    row.append(identity, source, trigger, files);
    row.addEventListener("click", () => selectSkill(skill.id));
    elements.list.append(row);
  };

  for (const group of groupSkillsByProvenance(skills)) {
    const heading = document.createElement("div");
    heading.className = `provenance-heading is-${group.kind}`;
    const label = document.createElement("span");
    label.textContent = group.kind === "github"
      ? t("provenance.githubRepository", { repository: group.value })
      : t(`provenance.${group.kind}`);
    const count = document.createElement("span");
    count.textContent = t("common.count", { count: group.skills.length });
    heading.append(label, count);
    elements.list.append(heading);
    group.skills.forEach(appendSkill);
  }
}

function collectionMemberCount(collection) {
  const known = new Set(state.skills.map((skill) => skill.id));
  return collection.memberIds.filter((id) => known.has(id)).length;
}

function renderCollections() {
  const rows = state.collections.map((collection) => {
    const row = document.createElement("div");
    row.className = `collection-row${collection.id === state.collectionId ? " is-active" : ""}`;
    const select = document.createElement("button");
    select.type = "button";
    select.className = "collection-select";
    select.innerHTML = '<i data-lucide="folder"></i><span></span><strong></strong>';
    select.querySelector("span").textContent = collection.name;
    select.querySelector("strong").textContent = String(collectionMemberCount(collection));
    select.addEventListener("click", () => {
      state.collectionId = state.collectionId === collection.id ? null : collection.id;
      if (state.collectionId) {
        state.source = "all";
        document.querySelectorAll("[data-source]").forEach((item) => {
          item.classList.toggle("is-active", item.dataset.source === "all");
          item.setAttribute("aria-pressed", String(item.dataset.source === "all"));
        });
      }
      renderCollections();
      renderList();
    });
    const menu = document.createElement("button");
    menu.type = "button";
    menu.className = "icon-button compact collection-menu";
    menu.title = t("common.rename");
    menu.innerHTML = '<i data-lucide="ellipsis"></i>';
    menu.addEventListener("click", () => openCollectionDialog(collection));
    row.append(select, menu);
    return row;
  });
  elements.collectionNav.replaceChildren(...rows);
  if (!rows.length) {
    const empty = document.createElement("p");
    empty.className = "collection-empty";
    empty.textContent = t("collections.empty");
    elements.collectionNav.append(empty);
  }
  refreshIcons();
}

async function loadCollections() {
  try {
    const snapshot = await desktop.listCollections();
    state.collections = snapshot.collections;
    if (state.collectionId && !state.collections.some((item) => item.id === state.collectionId)) {
      state.collectionId = null;
    }
    renderCollections();
    renderList();
  } catch (error) {
    showToast(localizedError(error), true);
  }
}

function openCollectionDialog(collection = null) {
  state.collectionEditId = collection?.id || null;
  elements.collectionDialogTitle.textContent = t(collection ? "collections.rename" : "collections.create");
  elements.collectionName.value = collection?.name || "";
  elements.deleteCollection.hidden = !collection;
  elements.collectionDialog.showModal();
  elements.collectionName.focus();
  elements.collectionName.select();
}

async function submitCollection(event) {
  event.preventDefault();
  try {
    const snapshot = state.collectionEditId
      ? await desktop.renameCollection(state.collectionEditId, elements.collectionName.value)
      : await desktop.createCollection(elements.collectionName.value);
    state.collections = snapshot.collections;
    elements.collectionDialog.close();
    renderCollections();
    renderList();
  } catch (error) {
    showToast(localizedError(error), true);
  }
}

function requestDeleteCollection(collection) {
  elements.collectionDialog.close();
  presentConfirmation({
    title: t("collections.deleteTitle"),
    message: t("collections.deleteMessage", { name: collection.name }),
    label: t("common.delete"),
    action: async () => {
      const snapshot = await desktop.deleteCollection(collection.id);
      state.collections = snapshot.collections;
      if (state.collectionId === collection.id) state.collectionId = null;
      renderCollections();
      renderList();
      showToast(t("collections.deleted"));
    }
  });
}

function openCollectionMembership(skill) {
  const rows = state.collections.map((collection) => {
    const label = document.createElement("label");
    label.className = "collection-membership-row";
    const checkbox = document.createElement("input");
    checkbox.type = "checkbox";
    checkbox.value = collection.id;
    checkbox.checked = collection.memberIds.includes(skill.id);
    const text = document.createElement("span");
    text.textContent = collection.name;
    label.append(checkbox, text);
    return label;
  });
  elements.collectionMembershipList.replaceChildren(...rows);
  if (!rows.length) {
    const empty = document.createElement("p");
    empty.className = "collection-empty";
    empty.textContent = t("collections.noMembership");
    elements.collectionMembershipList.append(empty);
  }
  elements.collectionMembershipDialog.dataset.skillId = skill.id;
  elements.collectionMembershipDialog.showModal();
}

async function saveCollectionMembership(event) {
  event.preventDefault();
  const skillId = elements.collectionMembershipDialog.dataset.skillId;
  try {
    const collectionIds = [...elements.collectionMembershipList.querySelectorAll('input[type="checkbox"]:checked')]
      .map((input) => input.value);
    const snapshot = await desktop.setSkillCollectionMemberships(skillId, collectionIds);
    state.collections = snapshot.collections;
    elements.collectionMembershipDialog.close();
    renderCollections();
    renderList();
    showToast(t("collections.saved"));
  } catch (error) {
    showToast(localizedError(error), true);
  }
}

function actionButton(label, icon, kind, onClick) {
  const button = document.createElement("button");
  button.type = "button";
  button.className = kind;
  const iconNode = document.createElement("i");
  iconNode.dataset.lucide = icon;
  const text = document.createElement("span");
  text.textContent = label;
  button.append(iconNode, text);
  button.addEventListener("click", onClick);
  return button;
}

function packageMutationForPath(path) {
  return state.packageWorkspace?.mutations.findLast?.((item) => item.path === path && item.action === "write")
    || [...(state.packageWorkspace?.mutations || [])].reverse().find((item) => item.path === path && item.action === "write");
}

function packageEntry(path) {
  return state.packageWorkspace?.entries.find((entry) => entry.path === path);
}

function packageDraftEntries() {
  if (!state.packageWorkspace) return [];
  const entries = new Map(state.packageWorkspace.snapshot.entries.map((entry) => [entry.path, { ...entry }]));
  for (const mutation of state.packageWorkspace.mutations) {
    if (mutation.action === "write") {
      entries.set(mutation.path, {
        path: mutation.path,
        kind: "file",
        mediaType: "text",
        size: new TextEncoder().encode(mutation.content).length,
        editable: true
      });
    } else if (mutation.action === "copyFile") {
      entries.set(mutation.path, {
        path: mutation.path,
        kind: "file",
        mediaType: mutation.mediaType || "binary",
        size: mutation.expectedSize,
        editable: false,
        imported: true
      });
    } else if (mutation.action === "createDirectory") {
      entries.set(mutation.path, { path: mutation.path, kind: "directory", mediaType: "directory", size: 0, editable: true });
    } else if (mutation.action === "delete") {
      for (const path of [...entries.keys()]) {
        if (path === mutation.path || path.startsWith(`${mutation.path}/`)) entries.delete(path);
      }
    } else if (mutation.action === "move") {
      const moved = [...entries.entries()].filter(([path]) => path === mutation.path || path.startsWith(`${mutation.path}/`));
      for (const [path, entry] of moved) {
        entries.delete(path);
        const nextPath = path === mutation.path ? mutation.destination : `${mutation.destination}${path.slice(mutation.path.length)}`;
        entries.set(nextPath, { ...entry, path: nextPath, originalPath: entry.originalPath || path });
      }
    }
  }
  return [...entries.values()].sort((left, right) => left.path.localeCompare(right.path));
}

function formatPackageBytes(value) {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / (1024 * 1024)).toFixed(1)} MB`;
}

function renderPackageTree() {
  const workspace = state.packageWorkspace;
  if (!workspace) return;
  workspace.entries = packageDraftEntries();
  const rows = workspace.entries.map((entry) => {
    const button = document.createElement("button");
    button.type = "button";
    button.className = `package-tree-row${entry.path === workspace.selectedPath ? " is-selected" : ""}`;
    button.style.setProperty("--tree-depth", String(entry.path.split("/").length - 1));
    const icon = entry.kind === "directory" ? "folder" : entry.mediaType.startsWith("image/") ? "image" : entry.mediaType === "text" ? "file-text" : "file";
    button.innerHTML = `<i data-lucide="${icon}"></i><span></span>`;
    button.querySelector("span").textContent = entry.path.split("/").at(-1);
    button.title = entry.path;
    button.addEventListener("click", () => selectPackageEntry(entry.path));
    return button;
  });
  elements.packageTree.replaceChildren(...rows);
  const files = workspace.entries.filter((entry) => entry.kind === "file");
  const bytes = files.reduce((sum, entry) => sum + (entry.size || 0), 0);
  elements.packageSummary.textContent = t("package.summary", { files: files.length, size: formatPackageBytes(bytes) });
  elements.packageStatus.textContent = workspace.mutations.length ? t("package.unsaved") : t("package.unchanged");
  elements.packageStatus.classList.toggle("is-dirty", workspace.mutations.length > 0);
  elements.packagePreview.disabled = !workspace.snapshot.editable || workspace.mutations.length === 0;
  elements.packageExport.disabled = workspace.mutations.length > 0 || state.singleSkillExportBusy;
  elements.packageNewFile.disabled = !workspace.snapshot.editable;
  elements.packageImportFile.disabled = !workspace.snapshot.editable;
  elements.packageNewFolder.disabled = !workspace.snapshot.editable;
  renderPackageReview(workspace.preview);
  refreshIcons();
}

function renderPackageReview(preview = null) {
  const validations = preview?.validations || state.packageWorkspace?.snapshot.validations || [];
  const validationRows = validations.map((item) => {
    const row = document.createElement("div");
    row.className = `package-validation-row ${item.severity}`;
    row.innerHTML = '<i data-lucide="circle-alert"></i><span><strong></strong><small></small></span>';
    const key = `package.validation.${item.code}`;
    row.querySelector("strong").textContent = t(key) === key ? item.message : t(key);
    row.querySelector("small").textContent = item.path || t("package.kicker");
    return row;
  });
  if (!validationRows.length) {
    const empty = document.createElement("p");
    empty.className = "package-review-empty";
    empty.textContent = t("package.noValidation");
    validationRows.push(empty);
  }
  elements.packageValidationList.replaceChildren(...validationRows);
  elements.packageValidationCount.textContent = String(validations.length);
  const changes = preview?.changes || [];
  const changeRows = changes.map((change) => {
    const item = buildPackageChangePresentation(change);
    const row = document.createElement("div");
    row.className = `package-change-row ${item.kind}`;
    const label = document.createElement("strong");
    label.textContent = t(`package.change.${item.kind}`);
    const path = document.createElement("span");
    path.textContent = item.destination ? `${item.path} → ${item.destination}` : item.path;
    const summary = document.createElement("div");
    summary.className = "package-change-summary";
    summary.append(label, path);
    row.append(summary);
    if (item.hasTextDiff) {
      const diff = document.createElement("div");
      diff.className = "package-change-diff";
      if (item.diff.truncated) {
        const notice = document.createElement("p");
        notice.className = "candidate-diff-truncated";
        notice.textContent = t("update.diffTooLarge");
        diff.append(notice);
      } else {
        diff.append(...item.diff.rows.map((diffRow) => renderDiffLine(diffRow)));
      }
      row.append(diff);
    }
    return row;
  });
  if (!changeRows.length) {
    const empty = document.createElement("p");
    empty.className = "package-review-empty";
    empty.textContent = t("package.noChanges");
    changeRows.push(empty);
  }
  elements.packageChangeList.replaceChildren(...changeRows);
  elements.packageChangeCount.textContent = String(changes.length);
  updatePackageSaveState();
  refreshIcons();
}

function updatePackageSaveState() {
  const workspace = state.packageWorkspace;
  elements.packageSave.disabled = !workspace?.preview?.canApply
    || !workspace.audit
    || workspace.audit.verdict === "block"
    || workspace.auditLoading;
}

function renderPackageAudit(audit) {
  if (!audit) {
    elements.packageAuditVerdict.textContent = t("audit.waiting");
    elements.packageAuditBadge.textContent = t("common.notRun");
    elements.packageAuditBadge.className = "verdict-badge";
    elements.packageAuditSummary.textContent = t("package.auditPrompt");
    elements.packageFindingCount.textContent = "0";
    elements.packageFindingList.replaceChildren();
    updatePackageSaveState();
    return;
  }
  renderBaselineAuditOutcome(audit, {
    verdict: elements.packageAuditVerdict,
    badge: elements.packageAuditBadge,
    summary: elements.packageAuditSummary,
    findingCount: elements.packageFindingCount,
    findingList: elements.packageFindingList
  });
  updatePackageSaveState();
}

function renderPackageAuditLoading() {
  elements.packageAuditVerdict.textContent = t("audit.loadingTitle");
  elements.packageAuditBadge.textContent = t("audit.inProgress");
  elements.packageAuditBadge.className = "verdict-badge is-loading";
  elements.packageAuditSummary.textContent = t("audit.loadingSummary");
  elements.packageFindingCount.textContent = "0";
  elements.packageFindingList.replaceChildren();
  updatePackageSaveState();
}

function renderPackageAuditError(message) {
  elements.packageAuditVerdict.textContent = t("audit.failed");
  elements.packageAuditBadge.textContent = t("common.error");
  elements.packageAuditBadge.className = "verdict-badge is-block";
  elements.packageAuditSummary.textContent = message;
  elements.packageFindingCount.textContent = "0";
  elements.packageFindingList.replaceChildren();
  updatePackageSaveState();
}

function renderPackageDeepAuditResult(result) {
  if (state.packageWorkspace) state.packageWorkspace.deepAudit = result;
  renderDeepAuditOutcome(result, {
    section: elements.packageDeepResultSection,
    verdict: elements.packageDeepResultVerdict,
    badge: elements.packageDeepResultBadge,
    summary: elements.packageDeepResultSummary,
    meta: elements.packageDeepResultMeta,
    findingList: elements.packageDeepFindingList
  });
}

async function openPackageWorkspace(skill) {
  try {
    const snapshot = await desktop.getSkillPackage(skill.id);
    const skillDocument = await desktop.readSkillPackageFile(skill.id, snapshot.revision, "SKILL.md");
    if (typeof skillDocument.content !== "string") {
      showToast(t("error.code.MISSING_SKILL_DOCUMENT"), true);
      return;
    }
    state.packageWorkspace = {
      skill,
      snapshot,
      entries: snapshot.entries,
      selectedPath: null,
      mutations: [],
      preview: null,
      audit: null,
      deepAudit: null,
      auditLoading: false,
      loadingSequence: 0,
      reviewSequence: 0,
      originalSkillMarkdown: skillDocument.content
    };
    elements.packageTitle.textContent = skill.displayName;
    elements.packageExport.hidden = skill.source !== "personal";
    elements.packageDeepAudit.hidden = !snapshot.editable;
    resetPackagePreview();
    renderPackageAudit(null);
    renderPackageDeepAuditResult(null);
    renderPackageTree();
    if (!snapshot.editable) showToast(t("package.readOnly"));
    elements.packageDialog.showModal();
    if (snapshot.editable) await runPackageBaselineAudit(state.packageWorkspace);
  } catch (error) {
    showToast(localizedError(error), true);
  }
}

function packageSkillMarkdown(workspace = state.packageWorkspace) {
  if (!workspace) return "";
  return workspace.mutations.findLast?.((item) => item.action === "write" && item.path === "SKILL.md")?.content
    ?? [...workspace.mutations].reverse().find((item) => item.action === "write" && item.path === "SKILL.md")?.content
    ?? workspace.originalSkillMarkdown;
}

function invalidatePackageAssessment(workspace = state.packageWorkspace) {
  if (!workspace) return;
  workspace.preview = null;
  workspace.audit = null;
  workspace.deepAudit = null;
  workspace.auditLoading = false;
  workspace.reviewSequence += 1;
  if (state.deepAuditContext?.kind === "package" && state.deepAuditContext.workspace === workspace) {
    state.deepAuditPreview = null;
    state.deepAuditContext = null;
  }
  renderPackageAudit(null);
  renderPackageDeepAuditResult(null);
}

async function runPackageBaselineAudit(workspace = state.packageWorkspace) {
  if (!workspace?.snapshot.editable) return null;
  const sequence = ++workspace.reviewSequence;
  workspace.auditLoading = true;
  renderPackageAuditLoading();
  try {
    const audit = await desktop.auditDraft(workspace.skill.id, packageSkillMarkdown(workspace));
    if (state.packageWorkspace !== workspace || sequence !== workspace.reviewSequence) return null;
    workspace.audit = audit;
    renderPackageAudit(audit);
    return audit;
  } catch (error) {
    if (state.packageWorkspace === workspace && sequence === workspace.reviewSequence) {
      renderPackageAuditError(localizedError(error));
    }
    return null;
  } finally {
    if (state.packageWorkspace === workspace && sequence === workspace.reviewSequence) {
      workspace.auditLoading = false;
      updatePackageSaveState();
    }
  }
}

function resetPackagePreview() {
  elements.packageEmptyPreview.hidden = false;
  elements.packageTextEditor.hidden = true;
  elements.packageImagePreview.hidden = true;
  elements.packageBinaryPreview.hidden = true;
  elements.packageFileActions.hidden = true;
  elements.packageFileName.textContent = t("package.selectFile");
  elements.packageFileMeta.textContent = "";
}

async function selectPackageEntry(path) {
  const workspace = state.packageWorkspace;
  const entry = packageEntry(path);
  if (!workspace || !entry) return;
  workspace.selectedPath = path;
  renderPackageTree();
  if (entry.kind === "directory") {
    resetPackagePreview();
    elements.packageFileName.textContent = path;
    elements.packageFileActions.hidden = !workspace.snapshot.editable;
    return;
  }
  const sequence = ++workspace.loadingSequence;
  elements.packageFileName.textContent = path;
  elements.packageFileMeta.textContent = `${entry.mediaType} · ${formatPackageBytes(entry.size || 0)}`;
  elements.packageFileActions.hidden = !workspace.snapshot.editable;
  elements.packageEmptyPreview.hidden = true;
  elements.packageTextEditor.hidden = true;
  elements.packageImagePreview.hidden = true;
  elements.packageBinaryPreview.hidden = true;
  const draft = packageMutationForPath(path);
  if (draft) {
    elements.packageTextEditor.value = draft.content;
    elements.packageTextEditor.hidden = false;
    return;
  }
  if (entry.imported) {
    elements.packageBinaryPreview.hidden = false;
    return;
  }
  try {
    const originalPath = entry.originalPath || path;
    const content = await desktop.readSkillPackageFile(workspace.skill.id, workspace.snapshot.revision, originalPath);
    if (sequence !== workspace.loadingSequence || state.packageWorkspace !== workspace) return;
    if (content.mediaType === "text") {
      elements.packageTextEditor.value = content.content || "";
      elements.packageTextEditor.readOnly = !workspace.snapshot.editable || !content.editable;
      elements.packageTextEditor.hidden = false;
    } else if (content.dataUrl) {
      elements.packageImage.src = content.dataUrl;
      elements.packageImagePreview.hidden = false;
    } else {
      elements.packageBinaryPreview.hidden = false;
    }
  } catch (error) {
    showToast(localizedError(error), true);
  }
}

function setPackageWrite(path, content) {
  const workspace = state.packageWorkspace;
  if (!workspace) return;
  workspace.mutations = workspace.mutations.filter((item) => !(item.action === "write" && item.path === path));
  workspace.mutations.push({ action: "write", path, content });
  invalidatePackageAssessment(workspace);
  workspace.entries = packageDraftEntries();
  elements.packageStatus.textContent = t("package.unsaved");
  elements.packageStatus.classList.add("is-dirty");
  elements.packagePreview.disabled = false;
  elements.packageExport.disabled = true;
  elements.packageSave.disabled = true;
  renderPackageReview(null);
}

function promptPackagePath(kind) {
  const path = window.prompt(t(kind === "file" ? "package.filePrompt" : "package.folderPrompt"));
  if (!path?.trim()) return;
  const normalized = path.trim();
  if (packageEntry(normalized)) return showToast(t("error.code.PACKAGE_PATH_CONFLICT"), true);
  state.packageWorkspace.mutations.push(kind === "file"
    ? { action: "write", path: normalized, content: "" }
    : { action: "createDirectory", path: normalized });
  invalidatePackageAssessment(state.packageWorkspace);
  renderPackageTree();
  selectPackageEntry(normalized);
}

async function importPackageFile() {
  try {
    const selected = await openDialog({
      directory: false,
      multiple: false,
      title: t("package.importFile")
    });
    if (typeof selected !== "string") return;
    const source = await desktop.inspectPackageImportSource(selected);
    const path = window.prompt(t("package.importPrompt"), `assets/${source.fileName}`);
    if (!path?.trim()) return;
    const normalized = path.trim();
    if (packageEntry(normalized)) return showToast(t("error.code.PACKAGE_PATH_CONFLICT"), true);
    state.packageWorkspace.mutations.push({
      action: "copyFile",
      path: normalized,
      sourcePath: source.sourcePath,
      expectedHash: source.contentHash,
      expectedSize: source.size,
      mediaType: source.mediaType
    });
    invalidatePackageAssessment(state.packageWorkspace);
    renderPackageTree();
    selectPackageEntry(normalized);
  } catch (error) {
    showToast(localizedError(error), true);
  }
}

function renamePackageEntry() {
  const workspace = state.packageWorkspace;
  const path = workspace?.selectedPath;
  if (!path || path === "SKILL.md") return;
  const destination = window.prompt(t("package.renamePrompt"), path);
  if (!destination?.trim() || destination.trim() === path) return;
  const target = destination.trim();
  workspace.mutations = renamePackageMutations(
    workspace.mutations,
    workspace.snapshot.entries,
    packageEntry(path),
    path,
    target
  );
  workspace.selectedPath = target;
  invalidatePackageAssessment(workspace);
  renderPackageTree();
  selectPackageEntry(workspace.selectedPath);
}

function deletePackageEntry() {
  const workspace = state.packageWorkspace;
  const path = workspace?.selectedPath;
  if (!path || path === "SKILL.md") return;
  presentConfirmation({
    title: t("package.deleteTitle"),
    message: t("package.deleteMessage", { path }),
    label: t("common.delete"),
    action: async () => {
      workspace.mutations = deletePackageMutations(
        workspace.mutations,
        workspace.snapshot.entries,
        packageEntry(path),
        path
      );
      workspace.selectedPath = null;
      invalidatePackageAssessment(workspace);
      resetPackagePreview();
      renderPackageTree();
    }
  });
}

async function previewPackageChanges() {
  const workspace = state.packageWorkspace;
  if (!workspace?.mutations.length) return false;
  const sequence = ++workspace.reviewSequence;
  workspace.auditLoading = true;
  elements.packagePreview.disabled = true;
  renderPackageAuditLoading();
  try {
    const [preview, audit] = await Promise.all([
      desktop.previewSkillPackage(workspace.skill.id, workspace.snapshot.revision, workspace.mutations),
      desktop.auditDraft(workspace.skill.id, packageSkillMarkdown(workspace))
    ]);
    if (state.packageWorkspace !== workspace || sequence !== workspace.reviewSequence) return false;
    workspace.preview = preview;
    workspace.audit = audit;
    renderPackageAudit(audit);
    renderPackageReview(workspace.preview);
    return true;
  } catch (error) {
    if (state.packageWorkspace === workspace && sequence === workspace.reviewSequence) {
      renderPackageAuditError(localizedError(error));
    }
    showToast(localizedError(error), true);
    return false;
  } finally {
    if (state.packageWorkspace === workspace && sequence === workspace.reviewSequence) {
      workspace.auditLoading = false;
      elements.packagePreview.disabled = !workspace.mutations.length;
      updatePackageSaveState();
    }
  }
}

async function savePackageChanges() {
  const workspace = state.packageWorkspace;
  if (!workspace?.preview?.canApply) return;
  elements.packageSave.disabled = true;
  elements.packageSave.querySelector("span").textContent = t("package.saving");
  try {
    const result = await desktop.saveSkillPackage(
      workspace.skill.id,
      workspace.snapshot.revision,
      workspace.preview.proposedRevision,
      workspace.mutations
    );
    state.packageWorkspace = null;
    elements.packageDialog.close();
    applyCatalogState(replaceCatalogSkill(state.skills, state.counts, workspace.skill.id, result.skill));
    state.selectedId = result.skill.id;
    state.detail = result.skill;
    renderDetail();
    showToast(t("package.saved"));
  } catch (error) {
    showToast(localizedError(error), true);
  } finally {
    elements.packageSave.querySelector("span").textContent = t("package.save");
    if (state.packageWorkspace === workspace) updatePackageSaveState();
  }
}

function requestPackageSave() {
  const workspace = state.packageWorkspace;
  if (!workspace?.preview?.canApply || !workspace.audit || workspace.audit.verdict === "block") return;
  const requiresReview = workspace.audit.verdict === "review"
    || ["review", "block"].includes(workspace.deepAudit?.verdict)
    || workspace.preview.validations.some((item) => item.severity === "warning");
  if (!requiresReview) {
    savePackageChanges();
    return;
  }
  presentConfirmation({
    title: t("editor.reviewSaveTitle"),
    message: workspace.deepAudit?.verdict === "block"
      ? t("editor.reviewSaveDeepMessage")
      : t("editor.reviewSaveMessage"),
    label: t("editor.confirmSave"),
    action: savePackageChanges
  });
}

function requestClosePackage() {
  if (!state.packageWorkspace?.mutations.length) {
    if (state.deepAuditContext?.kind === "package") {
      state.deepAuditPreview = null;
      state.deepAuditContext = null;
    }
    state.packageWorkspace = null;
    elements.packageDialog.close();
    return;
  }
  presentConfirmation({
    title: t("editor.discardTitle"),
    message: t("editor.discardMessage"),
    label: t("editor.discard"),
    action: async () => {
      if (state.deepAuditContext?.kind === "package") {
        state.deepAuditPreview = null;
        state.deepAuditContext = null;
      }
      state.packageWorkspace = null;
      elements.packageDialog.close();
    }
  });
}

async function selectSkill(id) {
  state.selectedId = id;
  renderList();
  elements.detailPanel.classList.add("is-open");
  try {
    state.detail = await api(`/api/skills/${encodeURIComponent(id)}`);
    renderDetail();
  } catch (error) {
    showToast(localizedError(error), true);
  }
}

function renderDetail() {
  const skill = state.detail;
  if (!skill) {
    elements.detailEmpty.hidden = false;
    elements.detailContent.hidden = true;
    return;
  }
  elements.detailEmpty.hidden = true;
  elements.detailContent.hidden = false;

  const icon = document.querySelector("#detail-icon");
  const newIcon = skillIcon(skill, "detail-icon");
  newIcon.id = "detail-icon";
  icon.replaceWith(newIcon);
  document.querySelector("#detail-title").textContent = skill.displayName;
  document.querySelector("#detail-source").textContent = sourceLabel(skill.source);
  document.querySelector("#detail-description").textContent = skill.summary;
  document.querySelector("#detail-state").textContent = ["system", "plugin"].includes(skill.source)
    ? t("detail.sourceEnabled")
    : skillStateLabel(skill.state);
  document.querySelector("#detail-trigger").textContent = ["system", "plugin"].includes(skill.source)
    ? t("trigger.sourceManagedDetail")
    : skill.triggerMode === "explicit"
      ? t("trigger.explicitDetail")
      : t("trigger.contextualDetail");
  document.querySelector("#detail-provenance").textContent = provenanceDetail(skill.acquisition);
  document.querySelector("#detail-files").textContent = t("common.count", { count: skill.fileCount });
  document.querySelector("#detail-updated").textContent = formatDate(skill.modifiedAt);
  document.querySelector("#detail-path").textContent = skill.path;
  document.querySelector("#detail-markdown").textContent = skill.markdown;

  const actions = document.querySelector("#detail-actions");
  actions.replaceChildren();
  const updateAction = skill.acquisition?.kind === "github" && skill.acquisition.repository
    ? actionButton(
      t("update.check"),
      "refresh-cw",
      "secondary-button",
      () => checkGithubSkillUpdate(skill)
    )
    : null;
  if (skill.source === "personal") {
    const primaryActions = [
      actionButton(t("detail.editPackage"), "files", "primary-button", () => openPackageWorkspace(skill)),
      actionButton(t("detail.collections"), "folder-heart", "secondary-button", () => openCollectionMembership(skill)),
      actionButton(t("detail.exportSkill"), "package-open", "secondary-button", () => exportSingleSkill(skill))
    ];
    if (updateAction) primaryActions.push(updateAction);
    actions.append(
      ...primaryActions,
      actionButton(t("detail.disable"), "circle-pause", "secondary-button", () => requestSkillLifecycle("disable", skill)),
      actionButton(t("detail.archive"), "archive", "secondary-button", () => requestSkillLifecycle("archive", skill))
    );
  } else if (skill.source === "disabled") {
    actions.append(
      actionButton(t("detail.editPackage"), "files", "secondary-button", () => openPackageWorkspace(skill)),
      actionButton(t("detail.collections"), "folder-heart", "secondary-button", () => openCollectionMembership(skill)),
      ...(updateAction ? [updateAction] : []),
      actionButton(t("detail.enable"), "circle-play", "primary-button", () => requestSkillLifecycle("enable", skill)),
      actionButton(t("detail.archive"), "archive", "secondary-button", () => requestSkillLifecycle("archive", skill))
    );
  } else if (skill.source === "archive") {
    actions.append(
      actionButton(t("detail.editPackage"), "files", "secondary-button", () => openPackageWorkspace(skill)),
      actionButton(t("detail.collections"), "folder-heart", "secondary-button", () => openCollectionMembership(skill)),
      ...(updateAction ? [updateAction] : []),
      actionButton(t("detail.restore"), "archive-restore", "primary-button", () => requestSkillLifecycle("restore", skill)),
      actionButton(t("detail.delete"), "trash-2", "danger-button", () => requestSkillLifecycle("delete", skill))
    );
  } else {
    const readonly = document.createElement("span");
    readonly.className = "trigger-badge";
    readonly.textContent = t("detail.readOnly");
    actions.append(
      actionButton(t("detail.viewPackage"), "files", "secondary-button", () => openPackageWorkspace(skill)),
      actionButton(t("detail.collections"), "folder-heart", "secondary-button", () => openCollectionMembership(skill)),
      readonly
    );
  }
  refreshIcons();
}

function provenanceDetail(acquisition) {
  if (acquisition?.kind === "github" && acquisition.repository) {
    const evidence = acquisition.confidence === "recorded"
      ? t("provenance.exactRevision")
      : t("provenance.confirmedRepository");
    const path = acquisition.skillPath ? ` · ${acquisition.skillPath}` : "";
    return `${acquisition.repository} · ${evidence}${path}`;
  }
  if (acquisition?.kind === "local") return t("provenance.localRecorded");
  return t("provenance.unknownDetail");
}

function presentConfirmation({ title, message, label, action, tone = "danger", requiredName = null }) {
  state.confirmAction = action;
  state.confirmRequiredName = requiredName;
  state.confirmLabel = label;
  elements.confirmTitle.textContent = title;
  elements.confirmMessage.textContent = message;
  elements.confirmSubmit.textContent = label;
  elements.confirmSubmit.className = tone === "primary" ? "primary-button" : "danger-button";
  elements.confirmNameField.hidden = !requiredName;
  elements.confirmName.value = "";
  elements.confirmName.placeholder = requiredName || "";
  elements.confirmSubmit.disabled = Boolean(requiredName);
  elements.confirmDialog.showModal();
  (requiredName ? elements.confirmName : elements.confirmCancel).focus();
}

function setConfirmationBusy(busy) {
  state.confirmBusy = busy;
  elements.confirmDialog.toggleAttribute("aria-busy", busy);
  elements.confirmCancel.disabled = busy;
  elements.confirmSubmit.disabled = busy || Boolean(
    state.confirmRequiredName && elements.confirmName.value !== state.confirmRequiredName
  );
  elements.confirmSubmit.textContent = busy ? t("confirm.submitting") : state.confirmLabel;
}

function setEditorMode(mode) {
  const guided = mode === "guided";
  elements.guidedEditor.hidden = !guided;
  elements.sourceEditor.hidden = guided;
  elements.guidedMode.classList.toggle("is-active", guided);
  elements.sourceMode.classList.toggle("is-active", !guided);
  elements.guidedMode.setAttribute("aria-selected", String(guided));
  elements.sourceMode.setAttribute("aria-selected", String(!guided));
  if (guided && state.editor) syncGuidedFields();
}

function syncGuidedFields() {
  if (!state.editor) return;
  const document = parseSkillDocument(state.editor.draftMarkdown);
  elements.draftName.value = document.name;
  elements.draftName.readOnly = !state.editor.isNew;
  elements.draftDescription.value = document.description;
  renderGuidedSections(document);
}

function renderGuidedSections(skillDocument) {
  const hasHeadings = skillDocument.sections.some((section) => section.kind === "heading");
  elements.draftSections.hidden = !hasHeadings;
  elements.draftBodyFallbackField.hidden = hasHeadings;
  elements.draftSectionCount.textContent = hasHeadings ? String(skillDocument.sections.length) : "1";

  if (!hasHeadings) {
    elements.draftBodyFallback.value = skillDocument.body;
    elements.draftSections.replaceChildren();
    return;
  }

  const sectionNodes = skillDocument.sections.map((section) => {
    const article = documentNode("article", "section-editor-item");
    article.style.setProperty("--outline-depth", String(Math.min(Math.max(section.level - 1, 0), 3)));

    const header = documentNode("div", "section-editor-header");
    const level = documentNode("span", "heading-level");
    level.textContent = section.kind === "preamble" ? t("editor.body") : `H${section.level}`;

    const title = document.createElement("input");
    title.className = "section-title-input";
    title.value = section.title;
    title.readOnly = !section.titleEditable;
    title.setAttribute("aria-label", section.titleEditable
      ? t("editor.sectionTitle", { title: section.title })
      : section.title);
    if (section.titleEditable) {
      title.addEventListener("change", () => {
        if (!title.value.trim()) {
          title.value = section.title;
          showToast(t("editor.sectionTitleRequired"), true);
          return;
        }
        setDraftMarkdown(updateSkillDocument(state.editor.draftMarkdown, {
          type: "section-title",
          index: section.index,
          value: title.value
        }));
      });
    }

    const toggle = document.createElement("button");
    toggle.type = "button";
    toggle.className = "icon-button compact section-toggle";
    toggle.title = t("editor.collapseSection");
    toggle.setAttribute("aria-label", t("editor.collapseNamed", { title: section.title }));
    toggle.setAttribute("aria-expanded", "true");
    const toggleIcon = document.createElement("i");
    toggleIcon.dataset.lucide = "chevron-down";
    toggle.append(toggleIcon);

    const content = document.createElement("textarea");
    content.className = "section-content-input";
    content.value = section.content;
    content.spellcheck = false;
    content.rows = Math.min(Math.max(section.content.split(/\r?\n/).length + 1, 3), 12);
    content.setAttribute("aria-label", t("editor.sectionContent", { title: section.title }));
    content.addEventListener("input", () => {
      setDraftMarkdown(updateSkillDocument(state.editor.draftMarkdown, {
        type: "section-content",
        index: section.index,
        value: content.value
      }));
    });

    toggle.addEventListener("click", () => {
      const expanded = toggle.getAttribute("aria-expanded") === "true";
      toggle.setAttribute("aria-expanded", String(!expanded));
      toggle.title = expanded ? t("editor.expandSection") : t("editor.collapseSection");
      toggle.setAttribute("aria-label", t(
        expanded ? "editor.expandNamed" : "editor.collapseNamed",
        { title: title.value }
      ));
      article.classList.toggle("is-collapsed", expanded);
      content.hidden = expanded;
      toggleIcon.dataset.lucide = expanded ? "chevron-right" : "chevron-down";
      refreshIcons();
    });

    header.append(level, title, toggle);
    article.append(header, content);
    return article;
  });
  elements.draftSections.replaceChildren(...sectionNodes);
  refreshIcons();
}

function documentNode(tagName, className) {
  const node = document.createElement(tagName);
  node.className = className;
  return node;
}

function editorChanged() {
  return Boolean(state.editor && state.editor.draftMarkdown !== state.editor.originalMarkdown);
}

function updateEditorStatus() {
  elements.draftStatus.textContent = t("editor.notCreated");
  elements.draftStatus.classList.toggle("is-dirty", editorChanged());
  elements.saveDraft.disabled = !state.editor?.preview?.canCreate || state.editor.auditLoading;
}

function setDraftMarkdown(markdown, { syncSource = true } = {}) {
  if (!state.editor) return;
  state.editor.draftMarkdown = markdown;
  if (syncSource) elements.draftSource.value = markdown;
  state.editor.audit = null;
  state.editor.preview = null;
  state.deepAuditPreview = null;
  state.deepAuditContext = null;
  renderDeepAuditResult(null);
  renderCreationPreview(null);
  updateEditorStatus();
  scheduleDraftAudit();
}

function renderCreationPreview(preview) {
  const creating = Boolean(state.editor?.isNew);
  elements.creationPreview.hidden = !creating;
  if (!creating) return;
  elements.creationDestination.textContent = preview?.destination || t("audit.waitingName");
  elements.creationState.textContent = preview?.conflict
    ? t("audit.creationConflict", { source: sourceLabel(preview.conflict.source) })
    : preview?.canCreate
      ? t("audit.createReady")
      : t("audit.resolveBeforeCreate");
  elements.creationPreview.classList.toggle("has-conflict", Boolean(preview?.conflict));
  elements.creationPreview.classList.toggle("is-ready", Boolean(preview?.canCreate));
}

function scheduleDraftAudit() {
  if (!state.editor) return;
  clearTimeout(state.editor.auditTimer);
  state.editor.auditTimer = setTimeout(() => runDraftAudit(), 420);
}

function renderAuditLoading() {
  elements.auditVerdict.textContent = t("audit.loadingTitle");
  elements.auditVerdictBadge.textContent = t("audit.inProgress");
  elements.auditVerdictBadge.className = "verdict-badge is-loading";
  elements.auditSummary.textContent = t("audit.loadingSummary");
  elements.auditDraft.disabled = true;
  updateEditorStatus();
}

function renderAuditError(message) {
  elements.auditVerdict.textContent = t("audit.failed");
  elements.auditVerdictBadge.textContent = t("common.error");
  elements.auditVerdictBadge.className = "verdict-badge is-block";
  elements.auditSummary.textContent = message;
  elements.findingCount.textContent = "0";
  elements.findingList.replaceChildren();
  elements.saveDraft.disabled = true;
}

function renderFinding(item) {
  const row = document.createElement("article");
  row.className = `finding-row ${item.severity}${item.disposition === "dismissed" ? " is-dismissed" : ""}`;

  const heading = document.createElement("div");
  heading.className = "finding-heading";
  const marker = document.createElement("span");
  marker.className = "finding-marker";
  const title = document.createElement("strong");
  const titleKey = `finding.${item.id}.title`;
  const explanationKey = `finding.${item.id}.explanation`;
  title.textContent = t(titleKey) === titleKey ? item.title : t(titleKey);
  const severity = document.createElement("span");
  severity.className = `severity-label is-${item.severity}`;
  severity.textContent = t(`audit.severity.${item.severity}`);
  const confidence = document.createElement("span");
  confidence.className = "confidence-label";
  confidence.textContent = item.disposition === "dismissed"
    ? t("audit.dismissed")
    : t(`audit.confidence.${item.confidence}`);
  heading.append(marker, title, severity, confidence);

  const explanation = document.createElement("p");
  explanation.textContent = t(explanationKey) === explanationKey
    ? item.explanation
    : t(explanationKey);
  const evidence = document.createElement("details");
  const summary = document.createElement("summary");
  summary.textContent = t("audit.viewEvidence");
  const evidenceText = document.createElement("p");
  evidenceText.textContent = item.evidence;
  evidence.append(summary, evidenceText);
  row.append(heading, explanation);
  if (item.filePath) {
    const location = document.createElement("code");
    location.className = "finding-location";
    const lines = item.lineStart
      ? `:${item.lineStart}${item.lineEnd && item.lineEnd !== item.lineStart ? `-${item.lineEnd}` : ""}`
      : "";
    location.textContent = `${item.filePath}${lines}`;
    row.append(location);
  }
  row.append(evidence);
  if (item.reviewNote) {
    const review = document.createElement("p");
    review.className = "review-note";
    review.textContent = t("audit.falsePositiveReview", { note: item.reviewNote });
    row.append(review);
  }
  return row;
}

function renderDeepAuditOutcome(result, view) {
  view.section.hidden = !result;
  if (!result) {
    view.findingList.replaceChildren();
    return;
  }
  const confirmed = result.findings.filter((finding) => finding.disposition !== "dismissed");
  const dismissed = result.findings.length - confirmed.length;
  const verdicts = {
    clear: [t("audit.deep.clearTitle"), t("audit.deep.clearBadge"), t("audit.deep.clearSummary")],
    review: [t("audit.deep.reviewTitle"), t("audit.deep.reviewBadge"), t("audit.deep.reviewSummary")],
    block: [t("audit.deep.blockTitle"), t("audit.deep.blockBadge"), t("audit.deep.blockSummary")]
  };
  const [title, badge, summary] = verdicts[result.verdict] || verdicts.review;
  view.verdict.textContent = title;
  view.badge.textContent = badge;
  view.badge.className = `verdict-badge is-${result.verdict}`;
  view.summary.textContent = summary;
  const apiMode = deepAuditApiModeLabels[result.apiMode] || result.apiMode;
  view.meta.textContent = t("audit.deep.meta", {
    mode: apiMode,
    model: result.model,
    files: result.files.length,
    requests: result.requestCount,
    dismissed: dismissed ? t("audit.deep.dismissedSuffix", { count: dismissed }) : ""
  });
  view.findingList.replaceChildren(...result.findings.map(renderFinding));
  refreshIcons();
}

function renderDeepAuditResult(result) {
  if (state.editor) state.editor.deepAudit = result;
  renderDeepAuditOutcome(result, {
    section: elements.deepResultSection,
    verdict: elements.deepResultVerdict,
    badge: elements.deepResultBadge,
    summary: elements.deepResultSummary,
    meta: elements.deepResultMeta,
    findingList: elements.deepFindingList
  });
}

function renderCandidateDeepAuditResult(result) {
  if (state.candidate) state.candidate.deepAudit = result;
  renderDeepAuditOutcome(result, {
    section: elements.candidateDeepResultSection,
    verdict: elements.candidateDeepResultVerdict,
    badge: elements.candidateDeepResultBadge,
    summary: elements.candidateDeepResultSummary,
    meta: elements.candidateDeepResultMeta,
    findingList: elements.candidateDeepFindingList
  });
  if (result && state.candidate?.context === "update") setUpdateAuditVisible(true);
}

function formatBytes(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(bytes < 10240 ? 1 : 0)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(bytes < 10 * 1024 * 1024 ? 1 : 0)} MB`;
}

function setCandidateSourceMode(mode) {
  state.candidateSourceMode = mode;
  const github = mode === "github";
  elements.candidateGithubMode.classList.toggle("is-active", github);
  elements.candidateLocalMode.classList.toggle("is-active", !github);
  elements.candidateGithubMode.setAttribute("aria-selected", String(github));
  elements.candidateLocalMode.setAttribute("aria-selected", String(!github));
  const sourceStep = !state.repositoryListing && !state.repositoryQueue;
  elements.candidateGithubField.hidden = !sourceStep || !github;
  elements.candidateLocalField.hidden = !sourceStep || github;
  if (github) elements.candidateGithubUrl.focus();
  refreshIcons();
}

function openCandidateIntake() {
  if (elements.candidateIntakeDialog.open) return;
  if (!state.candidateLocalPath) elements.candidateLocalPath.textContent = t("candidate.noFolder");
  setCandidateSourceMode("github");
  if (state.repositoryQueue) renderRepositoryQueue();
  else showCandidateSourceStep();
  elements.candidateIntakeDialog.showModal();
  if (!state.repositoryQueue) elements.candidateGithubUrl.focus();
  refreshIcons();
}

function clearRepositoryListing() {
  state.repositoryListing = null;
  elements.candidateRepositoryListing.hidden = true;
  elements.candidateRepositoryList.replaceChildren();
  elements.stageCandidate.querySelector("span").textContent = t("candidate.stageReview");
}

function setCandidateSourceStepVisible(visible) {
  elements.candidateSourceControl.hidden = !visible;
  elements.candidateGithubField.hidden = !visible || state.candidateSourceMode !== "github";
  elements.candidateLocalField.hidden = !visible || state.candidateSourceMode !== "local";
}

function showCandidateSourceStep({ clearUrl = false } = {}) {
  persistRepositoryQueue(null);
  clearRepositoryListing();
  setCandidateSourceStepVisible(true);
  if (clearUrl) elements.candidateGithubUrl.value = "";
  elements.stageCandidate.disabled = false;
  elements.candidateGithubUrl.focus();
}

async function discardRepositoryQueueSessions() {
  const sessions = drainRepositorySessions(state.repositorySessions);
  const sessionIds = [...new Set(sessions
    .map((candidate) => candidate?.review?.manifest?.sessionId)
    .filter(Boolean))];
  const results = await Promise.allSettled(
    sessionIds.map((sessionId) => desktop.discardStagedCandidate(sessionId))
  );
  const failed = results.find((result) => result.status === "rejected");
  if (failed) throw failed.reason;
}

function isGithubRepositoryRootUrl(value) {
  try {
    const url = new URL(value);
    const segments = url.pathname.split("/").filter(Boolean);
    return url.protocol === "https:"
      && url.hostname === "github.com"
      && !url.search
      && !url.hash
      && (segments.length === 2
        || (segments.length === 4 && segments[2] === "tree"));
  } catch {
    return false;
  }
}

function displayRepositorySkillPath(path) {
  return path || t("candidate.repositoryRootSkill");
}

function selectedRepositoryListingPaths() {
  return [...elements.candidateRepositoryList.querySelectorAll("input:checked")]
    .map((input) => input.value);
}

function renderRepositoryListing(listing) {
  const availability = filterUninstalledRepositoryCandidates(listing, state.skills);
  state.repositoryListing = {
    ...listing,
    candidates: availability.candidates,
    discoveredCount: listing.candidates.length,
    installedCount: availability.installedCount
  };
  setCandidateSourceStepVisible(false);
  elements.candidateRepositoryListing.hidden = false;
  elements.candidateRepositoryTitle.textContent = listing.repository;
  elements.candidateRepositoryVersion.textContent = t("candidate.repositoryRevision", {
    ref: listing.requestedRef,
    sha: listing.resolvedSha.slice(0, 12)
  });
  elements.candidateRepositoryNote.textContent = availability.installedCount === listing.candidates.length
    ? t("candidate.repositoryAllInstalled", { count: availability.installedCount })
    : availability.installedCount > 0
      ? t("candidate.repositoryFilteredNote", {
        available: availability.candidates.length,
        installed: availability.installedCount
      })
      : t("candidate.repositoryListingNote", { count: listing.candidates.length });
  elements.candidateResetRepository.hidden = false;
  elements.candidateResetRepository.querySelector("span").textContent = t("common.back");
  const rows = availability.candidates.map((candidate) => {
    const label = document.createElement("label");
    label.className = "candidate-repository-row";
    const checkbox = document.createElement("input");
    checkbox.type = "checkbox";
    checkbox.value = candidate.skillPath;
    checkbox.checked = false;
    checkbox.addEventListener("change", updateRepositoryListingAction);
    const copy = document.createElement("span");
    const name = document.createElement("strong");
    name.textContent = candidate.repositoryRoot
      ? t("candidate.repositoryRootSkill")
      : candidate.directoryName;
    const path = document.createElement("small");
    path.textContent = candidate.repositoryRoot
      ? t("candidate.repositoryRootLabel")
      : candidate.skillPath;
    copy.append(name, path);
    label.append(checkbox, copy);
    return label;
  });
  elements.candidateRepositoryList.replaceChildren(...rows);
  updateRepositoryListingAction();
}

function renderRepositoryQueue() {
  const queue = state.repositoryQueue;
  if (!queue) return clearRepositoryListing();
  clearRepositoryListing();
  setCandidateSourceStepVisible(false);
  elements.candidateRepositoryListing.hidden = false;
  elements.candidateRepositoryTitle.textContent = t("candidate.queueTitle", {
    count: queue.selectedPaths.length
  });
  elements.candidateRepositoryVersion.textContent = t("candidate.repositoryRevision", {
    ref: queue.requestedRef,
    sha: queue.resolvedSha.slice(0, 12)
  });
  elements.candidateRepositoryNote.textContent = t("candidate.queueNote", {
    repository: queue.sourceUrl,
    current: queue.currentPosition + 1,
    count: queue.selectedPaths.length
  });
  elements.candidateSelectAllRepository.hidden = true;
  elements.candidateResetRepository.hidden = false;
  elements.candidateResetRepository.querySelector("span").textContent = t("candidate.newImport");
  const rows = queue.selectedPaths.map((path, index) => {
    const row = document.createElement("button");
    row.type = "button";
    row.className = "candidate-repository-row is-queue";
    if (index === queue.currentPosition) row.classList.add("is-current");
    const order = document.createElement("span");
    order.className = "candidate-repository-order";
    order.textContent = String(index + 1);
    const copy = document.createElement("span");
    const name = document.createElement("strong");
    name.textContent = displayRepositorySkillPath(path);
    const location = document.createElement("small");
    location.textContent = path || t("candidate.repositoryRootLabel");
    copy.append(name, location);
    row.append(order, copy);
    row.addEventListener("click", () => {
      const selected = setRepositoryQueuePosition(state.repositoryQueue, index);
      if (!selected) return;
      persistRepositoryQueue(selected);
      renderRepositoryQueue();
    });
    return row;
  });
  elements.candidateRepositoryList.replaceChildren(...rows);
  elements.stageCandidate.querySelector("span").textContent = t("candidate.reviewQueueItem");
}

function updateRepositoryListingAction() {
  const selected = selectedRepositoryListingPaths();
  const candidateCount = state.repositoryListing?.candidates.length || 0;
  elements.candidateSelectAllRepository.hidden = candidateCount === 0;
  if (candidateCount === 0) {
    elements.stageCandidate.disabled = true;
    elements.stageCandidate.querySelector("span").textContent = t("candidate.noUninstalledSkills");
    return;
  }
  elements.candidateSelectAllRepository.querySelector("span").textContent = selected.length
    === candidateCount
    ? t("candidate.clearAll")
    : t("candidate.selectAll");
  elements.stageCandidate.disabled = selected.length === 0;
  elements.stageCandidate.querySelector("span").textContent = t("candidate.reviewSelection", {
    count: selected.length
  });
}

function persistRepositoryQueue(queue) {
  state.repositoryQueue = queue;
  if (queue) persistRepositoryReviewQueue(window.localStorage, queue);
  else clearRepositoryReviewQueue(window.localStorage);
}

function renderCandidateQueueNavigation(repositoryReview) {
  const queue = repositoryReview ? state.repositoryQueue : null;
  const visible = Boolean(queue && queue.selectedPaths.length > 1);
  elements.candidatePreviousQueueItem.hidden = !visible;
  elements.candidateNextQueueItem.hidden = !visible;
  if (!visible) return;
  elements.candidatePreviousQueueItem.disabled = state.repositoryNavigationBusy
    || adjacentRepositoryQueuePosition(queue, -1) === null;
  elements.candidateNextQueueItem.disabled = state.repositoryNavigationBusy
    || adjacentRepositoryQueuePosition(queue, 1) === null;
}

async function openRepositoryQueueEntry() {
  const queue = state.repositoryQueue;
  const skillPath = currentRepositoryQueuePath(queue);
  if (!queue || skillPath === null) return;
  let manifest = null;
  elements.stageCandidate.disabled = true;
  elements.stageCandidate.querySelector("span").textContent = t("candidate.staging");
  try {
    const { session: candidate } = await getOrStageRepositorySession(
      state.repositorySessions,
      skillPath,
      async () => {
        manifest = await desktop.stageGithubRepositoryCandidate(
          queue.sourceUrl,
          queue.requestedRef,
          queue.resolvedSha,
          skillPath
        );
        const review = await desktop.getStagedCandidateReview(
          manifest.sessionId,
          manifest.candidateHash
        );
        return {
          review,
          context: "repository",
          repositoryQueuePath: skillPath,
          selectedPath: "SKILL.md",
          preview: null,
          previewSequence: 0
        };
      }
    );
    state.candidate = candidate;
    elements.candidateIntakeDialog.close();
    renderCandidateReview();
    if (!elements.candidateReviewDialog.open) elements.candidateReviewDialog.showModal();
    await selectCandidateFile(candidate.selectedPath || "SKILL.md");
  } catch (error) {
    if (manifest) {
      try {
        await desktop.discardStagedCandidate(manifest.sessionId);
      } catch {
        // The app clears unused staging sessions on its next launch.
      }
    }
    showToast(localizedError(error), true);
  } finally {
    elements.stageCandidate.disabled = false;
    if (state.repositoryQueue) renderRepositoryQueue();
  }
}

async function navigateRepositoryQueue(offset) {
  const candidate = state.candidate;
  const queue = state.repositoryQueue;
  if (state.repositoryNavigationBusy || candidate?.context !== "repository" || !queue) return;
  const position = adjacentRepositoryQueuePosition(queue, offset);
  if (position === null) return;
  const nextQueue = setRepositoryQueuePosition(queue, position);
  if (!nextQueue) return;

  state.repositoryNavigationBusy = true;
  renderCandidateQueueNavigation(true);
  elements.candidateDeepAudit.disabled = true;
  elements.installCandidate.disabled = true;
  try {
    if (state.candidate !== candidate) return;
    state.candidate = null;
    persistRepositoryQueue(nextQueue);
    if (elements.candidateReviewDialog.open) elements.candidateReviewDialog.close();
    renderRepositoryQueue();
    elements.candidateIntakeDialog.showModal();
    refreshIcons();
    await openRepositoryQueueEntry();
  } finally {
    state.repositoryNavigationBusy = false;
    if (state.candidate) renderCandidateReview();
    else renderCandidateQueueNavigation(false);
  }
}

async function chooseCandidateFolder() {
  try {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: t("candidate.chooseFolderTitle")
    });
    if (typeof selected !== "string") return;
    state.candidateLocalPath = selected;
    elements.candidateLocalPath.textContent = selected;
  } catch (error) {
    showToast(localizedError(error), true);
  }
}

function personalExportSkills() {
  return state.skills.filter((skill) => skill.source === "personal");
}

function selectedBundleSkillIds() {
  return [...elements.bundleSkillList.querySelectorAll('input[type="checkbox"]:checked')]
    .map((input) => input.value);
}

function invalidateBundleExportPlan() {
  if (state.bundleExportBusy) return;
  invalidateExportOperations(state.bundleWorkflow);
  elements.previewBundleExport.querySelector("span").textContent = t("bundle.previewExport");
  state.bundleExportPlan = null;
  elements.bundleExportReview.hidden = true;
  elements.bundleExportBlocks.hidden = true;
  elements.bundleExportBlocks.replaceChildren();
  elements.applyBundleExport.disabled = true;
  const checkboxes = [...elements.bundleSkillList.querySelectorAll('input[type="checkbox"]')];
  const selected = checkboxes.filter((input) => input.checked).length;
  elements.bundleSelectionCount.textContent = t("bundle.selectionCount", { count: selected });
  elements.bundleSelectAll.checked = checkboxes.length > 0 && selected === checkboxes.length;
  elements.bundleSelectAll.indeterminate = selected > 0 && selected < checkboxes.length;
  elements.previewBundleExport.disabled = selected === 0;
}

function renderBundleSkillSelection(selectedId = null) {
  const skills = personalExportSkills();
  const rows = skills.map((skill) => {
    const label = document.createElement("label");
    label.className = "bundle-skill-row";
    const checkbox = document.createElement("input");
    checkbox.type = "checkbox";
    checkbox.value = skill.id;
    checkbox.checked = selectedId ? skill.id === selectedId : true;
    checkbox.addEventListener("change", invalidateBundleExportPlan);
    const content = document.createElement("span");
    content.className = "bundle-skill-copy";
    const title = document.createElement("strong");
    title.textContent = skill.displayName;
    const description = document.createElement("small");
    description.textContent = skill.description || skill.directoryName;
    content.append(title, description);
    const count = document.createElement("span");
    count.className = "bundle-skill-file-count";
    count.textContent = t("bundle.skillFileCount", { count: skill.fileCount });
    label.append(checkbox, content, count);
    return label;
  });
  elements.bundleSkillList.replaceChildren(...rows);
  if (!skills.length) {
    const empty = document.createElement("p");
    empty.className = "bundle-export-empty";
    empty.textContent = t("bundle.noExportable");
    elements.bundleSkillList.append(empty);
  }
  invalidateBundleExportPlan();
}

function openBundleExport(selectedId = null) {
  invalidateExportOperations(state.bundleWorkflow);
  state.bundleExportBusy = false;
  renderBundleSkillSelection(selectedId);
  elements.bundleExportReceipt.hidden = true;
  elements.previewBundleExport.hidden = false;
  elements.applyBundleExport.hidden = false;
  elements.cancelBundleExport.textContent = t("common.cancel");
  elements.closeBundleExport.disabled = false;
  elements.cancelBundleExport.disabled = false;
  elements.bundleExportDialog.removeAttribute("aria-busy");
  elements.bundleExportDialog.showModal();
  refreshIcons();
}

function renderBundleExportPlan(plan) {
  state.bundleExportPlan = plan;
  elements.bundleExportReview.hidden = false;
  elements.bundleExportSkillCount.textContent = String(plan.skills.length);
  elements.bundleExportFileCount.textContent = String(plan.totalFiles);
  elements.bundleExportByteCount.textContent = formatBytes(plan.totalBytes);
  const blockRows = plan.blocked.map((finding) => {
    const row = document.createElement("div");
    row.className = "bundle-export-block";
    const icon = document.createElement("i");
    icon.setAttribute("data-lucide", "circle-alert");
    const copy = document.createElement("span");
    const title = document.createElement("strong");
    title.textContent = finding.skillName;
    const detail = document.createElement("small");
    detail.textContent = `${bundleExportBlockMessage(finding)}${finding.relativePath ? ` · ${finding.relativePath}` : ""}`;
    copy.append(title, detail);
    row.append(icon, copy);
    return row;
  });
  elements.bundleExportBlocks.replaceChildren(...blockRows);
  elements.bundleExportBlocks.hidden = blockRows.length === 0;
  elements.applyBundleExport.disabled = !plan.canExport;
  refreshIcons();
}

function bundleExportBlockMessage(finding) {
  const key = `bundle.exportBlock.${finding.ruleId}`;
  const message = t(key);
  return message === key ? t("bundle.exportBlock.default") : message;
}

function bundleExportErrorMessage(error) {
  return localizedError(error);
}

async function exportSingleSkill(skill) {
  if (!skill?.id || state.singleSkillExportBusy) return;
  state.singleSkillExportBusy = true;
  if (state.packageWorkspace?.skill.id === skill.id) elements.packageExport.disabled = true;
  try {
    const plan = await desktop.previewBundleExport([skill.id]);
    const issue = singleSkillExportIssue(plan, skill.id);
    if (issue) {
      showToast(bundleExportBlockMessage(issue), true);
      return;
    }
    const destination = await saveDialog({
      title: t("package.exportDialogTitle", { name: skill.displayName }),
      defaultPath: `${skill.name}.skillbundle`,
      filters: [{ name: t("bundle.fileType"), extensions: ["skillbundle"] }]
    });
    if (typeof destination !== "string") return;
    const receipt = await desktop.exportSkillBundle(plan.planRevision, destination);
    showToast(t("package.exported", {
      name: skill.displayName,
      destination: receipt.destination
    }));
  } catch (error) {
    showToast(bundleExportErrorMessage(error), true);
  } finally {
    state.singleSkillExportBusy = false;
    if (state.packageWorkspace?.skill.id === skill.id) {
      elements.packageExport.disabled = state.packageWorkspace.mutations.length > 0;
    }
  }
}

async function previewBundleExport(event) {
  event.preventDefault();
  const skillIds = selectedBundleSkillIds();
  if (!skillIds.length) return;
  const selectionFingerprint = skillIds.join("\0");
  const operation = beginExportOperation(state.bundleWorkflow, "preview", selectionFingerprint);
  elements.previewBundleExport.disabled = true;
  elements.previewBundleExport.querySelector("span").textContent = t("common.checking");
  try {
    const plan = await desktop.previewBundleExport(skillIds);
    if (!isCurrentExportOperation(state.bundleWorkflow, operation)
      || selectedBundleSkillIds().join("\0") !== selectionFingerprint) return;
    renderBundleExportPlan(plan);
  } catch (error) {
    if (isCurrentExportOperation(state.bundleWorkflow, operation)) {
      showToast(bundleExportErrorMessage(error), true);
    }
  } finally {
    if (isCurrentExportOperation(state.bundleWorkflow, operation)) {
      elements.previewBundleExport.querySelector("span").textContent = t("bundle.previewExport");
      elements.previewBundleExport.disabled = selectedBundleSkillIds().length === 0;
    }
  }
}

function setBundleExportBusy(busy) {
  state.bundleExportBusy = busy;
  elements.closeBundleExport.disabled = busy;
  elements.cancelBundleExport.disabled = busy;
  elements.bundleExportDialog.toggleAttribute("aria-busy", busy);
  elements.bundleSelectAll.disabled = busy;
  for (const checkbox of elements.bundleSkillList.querySelectorAll('input[type="checkbox"]')) {
    checkbox.disabled = busy || !elements.bundleExportReceipt.hidden;
  }
  if (busy) {
    elements.previewBundleExport.disabled = true;
    elements.applyBundleExport.disabled = true;
  } else {
    elements.previewBundleExport.disabled = selectedBundleSkillIds().length === 0;
    elements.applyBundleExport.disabled = !state.bundleExportPlan?.canExport;
  }
}

function requestCloseBundleExport() {
  if (state.bundleExportBusy) return;
  invalidateExportOperations(state.bundleWorkflow);
  state.bundleExportPlan = null;
  elements.bundleExportDialog.close();
}

async function applyBundleExport() {
  const plan = state.bundleExportPlan;
  if (!plan?.canExport) return;
  try {
    const destination = await saveDialog({
      title: t("bundle.exportDialogTitle"),
      defaultPath: "codex-skills.skillbundle",
      filters: [{ name: t("bundle.fileType"), extensions: ["skillbundle"] }]
    });
    if (typeof destination !== "string") return;
    const operation = beginExportOperation(state.bundleWorkflow, "commit", plan.planRevision);
    setBundleExportBusy(true);
    elements.applyBundleExport.querySelector("span").textContent = t("bundle.exporting");
    try {
      const receipt = await desktop.exportSkillBundle(plan.planRevision, destination);
      if (!isCurrentExportOperation(state.bundleWorkflow, operation)
        || !elements.bundleExportDialog.open) return;
    state.bundleExportPlan = null;
    elements.bundleReceiptDestination.textContent = receipt.destination;
    elements.bundleReceiptRevision.textContent = receipt.bundleRevision;
    elements.bundleReceiptSkillCount.textContent = String(receipt.skillCount);
    elements.bundleReceiptFileCount.textContent = String(receipt.fileCount);
    elements.bundleReceiptByteCount.textContent = formatBytes(receipt.archiveBytes);
    elements.serverInstallPrompt.textContent = serverInstallPrompt(
      receipt.destination,
      localization.locale
    );
    elements.bundleExportReceipt.hidden = false;
    for (const checkbox of elements.bundleSkillList.querySelectorAll('input[type="checkbox"]')) {
      checkbox.disabled = true;
    }
    elements.previewBundleExport.hidden = true;
    elements.applyBundleExport.hidden = true;
      elements.cancelBundleExport.textContent = t("common.done");
    } catch (error) {
      if (!isCurrentExportOperation(state.bundleWorkflow, operation)) return;
      if (["BUNDLE_EXPORT_PLAN_STALE", "BUNDLE_EXPORT_SOURCE_CHANGED"].includes(error?.code)) {
        state.bundleExportPlan = null;
        elements.bundleExportReview.hidden = true;
        elements.bundleExportBlocks.hidden = true;
        elements.bundleExportBlocks.replaceChildren();
      }
      showToast(bundleExportErrorMessage(error), true);
    } finally {
      if (finishExportCommit(state.bundleWorkflow, operation)) {
        elements.applyBundleExport.querySelector("span").textContent = t("bundle.chooseExport");
        setBundleExportBusy(false);
      }
    }
  } catch (error) {
    showToast(bundleExportErrorMessage(error), true);
  }
}

function importStatusLabel(status) {
  return {
    compatible: t("bundle.importCompatible"),
    review: t("bundle.importReview"),
    incompatible: t("bundle.importIncompatible")
  }[status] || status;
}

function bundleImportErrorMessage(error) {
  return localizedError(error);
}

function importAuditPresentation(verdict) {
  return {
    clear: [t("audit.baseline.clearTitle"), t("audit.baseline.clearBadge"), t("audit.baseline.clearSummary")],
    review: [t("audit.baseline.reviewTitle"), t("audit.baseline.reviewBadge"), t("audit.baseline.reviewSummary")],
    block: [t("audit.baseline.blockTitle"), t("audit.baseline.blockBadge"), t("audit.baseline.blockSummary")]
  }[verdict] || [t("audit.failed"), t("common.error"), t("error.generic")];
}

function bundleEvidenceDetails(label, value) {
  const details = document.createElement("details");
  details.className = "bundle-import-hash-details";
  const summary = document.createElement("summary");
  summary.textContent = label;
  const code = document.createElement("code");
  code.textContent = value;
  details.append(summary, code);
  return details;
}

function importClassificationPresentation(classification) {
  return {
    new: [t("bundle.classification.new"), "is-new"],
    identical: [t("bundle.classification.identical"), "is-identical"],
    userConflict: [t("bundle.classification.userConflict"), "is-user-conflict"],
    managedConflict: [t("bundle.classification.managedConflict"), "is-managed-conflict"],
    incompatible: [t("bundle.classification.incompatible"), "is-incompatible"]
  }[classification] || [classification, "is-incompatible"];
}

function bundleDecisionSummary(decision) {
  if (localization.locale === "zh-CN" && decision?.summary) return decision.summary;
  const key = decision?.baselineBlocked
    ? decision.classification === "userConflict"
      ? "bundle.decision.userBlocked"
      : "bundle.decision.newBlocked"
    : `bundle.decision.${decision?.classification}`;
  const summary = t(key);
  return summary === key ? t("bundle.unknownDecision") : summary;
}

function bundleOfferSummary(offer) {
  if (!offer) return "";
  const key = offer.kind === "replacePersonal"
    ? "bundle.offer.replace"
    : offer.kind === "createKeepingDormant"
      ? "bundle.offer.createDormant"
      : "bundle.offer.create";
  const summary = t(key);
  return summary === key ? offer.summary : summary;
}

function fileDeltaLabel(status) {
  const key = `bundle.delta.${status}`;
  const label = t(key);
  return label === key ? status : label;
}

function renderCatalogMatches(decision) {
  const container = document.createElement("div");
  container.className = "bundle-import-matches";
  if (!decision?.matches?.length) return container;
  const heading = document.createElement("div");
  heading.className = "bundle-import-subheading";
  heading.textContent = t("bundle.sameNameHeading", { count: decision.matches.length });
  container.append(heading);
  for (const match of decision.matches) {
    const details = document.createElement("details");
    details.className = "bundle-import-match";
    const summary = document.createElement("summary");
    const label = document.createElement("strong");
    label.textContent = `${sourceLabel(match.source)} · ${skillStateLabel(match.state)}`;
    const relation = document.createElement("span");
    relation.textContent = match.identical ? t("bundle.sameRevision") : t("bundle.different");
    summary.append(label, relation);
    const path = document.createElement("code");
    path.textContent = match.path;
    const revision = document.createElement("code");
    revision.textContent = match.revision
      ? t("bundle.revision", { revision: match.revision })
      : t("bundle.revisionUnavailable");
    const deltas = document.createElement("div");
    deltas.className = "bundle-import-deltas";
    for (const delta of match.fileDeltas) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = `bundle-import-compare-button is-${delta.status}`;
      button.dataset.directoryName = decision.directoryName;
      button.dataset.matchId = match.id;
      button.dataset.path = delta.path;
      const filePath = document.createElement("code");
      filePath.textContent = delta.path;
      const status = document.createElement("small");
      status.textContent = fileDeltaLabel(delta.status);
      button.append(filePath, status);
      deltas.append(button);
    }
    details.append(summary, path, revision, deltas);
    container.append(details);
  }
  return container;
}

function renderInstallOffer(decision) {
  if (!decision?.installOffer) return document.createDocumentFragment();
  const label = document.createElement("label");
  label.className = `bundle-install-offer ${decision.classification === "userConflict" ? "is-conflict" : ""}`;
  const checkbox = document.createElement("input");
  checkbox.type = "checkbox";
  checkbox.className = "bundle-install-selection";
  checkbox.dataset.directoryName = decision.directoryName;
  checkbox.dataset.offerToken = decision.installOffer.token;
  checkbox.checked = decision.classification === "new";
  const copy = document.createElement("span");
  const title = document.createElement("strong");
  title.textContent = decision.installOffer.kind === "replacePersonal"
    ? t("bundle.replacePersonal")
    : t("bundle.installVersion");
  const summary = document.createElement("small");
    summary.textContent = bundleOfferSummary(decision.installOffer);
  const destination = document.createElement("code");
  destination.textContent = decision.installOffer.destination;
  copy.append(title, summary, destination);
  label.append(checkbox, copy);
  return label;
}

function selectedBundleInstallSelections() {
  return [...elements.bundleImportSkillList.querySelectorAll(".bundle-install-selection:checked")]
    .map((checkbox) => ({
      directoryName: checkbox.dataset.directoryName,
      offerToken: checkbox.dataset.offerToken
    }));
}

function updateBundleInstallSelection() {
  const review = state.bundleWorkflow.importReview;
  const offerCount = review?.decisions?.filter((decision) => decision.installOffer).length || 0;
  const selected = selectedBundleInstallSelections().length;
  elements.installBundleSkills.hidden = offerCount === 0;
  elements.installBundleSkills.disabled = state.bundleInstallBusy
    || state.bundleInstallReviewStale
    || selected === 0;
  elements.installBundleSkills.querySelector("span").textContent = selected
    ? t("bundle.selectionReview", { count: selected })
    : t("bundle.selectionReviewEmpty");
}

function renderBundleImportReview(review) {
  setImportReview(state.bundleWorkflow, review);
  state.bundleInstallReviewStale = false;
  elements.bundleImportTitle.textContent = t("bundle.importVerified");
  elements.bundleImportPhase.textContent = t("bundle.staged");
  elements.bundleImportMutationState.innerHTML = '<i data-lucide="shield-check"></i><span></span>';
  elements.bundleImportMutationState.querySelector("span").textContent = t("bundle.notInstalled");
  elements.bundleInstallResults.hidden = true;
  elements.bundleInstallResults.replaceChildren();
  const decisions = new Map(review.decisions.map((decision) => [decision.directoryName, decision]));
  elements.bundleImportSkillCount.textContent = String(review.skills.length);
  elements.bundleImportSource.replaceChildren(
    candidateSourceRow(t("bundle.sourceFile"), review.sourceFileName),
    candidateSourceRow(t("bundle.sourceFileRevision"), review.sourceRevision),
    candidateSourceRow(t("bundle.bundleRevision"), review.bundleRevision),
    candidateSourceRow(t("detail.fileCount"), t("bundle.fileCountWithBytes", {
      count: review.totalFiles,
      bytes: formatBytes(review.totalBytes)
    }))
  );
  const sections = review.skills.map((skill) => {
    const decision = decisions.get(skill.directoryName);
    const section = document.createElement("section");
    section.className = "bundle-import-skill";
    const header = document.createElement("div");
    header.className = "bundle-import-skill-header";
    const titleGroup = document.createElement("div");
    titleGroup.className = "bundle-import-title-group";
    const title = document.createElement("h3");
    title.textContent = skill.directoryName;
    const statusGroup = document.createElement("div");
    statusGroup.className = "bundle-import-status-group";
    const status = document.createElement("span");
    status.className = `candidate-status is-${skill.compatibility.status}`;
    status.textContent = importStatusLabel(skill.compatibility.status);
    const [auditTitle, auditLabel, auditSummaryText] = importAuditPresentation(skill.audit.verdict);
    const auditStatus = document.createElement("span");
    auditStatus.className = `verdict-badge is-${skill.audit.verdict}`;
    auditStatus.textContent = t("bundle.auditPrefix", { label: auditLabel });
    const [classificationLabel, classificationClass] = importClassificationPresentation(
      decision?.classification || "incompatible"
    );
    const classification = document.createElement("span");
    classification.className = `bundle-import-classification ${classificationClass}`;
    classification.textContent = classificationLabel;
    titleGroup.append(title, classification);
    statusGroup.append(status, auditStatus);
    header.append(titleGroup, statusGroup);
    const skillRevision = bundleEvidenceDetails(t("bundle.viewRevision"), skill.revision);
    const files = document.createElement("div");
    files.className = "bundle-import-file-list";
    for (const file of skill.files) {
      const button = document.createElement("button");
      button.className = "bundle-import-file-button";
      button.type = "button";
      button.dataset.directoryName = skill.directoryName;
      button.dataset.path = file.path;
      const path = document.createElement("code");
      path.textContent = file.path;
      const meta = document.createElement("small");
      meta.textContent = `${formatBytes(file.size)}${file.executableAfterInstall ? ` · ${t("bundle.executableAfterInstall")}` : ""}`;
      button.append(path, meta);
      const entry = document.createElement("div");
      entry.className = "bundle-import-file-entry";
      entry.append(button, bundleEvidenceDetails(t("bundle.viewSha"), file.sha256));
      files.append(entry);
    }
    const checks = document.createElement("div");
    checks.className = "bundle-import-checks";
    for (const check of skill.compatibility.checks) {
      const row = document.createElement("div");
      row.className = `bundle-import-check is-${check.status}`;
      const label = document.createElement("strong");
      label.textContent = compatibilityCheckLabel(check);
      const detail = document.createElement("small");
      detail.textContent = compatibilityCheckDetail(check);
      row.append(label, detail);
      checks.append(row);
    }
    const findings = document.createElement("div");
    findings.className = "bundle-import-findings finding-list";
    findings.replaceChildren(...skill.audit.findings.map(renderFinding));
    const compatibilityHeading = document.createElement("div");
    compatibilityHeading.className = "bundle-import-subheading";
    compatibilityHeading.textContent = t("bundle.compatibilityHeading");
    const auditHeading = document.createElement("div");
    auditHeading.className = "bundle-import-audit-heading";
    const auditTitleElement = document.createElement("strong");
    auditTitleElement.textContent = auditTitle;
    const auditSummary = document.createElement("p");
    auditSummary.textContent = auditSummaryText;
    auditHeading.append(auditTitleElement, auditSummary);
    const filesHeading = document.createElement("div");
    filesHeading.className = "bundle-import-subheading";
    filesHeading.textContent = t("bundle.verifiedFiles");
    const decisionSummary = document.createElement("p");
    decisionSummary.className = "bundle-import-decision-summary";
    decisionSummary.textContent = bundleDecisionSummary(decision);
    const matches = renderCatalogMatches(decision);
    const offer = renderInstallOffer(decision);
    section.append(
      header,
      decisionSummary,
      skillRevision,
      compatibilityHeading,
      checks,
      auditHeading,
      findings,
      filesHeading,
      files,
      matches,
      offer
    );
    return section;
  });
  elements.bundleImportSkillList.replaceChildren(...sections);
  elements.bundleImportStatus.textContent = t("bundle.stagedNotInstalled");
  elements.bundleImportPreviewTitle.textContent = t("candidate.filePreview");
  elements.bundleImportPreviewEmpty.hidden = false;
  elements.bundleImportPreviewEmpty.textContent = t("bundle.previewPrompt");
  elements.bundleImportFilePreview.hidden = true;
  elements.bundleImportCurrentFilePreview.hidden = true;
  updateBundleInstallSelection();
  refreshIcons();
}

function markBundleInstallReviewStale(message) {
  state.bundleInstallReviewStale = true;
  for (const control of elements.bundleImportSkillList.querySelectorAll(
    ".bundle-install-selection, .bundle-import-compare-button, .bundle-import-file-button"
  )) {
    control.disabled = true;
  }
  elements.installBundleSkills.disabled = true;
  elements.bundleImportStatus.textContent = message;
}

function renderBundleInstallOutcomes(result) {
  const rows = result.outcomes.map((outcome) => {
    const row = document.createElement("div");
    row.className = `bundle-install-result is-${outcome.status}`;
    const heading = document.createElement("div");
    const name = document.createElement("strong");
    name.textContent = outcome.directoryName;
    const status = document.createElement("span");
    const statusKey = `bundle.status.${outcome.status}`;
    const statusLabel = t(statusKey);
    status.textContent = statusLabel === statusKey ? outcome.status : statusLabel;
    const message = document.createElement("small");
    message.textContent = localization.locale === "zh-CN" ? outcome.message : status.textContent;
    heading.append(name, status);
    row.append(heading, message);
    return row;
  });
  elements.bundleInstallResults.replaceChildren(...rows);
  elements.bundleInstallResults.hidden = rows.length === 0;
  elements.bundleImportTitle.textContent = t("bundle.installResult");
  elements.bundleImportPhase.textContent = t("bundle.receipt");
  elements.bundleImportMutationState.innerHTML = '<i data-lucide="list-checks"></i><span></span>';
  elements.bundleImportMutationState.querySelector("span").textContent = t("bundle.processed");
  refreshIcons();
}

async function openBundleImport() {
  try {
    const selected = await openDialog({
      multiple: false,
      title: t("bundle.chooseImport"),
      filters: [{ name: t("bundle.fileType"), extensions: ["skillbundle"] }]
    });
    if (typeof selected !== "string") return;
    elements.importBundleButton.disabled = true;
    state.bundleInstallResult = null;
    const review = await desktop.stageSkillBundle(selected);
    renderBundleImportReview(review);
    elements.bundleImportDialog.showModal();
    refreshIcons();
  } catch (error) {
    showToast(bundleImportErrorMessage(error), true);
  } finally {
    elements.importBundleButton.disabled = false;
  }
}

async function previewImportedBundleFile(directoryName, path) {
  if (state.bundleInstallReviewStale) return;
  const operation = beginImportPreview(state.bundleWorkflow);
  if (!operation) return;
  elements.bundleImportPreviewTitle.textContent = path;
  elements.bundleImportPreviewEmpty.textContent = t("bundle.previewLoading");
  elements.bundleImportPreviewEmpty.hidden = false;
  elements.bundleImportFilePreview.hidden = true;
  elements.bundleImportCurrentFilePreview.hidden = true;
  try {
    const result = await desktop.readImportedBundleFile(
      operation.sessionId,
      operation.bundleRevision,
      directoryName,
      path
    );
    if (!isCurrentImportPreview(state.bundleWorkflow, operation)
      || !elements.bundleImportDialog.open) return;
    if (!result.isText) {
      elements.bundleImportPreviewEmpty.textContent = t("bundle.previewBinary");
      return;
    }
    elements.bundleImportPreviewEmpty.hidden = true;
    elements.bundleImportFilePreview.hidden = false;
    elements.bundleImportFilePreview.textContent = result.content || "";
    if (result.truncated) {
      elements.bundleImportPreviewEmpty.hidden = false;
      elements.bundleImportPreviewEmpty.textContent = t("bundle.installPreviewTruncated", {
        bytes: formatBytes(result.previewBytes)
      });
    }
  } catch (error) {
    if (isCurrentImportPreview(state.bundleWorkflow, operation)
      && elements.bundleImportDialog.open) {
      elements.bundleImportPreviewEmpty.textContent = bundleImportErrorMessage(error);
    }
  }
}

function comparisonSideText(label, side) {
  if (!side.exists) return `${label}\n\n${t("bundle.fileComparisonMissing")}`;
  const metadata = `${formatBytes(side.size)} · SHA-256 ${side.sha256}${side.executable ? ` · ${t("common.executable")}` : ""}`;
  if (!side.isText) return `${label}\n${metadata}\n\n${t("bundle.fileComparisonNonText")}`;
  const truncated = side.truncated
    ? `\n\n${t("bundle.fileComparisonTruncated", { bytes: formatBytes(side.previewBytes) })}`
    : "";
  return `${label}\n${metadata}\n\n${side.content || ""}${truncated}`;
}

async function compareImportedBundleFile(directoryName, matchId, path) {
  if (state.bundleInstallReviewStale) return;
  const operation = beginImportPreview(state.bundleWorkflow);
  if (!operation) return;
  elements.bundleImportPreviewTitle.textContent = t("bundle.compareTitle", { path });
  elements.bundleImportPreviewEmpty.hidden = false;
  elements.bundleImportPreviewEmpty.textContent = t("bundle.compareLoading");
  elements.bundleImportFilePreview.hidden = true;
  elements.bundleImportCurrentFilePreview.hidden = true;
  try {
    const result = await desktop.compareImportedBundleFile(
      operation.sessionId,
      operation.bundleRevision,
      directoryName,
      matchId,
      path
    );
    if (!isCurrentImportPreview(state.bundleWorkflow, operation)
      || !elements.bundleImportDialog.open) return;
    elements.bundleImportPreviewEmpty.hidden = true;
    elements.bundleImportFilePreview.hidden = false;
    elements.bundleImportCurrentFilePreview.hidden = false;
    elements.bundleImportFilePreview.textContent = comparisonSideText(t("bundle.importedVersion"), result.imported);
    elements.bundleImportCurrentFilePreview.textContent = comparisonSideText(t("bundle.currentVersion"), result.current);
  } catch (error) {
    if (isCurrentImportPreview(state.bundleWorkflow, operation)
      && elements.bundleImportDialog.open) {
      elements.bundleImportPreviewEmpty.hidden = false;
      elements.bundleImportPreviewEmpty.textContent = bundleImportErrorMessage(error);
    }
  }
}

function requestBundleInstall() {
  const review = state.bundleWorkflow.importReview;
  const selections = selectedBundleInstallSelections();
  if (!review || selections.length === 0 || state.bundleInstallBusy) return;
  const selectedNames = new Set(selections.map((selection) => selection.directoryName));
  const selectedDecisions = review.decisions.filter((decision) => selectedNames.has(decision.directoryName));
  const replacements = selectedDecisions.filter(
    (decision) => decision.installOffer?.kind === "replacePersonal"
  );
  const lines = selectedDecisions.map((decision) => {
    const action = decision.installOffer?.kind === "replacePersonal"
      ? t("bundle.installCurrent")
      : t("bundle.installEnabled");
    return `${decision.directoryName}：${action}`;
  });
  const recovery = replacements.length ? t("bundle.recoveryNotice") : "";
  presentConfirmation({
    title: t("bundle.installConfirmTitle"),
    message: `${lines.join("\n")}\n\n${t("bundle.installConfirmMessage", { recovery })}`,
    label: t("bundle.selectionReview", { count: selections.length }),
    tone: replacements.length ? "danger" : "primary",
    action: () => performBundleInstall(review, selections)
  });
}

function bundleInstallStatusText(result) {
  const installed = result.outcomes.filter((outcome) => ["installed", "replaced"].includes(outcome.status)).length;
  const skipped = result.outcomes.filter((outcome) => outcome.status === "skippedIdentical").length;
  const failed = result.outcomes.filter((outcome) => outcome.status === "failed").length;
  const restart = installed && result.restartRecommended ? t("bundle.restartSuffix") : "";
  return {
    failed,
    text: t("bundle.installStatus", {
      installed,
      skipped: skipped ? t("bundle.skippedSuffix", { count: skipped }) : "",
      failed: failed ? t("bundle.failedSuffix", { count: failed }) : "",
      restart
    })
  };
}

async function performBundleInstall(review, selections) {
  state.bundleInstallBusy = true;
  elements.closeBundleImport.disabled = true;
  elements.discardBundleImport.disabled = true;
  updateBundleInstallSelection();
  try {
    const result = await desktop.installImportedBundle(
      review.sessionId,
      review.bundleRevision,
      review.reviewRevision,
      selections
    );
    state.bundleInstallResult = result;
    for (const outcome of result.outcomes) {
      if (!outcome.skill) continue;
      applyCatalogState(applyInstallOutcome(state.skills, state.counts, outcome));
    }
    const { failed, text: statusText } = bundleInstallStatusText(result);
    renderBundleInstallOutcomes(result);
    elements.bundleImportStatus.textContent = statusText;
    if (result.catalogRefreshNeeded) {
      await loadSkills({ preserveSelection: false, forceRefresh: true });
    }
    try {
      const refreshed = await desktop.reviewImportedBundle(review.sessionId, review.bundleRevision);
      renderBundleImportReview(refreshed);
      renderBundleInstallOutcomes(result);
      elements.bundleImportStatus.textContent = statusText;
    } catch (refreshError) {
      markBundleInstallReviewStale(t("bundle.receiptRetained"));
      showToast(t("bundle.receiptRefreshFailed", {
        message: bundleImportErrorMessage(refreshError)
      }), true);
    }
    showToast(failed ? t("bundle.partialFailure") : t("bundle.allProcessed"), failed);
  } catch (error) {
    if (["BUNDLE_INSTALL_REVIEW_STALE", "BUNDLE_INSTALL_MATCH_UNKNOWN"].includes(error?.code)) {
      try {
        const refreshed = await desktop.reviewImportedBundle(review.sessionId, review.bundleRevision);
        renderBundleImportReview(refreshed);
      } catch {
        markBundleInstallReviewStale(t("bundle.staleRefreshFailed"));
      }
    }
    throw new Error(bundleImportErrorMessage(error));
  } finally {
    state.bundleInstallBusy = false;
    elements.closeBundleImport.disabled = false;
    elements.discardBundleImport.disabled = false;
    updateBundleInstallSelection();
  }
}

async function discardBundleImport() {
  if (state.bundleInstallBusy) return;
  const review = clearImportReview(state.bundleWorkflow);
  if (elements.bundleImportDialog.open) elements.bundleImportDialog.close();
  let cleanupError = null;
  if (review) {
    try {
      await desktop.discardImportedBundle(review.sessionId);
    } catch (error) {
      cleanupError = error;
    }
  }
  if (cleanupError && !state.bundleWorkflow.importReview) {
    showToast(bundleImportErrorMessage(cleanupError), true);
  }
}

function candidateSourceRow(label, value) {
  const row = document.createElement("div");
  const term = document.createElement("dt");
  term.textContent = label;
  const detail = document.createElement("dd");
  detail.textContent = value;
  detail.title = value;
  row.append(term, detail);
  return row;
}

function compatibilityCheckLabel(check) {
  const key = `compat.${check.id}.label`;
  const label = t(key);
  return label === key ? check.label : label;
}

function compatibilityCheckDetail(check) {
  const key = `compat.${check.id}.${check.status}`;
  const fallbackKey = `compat.${check.id}.detail`;
  const detail = t(key);
  if (detail !== key) return detail;
  const fallback = t(fallbackKey);
  return fallback === fallbackKey ? check.detail : fallback;
}

function renderCandidateSource(manifest) {
  const source = manifest.source;
  const requestedRef = source.requestedRef || source.requested_ref;
  const resolvedSha = source.resolvedSha || source.resolvedSHA || source.resolved_sha;
  const skillPath = source.skillPath || source.skill_path;
  const selectedPath = source.selectedPath || source.selected_path;
  const rows = source.kind === "github"
    ? [
      candidateSourceRow(t("candidate.source"), t("candidate.githubPublic")),
      candidateSourceRow(t("candidate.repository"), source.repository),
      candidateSourceRow(t("candidate.requestedRef"), requestedRef),
      candidateSourceRow(t("candidate.resolvedCommit"), resolvedSha),
      candidateSourceRow(t("candidate.skillPath"), skillPath || t("candidate.repositoryRoot")),
      candidateSourceRow(t("candidate.hash"), manifest.candidateHash)
    ]
    : [
      candidateSourceRow(t("candidate.source"), t("candidate.localSource")),
      candidateSourceRow(t("candidate.selectedPath"), selectedPath),
      candidateSourceRow(t("candidate.hash"), manifest.candidateHash)
    ];
  elements.candidateSourceList.replaceChildren(...rows);
}

function renderCandidateCompatibility(compatibility) {
  const labels = {
    compatible: t("candidate.compatible"),
    review: t("candidate.needsReview"),
    incompatible: t("candidate.incompatible")
  };
  elements.candidateCompatibilityStatus.textContent = labels[compatibility.status] || compatibility.status;
  elements.candidateCompatibilityStatus.className = `candidate-status is-${compatibility.status}`;
  elements.candidateCompatibilitySummary.textContent = localization.locale === "zh-CN"
    ? compatibility.summary
    : labels[compatibility.status];
  const rows = compatibility.checks.map((check) => {
    const row = document.createElement("div");
    row.className = `candidate-compatibility-row is-${check.status}`;
    const marker = document.createElement("i");
    marker.dataset.lucide = check.status === "pass"
      ? "circle-check"
      : check.status === "review"
        ? "circle-alert"
        : "circle-x";
    const copy = document.createElement("div");
    const label = document.createElement("strong");
    label.textContent = compatibilityCheckLabel(check);
    const detail = document.createElement("p");
    detail.textContent = compatibilityCheckDetail(check);
    copy.append(label, detail);
    row.append(marker, copy);
    return row;
  });
  elements.candidateCompatibilityList.replaceChildren(...rows);
}

function renderCandidateFiles() {
  const candidate = state.candidate;
  if (!candidate) return;
  const files = candidate.context === "update"
    ? candidate.updateFiles.filter((file) => file.status !== "unchanged")
    : candidate.review.manifest.files;
  const rows = files.map((file) => {
    const row = document.createElement("button");
    row.type = "button";
    row.className = `candidate-file-row${candidate.selectedPath === file.path ? " is-selected" : ""}`;
    row.setAttribute("aria-pressed", String(candidate.selectedPath === file.path));
    const icon = document.createElement("i");
    icon.dataset.lucide = file.executable ? "terminal" : "file-text";
    const copy = document.createElement("span");
    const path = document.createElement("strong");
    path.textContent = file.path;
    const meta = document.createElement("small");
    if (candidate.context === "update") {
      const stateLabel = document.createElement("em");
      stateLabel.className = `candidate-file-state is-${file.status}`;
      stateLabel.textContent = t(`update.file.${file.status}`);
      path.append(stateLabel);
      const sourceFile = file.remote || file.local;
      meta.textContent = `${formatBytes(sourceFile.size)}${file.modeChanged ? ` · ${t("update.permissionChanged")}` : ""}`;
    } else {
      meta.textContent = `${formatBytes(file.size)} · SHA-256 ${file.sha256.slice(0, 12)}…${file.executable ? ` · ${t("common.executable")}` : ""}`;
    }
    copy.append(path, meta);
    row.append(icon, copy);
    row.addEventListener("click", () => selectCandidateFile(file.path));
    return row;
  });
  elements.candidateFiles.replaceChildren(...rows);
}

function candidateUpdateFiles(result) {
  return buildUpdateComparison(result.localFiles, result.manifest.files);
}

function renderCandidatePreview() {
  const candidate = state.candidate;
  const preview = candidate?.preview;
  if (!candidate?.selectedPath) {
    elements.candidatePreviewTitle.textContent = t("candidate.filePreview");
    elements.candidatePreviewMeta.textContent = "";
    elements.candidatePreviewEmpty.hidden = false;
    elements.candidateFilePreview.hidden = true;
    return;
  }
  const file = candidate.review.manifest.files.find((item) => item.path === candidate.selectedPath);
  elements.candidatePreviewTitle.textContent = candidate.selectedPath;
  elements.candidatePreviewMeta.textContent = file ? `SHA-256 ${file.sha256.slice(0, 12)}…` : "";
  if (preview?.loading) {
    elements.candidatePreviewEmpty.textContent = t("candidate.previewLoading");
    elements.candidatePreviewEmpty.hidden = false;
    elements.candidateFilePreview.hidden = true;
    return;
  }
  if (!preview?.isText) {
    elements.candidatePreviewEmpty.textContent = t("candidate.previewBinary");
    elements.candidatePreviewEmpty.hidden = false;
    elements.candidateFilePreview.hidden = true;
    return;
  }
  elements.candidatePreviewEmpty.hidden = true;
  elements.candidateFilePreview.hidden = false;
  elements.candidateFilePreview.textContent = preview.content || "";
}

function renderUpdateFilePreview() {
  const candidate = state.candidate;
  if (candidate?.context !== "update") return;
  const comparison = candidate.updateFiles.find((file) => file.path === candidate.selectedPath);
  elements.candidateComparisonPath.textContent = candidate.selectedPath || t("update.selectChangedFile");
  if (!comparison) return;
  const local = candidate.localPreview;
  const remote = candidate.preview;
  const attribution = updateAttribution(candidate.updateCheck.status);
  elements.candidateUpdateAttribution.textContent = t(`update.attribution.${attribution}`);
  elements.candidateUpdateAttribution.className = `update-attribution is-${attribution}`;
  const syncAction = candidate.skill.source === "personal" && comparison.status === "added"
    ? "add"
    : candidate.skill.source === "personal" && comparison.status === "removed"
      ? "delete"
      : candidate.skill.source === "personal" && comparison.status === "modified"
        ? "replace"
      : null;
  elements.candidateSyncFile.hidden = !syncAction;
  elements.candidateSyncFile.dataset.action = syncAction || "";
  if (syncAction) {
    elements.candidateSyncFile.querySelector("span").textContent = t(`update.sync.${syncAction}`);
    elements.candidateSyncFile.className = `${
      syncAction === "replace" && attribution !== "remote"
        ? "danger-button"
        : "primary-button"
    } compact-button`;
  }
  const renderSide = (file, preview, empty, content, meta, missingKey) => {
    meta.textContent = file ? `${formatBytes(file.size)} · ${file.sha256.slice(0, 12)}…` : "";
    if (!file) {
      empty.textContent = t(missingKey);
      empty.hidden = false;
      content.hidden = true;
    } else if (preview?.loading) {
      empty.textContent = t("candidate.previewLoading");
      empty.hidden = false;
      content.hidden = true;
    } else if (!(preview?.isText || preview?.mediaType === "text")) {
      empty.textContent = t("candidate.previewBinary");
      empty.hidden = false;
      content.hidden = true;
    } else {
      empty.hidden = true;
      content.hidden = false;
      content.textContent = preview.content || "";
    }
  };
  renderSide(
    comparison.local,
    local,
    elements.candidateLocalPreviewEmpty,
    elements.candidateLocalFilePreview,
    elements.candidateLocalFileMeta,
    "update.notOnLocal"
  );
  renderSide(
    comparison.remote,
    remote,
    elements.candidateRemotePreviewEmpty,
    elements.candidateRemoteFilePreview,
    elements.candidateRemoteFileMeta,
    "update.notOnGithub"
  );
  const canDiff = comparison.status === "modified"
    && (local?.isText || local?.mediaType === "text")
    && remote?.isText;
  if (!canDiff) elements.candidateUnifiedDiff.replaceChildren();
  elements.candidateUnifiedDiff.hidden = !canDiff;
  const renderedUnifiedDiff = canDiff
    ? renderUnifiedDiff(local.content || "", remote.content || "")
    : false;
  document.querySelector(".candidate-comparison-columns").hidden = canDiff && renderedUnifiedDiff;
}

function updateAttribution(status) {
  if (status === "remoteChanged") return "remote";
  if (status === "localChanged") return "local";
  return "unknown";
}

async function requestCandidateFileSync() {
  const candidate = state.candidate;
  if (candidate?.context !== "update") return;
  const comparison = candidate.updateFiles.find((file) => file.path === candidate.selectedPath);
  const action = elements.candidateSyncFile.dataset.action;
  if (!comparison || !["add", "delete", "replace"].includes(action)) return;
  elements.candidateSyncFile.disabled = true;
  try {
    const preview = await desktop.previewStagedCandidateFileSync(
      candidate.skill.id,
      candidate.review.manifest.sessionId,
      candidate.review.manifest.candidateHash,
      candidate.updateCheck.localRevision,
      comparison.path,
      action
    );
    if (state.candidate !== candidate || !preview.canApply) return;
    const attribution = updateAttribution(candidate.updateCheck.status);
    const messageKey = action === "replace"
      ? `update.syncMessage.replace.${attribution}`
      : `update.syncMessage.${action}`;
    presentConfirmation({
      title: t(`update.syncTitle.${action}`),
      message: t(messageKey, { path: comparison.path }),
      label: t(`update.syncConfirm.${action}`),
      action: () => performCandidateFileSync(candidate, comparison, action, preview),
      tone: action === "delete" || (action === "replace" && attribution !== "remote")
        ? "danger"
        : "primary"
    });
  } catch (error) {
    showToast(localizedError(error), true);
  } finally {
    if (state.candidate === candidate) elements.candidateSyncFile.disabled = false;
  }
}

async function performCandidateFileSync(candidate, comparison, action, preview) {
  const result = await desktop.applyStagedCandidateFileSync(
    candidate.skill.id,
    candidate.review.manifest.sessionId,
    candidate.review.manifest.candidateHash,
    candidate.updateCheck.localRevision,
    preview.proposedRevision,
    comparison.path,
    action
  );
  try {
    await desktop.discardStagedCandidate(candidate.review.manifest.sessionId);
  } catch {
    // The committed local change is authoritative; stale staging clears on restart.
  }
  if (state.candidate === candidate) state.candidate = null;
  elements.candidateReviewDialog.close();
  state.detail = result.skill;
  applyCatalogState(replaceCatalogSkill(
    state.skills,
    state.counts,
    candidate.skill.id,
    result.skill
  ));
  renderDetail();
  showToast(t(`update.synced.${action}`, { path: comparison.path }));
  await checkGithubSkillUpdate(result.skill);
}

function renderUnifiedDiff(localContent, remoteContent) {
  const diff = createLineDiff(localContent, remoteContent);
  if (diff.truncated) {
    elements.candidateUnifiedDiff.replaceChildren();
    const notice = document.createElement("p");
    notice.className = "candidate-diff-truncated";
    notice.textContent = t("update.diffTooLarge");
    elements.candidateUnifiedDiff.append(notice);
    return false;
  }
  const rows = diff.rows.map((row) => renderDiffLine(row));
  elements.candidateUnifiedDiff.replaceChildren(...rows);
  return true;
}

function renderDiffLine(row) {
  const line = document.createElement("div");
  line.className = `candidate-diff-line is-${row.kind}`;
  if (row.kind === "skip") {
    line.textContent = t("update.unchangedLines", { count: row.count });
    return line;
  }
  const oldNumber = document.createElement("span");
  oldNumber.textContent = row.oldLine ?? "";
  const newNumber = document.createElement("span");
  newNumber.textContent = row.newLine ?? "";
  const marker = document.createElement("span");
  marker.textContent = row.kind === "add" ? "+" : row.kind === "remove" ? "-" : " ";
  const content = document.createElement("code");
  content.textContent = row.text || " ";
  line.append(oldNumber, newNumber, marker, content);
  return line;
}

function renderUpdateSummary(candidate) {
  const result = candidate.updateCheck;
  const status = result.status;
  elements.candidateUpdateTitle.textContent = t(`update.title.${status}`);
  elements.candidateUpdateDescription.textContent = updateCheckMessage(result);
  elements.candidateUpdateSummary.className = `update-summary is-${status}`;
  elements.candidateUpdateSummary.hidden = false;
}

function setUpdateAuditVisible(visible) {
  const candidate = state.candidate;
  if (candidate?.context !== "update") return;
  candidate.auditVisible = visible;
  elements.candidateReviewPane.classList.toggle("is-audit-visible", visible);
  elements.candidateToggleAudit.querySelector("span").textContent = visible
    ? t("update.hideChecks")
    : t("update.showChecks");
}

function renderCandidateAudit(audit) {
  const verdicts = {
    clear: [t("audit.baseline.clearTitle"), t("audit.baseline.clearBadge"), t("audit.baseline.clearSummary")],
    review: [t("audit.baseline.reviewTitle"), t("audit.baseline.reviewBadge"), t("audit.baseline.reviewSummary")],
    block: [t("audit.baseline.blockTitle"), t("audit.baseline.blockBadge"), t("audit.baseline.blockSummary")]
  };
  const [title, badge, summary] = verdicts[audit.verdict] || verdicts.review;
  elements.candidateAuditVerdict.textContent = title;
  elements.candidateAuditBadge.textContent = badge;
  elements.candidateAuditBadge.className = `verdict-badge is-${audit.verdict}`;
  elements.candidateAuditSummary.textContent = summary;
  elements.candidateFindingCount.textContent = String(audit.findings.length);
  elements.candidateFindingList.replaceChildren(...audit.findings.map(renderFinding));
}

function renderCandidateSkipped(entries) {
  elements.candidateSkippedCount.textContent = String(entries.length);
  elements.candidateSkippedList.replaceChildren();
  if (entries.length === 0) {
    elements.candidateSkippedSummary.textContent = t("candidate.allIncluded");
    return;
  }
  elements.candidateSkippedSummary.textContent = t("candidate.excludedIntro");
  const rows = entries.map((entry) => {
    const row = document.createElement("p");
    row.textContent = `${entry.path} · ${entry.reason}`;
    return row;
  });
  elements.candidateSkippedList.replaceChildren(...rows);
}

function renderCandidateReview() {
  const candidate = state.candidate;
  if (!candidate) return;
  const { review } = candidate;
  const name = review.audit.document.name || t("candidate.defaultName");
  elements.candidateReviewTitle.textContent = name;
  renderCandidateSource(review.manifest);
  renderCandidateCompatibility(review.compatibility);
  const updateReview = candidate.context === "update";
  const repositoryReview = candidate.context === "repository";
  const changedFiles = updateReview
    ? candidate.updateFiles.filter((file) => file.status !== "unchanged")
    : review.manifest.files;
  elements.candidateFileCount.textContent = String(changedFiles.length);
  elements.candidateFilesTitle.textContent = updateReview ? t("update.changedFiles") : t("candidate.stagedFiles");
  renderCandidateFiles();
  renderCandidatePreview();
  renderCandidateAudit(review.audit);
  renderCandidateDeepAuditResult(candidate.deepAudit || null);
  renderCandidateSkipped(review.skippedEntries);
  const blocked = review.compatibility.status === "incompatible" || review.audit.verdict === "block";
  elements.candidateUpdateSummary.hidden = !updateReview;
  elements.candidateSourceSection.hidden = updateReview;
  elements.candidateCompatibilitySection.hidden = updateReview;
  elements.candidatePreviewSection.hidden = updateReview;
  elements.candidateUpdateComparison.hidden = !updateReview;
  elements.candidateReviewPane.classList.toggle("is-update-review", updateReview);
  if (updateReview) {
    renderUpdateSummary(candidate);
    renderUpdateFilePreview();
    setUpdateAuditVisible(Boolean(candidate.auditVisible));
  }
  elements.candidateReviewKicker.textContent = updateReview
    ? t("update.reviewKicker")
    : repositoryReview
      ? t("candidate.repositoryReviewKicker")
      : t("candidate.reviewKicker");
  elements.candidateMutationState.textContent = updateReview
    ? t("update.reviewOnly")
    : repositoryReview && state.repositoryQueue
      ? t("candidate.queuePosition", {
        current: state.repositoryQueue.currentPosition + 1,
        count: state.repositoryQueue.selectedPaths.length
      })
      : t("candidate.notInstalled");
  renderCandidateQueueNavigation(repositoryReview);
  elements.installCandidate.hidden = updateReview;
  elements.installCandidate.disabled = updateReview || blocked || state.repositoryNavigationBusy;
  elements.candidateDeepAudit.disabled = state.repositoryNavigationBusy;
  elements.installCandidate.title = blocked
    ? t("candidate.installBlockedTitle")
    : t("candidate.installReadyTitle");
  refreshIcons();
}

function updateCheckMessage(result) {
  return t(`update.status.${result.status}`, {
    local: result.localCandidateHash.slice(0, 12),
    remote: result.remoteSha.slice(0, 12),
    installed: result.installedSha?.slice(0, 12) || t("update.unknownRevision")
  });
}

async function openCheckedUpdateReview(skill, result) {
  let review = null;
  try {
    review = await desktop.getStagedCandidateReview(
      result.manifest.sessionId,
      result.manifest.candidateHash
    );
    const updateFiles = candidateUpdateFiles(result);
    const firstChanged = updateFiles.find((file) => file.status !== "unchanged");
    state.candidate = {
      review,
      context: "update",
      updateCheck: result,
      updateFiles,
      skill,
      selectedPath: firstChanged?.path || "SKILL.md",
      preview: null,
      localPreview: null,
      previewSequence: 0
    };
    renderCandidateReview();
    elements.candidateReviewDialog.showModal();
    await selectCandidateFile(state.candidate.selectedPath);
  } catch (error) {
    if (!review) {
      try {
        await desktop.discardStagedCandidate(result.manifest.sessionId);
      } catch {
        // The app clears unused staging sessions on its next launch.
      }
    }
    throw error;
  }
}

async function checkGithubSkillUpdate(skill) {
  const button = [...document.querySelectorAll("#detail-actions button")]
    .find((item) => item.textContent.trim() === t("update.check"));
  if (button) button.disabled = true;
  try {
    const result = await desktop.checkGithubSkillUpdate(skill.id);
    if (result.status === "identical") {
      try {
        await desktop.discardStagedCandidate(result.manifest.sessionId);
      } catch {
        // The app clears unused staging sessions on its next launch.
      }
      elements.updateResultDescription.textContent = t("update.status.identical", {
        remote: result.remoteSha.slice(0, 12)
      });
      elements.updateResultRepository.textContent = result.manifest.source.repository;
      elements.updateResultCommit.textContent = result.remoteSha;
      elements.updateResultDialog.showModal();
      refreshIcons();
      return;
    }
    await openCheckedUpdateReview(skill, result);
  } catch (error) {
    showToast(localizedError(error), true);
  } finally {
    if (button?.isConnected) button.disabled = false;
  }
}

function candidateVersionLabel(manifest) {
  if (manifest.source.kind === "github") {
    const resolvedSha = manifest.source.resolvedSha
      || manifest.source.resolvedSHA
      || manifest.source.resolved_sha;
    const version = resolvedSha
      ? t("candidate.versionCommit", { value: resolvedSha.slice(0, 12) })
      : t("candidate.versionHash", { value: manifest.candidateHash.slice(0, 12) });
    return `${manifest.source.repository} · ${version}`;
  }
  return manifest.source.selectedPath || manifest.source.selected_path;
}

async function performCandidateInstall(candidate, preview) {
  const { manifest } = candidate.review;
  elements.installCandidate.disabled = true;
  elements.installCandidate.querySelector("span").textContent = t("candidate.installing");
  try {
    const result = await desktop.installStagedCandidate(
      manifest.sessionId,
      manifest.candidateHash,
      preview.installRevision
    );
    try {
      await desktop.discardStagedCandidate(manifest.sessionId);
    } catch {
      // Installation has already committed; stale staging is cleared on the next launch.
    }
    if (candidate.context === "repository") {
      removeRepositorySession(state.repositorySessions, candidate.repositoryQueuePath);
    }
    if (state.candidate === candidate) state.candidate = null;
    if (elements.candidateReviewDialog.open) elements.candidateReviewDialog.close();
    if (result.status === "skippedIdentical") {
      if (result.skill) {
        state.selectedId = result.skill.id;
        state.detail = result.skill;
        elements.detailPanel.classList.add("is-open");
        renderDetail();
      }
      showToast(t("candidate.skippedIdentical"));
    } else if (result.skill) {
      state.selectedId = result.skill.id;
      state.detail = result.skill;
      applyCatalogState(addCatalogSkill(state.skills, state.counts, result.skill));
      elements.detailPanel.classList.add("is-open");
      renderDetail();
      showToast(t("candidate.installed"));
    } else {
      await loadSkills({ preserveSelection: false, forceRefresh: true });
      const installed = state.skills.find((skill) => skill.source === "personal" && skill.name === preview.name);
      if (installed) await selectSkill(installed.id);
      showToast(t("candidate.installedReindexed"));
    }
    if (result.status === "installed" && !result.provenanceRecorded) {
      showToast(t("provenance.recordFailed"), true);
    }
    if (candidate.context === "repository" && state.repositoryQueue) {
      persistRepositoryQueue(removeCurrentRepositoryQueuePath(state.repositoryQueue));
      if (state.repositoryQueue) {
        renderRepositoryQueue();
        elements.candidateIntakeDialog.showModal();
      } else {
        showToast(t("candidate.queueComplete"));
      }
      refreshIcons();
    }
  } finally {
    if (state.candidate === candidate) {
      elements.installCandidate.disabled = false;
      elements.installCandidate.querySelector("span").textContent = t("candidate.install");
    }
  }
}

async function requestCandidateInstall() {
  const candidate = state.candidate;
  if (!candidate) return;
  const { review } = candidate;
  if (review.compatibility.status === "incompatible" || review.audit.verdict === "block") return;
  elements.installCandidate.disabled = true;
  elements.installCandidate.querySelector("span").textContent = t("candidate.checkingInstall");
  try {
    const preview = await desktop.previewStagedCandidateInstall(
      review.manifest.sessionId,
      review.manifest.candidateHash
    );
    if (state.candidate !== candidate) return;
    if (!preview.canInstall) {
      const conflictPath = preview.conflict?.path;
      showToast(
        conflictPath
          ? t("candidate.installConflict", { path: conflictPath })
          : t("candidate.cannotInstall"),
        true
      );
      return;
    }
    const auditState = preview.auditVerdict === "review"
      ? t("candidate.auditReview")
      : t("candidate.auditClear");
    const deepAuditState = candidate.deepAudit
      ? t("candidate.deepState", { state: {
        clear: t("candidate.deepClear"),
        review: t("candidate.deepReview"),
        block: t("candidate.deepBlock")
      }[candidate.deepAudit.verdict] || candidate.deepAudit.verdict })
      : t("candidate.deepNotRun");
    presentConfirmation({
      title: t("candidate.installTitle", { name: preview.name }),
      message: [
        t("candidate.installSource", { value: candidateVersionLabel(review.manifest) }),
        t("candidate.installDestination", { value: preview.destination }),
        t("candidate.installFiles", { count: preview.fileCount }),
        t("candidate.installAudit", { value: auditState }),
        deepAuditState,
        t("candidate.installGuarantee")
      ].join("\n"),
      label: t("candidate.confirmInstall"),
      action: () => performCandidateInstall(candidate, preview),
      tone: "primary"
    });
  } catch (error) {
    showToast(localizedError(error), true);
  } finally {
    if (state.candidate === candidate) {
      elements.installCandidate.disabled = false;
      elements.installCandidate.querySelector("span").textContent = t("candidate.install");
    }
  }
}

async function requestCandidateDeepAudit() {
  const candidate = state.candidate;
  if (!candidate) return;
  elements.candidateDeepAudit.disabled = true;
  try {
    const settings = await desktop.getDeepAuditSettings();
    state.deepAuditSettings = settings;
    if (!settings.hasApiKey || !settings.endpoint || !settings.model) {
      showToast(t("deep.notConfigured"), true);
      return;
    }
    const { manifest } = candidate.review;
    const preview = await desktop.previewStagedCandidateDeepAudit(
      manifest.sessionId,
      manifest.candidateHash
    );
    if (state.candidate !== candidate) return;
    if (preview.sourceRevision !== manifest.candidateHash) {
      throw new Error(t("candidate.changed"));
    }
    state.deepAuditContext = { kind: "candidate", candidate };
    state.deepAuditPreview = preview;
    renderDeepAuditConsent(preview);
    elements.deepConsentDialog.showModal();
    refreshIcons();
  } catch (error) {
    showToast(localizedError(error), true);
  } finally {
    elements.candidateDeepAudit.disabled = false;
  }
}

async function selectCandidateFile(path) {
  const candidate = state.candidate;
  if (!candidate) return;
  candidate.selectedPath = path;
  const sequence = (candidate.previewSequence || 0) + 1;
  candidate.previewSequence = sequence;
  candidate.preview = { loading: true };
  if (candidate.context === "update") candidate.localPreview = { loading: true };
  renderCandidateFiles();
  renderCandidatePreview();
  renderUpdateFilePreview();
  try {
    const comparison = candidate.context === "update"
      ? candidate.updateFiles.find((file) => file.path === path)
      : null;
    const [preview, localPreview] = await Promise.all([
      comparison?.remote === null
        ? Promise.resolve(null)
        : desktop.readStagedCandidateFile(
          candidate.review.manifest.sessionId,
          candidate.review.manifest.candidateHash,
          path
        ),
      candidate.context === "update" && comparison?.local
        ? desktop.readSkillPackageFile(
          candidate.skill.id,
          candidate.updateCheck.localRevision,
          path
        )
        : Promise.resolve(null)
    ]);
    if (state.candidate !== candidate || candidate.previewSequence !== sequence) return;
    candidate.preview = preview;
    candidate.localPreview = localPreview;
    renderCandidatePreview();
    renderUpdateFilePreview();
  } catch (error) {
    if (state.candidate !== candidate || candidate.previewSequence !== sequence) return;
    candidate.preview = null;
    candidate.localPreview = null;
    renderCandidatePreview();
    renderUpdateFilePreview();
    showToast(localizedError(error), true);
  }
}

async function stageCandidate(event) {
  event.preventDefault();
  if (state.repositoryQueue) {
    await openRepositoryQueueEntry();
    return;
  }
  if (state.repositoryListing) {
    const selectedPaths = selectedRepositoryListingPaths();
    const queue = createRepositoryReviewQueue({
      sourceUrl: elements.candidateGithubUrl.value.trim(),
      requestedRef: state.repositoryListing.requestedRef,
      resolvedSha: state.repositoryListing.resolvedSha,
      selectedPaths
    });
    if (!queue) {
      showToast(t("candidate.selectRepositorySkill"), true);
      return;
    }
    persistRepositoryQueue(queue);
    renderRepositoryQueue();
    await openRepositoryQueueEntry();
    return;
  }
  const github = state.candidateSourceMode === "github";
  const source = github ? elements.candidateGithubUrl.value.trim() : state.candidateLocalPath;
  if (!source) {
    showToast(github ? t("candidate.enterGithub") : t("candidate.selectFolder"), true);
    return;
  }
  let manifest = null;
  elements.stageCandidate.disabled = true;
  elements.stageCandidate.querySelector("span").textContent = t("candidate.staging");
  try {
    if (github && isGithubRepositoryRootUrl(source)) {
      const listing = await desktop.listGithubRepositoryCandidates(source);
      if (listing.candidates.length === 0) {
        throw new Error(t("candidate.noRepositorySkills"));
      }
      renderRepositoryListing(listing);
      return;
    }
    manifest = github
      ? await desktop.stageGithubCandidate(source)
      : await desktop.stageLocalCandidate(source);
    const review = await desktop.getStagedCandidateReview(manifest.sessionId, manifest.candidateHash);
    state.candidate = {
      review,
      context: "intake",
      selectedPath: "SKILL.md",
      preview: null,
      previewSequence: 0
    };
    elements.candidateIntakeDialog.close();
    renderCandidateReview();
    elements.candidateReviewDialog.showModal();
    await selectCandidateFile("SKILL.md");
  } catch (error) {
    if (manifest) {
      try {
        await desktop.discardStagedCandidate(manifest.sessionId);
      } catch {
        // The app clears unused staging sessions on its next launch.
      }
    }
    showToast(localizedError(error), true);
  } finally {
    elements.stageCandidate.disabled = false;
    if (!state.repositoryListing && !state.repositoryQueue) {
      elements.stageCandidate.querySelector("span").textContent = t("candidate.stageReview");
    }
  }
}

async function closeCandidateReview() {
  const candidate = state.candidate;
  state.candidate = null;
  if (elements.candidateReviewDialog.open) elements.candidateReviewDialog.close();
  if (!candidate) return;
  if (candidate.context === "repository" && state.repositoryQueue) {
    renderRepositoryQueue();
    elements.candidateIntakeDialog.showModal();
    refreshIcons();
    return;
  }
  try {
    await desktop.discardStagedCandidate(candidate.review.manifest.sessionId);
  } catch (error) {
    showToast(localizedError(error), true);
  }
}

function clearConnectionTestResult() {
  state.providerTestSequence += 1;
  elements.deepConnectionStatus.hidden = true;
  elements.deepConnectionStatus.textContent = "";
  elements.deepConnectionStatus.className = "connection-status";
}

function renderCredentialState() {
  const settings = state.deepAuditSettings;
  const hasKey = Boolean(settings?.hasApiKey);
  elements.deepApiKey.placeholder = hasKey
    ? t("settings.keyStoredPlaceholder")
    : t("settings.keyPlaceholder");
  elements.deepCredentialState.textContent = hasKey
    ? t("settings.keyStored")
    : t("settings.keyWillStore");
}

async function openSettings() {
  const focusControl = () => elements.deepEndpoint.focus();
  try {
    if (elements.settingsDialog.open) {
      focusControl();
      return;
    }
    const settings = await desktop.getDeepAuditSettings();
    state.deepAuditSettings = settings;
    clearConnectionTestResult();
    elements.deepApiMode.value = settings.apiMode || "chatCompletions";
    elements.deepEndpoint.value = settings.endpoint;
    elements.deepModel.value = settings.model;
    elements.deepApiKey.value = "";
    renderCredentialState();
    elements.clearDeepSettings.disabled = !settings.hasApiKey && !settings.endpoint && !settings.model;
    elements.settingsDialog.showModal();
    focusControl();
    refreshIcons();
  } catch (error) {
    showToast(localizedError(error), true);
  }
}

async function testDeepAuditConnection() {
  const sequence = ++state.providerTestSequence;
  const apiKey = elements.deepApiKey.value.trim() || null;
  elements.testDeepConnection.disabled = true;
  elements.testDeepConnection.querySelector("span").textContent = t("settings.testing");
  elements.deepConnectionStatus.hidden = false;
  elements.deepConnectionStatus.className = "connection-status";
  elements.deepConnectionStatus.textContent = t("settings.connecting");
  try {
    const result = await desktop.testDeepAuditConnection(
      elements.deepApiMode.value,
      elements.deepEndpoint.value,
      elements.deepModel.value,
      apiKey
    );
    if (sequence !== state.providerTestSequence) return;
    const apiMode = deepAuditApiModeLabels[result.apiMode] || result.apiMode;
    elements.deepConnectionStatus.classList.add("is-success");
    elements.deepConnectionStatus.textContent = t("settings.connectionSuccess", {
      mode: apiMode,
      endpoint: result.endpoint
    });
  } catch (error) {
    if (sequence !== state.providerTestSequence) return;
    elements.deepConnectionStatus.classList.add("is-error");
    elements.deepConnectionStatus.textContent = t("settings.connectionFailed", {
      message: localizedError(error)
    });
  } finally {
    elements.testDeepConnection.disabled = false;
    elements.testDeepConnection.querySelector("span").textContent = t("settings.test");
  }
}

async function saveDeepAuditSettings(event) {
  event.preventDefault();
  elements.saveDeepSettings.disabled = true;
  try {
    const apiKey = elements.deepApiKey.value.trim() || null;
    const settings = await desktop.saveDeepAuditSettings(
      elements.deepApiMode.value,
      elements.deepEndpoint.value,
      elements.deepModel.value,
      apiKey
    );
    state.deepAuditSettings = settings;
    elements.settingsDialog.close();
    showToast(t("settings.saved"));
  } catch (error) {
    showToast(localizedError(error), true);
  } finally {
    elements.saveDeepSettings.disabled = false;
  }
}

function renderDeepAuditConsent(preview) {
  elements.deepConsentApiMode.textContent = deepAuditApiModeLabels[preview.apiMode] || preview.apiMode;
  elements.deepConsentEndpoint.textContent = preview.endpoint;
  elements.deepConsentModel.textContent = preview.model;
  elements.deepConsentRequests.textContent = t("consent.requestCount", {
    count: preview.requestCount
  });
  const fileRows = preview.files.map((file) => {
    const label = document.createElement("label");
    label.className = "consent-file-row";
    const checkbox = document.createElement("input");
    checkbox.type = "checkbox";
    checkbox.value = file.path;
    checkbox.checked = true;
    checkbox.disabled = file.required;
    const copy = document.createElement("span");
    const path = document.createElement("strong");
    path.textContent = file.path;
    const metadata = document.createElement("small");
    metadata.textContent = t("deep.fileMetadata", {
      size: formatBytes(file.size),
      hash: `${file.sha256.slice(0, 12)}…`,
      required: file.required ? t("deep.requiredSuffix") : ""
    });
    copy.append(path, metadata);
    label.append(checkbox, copy);
    return label;
  });
  elements.deepConsentFiles.replaceChildren(...fileRows);
  elements.deepSkippedFiles.hidden = preview.skippedFiles.length === 0;
  elements.deepSkippedSummary.textContent = t("deep.skippedSummary", {
    count: preview.skippedFiles.length
  });
  elements.deepSkippedList.replaceChildren(...preview.skippedFiles.map((file) => {
    const row = document.createElement("p");
    row.textContent = `${file.path} · ${file.reason}`;
    return row;
  }));
}

async function requestDeepAudit() {
  if (!state.editor) return;
  elements.deepAudit.disabled = true;
  try {
    const settings = await desktop.getDeepAuditSettings();
    state.deepAuditSettings = settings;
    if (!settings.hasApiKey || !settings.endpoint || !settings.model) {
      showToast(t("deep.notConfigured"), true);
      return;
    }
    const preview = await desktop.previewDeepAudit(null, state.editor.draftMarkdown);
    state.deepAuditContext = { kind: "editor", editor: state.editor };
    state.deepAuditPreview = preview;
    renderDeepAuditConsent(preview);
    elements.deepConsentDialog.showModal();
    refreshIcons();
  } catch (error) {
    showToast(localizedError(error), true);
  } finally {
    elements.deepAudit.disabled = false;
  }
}

async function requestPackageDeepAudit() {
  const workspace = state.packageWorkspace;
  if (!workspace?.snapshot.editable) return;
  elements.packageDeepAudit.disabled = true;
  try {
    const settings = await desktop.getDeepAuditSettings();
    state.deepAuditSettings = settings;
    if (!settings.hasApiKey || !settings.endpoint || !settings.model) {
      showToast(t("deep.notConfigured"), true);
      return;
    }
    if (workspace.mutations.length && (!workspace.preview || !workspace.audit)) {
      const reviewed = await previewPackageChanges();
      if (!reviewed || state.packageWorkspace !== workspace) return;
    } else if (!workspace.audit) {
      await runPackageBaselineAudit(workspace);
      if (!workspace.audit || state.packageWorkspace !== workspace) return;
    }
    const proposedRevision = workspace.preview?.proposedRevision || workspace.snapshot.revision;
    const preview = await desktop.previewSkillPackageDeepAudit(
      workspace.skill.id,
      workspace.snapshot.revision,
      proposedRevision,
      workspace.mutations
    );
    if (state.packageWorkspace !== workspace) return;
    state.deepAuditContext = { kind: "package", workspace, proposedRevision };
    state.deepAuditPreview = preview;
    renderDeepAuditConsent(preview);
    elements.deepConsentDialog.showModal();
    refreshIcons();
  } catch (error) {
    showToast(localizedError(error), true);
  } finally {
    elements.packageDeepAudit.disabled = false;
  }
}

async function performDeepAudit(event) {
  event.preventDefault();
  if (!state.deepAuditPreview || !state.deepAuditContext) return;
  const context = state.deepAuditContext;
  const editor = context.kind === "editor" ? context.editor : null;
  const candidate = context.kind === "candidate" ? context.candidate : null;
  const packageWorkspace = context.kind === "package" ? context.workspace : null;
  if (context.kind === "editor" && state.editor !== editor) return;
  if (context.kind === "candidate" && state.candidate !== candidate) return;
  if (context.kind === "package" && state.packageWorkspace !== packageWorkspace) return;
  const selections = [...elements.deepConsentFiles.querySelectorAll("input")]
    .filter((input) => input.checked)
    .map((input) => {
      const file = state.deepAuditPreview.files.find((item) => item.path === input.value);
      return file ? { path: file.path, sha256: file.sha256 } : null;
    })
    .filter(Boolean);
  elements.runDeepAudit.disabled = true;
  elements.runDeepAudit.querySelector("span").textContent = t("consent.running");
  try {
    const result = context.kind === "candidate"
      ? await desktop.runStagedCandidateDeepAudit(
        candidate.review.manifest.sessionId,
        state.deepAuditPreview.sourceRevision,
        selections,
        state.deepAuditPreview.candidateHash,
        state.deepAuditPreview.providerHash
      )
      : context.kind === "package"
        ? await desktop.runSkillPackageDeepAudit(
          packageWorkspace.skill.id,
          packageWorkspace.snapshot.revision,
          context.proposedRevision,
          packageWorkspace.mutations,
          selections,
          state.deepAuditPreview.candidateHash,
          state.deepAuditPreview.providerHash
        )
        : await desktop.runDeepAudit(
        null,
        editor.draftMarkdown,
        selections,
        state.deepAuditPreview.candidateHash,
        state.deepAuditPreview.providerHash
      );
    elements.deepConsentDialog.close();
    if (context.kind === "candidate" && state.candidate === candidate) {
      renderCandidateDeepAuditResult(result);
    } else if (context.kind === "package" && state.packageWorkspace === packageWorkspace) {
      renderPackageDeepAuditResult(result);
      updatePackageSaveState();
    } else if (context.kind === "editor" && state.editor === editor) {
      renderDeepAuditResult(result);
    }
    showToast(t("consent.completed"));
  } catch (error) {
    elements.deepConsentDialog.close();
    showToast(localizedError(error), true);
  } finally {
    elements.runDeepAudit.disabled = false;
    elements.runDeepAudit.querySelector("span").textContent = t("consent.run");
    state.deepAuditPreview = null;
    state.deepAuditContext = null;
  }
}

function baselineAuditPresentation(verdict) {
  const verdicts = {
    clear: {
      title: t("audit.baseline.clearTitle"),
      badge: t("audit.baseline.clearBadge"),
      summary: t("audit.baseline.clearSummary")
    },
    review: {
      title: t("audit.baseline.reviewTitle"),
      badge: t("audit.baseline.reviewBadge"),
      summary: t("audit.baseline.reviewSummary")
    },
    block: {
      title: t("audit.baseline.blockTitle"),
      badge: t("audit.baseline.blockBadge"),
      summary: t("audit.baseline.blockSummary")
    }
  };
  return verdicts[verdict] || verdicts.review;
}

function renderBaselineAuditOutcome(audit, view) {
  const verdict = baselineAuditPresentation(audit.verdict);
  view.verdict.textContent = verdict.title;
  view.badge.textContent = verdict.badge;
  view.badge.className = `verdict-badge is-${audit.verdict}`;
  view.summary.textContent = verdict.summary;
  view.findingCount.textContent = String(audit.findings.length);
  view.findingList.replaceChildren(...audit.findings.map(renderFinding));
  refreshIcons();
}

function renderDraftAudit(audit) {
  renderBaselineAuditOutcome(audit, {
    verdict: elements.auditVerdict,
    badge: elements.auditVerdictBadge,
    summary: elements.auditSummary,
    findingCount: elements.findingCount,
    findingList: elements.findingList
  });

  const totalChanges = audit.diff.addedCount + audit.diff.removedCount;
  elements.diffCount.textContent = String(totalChanges);
  elements.diffEmpty.hidden = audit.diff.changed;
  elements.diffView.hidden = !audit.diff.changed;
  if (audit.diff.changed) {
    elements.diffBefore.textContent = audit.diff.before.join("\n") || t("common.emptyContent");
    elements.diffAfter.textContent = audit.diff.after.join("\n") || t("common.emptyContent");
  }
  elements.auditDraft.disabled = false;
  updateEditorStatus();
  refreshIcons();
}

async function runDraftAudit() {
  if (!state.editor) return;
  clearTimeout(state.editor.auditTimer);
  const sequence = ++state.auditSequence;
  const editorId = state.editor.id;
  const markdown = state.editor.draftMarkdown;
  state.editor.auditLoading = true;
  renderAuditLoading();
  try {
    const result = await desktop.previewNewSkill(markdown);
    if (!state.editor || state.editor.id !== editorId || sequence !== state.auditSequence) return;
    const audit = result.audit;
    state.editor.audit = audit;
    state.editor.preview = result;
    renderCreationPreview(state.editor.preview);
    renderDraftAudit(audit);
  } catch (error) {
    if (sequence === state.auditSequence) renderAuditError(localizedError(error));
  } finally {
    if (state.editor && state.editor.id === editorId && sequence === state.auditSequence) {
      state.editor.auditLoading = false;
      elements.auditDraft.disabled = false;
      updateEditorStatus();
    }
  }
}

function newSkillMarkdown() {
  if (localization.locale === "zh-CN") {
    return `---\nname: new-skill\ndescription: >-\n  当用户需要处理可重复执行的任务时使用。\n---\n\n# 新技能\n\n## 工作步骤\n\n1. 阅读用户请求和已有上下文。\n2. 完成任务并执行必要检查。\n3. 返回简洁、实用的结果。\n`;
  }
  return `---\nname: new-skill\ndescription: >-\n  Use when the user asks for a repeatable task.\n---\n\n# New Skill\n\n## Workflow\n\n1. Read the request and the available context.\n2. Complete the task with the required checks.\n3. Return a concise, useful result.\n`;
}

async function openNewSkill() {
  const markdown = newSkillMarkdown();
  state.editor = {
    id: "new",
    originalMarkdown: markdown,
    draftMarkdown: markdown,
    audit: null,
    preview: null,
    auditLoading: false,
    auditTimer: null,
    isNew: true
  };
  state.deepAuditPreview = null;
  state.deepAuditContext = null;
  renderDeepAuditResult(null);
  elements.editorTitle.textContent = t("editor.newTitle");
  elements.saveDraft.innerHTML = '<i data-lucide="plus"></i><span></span>';
  elements.saveDraft.querySelector("span").textContent = t("editor.create");
  elements.draftSource.value = markdown;
  syncGuidedFields();
  setEditorMode("source");
  renderCreationPreview(null);
  updateEditorStatus();
  elements.editorDialog.showModal();
  refreshIcons();
  await runDraftAudit();
}

async function performDraftCreate() {
  if (!state.editor?.isNew || !state.editor.preview?.canCreate) return;
  const { draftMarkdown, preview } = state.editor;
  elements.saveDraft.disabled = true;
  try {
    const created = await desktop.createSkill(draftMarkdown, preview.draftHash);
    elements.editorDialog.close();
    state.editor = null;
    showToast(t("editor.created"));
    await loadSkills({ preserveSelection: false });
    await selectSkill(created.id);
  } catch (error) {
    showToast(localizedError(error), true);
  } finally {
    if (state.editor) updateEditorStatus();
  }
}

function requestDraftSave() {
  if (!state.editor?.isNew || !state.editor.audit || state.editor.audit.verdict === "block"
    || !state.editor.preview?.canCreate) return;
  presentConfirmation({
    title: t("editor.createConfirmTitle"),
    message: state.editor.deepAudit?.verdict === "block"
      ? t("editor.createRiskMessage", { destination: state.editor.preview.destination })
      : t("editor.createMessage", { destination: state.editor.preview.destination }),
    label: t("editor.confirmCreate"),
    action: performDraftCreate,
    tone: "primary"
  });
}

function requestCloseEditor() {
  if (!state.editor || !editorChanged()) {
    elements.editorDialog.close();
    state.editor = null;
    return;
  }
  presentConfirmation({
    title: t("editor.discardTitle"),
    message: t("editor.discardMessage"),
    label: t("editor.discard"),
    action: async () => {
      elements.editorDialog.close();
      state.editor = null;
    }
  });
}

async function requestSkillLifecycle(action, skill) {
  const config = {
    disable: [t("lifecycle.disableTitle"), t("lifecycle.disableMessage", { name: skill.displayName }), t("lifecycle.disableConfirm"), "primary", "lifecycle.actionDisable"],
    enable: [t("lifecycle.enableTitle"), t("lifecycle.enableMessage", { name: skill.displayName }), t("lifecycle.enableConfirm"), "primary", "lifecycle.actionEnable"],
    archive: [t("lifecycle.archiveTitle"), t("lifecycle.archiveMessage", { name: skill.displayName }), t("lifecycle.archiveConfirm"), "primary", "lifecycle.actionArchive"],
    restore: [t("lifecycle.restoreTitle"), t("lifecycle.restoreMessage", { name: skill.displayName }), t("lifecycle.restoreConfirm"), "primary", "lifecycle.actionRestore"],
    delete: [t("lifecycle.deleteTitle"), t("lifecycle.deleteMessage", { name: skill.displayName }), t("lifecycle.deleteConfirm"), "danger", null]
  }[action];
  try {
    const preview = await desktop.previewSkillLifecycle(skill.id, action);
    if (!preview.canApply) {
      showToast(t("lifecycle.destinationConflict", {
        path: preview.conflict?.path || preview.destination
      }), true);
      return;
    }
    const apply = action === "delete"
      ? async () => {
        const result = await desktop.deleteArchivedSkill(skill.id, preview.directoryRevision, elements.confirmName.value);
        state.selectedId = null;
        state.detail = null;
        applyCatalogState(removeCatalogSkill(state.skills, state.counts, skill.id));
        elements.detailPanel.classList.remove("is-open");
        renderDetail();
        state.collections = result.collections.collections;
        renderCollections();
        showToast(t("lifecycle.deleted", { name: skill.displayName }));
      }
      : async () => {
        const result = await desktop.applySkillLifecycle(skill.id, action, preview.directoryRevision);
        state.selectedId = result.skill.id;
        state.detail = result.skill;
        applyCatalogState(replaceCatalogSkill(state.skills, state.counts, skill.id, result.skill));
        elements.detailPanel.classList.add("is-open");
        renderDetail();
        state.collections = result.collections.collections;
        renderCollections();
        showToast(t("lifecycle.completed", { action: t(config[4]) }));
      };
    presentConfirmation({
      title: config[0],
      message: `${config[1]}${preview.destination ? `\n${t("lifecycle.destination", { path: preview.destination })}` : ""}`,
      label: config[2],
      action: apply,
      tone: config[3],
      requiredName: action === "delete" ? skill.name : null
    });
  } catch (error) {
    showToast(localizedError(error), true);
  }
}

function showToast(message, isError = false) {
  clearTimeout(state.toastTimer);
  const toastHost = elements.candidateReviewDialog.open
    ? elements.candidateReviewDialog
    : elements.candidateIntakeDialog.open
      ? elements.candidateIntakeDialog
      : elements.bundleExportDialog.open
        ? elements.bundleExportDialog
      : elements.bundleImportDialog.open
        ? elements.bundleImportDialog
      : elements.deepConsentDialog.open
    ? elements.deepConsentDialog
      : elements.settingsDialog.open
        ? elements.settingsDialog
        : elements.packageDialog.open
          ? elements.packageDialog
          : elements.editorDialog.open
            ? elements.editorDialog
            : document.body;
  if (elements.toast.parentElement !== toastHost) toastHost.append(elements.toast);
  elements.toast.classList.toggle("is-error", isError);
  elements.toastMessage.textContent = message;
  elements.toast.hidden = false;
  state.toastTimer = setTimeout(() => {
    elements.toast.hidden = true;
    if (elements.toast.parentElement !== document.body) document.body.append(elements.toast);
  }, isError ? 6500 : 4200);
}

async function loadSkills({ preserveSelection = true, selectedDetail = null, forceRefresh = false } = {}) {
  elements.refresh.classList.add("is-spinning");
  elements.refresh.disabled = true;
  try {
    const data = forceRefresh ? await desktop.refreshSkills() : await api("/api/skills");
    state.skills = data.skills;
    state.counts = data.counts;
    state.roots = data.roots;
    state.codexHome = data.codexHome;
    if (selectedDetail && state.skills.some((skill) => skill.id === selectedDetail.id)) {
      state.selectedId = selectedDetail.id;
      state.detail = selectedDetail;
      elements.detailPanel.classList.add("is-open");
      renderDetail();
    } else if (!preserveSelection || !state.skills.some((skill) => skill.id === state.selectedId)) {
      state.selectedId = null;
      state.detail = null;
      renderDetail();
    }
    updateCounts();
    renderList();
    refreshIcons();
  } catch (error) {
    showToast(localizedError(error), true);
  } finally {
    elements.refresh.classList.remove("is-spinning");
    elements.refresh.disabled = false;
  }
}

function refreshLocalizedInterface() {
  updateCounts();
  renderCollections();
  renderList();
  renderDetail();
  if (state.packageWorkspace) {
    elements.packageTitle.textContent = state.packageWorkspace.skill.displayName;
    renderPackageTree();
    renderPackageReview(state.packageWorkspace.preview);
    if (state.packageWorkspace.auditLoading) renderPackageAuditLoading();
    else renderPackageAudit(state.packageWorkspace.audit);
    renderPackageDeepAuditResult(state.packageWorkspace.deepAudit);
  }

  if (state.editor) {
    elements.editorTitle.textContent = t("editor.newTitle");
    const saveLabel = t("editor.create");
    const saveText = elements.saveDraft.querySelector("span");
    if (saveText) saveText.textContent = saveLabel;
    syncGuidedFields();
    renderCreationPreview(state.editor.preview);
    if (state.editor.audit) renderDraftAudit(state.editor.audit);
    else if (state.editor.auditLoading) renderAuditLoading();
    renderDeepAuditResult(state.editor.deepAudit || null);
    updateEditorStatus();
  }

  if (state.candidate) renderCandidateReview();

  if (state.candidateLocalPath) {
    elements.candidateLocalPath.textContent = state.candidateLocalPath;
  } else {
    elements.candidateLocalPath.textContent = t("candidate.noFolder");
  }

  const review = state.bundleWorkflow.importReview;
  if (review) {
    const wasStale = state.bundleInstallReviewStale;
    renderBundleImportReview(review);
    if (state.bundleInstallResult) {
      renderBundleInstallOutcomes(state.bundleInstallResult);
      elements.bundleImportStatus.textContent = bundleInstallStatusText(state.bundleInstallResult).text;
    }
    if (wasStale) markBundleInstallReviewStale(t("bundle.receiptRetained"));
  }

  if (!elements.bundleExportReceipt.hidden && elements.bundleReceiptDestination.textContent) {
    elements.serverInstallPrompt.textContent = serverInstallPrompt(
      elements.bundleReceiptDestination.textContent,
      localization.locale
    );
  }

  const settings = state.deepAuditSettings;
  if (settings) renderCredentialState();
  if (state.deepAuditPreview) renderDeepAuditConsent(state.deepAuditPreview);
  clearConnectionTestResult();
  refreshIcons();
}

localization.subscribe(() => {
  refreshLocalizedInterface();
  desktop.setInterfaceLocale(localization.locale).catch(() => {});
});

document.querySelector("#source-nav").addEventListener("click", (event) => {
  const button = event.target.closest("[data-source]");
  if (!button) return;
  setSourceFilter(button.dataset.source);
});

elements.search.addEventListener("input", () => {
  state.query = elements.search.value;
  renderList();
});

elements.sort.addEventListener("change", () => {
  state.sort = elements.sort.value;
  renderList();
});

elements.refresh.addEventListener("click", () => loadSkills({ forceRefresh: true }));
elements.create.addEventListener("click", openNewSkill);
elements.exportSkills.addEventListener("click", openBundleExport);
elements.importBundleButton.addEventListener("click", openBundleImport);
elements.reviewCandidate.addEventListener("click", openCandidateIntake);
elements.closeDetail.addEventListener("click", () => elements.detailPanel.classList.remove("is-open"));
elements.closeEditor.addEventListener("click", requestCloseEditor);
elements.addCollection.addEventListener("click", () => openCollectionDialog());
elements.collectionForm.addEventListener("submit", submitCollection);
elements.cancelCollection.addEventListener("click", () => elements.collectionDialog.close());
elements.deleteCollection.addEventListener("click", () => {
  const collection = state.collections.find((item) => item.id === state.collectionEditId);
  if (collection) requestDeleteCollection(collection);
});
elements.collectionMembershipForm.addEventListener("submit", saveCollectionMembership);
elements.manageCollections.addEventListener("click", () => {
  elements.collectionMembershipDialog.close();
  openCollectionDialog();
});
elements.closePackage.addEventListener("click", requestClosePackage);
elements.packageDialog.addEventListener("cancel", (event) => {
  event.preventDefault();
  requestClosePackage();
});
elements.packageExport.addEventListener("click", () => {
  const skill = state.packageWorkspace?.skill;
  if (skill) exportSingleSkill(skill);
});
elements.packageNewFile.addEventListener("click", () => promptPackagePath("file"));
elements.packageImportFile.addEventListener("click", importPackageFile);
elements.packageNewFolder.addEventListener("click", () => promptPackagePath("folder"));
elements.packageRenameFile.addEventListener("click", renamePackageEntry);
elements.packageDeleteFile.addEventListener("click", deletePackageEntry);
elements.packagePreview.addEventListener("click", previewPackageChanges);
elements.packageDeepAudit.addEventListener("click", requestPackageDeepAudit);
elements.packageSave.addEventListener("click", requestPackageSave);
elements.packageTextEditor.addEventListener("input", () => {
  if (state.packageWorkspace?.selectedPath) {
    setPackageWrite(state.packageWorkspace.selectedPath, elements.packageTextEditor.value);
  }
});
elements.guidedMode.addEventListener("click", () => setEditorMode("guided"));
elements.sourceMode.addEventListener("click", () => setEditorMode("source"));
elements.auditDraft.addEventListener("click", runDraftAudit);
elements.deepAudit.addEventListener("click", requestDeepAudit);
elements.candidateGithubMode.addEventListener("click", () => setCandidateSourceMode("github"));
elements.candidateLocalMode.addEventListener("click", () => setCandidateSourceMode("local"));
elements.candidateToggleAudit.addEventListener("click", () => {
  setUpdateAuditVisible(!state.candidate?.auditVisible);
});
elements.candidateSyncFile.addEventListener("click", requestCandidateFileSync);
elements.closeUpdateResult.addEventListener("click", () => elements.updateResultDialog.close());
elements.chooseCandidateFolder.addEventListener("click", chooseCandidateFolder);
elements.bundleExportForm.addEventListener("submit", previewBundleExport);
elements.bundleSelectAll.addEventListener("change", () => {
  for (const checkbox of elements.bundleSkillList.querySelectorAll('input[type="checkbox"]')) {
    checkbox.checked = elements.bundleSelectAll.checked;
  }
  invalidateBundleExportPlan();
});
elements.copyServerInstallPrompt.addEventListener("click", async () => {
  const prompt = elements.serverInstallPrompt.textContent;
  if (!prompt) return;
  try {
    await navigator.clipboard.writeText(prompt);
    showToast(t("copy.serverPrompt"));
  } catch (error) {
    showToast(t("common.copyFailed", { message: localizedError(error) }), true);
  }
});
elements.applyBundleExport.addEventListener("click", applyBundleExport);
elements.closeBundleExport.addEventListener("click", requestCloseBundleExport);
elements.cancelBundleExport.addEventListener("click", requestCloseBundleExport);
elements.bundleExportDialog.addEventListener("cancel", (event) => {
  event.preventDefault();
  requestCloseBundleExport();
});
elements.bundleImportSkillList.addEventListener("click", (event) => {
  const comparison = event.target.closest(".bundle-import-compare-button");
  if (comparison) {
    compareImportedBundleFile(
      comparison.dataset.directoryName,
      comparison.dataset.matchId,
      comparison.dataset.path
    );
    return;
  }
  const button = event.target.closest(".bundle-import-file-button");
  if (button) previewImportedBundleFile(button.dataset.directoryName, button.dataset.path);
});
elements.bundleImportSkillList.addEventListener("change", (event) => {
  if (event.target.matches(".bundle-install-selection")) updateBundleInstallSelection();
});
elements.installBundleSkills.addEventListener("click", requestBundleInstall);
elements.closeBundleImport.addEventListener("click", discardBundleImport);
elements.discardBundleImport.addEventListener("click", discardBundleImport);
elements.bundleImportDialog.addEventListener("cancel", (event) => {
  event.preventDefault();
  discardBundleImport();
});
elements.bundleImportDialog.addEventListener("close", () => {
  if (elements.toast.parentElement !== document.body) document.body.append(elements.toast);
});
elements.candidateIntakeForm.addEventListener("submit", stageCandidate);
elements.candidateResetRepository.addEventListener("click", async () => {
  const clearUrl = Boolean(state.repositoryQueue);
  elements.candidateResetRepository.disabled = true;
  try {
    await discardRepositoryQueueSessions();
  } catch (error) {
    showToast(localizedError(error), true);
  } finally {
    elements.candidateResetRepository.disabled = false;
  }
  showCandidateSourceStep({ clearUrl });
});
elements.candidateSelectAllRepository.addEventListener("click", () => {
  if (!state.repositoryListing) return;
  const inputs = [...elements.candidateRepositoryList.querySelectorAll("input")];
  const selectAll = inputs.some((input) => !input.checked);
  for (const input of inputs) input.checked = selectAll;
  updateRepositoryListingAction();
});
document.querySelector("#close-candidate-intake").addEventListener("click", () => elements.candidateIntakeDialog.close());
document.querySelector("#cancel-candidate-intake").addEventListener("click", () => elements.candidateIntakeDialog.close());
document.querySelector("#close-candidate-review").addEventListener("click", closeCandidateReview);
elements.candidatePreviousQueueItem.addEventListener("click", () => navigateRepositoryQueue(-1));
elements.candidateNextQueueItem.addEventListener("click", () => navigateRepositoryQueue(1));
elements.installCandidate.addEventListener("click", requestCandidateInstall);
elements.candidateDeepAudit.addEventListener("click", requestCandidateDeepAudit);
elements.candidateReviewDialog.addEventListener("cancel", (event) => {
  event.preventDefault();
  closeCandidateReview();
});
elements.candidateReviewDialog.addEventListener("close", () => {
  if (elements.toast.parentElement !== document.body) document.body.append(elements.toast);
});
elements.settingsForm.addEventListener("submit", saveDeepAuditSettings);
elements.testDeepConnection.addEventListener("click", testDeepAuditConnection);
elements.deepApiMode.addEventListener("change", clearConnectionTestResult);
for (const field of [elements.deepEndpoint, elements.deepModel, elements.deepApiKey]) {
  field.addEventListener("input", clearConnectionTestResult);
}
elements.deepConsentForm.addEventListener("submit", performDeepAudit);
document.querySelector("#close-settings").addEventListener("click", () => elements.settingsDialog.close());
document.querySelector("#cancel-settings").addEventListener("click", () => elements.settingsDialog.close());
document.querySelector("#close-deep-audit-consent").addEventListener("click", () => {
  state.deepAuditPreview = null;
  state.deepAuditContext = null;
  elements.deepConsentDialog.close();
});
document.querySelector("#cancel-deep-audit-consent").addEventListener("click", () => {
  state.deepAuditPreview = null;
  state.deepAuditContext = null;
  elements.deepConsentDialog.close();
});
elements.clearDeepSettings.addEventListener("click", () => {
  presentConfirmation({
    title: t("settings.removeTitle"),
    message: t("settings.removeMessage"),
    label: t("settings.removeConfirm"),
    action: async () => {
      state.deepAuditSettings = await desktop.clearDeepAuditSettings();
      elements.settingsDialog.close();
      showToast(t("settings.removed"));
    }
  });
});
elements.saveDraft.addEventListener("click", requestDraftSave);
elements.editorDialog.addEventListener("cancel", (event) => {
  event.preventDefault();
  requestCloseEditor();
});
elements.editorDialog.addEventListener("close", () => {
  if (elements.toast.parentElement !== document.body) document.body.append(elements.toast);
});
elements.draftDescription.addEventListener("input", () => {
  if (!state.editor) return;
  setDraftMarkdown(updateSkillDocument(state.editor.draftMarkdown, {
    type: "description",
    value: elements.draftDescription.value
  }));
});
elements.draftName.addEventListener("input", () => {
  if (!state.editor?.isNew) return;
  setDraftMarkdown(updateSkillDocument(state.editor.draftMarkdown, {
    type: "name",
    value: elements.draftName.value
  }));
});
elements.draftBodyFallback.addEventListener("input", () => {
  if (!state.editor) return;
  setDraftMarkdown(updateSkillDocument(state.editor.draftMarkdown, {
    type: "body",
    value: elements.draftBodyFallback.value
  }));
});
elements.draftSource.addEventListener("input", () => {
  setDraftMarkdown(elements.draftSource.value, { syncSource: false });
});
elements.confirmCancel.addEventListener("click", () => {
  if (state.confirmBusy) return;
  state.confirmAction = null;
  state.confirmRequiredName = null;
  elements.confirmDialog.close();
});
elements.confirmDialog.addEventListener("cancel", (event) => {
  if (state.confirmBusy) event.preventDefault();
});
elements.confirmName.addEventListener("input", () => {
  elements.confirmSubmit.disabled = Boolean(
    state.confirmRequiredName && elements.confirmName.value !== state.confirmRequiredName
  );
});

document.querySelector("#copy-path-button").addEventListener("click", async () => {
  if (!state.detail) return;
  await navigator.clipboard.writeText(state.detail.path);
  showToast(t("detail.pathCopied"));
});

elements.confirmForm.addEventListener("submit", async (event) => {
  const submitter = event.submitter;
  if (submitter?.value !== "default" || !state.confirmAction) return;
  event.preventDefault();
  setConfirmationBusy(true);
  try {
    await state.confirmAction();
    elements.confirmDialog.close();
  } catch (error) {
    elements.confirmDialog.close();
    showToast(localizedError(error), true);
  } finally {
    setConfirmationBusy(false);
    state.confirmAction = null;
    state.confirmRequiredName = null;
  }
});


document.addEventListener("keydown", (event) => {
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "f") {
    event.preventDefault();
    elements.search.focus();
    elements.search.select();
  }
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "s" && elements.editorDialog.open) {
    event.preventDefault();
    if (!elements.saveDraft.disabled) requestDraftSave();
  }
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "s" && elements.packageDialog.open) {
    event.preventDefault();
    if (!elements.packageSave.disabled) requestPackageSave();
  }
  if (event.key === "/" && !["INPUT", "TEXTAREA", "SELECT"].includes(document.activeElement.tagName)) {
    event.preventDefault();
    elements.search.focus();
  }
  if (event.key === "Escape" && elements.detailPanel.classList.contains("is-open")) {
    elements.detailPanel.classList.remove("is-open");
  }
});

refreshIcons();
listen("studio-menu-action", ({ payload }) => {
  if (payload === "open-settings") {
    openSettings();
  } else if (payload === "locale:zh-CN") {
    localization.setLocale("zh-CN");
  } else if (payload === "locale:en") {
    localization.setLocale("en");
  }
}).catch(() => {});
desktop.setInterfaceLocale(localization.locale).catch(() => {});
loadSkills({ preserveSelection: false });
loadCollections();
