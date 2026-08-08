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
  bundleExportPlan: null,
  bundleExportBusy: false,
  bundleInstallBusy: false,
  bundleInstallReviewStale: false,
  bundleInstallResult: null,
  bundleWorkflow: createBundleWorkflowState(),
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
  candidateGithubField: document.querySelector("#candidate-github-field"),
  candidateLocalField: document.querySelector("#candidate-local-field"),
  candidateGithubUrl: document.querySelector("#candidate-github-url"),
  candidateLocalPath: document.querySelector("#candidate-local-path"),
  chooseCandidateFolder: document.querySelector("#choose-candidate-folder"),
  stageCandidate: document.querySelector("#stage-candidate-button"),
  candidateReviewDialog: document.querySelector("#candidate-review-dialog"),
  candidateReviewTitle: document.querySelector("#candidate-review-title"),
  candidateDeepAudit: document.querySelector("#candidate-deep-audit-button"),
  installCandidate: document.querySelector("#install-candidate-button"),
  candidateSourceList: document.querySelector("#candidate-source-list"),
  candidateCompatibilityStatus: document.querySelector("#candidate-compatibility-status"),
  candidateCompatibilitySummary: document.querySelector("#candidate-compatibility-summary"),
  candidateCompatibilityList: document.querySelector("#candidate-compatibility-list"),
  candidateFileCount: document.querySelector("#candidate-file-count"),
  candidateFiles: document.querySelector("#candidate-files"),
  candidatePreviewTitle: document.querySelector("#candidate-preview-title"),
  candidatePreviewMeta: document.querySelector("#candidate-preview-meta"),
  candidatePreviewEmpty: document.querySelector("#candidate-preview-empty"),
  candidateFilePreview: document.querySelector("#candidate-file-preview"),
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
    const haystack = `${skill.name} ${skill.displayName} ${skill.summary} ${skill.description}`.toLocaleLowerCase(localization.locale);
    return matchesSource && (!query || haystack.includes(query));
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
  document.querySelectorAll("[data-source]").forEach((item) => {
    const active = item.dataset.source === source;
    item.classList.toggle("is-active", active);
    item.setAttribute("aria-pressed", String(active));
  });
  renderList();
}

async function inspectHealthIssue(skill) {
  state.query = "";
  elements.search.value = "";
  setSourceFilter("personal");
  await selectSkill(skill.id);
  if (state.detail?.id !== skill.id) return;
  try {
    await openEditor(skill.id);
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

  for (const skill of skills) {
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
  document.querySelector("#detail-files").textContent = t("common.count", { count: skill.fileCount });
  document.querySelector("#detail-updated").textContent = formatDate(skill.modifiedAt);
  document.querySelector("#detail-path").textContent = skill.path;
  document.querySelector("#detail-markdown").textContent = skill.markdown;

  const actions = document.querySelector("#detail-actions");
  actions.replaceChildren();
  if (skill.source === "personal") {
    actions.append(
      actionButton(t("detail.edit"), "square-pen", "primary-button", () => openEditor(skill.id)),
      actionButton(t("detail.disable"), "circle-pause", "secondary-button", () => requestSkillLifecycle("disable", skill)),
      actionButton(t("detail.archive"), "archive", "secondary-button", () => requestSkillLifecycle("archive", skill))
    );
  } else if (skill.source === "disabled") {
    actions.append(
      actionButton(t("detail.enable"), "circle-play", "primary-button", () => requestSkillLifecycle("enable", skill)),
      actionButton(t("detail.archive"), "archive", "secondary-button", () => requestSkillLifecycle("archive", skill))
    );
  } else if (skill.source === "archive") {
    actions.append(
      actionButton(t("detail.restore"), "archive-restore", "primary-button", () => requestSkillLifecycle("restore", skill)),
      actionButton(t("detail.delete"), "trash-2", "danger-button", () => requestSkillLifecycle("delete", skill))
    );
  } else {
    const readonly = document.createElement("span");
    readonly.className = "trigger-badge";
    readonly.textContent = t("detail.readOnly");
    actions.append(readonly);
  }
  refreshIcons();
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
  const changed = editorChanged();
  const creating = state.editor?.isNew;
  elements.draftStatus.textContent = creating
    ? t("editor.notCreated")
    : changed
      ? t("editor.unsaved")
      : t("editor.unchanged");
  elements.draftStatus.classList.toggle("is-dirty", changed);
  elements.saveDraft.disabled = creating
    ? !state.editor?.preview?.canCreate || state.editor.auditLoading
    : !changed || !state.editor?.audit || state.editor.audit.verdict === "block" || state.editor.auditLoading;
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
  elements.candidateGithubField.hidden = !github;
  elements.candidateLocalField.hidden = github;
  if (github) elements.candidateGithubUrl.focus();
  refreshIcons();
}

function openCandidateIntake() {
  if (elements.candidateIntakeDialog.open) return;
  if (!state.candidateLocalPath) elements.candidateLocalPath.textContent = t("candidate.noFolder");
  setCandidateSourceMode("github");
  elements.candidateIntakeDialog.showModal();
  elements.candidateGithubUrl.focus();
  refreshIcons();
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

function renderBundleSkillSelection() {
  const skills = personalExportSkills();
  const rows = skills.map((skill) => {
    const label = document.createElement("label");
    label.className = "bundle-skill-row";
    const checkbox = document.createElement("input");
    checkbox.type = "checkbox";
    checkbox.value = skill.id;
    checkbox.checked = true;
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

function openBundleExport() {
  invalidateExportOperations(state.bundleWorkflow);
  state.bundleExportBusy = false;
  renderBundleSkillSelection();
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
      filters: [{ name: "Skill Bundle", extensions: ["skillbundle"] }]
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
      filters: [{ name: "Skill Bundle", extensions: ["skillbundle"] }]
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
  const rows = candidate.review.manifest.files.map((file) => {
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
    meta.textContent = `${formatBytes(file.size)} · SHA-256 ${file.sha256.slice(0, 12)}…${file.executable ? ` · ${t("common.executable")}` : ""}`;
    copy.append(path, meta);
    row.append(icon, copy);
    row.addEventListener("click", () => selectCandidateFile(file.path));
    return row;
  });
  elements.candidateFiles.replaceChildren(...rows);
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
  elements.candidateFileCount.textContent = String(review.manifest.files.length);
  renderCandidateFiles();
  renderCandidatePreview();
  renderCandidateAudit(review.audit);
  renderCandidateDeepAuditResult(candidate.deepAudit || null);
  renderCandidateSkipped(review.skippedEntries);
  const blocked = review.compatibility.status === "incompatible" || review.audit.verdict === "block";
  elements.installCandidate.disabled = blocked;
  elements.installCandidate.title = blocked
    ? t("candidate.installBlockedTitle")
    : t("candidate.installReadyTitle");
  refreshIcons();
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
    if (state.candidate === candidate) state.candidate = null;
    if (elements.candidateReviewDialog.open) elements.candidateReviewDialog.close();
    if (result.skill) {
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
  renderCandidateFiles();
  renderCandidatePreview();
  try {
    const preview = await desktop.readStagedCandidateFile(
      candidate.review.manifest.sessionId,
      candidate.review.manifest.candidateHash,
      path
    );
    if (state.candidate !== candidate || candidate.previewSequence !== sequence) return;
    candidate.preview = preview;
    renderCandidatePreview();
  } catch (error) {
    if (state.candidate !== candidate || candidate.previewSequence !== sequence) return;
    candidate.preview = null;
    renderCandidatePreview();
    showToast(localizedError(error), true);
  }
}

async function stageCandidate(event) {
  event.preventDefault();
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
    manifest = github
      ? await desktop.stageGithubCandidate(source)
      : await desktop.stageLocalCandidate(source);
    const review = await desktop.getStagedCandidateReview(manifest.sessionId, manifest.candidateHash);
    state.candidate = {
      review,
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
    elements.stageCandidate.querySelector("span").textContent = t("candidate.stageReview");
  }
}

async function closeCandidateReview() {
  const candidate = state.candidate;
  state.candidate = null;
  if (elements.candidateReviewDialog.open) elements.candidateReviewDialog.close();
  if (!candidate) return;
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

function deepAuditEditorId() {
  return state.editor?.isNew ? null : state.editor?.id || null;
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
    const preview = await desktop.previewDeepAudit(deepAuditEditorId(), state.editor.draftMarkdown);
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

async function performDeepAudit(event) {
  event.preventDefault();
  if (!state.deepAuditPreview || !state.deepAuditContext) return;
  const context = state.deepAuditContext;
  const editor = context.kind === "editor" ? context.editor : null;
  const candidate = context.kind === "candidate" ? context.candidate : null;
  if (context.kind === "editor" && state.editor !== editor) return;
  if (context.kind === "candidate" && state.candidate !== candidate) return;
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
      : await desktop.runDeepAudit(
        deepAuditEditorId(),
        editor.draftMarkdown,
        selections,
        state.deepAuditPreview.candidateHash,
        state.deepAuditPreview.providerHash
      );
    elements.deepConsentDialog.close();
    if (context.kind === "candidate" && state.candidate === candidate) {
      renderCandidateDeepAuditResult(result);
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

function renderDraftAudit(audit) {
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
  const verdict = verdicts[audit.verdict];
  elements.auditVerdict.textContent = verdict.title;
  elements.auditVerdictBadge.textContent = verdict.badge;
  elements.auditVerdictBadge.className = `verdict-badge is-${audit.verdict}`;
  elements.auditSummary.textContent = verdict.summary;
  elements.findingCount.textContent = String(audit.findings.length);
  elements.findingList.replaceChildren(...audit.findings.map(renderFinding));

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
    const result = state.editor.isNew
      ? await desktop.previewNewSkill(markdown)
      : await api(`/api/skills/${encodeURIComponent(editorId)}/audit`, {
        method: "POST",
        body: JSON.stringify({ markdown })
      });
    if (!state.editor || state.editor.id !== editorId || sequence !== state.auditSequence) return;
    const audit = state.editor.isNew ? result.audit : result;
    state.editor.audit = audit;
    state.editor.preview = state.editor.isNew ? result : null;
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

async function openEditor(id) {
  const detail = state.detail?.id === id ? state.detail : await api(`/api/skills/${encodeURIComponent(id)}`);
  if (!detail.editable) {
    showToast(t("editor.managedReadOnly"), true);
    return;
  }
  state.editor = {
    id,
    expectedHash: detail.contentHash,
    originalMarkdown: detail.markdown,
    draftMarkdown: detail.markdown,
    audit: null,
    preview: null,
    auditLoading: false,
    auditTimer: null,
    isNew: false
  };
  state.deepAuditPreview = null;
  state.deepAuditContext = null;
  renderDeepAuditResult(null);
  elements.editorTitle.textContent = detail.displayName;
  elements.saveDraft.innerHTML = '<i data-lucide="save"></i><span></span>';
  elements.saveDraft.querySelector("span").textContent = t("editor.save");
  elements.draftSource.value = detail.markdown;
  syncGuidedFields();
  setEditorMode("guided");
  updateEditorStatus();
  elements.editorDialog.showModal();
  refreshIcons();
  await runDraftAudit();
}

function newSkillMarkdown() {
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

async function performDraftSave() {
  if (!state.editor) return;
  const { id, draftMarkdown, expectedHash } = state.editor;
  elements.saveDraft.disabled = true;
  try {
    await api(`/api/skills/${encodeURIComponent(id)}`, {
      method: "PUT",
      body: JSON.stringify({ markdown: draftMarkdown, expectedHash })
    });
    elements.editorDialog.close();
    state.editor = null;
    showToast(t("editor.saved"));
    await loadSkills();
    await selectSkill(id);
  } catch (error) {
    showToast(localizedError(error), true);
  } finally {
    if (state.editor) updateEditorStatus();
  }
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
  if (!state.editor?.audit || state.editor.audit.verdict === "block") return;
  if (state.editor.isNew) {
    if (!state.editor.preview?.canCreate) return;
    presentConfirmation({
      title: t("editor.createConfirmTitle"),
      message: state.editor.deepAudit?.verdict === "block"
        ? t("editor.createRiskMessage", { destination: state.editor.preview.destination })
        : t("editor.createMessage", { destination: state.editor.preview.destination }),
      label: t("editor.confirmCreate"),
      action: performDraftCreate,
      tone: "primary"
    });
    return;
  }
  if (state.editor.audit.verdict === "clear" && !["review", "block"].includes(state.editor.deepAudit?.verdict)) {
    performDraftSave();
    return;
  }
  presentConfirmation({
    title: t("editor.reviewSaveTitle"),
    message: state.editor.deepAudit?.verdict === "block"
      ? t("editor.reviewSaveDeepMessage")
      : t("editor.reviewSaveMessage"),
    label: t("editor.confirmSave"),
    action: performDraftSave
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
        await desktop.deleteArchivedSkill(skill.id, preview.directoryRevision, elements.confirmName.value);
        state.selectedId = null;
        state.detail = null;
        applyCatalogState(removeCatalogSkill(state.skills, state.counts, skill.id));
        elements.detailPanel.classList.remove("is-open");
        renderDetail();
        showToast(t("lifecycle.deleted", { name: skill.displayName }));
      }
      : async () => {
        const result = await desktop.applySkillLifecycle(skill.id, action, preview.directoryRevision);
        state.selectedId = result.skill.id;
        state.detail = result.skill;
        applyCatalogState(replaceCatalogSkill(state.skills, state.counts, skill.id, result.skill));
        elements.detailPanel.classList.add("is-open");
        renderDetail();
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
  renderList();
  renderDetail();

  if (state.editor) {
    if (state.editor.isNew) elements.editorTitle.textContent = t("editor.newTitle");
    const saveLabel = state.editor.isNew ? t("editor.create") : t("editor.save");
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
elements.guidedMode.addEventListener("click", () => setEditorMode("guided"));
elements.sourceMode.addEventListener("click", () => setEditorMode("source"));
elements.auditDraft.addEventListener("click", runDraftAudit);
elements.deepAudit.addEventListener("click", requestDeepAudit);
elements.candidateGithubMode.addEventListener("click", () => setCandidateSourceMode("github"));
elements.candidateLocalMode.addEventListener("click", () => setCandidateSourceMode("local"));
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
document.querySelector("#close-candidate-intake").addEventListener("click", () => elements.candidateIntakeDialog.close());
document.querySelector("#cancel-candidate-intake").addEventListener("click", () => elements.candidateIntakeDialog.close());
document.querySelector("#close-candidate-review").addEventListener("click", closeCandidateReview);
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
