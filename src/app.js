import { createIcons, icons } from "lucide";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
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

window.lucide = {
  createIcons: (options = {}) => createIcons({ icons, ...options })
};

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
  confirmLabel: "确认",
  providerTestSequence: 0,
  candidate: null,
  candidateSourceMode: "github",
  candidateLocalPath: null,
  bundleExportPlan: null,
  bundleExportBusy: false,
  bundleInstallBusy: false,
  bundleInstallReviewStale: false,
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
  settings: document.querySelector("#settings-button"),
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

const sourceLabels = {
  personal: "个人",
  disabled: "已停用",
  system: "系统",
  plugin: "插件",
  archive: "归档"
};

const skillStateLabels = {
  active: "启用",
  disabled: "停用",
  archived: "归档"
};

const deepAuditApiModeLabels = {
  chatCompletions: "Chat Completions",
  responses: "Responses"
};

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
  throw new Error("此操作将在后续阶段提供。");
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
  return new Intl.DateTimeFormat("zh-CN", {
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
  const query = state.query.trim().toLocaleLowerCase("zh-CN");
  const filtered = state.skills.filter((skill) => {
    const matchesSource = state.source === "all" || skill.source === state.source;
    const haystack = `${skill.name} ${skill.displayName} ${skill.summary} ${skill.description}`.toLocaleLowerCase("zh-CN");
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
    showToast(error.message, true);
  }
}

function renderHealthIssues() {
  const issues = personalSkillsNeedingAttention(state.skills);
  const rows = issues.map((skill) => {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "audit-issue";
    button.title = `查看 ${skill.displayName} 的阻断问题`;
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
    ? `${counts.needsAttention} 项存在阻断问题`
    : "个人 Skill 状态正常";
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
  elements.resultSummary.textContent = `${skills.length} 个结果 · ${state.counts.total || 0} 个已收录`;

  for (const skill of skills) {
    const row = document.createElement("button");
    row.type = "button";
    row.className = `skill-row${skill.id === state.selectedId ? " is-selected" : ""}`;
    row.dataset.id = skill.id;
    row.setAttribute("aria-label", `查看 ${skill.displayName}`);
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
    source.textContent = sourceLabels[skill.source];

    const trigger = document.createElement("span");
    const readonlySource = ["system", "plugin"].includes(skill.source);
    trigger.className = `trigger-badge ${readonlySource ? "source-managed" : "good"}`;
    trigger.textContent = readonlySource ? "来源管理" : skill.triggerMode === "explicit" ? "明确点名" : "按意图触发";

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
    showToast(error.message, true);
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
  document.querySelector("#detail-source").textContent = sourceLabels[skill.source];
  document.querySelector("#detail-description").textContent = skill.summary;
  document.querySelector("#detail-state").textContent = ["system", "plugin"].includes(skill.source)
    ? "随来源启用"
    : skill.state === "active"
      ? "启用"
      : skill.state === "disabled"
        ? "停用"
        : "已归档";
  document.querySelector("#detail-trigger").textContent = ["system", "plugin"].includes(skill.source)
    ? "由安装来源管理"
    : skill.triggerMode === "explicit"
      ? "仅在明确点名时触发"
      : "可根据任务意图触发";
  document.querySelector("#detail-files").textContent = `${skill.fileCount} 个`;
  document.querySelector("#detail-updated").textContent = formatDate(skill.modifiedAt);
  document.querySelector("#detail-path").textContent = skill.path;
  document.querySelector("#detail-markdown").textContent = skill.markdown;

  const actions = document.querySelector("#detail-actions");
  actions.replaceChildren();
  if (skill.source === "personal") {
    actions.append(
      actionButton("编辑", "square-pen", "primary-button", () => openEditor(skill.id)),
      actionButton("停用", "circle-pause", "secondary-button", () => requestSkillLifecycle("disable", skill)),
      actionButton("归档", "archive", "secondary-button", () => requestSkillLifecycle("archive", skill))
    );
  } else if (skill.source === "disabled") {
    actions.append(
      actionButton("重新启用", "circle-play", "primary-button", () => requestSkillLifecycle("enable", skill)),
      actionButton("归档", "archive", "secondary-button", () => requestSkillLifecycle("archive", skill))
    );
  } else if (skill.source === "archive") {
    actions.append(
      actionButton("恢复", "archive-restore", "primary-button", () => requestSkillLifecycle("restore", skill)),
      actionButton("永久删除", "trash-2", "danger-button", () => requestSkillLifecycle("delete", skill))
    );
  } else {
    const readonly = document.createElement("span");
    readonly.className = "trigger-badge";
    readonly.textContent = "只读管理";
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
}

function setConfirmationBusy(busy) {
  state.confirmBusy = busy;
  elements.confirmDialog.toggleAttribute("aria-busy", busy);
  elements.confirmCancel.disabled = busy;
  elements.confirmSubmit.disabled = busy || Boolean(
    state.confirmRequiredName && elements.confirmName.value !== state.confirmRequiredName
  );
  elements.confirmSubmit.textContent = busy ? "正在提交" : state.confirmLabel;
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
    level.textContent = section.kind === "preamble" ? "正文" : `H${section.level}`;

    const title = document.createElement("input");
    title.className = "section-title-input";
    title.value = section.title;
    title.readOnly = !section.titleEditable;
    title.setAttribute("aria-label", section.titleEditable ? `${section.title} 标题` : section.title);
    if (section.titleEditable) {
      title.addEventListener("change", () => {
        if (!title.value.trim()) {
          title.value = section.title;
          showToast("章节标题不能为空。", true);
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
    toggle.title = "收起章节";
    toggle.setAttribute("aria-label", `收起 ${section.title}`);
    toggle.setAttribute("aria-expanded", "true");
    const toggleIcon = document.createElement("i");
    toggleIcon.dataset.lucide = "chevron-down";
    toggle.append(toggleIcon);

    const content = document.createElement("textarea");
    content.className = "section-content-input";
    content.value = section.content;
    content.spellcheck = false;
    content.rows = Math.min(Math.max(section.content.split(/\r?\n/).length + 1, 3), 12);
    content.setAttribute("aria-label", `${section.title} 内容`);
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
      toggle.title = expanded ? "展开章节" : "收起章节";
      toggle.setAttribute("aria-label", `${expanded ? "展开" : "收起"} ${title.value}`);
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
  elements.draftStatus.textContent = creating ? "尚未创建" : changed ? "有未保存修改" : "未修改";
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
  elements.creationDestination.textContent = preview?.destination || "等待有效名称";
  elements.creationState.textContent = preview?.conflict
    ? `与${sourceLabels[preview.conflict.source] || preview.conflict.source}来源中的同名 Skill 冲突`
    : preview?.canCreate
      ? "可创建"
      : "请解决阻断项后再创建";
  elements.creationPreview.classList.toggle("has-conflict", Boolean(preview?.conflict));
  elements.creationPreview.classList.toggle("is-ready", Boolean(preview?.canCreate));
}

function scheduleDraftAudit() {
  if (!state.editor) return;
  clearTimeout(state.editor.auditTimer);
  state.editor.auditTimer = setTimeout(() => runDraftAudit(), 420);
}

function renderAuditLoading() {
  elements.auditVerdict.textContent = "正在检查";
  elements.auditVerdictBadge.textContent = "检查中";
  elements.auditVerdictBadge.className = "verdict-badge is-loading";
  elements.auditSummary.textContent = "正在分析结构、触发范围和高影响操作。";
  elements.auditDraft.disabled = true;
  updateEditorStatus();
}

function renderAuditError(message) {
  elements.auditVerdict.textContent = "检查未完成";
  elements.auditVerdictBadge.textContent = "错误";
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
  title.textContent = item.title;
  const severity = document.createElement("span");
  severity.className = `severity-label is-${item.severity}`;
  severity.textContent = { blocker: "阻断", warning: "警告", info: "信息" }[item.severity] || item.severity;
  const confidence = document.createElement("span");
  confidence.className = "confidence-label";
  confidence.textContent = item.disposition === "dismissed"
    ? "复核后排除"
    : { high: "高置信", medium: "中置信", low: "低置信" }[item.confidence] || item.confidence;
  heading.append(marker, title, severity, confidence);

  const explanation = document.createElement("p");
  explanation.textContent = item.explanation;
  const evidence = document.createElement("details");
  const summary = document.createElement("summary");
  summary.textContent = "查看证据";
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
    review.textContent = `误报复核：${item.reviewNote}`;
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
    clear: ["未发现经复核的语义风险", "未命中", "这次云端审查没有保留风险项；不代表 Skill 安全。"],
    review: ["发现需要人工复核的行为", "需复核", "请根据文件证据判断这些能力是否符合预期。"],
    block: ["发现高影响语义风险", "高风险", "模型审查保留了高影响风险项，请先修改或人工确认来源。"]
  };
  const [title, badge, summary] = verdicts[result.verdict] || verdicts.review;
  view.verdict.textContent = title;
  view.badge.textContent = badge;
  view.badge.className = `verdict-badge is-${result.verdict}`;
  view.summary.textContent = summary;
  const apiMode = deepAuditApiModeLabels[result.apiMode] || result.apiMode;
  view.meta.textContent = `${apiMode} · ${result.model} · ${result.files.length} 个文件 · ${result.requestCount} 次请求${dismissed ? ` · 排除 ${dismissed} 项` : ""}`;
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
      title: "选择包含 SKILL.md 的文件夹"
    });
    if (typeof selected !== "string") return;
    state.candidateLocalPath = selected;
    elements.candidateLocalPath.textContent = selected;
  } catch (error) {
    showToast(error.message, true);
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
  elements.previewBundleExport.querySelector("span").textContent = "检查导出内容";
  state.bundleExportPlan = null;
  elements.bundleExportReview.hidden = true;
  elements.bundleExportBlocks.hidden = true;
  elements.bundleExportBlocks.replaceChildren();
  elements.applyBundleExport.disabled = true;
  const checkboxes = [...elements.bundleSkillList.querySelectorAll('input[type="checkbox"]')];
  const selected = checkboxes.filter((input) => input.checked).length;
  elements.bundleSelectionCount.textContent = `${selected} 项`;
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
    count.textContent = `${skill.fileCount} 个文件`;
    label.append(checkbox, content, count);
    return label;
  });
  elements.bundleSkillList.replaceChildren(...rows);
  if (!skills.length) {
    const empty = document.createElement("p");
    empty.className = "bundle-export-empty";
    empty.textContent = "当前没有可导出的启用个人 Skill。";
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
  elements.cancelBundleExport.textContent = "取消";
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
  return {
    "credential-path": "路径看起来用于保存凭据，已阻止导出。",
    "private-key-material": "文件中发现私钥内容，已阻止导出。",
    "known-token-format": "文件中发现符合已知令牌格式的内容，已阻止导出。",
    "credential-assignment": "文件中发现高置信度凭据赋值，已阻止导出。",
    "resource-limit": "Skill 超过 Bundle v1 的文件、大小或目录深度限制。",
    "bundle-size-limit": "所选内容超过 Bundle v1 的归档大小限制。",
    "unsafe-entry": "Skill 包含链接、特殊文件或不受支持的路径。",
    "source-changed": "检查期间 Skill 发生变化，请重新检查。",
    "source-not-exportable": "Bundle v1 只导出启用的个人 Skill。",
    "duplicate-selection": "同一个 Skill 被重复选择。",
    "skill-not-found": "Skill 已不在当前目录中，请刷新后重试。"
  }[finding.ruleId] || "该 Skill 当前不能导出。";
}

function bundleExportErrorMessage(error) {
  return {
    BUNDLE_EXPORT_PLAN_STALE: "导出预览已过期，请重新检查所选 Skills。",
    BUNDLE_EXPORT_SOURCE_CHANGED: "Skill 在导出前发生变化，请重新检查。",
    BUNDLE_EXPORT_DESTINATION_EXISTS: "该位置已经有同名文件，请选择新文件名。",
    BUNDLE_EXPORT_DESTINATION_INVALID: "请选择现有文件夹中的新 .skillbundle 文件。",
    BUNDLE_LIMIT_EXCEEDED: "所选内容超过 Bundle v1 的大小限制。"
  }[error?.code] || error?.message || "导出没有完成，请重试。";
}

async function previewBundleExport(event) {
  event.preventDefault();
  const skillIds = selectedBundleSkillIds();
  if (!skillIds.length) return;
  const selectionFingerprint = skillIds.join("\0");
  const operation = beginExportOperation(state.bundleWorkflow, "preview", selectionFingerprint);
  elements.previewBundleExport.disabled = true;
  elements.previewBundleExport.querySelector("span").textContent = "正在检查";
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
      elements.previewBundleExport.querySelector("span").textContent = "检查导出内容";
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
      title: "导出 Skill Bundle",
      defaultPath: "codex-skills.skillbundle",
      filters: [{ name: "Skill Bundle", extensions: ["skillbundle"] }]
    });
    if (typeof destination !== "string") return;
    const operation = beginExportOperation(state.bundleWorkflow, "commit", plan.planRevision);
    setBundleExportBusy(true);
    elements.applyBundleExport.querySelector("span").textContent = "正在导出";
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
    elements.serverInstallPrompt.textContent = serverInstallPrompt(receipt.destination);
    elements.bundleExportReceipt.hidden = false;
    for (const checkbox of elements.bundleSkillList.querySelectorAll('input[type="checkbox"]')) {
      checkbox.disabled = true;
    }
    elements.previewBundleExport.hidden = true;
    elements.applyBundleExport.hidden = true;
      elements.cancelBundleExport.textContent = "完成";
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
        elements.applyBundleExport.querySelector("span").textContent = "选择位置并导出";
        setBundleExportBusy(false);
      }
    }
  } catch (error) {
    showToast(bundleExportErrorMessage(error), true);
  }
}

function importStatusLabel(status) {
  return { compatible: "符合基础要求", review: "需要复核", incompatible: "不兼容" }[status] || status;
}

function bundleImportErrorMessage(error) {
  return {
    BUNDLE_IMPORT_SOURCE_INVALID: "请选择一个普通的 .skillbundle 文件。",
    BUNDLE_IMPORT_SOURCE_CHANGED: "读取期间 Bundle 发生变化，请重新选择。",
    BUNDLE_IMPORT_SESSION_UNKNOWN: "暂存会话已结束，请重新导入。",
    BUNDLE_IMPORT_SESSION_CHANGED: "暂存内容发生变化，请重新导入。",
    BUNDLE_LIMIT_EXCEEDED: "该 Bundle 超过允许的大小或文件数量限制。",
    INVALID_BUNDLE: "该文件不是有效的 Skill Bundle。",
    INVALID_BUNDLE_MANIFEST: "Bundle 清单无效或不受支持。",
    BUNDLE_HASH_MISMATCH: "Bundle 中的文件哈希与清单不一致。",
    UNSUPPORTED_BUNDLE_VERSION: "该 Bundle 版本尚不受支持。",
    UNSUPPORTED_ARCHIVE_FEATURE: "该归档使用了 Bundle v1 不支持的压缩特性。",
    UNSAFE_ARCHIVE_ENTRY: "归档中包含不安全的路径、链接或特殊条目。",
    DUPLICATE_ARCHIVE_ENTRY: "归档中包含重复文件条目。",
    BUNDLE_MANIFEST_MISSING: "归档中缺少 Skill Bundle 清单。",
    UNEXPECTED_BUNDLE_FILE: "归档中包含清单未声明的文件。",
    MISSING_BUNDLE_FILE: "归档缺少清单声明的文件。",
    BUNDLE_SIZE_MISMATCH: "Bundle 中的文件大小与清单不一致。",
    BUNDLE_REVISION_MISMATCH: "Skill 或 Bundle revision 与清单证据不一致。",
    BUNDLE_IO_ERROR: "读取 Bundle 时发生错误。",
    BUNDLE_IMPORT_IO_ERROR: "无法在应用缓存中暂存该 Bundle。",
    BUNDLE_INSTALL_REVIEW_STALE: "安装审查已过期，请根据最新同名版本重新选择。",
    BUNDLE_INSTALL_MATCH_UNKNOWN: "同名版本已变化，请重新查看比较结果。",
    BUNDLE_INSTALL_FILE_UNKNOWN: "该文件已不属于当前版本比较。",
    BUNDLE_INSTALL_SELECTION_INVALID: "请至少选择一个当前可安装的导入版本。",
    BUNDLE_INSTALL_BLOCKED: "当前分类、兼容性或审查结果不允许安装。",
    BUNDLE_INSTALL_IO_ERROR: "安装提交遇到本地文件错误；请检查每项结果。"
  }[error?.code] || error?.message || "导入没有完成。";
}

function importAuditPresentation(verdict) {
  return {
    clear: ["基础审查未发现阻断项", "未阻断", "本地规则未命中阻断问题；这不代表 Skill 安全。"],
    review: ["基础审查需要人工复核", "需复核", "请逐项检查用途、行为和文件证据。"],
    block: ["基础审查发现阻断问题", "阻断", "存在阻断项；后续不能直接进入安装确认。"]
  }[verdict] || ["基础审查结果未知", "未知", "请重新导入并检查审查证据。"];
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
    new: ["新 Skill", "is-new"],
    identical: ["完全相同 · 自动跳过", "is-identical"],
    userConflict: ["个人版本冲突", "is-user-conflict"],
    managedConflict: ["受管版本冲突", "is-managed-conflict"],
    incompatible: ["不兼容", "is-incompatible"]
  }[classification] || [classification, "is-incompatible"];
}

function fileDeltaLabel(status) {
  return {
    unchanged: "相同",
    modified: "已修改",
    importedOnly: "仅导入版本",
    currentOnly: "仅当前版本"
  }[status] || status;
}

function renderCatalogMatches(decision) {
  const container = document.createElement("div");
  container.className = "bundle-import-matches";
  if (!decision?.matches?.length) return container;
  const heading = document.createElement("div");
  heading.className = "bundle-import-subheading";
  heading.textContent = `同名版本 · ${decision.matches.length}`;
  container.append(heading);
  for (const match of decision.matches) {
    const details = document.createElement("details");
    details.className = "bundle-import-match";
    const summary = document.createElement("summary");
    const label = document.createElement("strong");
    label.textContent = `${sourceLabels[match.source] || match.source} · ${skillStateLabels[match.state] || match.state}`;
    const relation = document.createElement("span");
    relation.textContent = match.identical ? "完整 revision 相同" : "存在差异";
    summary.append(label, relation);
    const path = document.createElement("code");
    path.textContent = match.path;
    const revision = document.createElement("code");
    revision.textContent = match.revision
      ? `Skill 版本校验值 ${match.revision}`
      : "该目录无法计算可迁移版本校验值";
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
    ? "使用导入版本替换当前个人 Skill"
    : "安装这个导入版本";
  const summary = document.createElement("small");
  summary.textContent = decision.installOffer.summary;
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
    ? `复核安装 ${selected} 项`
    : "复核安装";
}

function renderBundleImportReview(review) {
  setImportReview(state.bundleWorkflow, review);
  state.bundleInstallReviewStale = false;
  elements.bundleImportTitle.textContent = "已验证，尚未安装";
  elements.bundleImportPhase.textContent = "暂存";
  elements.bundleImportMutationState.innerHTML = '<i data-lucide="shield-check"></i>未安装';
  elements.bundleInstallResults.hidden = true;
  elements.bundleInstallResults.replaceChildren();
  const decisions = new Map(review.decisions.map((decision) => [decision.directoryName, decision]));
  elements.bundleImportSkillCount.textContent = String(review.skills.length);
  elements.bundleImportSource.replaceChildren(
    candidateSourceRow("来源文件", review.sourceFileName),
    candidateSourceRow("来源文件校验值", review.sourceRevision),
    candidateSourceRow("Bundle 校验值", review.bundleRevision),
    candidateSourceRow("文件数量", `${review.totalFiles} 个 · ${formatBytes(review.totalBytes)}`)
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
    auditStatus.textContent = `审查：${auditLabel}`;
    const [classificationLabel, classificationClass] = importClassificationPresentation(
      decision?.classification || "incompatible"
    );
    const classification = document.createElement("span");
    classification.className = `bundle-import-classification ${classificationClass}`;
    classification.textContent = classificationLabel;
    titleGroup.append(title, classification);
    statusGroup.append(status, auditStatus);
    header.append(titleGroup, statusGroup);
    const skillRevision = bundleEvidenceDetails("查看 Skill 版本校验值", skill.revision);
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
      meta.textContent = `${formatBytes(file.size)}${file.executableAfterInstall ? " · 安装后可执行" : ""}`;
      button.append(path, meta);
      const entry = document.createElement("div");
      entry.className = "bundle-import-file-entry";
      entry.append(button, bundleEvidenceDetails("查看 SHA-256", file.sha256));
      files.append(entry);
    }
    const checks = document.createElement("div");
    checks.className = "bundle-import-checks";
    for (const check of skill.compatibility.checks) {
      const row = document.createElement("div");
      row.className = `bundle-import-check is-${check.status}`;
      const label = document.createElement("strong");
      label.textContent = check.label;
      const detail = document.createElement("small");
      detail.textContent = check.detail;
      row.append(label, detail);
      checks.append(row);
    }
    const findings = document.createElement("div");
    findings.className = "bundle-import-findings finding-list";
    findings.replaceChildren(...skill.audit.findings.map(renderFinding));
    const compatibilityHeading = document.createElement("div");
    compatibilityHeading.className = "bundle-import-subheading";
    compatibilityHeading.textContent = "Codex 兼容性";
    const auditHeading = document.createElement("div");
    auditHeading.className = "bundle-import-audit-heading";
    const auditTitleElement = document.createElement("strong");
    auditTitleElement.textContent = auditTitle;
    const auditSummary = document.createElement("p");
    auditSummary.textContent = auditSummaryText;
    auditHeading.append(auditTitleElement, auditSummary);
    const filesHeading = document.createElement("div");
    filesHeading.className = "bundle-import-subheading";
    filesHeading.textContent = "已验证文件";
    const decisionSummary = document.createElement("p");
    decisionSummary.className = "bundle-import-decision-summary";
    decisionSummary.textContent = decision?.summary || "未能判断这个 Skill 与当前目录的关系。";
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
  elements.bundleImportStatus.textContent = "内容已验证并暂存到应用缓存，尚未安装。";
  elements.bundleImportPreviewTitle.textContent = "文件预览";
  elements.bundleImportPreviewEmpty.hidden = false;
  elements.bundleImportPreviewEmpty.textContent = "选择一个文件以查看已验证暂存内容。";
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
  const labels = {
    installed: "已安装",
    replaced: "已替换",
    skippedIdentical: "已跳过相同版本",
    failed: "失败"
  };
  const rows = result.outcomes.map((outcome) => {
    const row = document.createElement("div");
    row.className = `bundle-install-result is-${outcome.status}`;
    const heading = document.createElement("div");
    const name = document.createElement("strong");
    name.textContent = outcome.directoryName;
    const status = document.createElement("span");
    status.textContent = labels[outcome.status] || outcome.status;
    const message = document.createElement("small");
    message.textContent = outcome.message;
    heading.append(name, status);
    row.append(heading, message);
    return row;
  });
  elements.bundleInstallResults.replaceChildren(...rows);
  elements.bundleInstallResults.hidden = rows.length === 0;
  elements.bundleImportTitle.textContent = "安装结果";
  elements.bundleImportPhase.textContent = "回执";
  elements.bundleImportMutationState.innerHTML = '<i data-lucide="list-checks"></i>已处理';
  refreshIcons();
}

async function openBundleImport() {
  try {
    const selected = await openDialog({
      multiple: false,
      title: "选择 Skill Bundle",
      filters: [{ name: "Skill Bundle", extensions: ["skillbundle"] }]
    });
    if (typeof selected !== "string") return;
    elements.importBundleButton.disabled = true;
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
  elements.bundleImportPreviewEmpty.textContent = "正在读取已验证暂存内容。";
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
      elements.bundleImportPreviewEmpty.textContent = "该文件不是 UTF-8 文本，完整哈希仍已验证。";
      return;
    }
    elements.bundleImportPreviewEmpty.hidden = true;
    elements.bundleImportFilePreview.hidden = false;
    elements.bundleImportFilePreview.textContent = result.content || "";
    if (result.truncated) {
      elements.bundleImportPreviewEmpty.hidden = false;
      elements.bundleImportPreviewEmpty.textContent = `仅显示前 ${formatBytes(result.previewBytes)}；完整文件哈希仍已验证。`;
    }
  } catch (error) {
    if (isCurrentImportPreview(state.bundleWorkflow, operation)
      && elements.bundleImportDialog.open) {
      elements.bundleImportPreviewEmpty.textContent = bundleImportErrorMessage(error);
    }
  }
}

function comparisonSideText(label, side) {
  if (!side.exists) return `${label}\n\n该版本中没有此文件。`;
  const metadata = `${formatBytes(side.size)} · SHA-256 ${side.sha256}${side.executable ? " · 可执行" : ""}`;
  if (!side.isText) return `${label}\n${metadata}\n\n该文件不是 UTF-8 文本。`;
  const truncated = side.truncated ? `\n\n仅显示前 ${formatBytes(side.previewBytes)}。` : "";
  return `${label}\n${metadata}\n\n${side.content || ""}${truncated}`;
}

async function compareImportedBundleFile(directoryName, matchId, path) {
  if (state.bundleInstallReviewStale) return;
  const operation = beginImportPreview(state.bundleWorkflow);
  if (!operation) return;
  elements.bundleImportPreviewTitle.textContent = `版本比较 · ${path}`;
  elements.bundleImportPreviewEmpty.hidden = false;
  elements.bundleImportPreviewEmpty.textContent = "正在重新核对双方 revision 与文件内容。";
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
    elements.bundleImportFilePreview.textContent = comparisonSideText("导入版本", result.imported);
    elements.bundleImportCurrentFilePreview.textContent = comparisonSideText("当前版本", result.current);
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
      ? "替换当前启用的个人 Skill"
      : "安装为启用的个人 Skill";
    return `${decision.directoryName}：${action}`;
  });
  presentConfirmation({
    title: "安装所选 Skill 版本",
    message: `${lines.join("\n")}\n\n安装前会重新验证 Bundle、全部同名版本和目标目录。${replacements.length ? " 被替换的个人 Skill 会先保留恢复副本。" : ""}`,
    label: `确认安装 ${selections.length} 项`,
    tone: replacements.length ? "danger" : "primary",
    action: () => performBundleInstall(review, selections)
  });
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
    for (const outcome of result.outcomes) {
      if (!outcome.skill) continue;
      applyCatalogState(applyInstallOutcome(state.skills, state.counts, outcome));
    }
    const installed = result.outcomes.filter((outcome) => ["installed", "replaced"].includes(outcome.status)).length;
    const skipped = result.outcomes.filter((outcome) => outcome.status === "skippedIdentical").length;
    const failed = result.outcomes.filter((outcome) => outcome.status === "failed").length;
    const restart = installed && result.restartRecommended ? " · 请重启 Codex 以加载变更" : "";
    const statusText = `安装结果：完成 ${installed} 项${skipped ? ` · 跳过相同 ${skipped} 项` : ""}${failed ? ` · 失败 ${failed} 项` : ""}${restart}。`;
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
      markBundleInstallReviewStale("安装回执已保留，但 Catalog 重新检查失败。重新导入后才能继续比较或安装。");
      showToast(`安装回执已保留；重新检查 Catalog 失败：${bundleImportErrorMessage(refreshError)}`, true);
    }
    showToast(failed ? "部分 Skill 未能安装，请查看逐项回执。" : "所选 Skill 版本已处理完成。", failed);
  } catch (error) {
    if (["BUNDLE_INSTALL_REVIEW_STALE", "BUNDLE_INSTALL_MATCH_UNKNOWN"].includes(error?.code)) {
      try {
        const refreshed = await desktop.reviewImportedBundle(review.sessionId, review.bundleRevision);
        renderBundleImportReview(refreshed);
      } catch {
        markBundleInstallReviewStale("同名版本已变化且重新检查失败。重新导入后才能继续比较或安装。");
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

function renderCandidateSource(manifest) {
  const source = manifest.source;
  const requestedRef = source.requestedRef || source.requested_ref;
  const resolvedSha = source.resolvedSha || source.resolvedSHA || source.resolved_sha;
  const skillPath = source.skillPath || source.skill_path;
  const selectedPath = source.selectedPath || source.selected_path;
  const rows = source.kind === "github"
    ? [
      candidateSourceRow("来源", "GitHub 公开仓库"),
      candidateSourceRow("仓库", source.repository),
      candidateSourceRow("请求分支", requestedRef),
      candidateSourceRow("固定提交", resolvedSha),
      candidateSourceRow("Skill 路径", skillPath || "仓库根目录"),
      candidateSourceRow("候选哈希", manifest.candidateHash)
    ]
    : [
      candidateSourceRow("来源", "本地文件夹"),
      candidateSourceRow("选择位置", selectedPath),
      candidateSourceRow("候选哈希", manifest.candidateHash)
    ];
  elements.candidateSourceList.replaceChildren(...rows);
}

function renderCandidateCompatibility(compatibility) {
  const labels = {
    compatible: "基础兼容",
    review: "需复核",
    incompatible: "不兼容"
  };
  elements.candidateCompatibilityStatus.textContent = labels[compatibility.status] || compatibility.status;
  elements.candidateCompatibilityStatus.className = `candidate-status is-${compatibility.status}`;
  elements.candidateCompatibilitySummary.textContent = compatibility.summary;
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
    label.textContent = check.label;
    const detail = document.createElement("p");
    detail.textContent = check.detail;
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
    meta.textContent = `${formatBytes(file.size)} · SHA-256 ${file.sha256.slice(0, 12)}…${file.executable ? " · 可执行" : ""}`;
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
    elements.candidatePreviewTitle.textContent = "文件预览";
    elements.candidatePreviewMeta.textContent = "";
    elements.candidatePreviewEmpty.hidden = false;
    elements.candidateFilePreview.hidden = true;
    return;
  }
  const file = candidate.review.manifest.files.find((item) => item.path === candidate.selectedPath);
  elements.candidatePreviewTitle.textContent = candidate.selectedPath;
  elements.candidatePreviewMeta.textContent = file ? `SHA-256 ${file.sha256.slice(0, 12)}…` : "";
  if (preview?.loading) {
    elements.candidatePreviewEmpty.textContent = "正在读取暂存内容。";
    elements.candidatePreviewEmpty.hidden = false;
    elements.candidateFilePreview.hidden = true;
    return;
  }
  if (!preview?.isText) {
    elements.candidatePreviewEmpty.textContent = "该文件不是 UTF-8 文本，无法在此预览。文件哈希仍已列入上方清单。";
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
    clear: ["基础检查未发现已知阻断模式", "可继续", "内置规则未命中问题，不代表候选已通过完整安全审计。"],
    review: ["需要人工复核", "需复核", "请阅读下方证据，并确认这些行为符合预期。"],
    block: ["建议阻止", "已阻止", "候选包含结构错误或高影响操作，安装前不应继续。"]
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
    elements.candidateSkippedSummary.textContent = "所有暂存文件都在左侧清单中。不支持的条目会直接中止审查，不会被静默忽略。";
    return;
  }
  elements.candidateSkippedSummary.textContent = "以下条目未被纳入候选内容：";
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
  const name = review.audit.document.name || "候选 Skill";
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
    ? "先解决不兼容结构或阻断问题"
    : "复核安装位置并确认安装";
  refreshIcons();
}

function candidateVersionLabel(manifest) {
  if (manifest.source.kind === "github") {
    const resolvedSha = manifest.source.resolvedSha
      || manifest.source.resolvedSHA
      || manifest.source.resolved_sha;
    const version = resolvedSha
      ? `提交 ${resolvedSha.slice(0, 12)}`
      : `候选 ${manifest.candidateHash.slice(0, 12)}`;
    return `${manifest.source.repository} · ${version}`;
  }
  return manifest.source.selectedPath || manifest.source.selected_path;
}

async function performCandidateInstall(candidate, preview) {
  const { manifest } = candidate.review;
  elements.installCandidate.disabled = true;
  elements.installCandidate.querySelector("span").textContent = "正在安装";
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
      showToast("Skill 已安装。请在新任务中使用最新版本。");
    } else {
      await loadSkills({ preserveSelection: false, forceRefresh: true });
      const installed = state.skills.find((skill) => skill.source === "personal" && skill.name === preview.name);
      if (installed) await selectSkill(installed.id);
      showToast("Skill 已安装；目录索引已重新读取。请在新任务中使用最新版本。");
    }
  } finally {
    if (state.candidate === candidate) {
      elements.installCandidate.disabled = false;
      elements.installCandidate.querySelector("span").textContent = "安装 Skill";
    }
  }
}

async function requestCandidateInstall() {
  const candidate = state.candidate;
  if (!candidate) return;
  const { review } = candidate;
  if (review.compatibility.status === "incompatible" || review.audit.verdict === "block") return;
  elements.installCandidate.disabled = true;
  elements.installCandidate.querySelector("span").textContent = "正在检查";
  try {
    const preview = await desktop.previewStagedCandidateInstall(
      review.manifest.sessionId,
      review.manifest.candidateHash
    );
    if (state.candidate !== candidate) return;
    if (!preview.canInstall) {
      const conflictPath = preview.conflict?.path;
      showToast(
        conflictPath ? `目标位置已有同名 Skill：${conflictPath}` : "该候选当前不能安装，请复核阻断项。",
        true
      );
      return;
    }
    const auditState = preview.auditVerdict === "review" ? "基础检查有需人工复核项" : "基础检查无已知阻断项";
    const deepAuditState = candidate.deepAudit
      ? `云端深度审查：${{ clear: "未保留语义风险项", review: "有需人工复核项", block: "有高影响风险项" }[candidate.deepAudit.verdict] || candidate.deepAudit.verdict}`
      : "云端深度审查：未运行（不影响基础安装门槛）";
    presentConfirmation({
      title: `安装 ${preview.name}`,
      message: [
        `来源版本：${candidateVersionLabel(review.manifest)}`,
        `安装位置：${preview.destination}`,
        `文件：${preview.fileCount} 个（安装前将再次核对完整清单与哈希）`,
        `审查状态：${auditState}`,
        deepAuditState,
        "安装不会执行候选中的脚本，也不会覆盖同名 Skill。"
      ].join("\n"),
      label: "确认安装",
      action: () => performCandidateInstall(candidate, preview),
      tone: "primary"
    });
  } catch (error) {
    showToast(error.message, true);
  } finally {
    if (state.candidate === candidate) {
      elements.installCandidate.disabled = false;
      elements.installCandidate.querySelector("span").textContent = "安装 Skill";
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
      showToast("尚未配置深度审查模型。请先在“设置”中填写 API 模式、Base URL、模型和 API key。", true);
      return;
    }
    const { manifest } = candidate.review;
    const preview = await desktop.previewStagedCandidateDeepAudit(
      manifest.sessionId,
      manifest.candidateHash
    );
    if (state.candidate !== candidate) return;
    if (preview.sourceRevision !== manifest.candidateHash) {
      throw new Error("候选内容已变化，请关闭后重新暂存并审查。");
    }
    state.deepAuditContext = { kind: "candidate", candidate };
    state.deepAuditPreview = preview;
    renderDeepAuditConsent(preview);
    elements.deepConsentDialog.showModal();
    refreshIcons();
  } catch (error) {
    showToast(error.message, true);
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
    showToast(error.message, true);
  }
}

async function stageCandidate(event) {
  event.preventDefault();
  const github = state.candidateSourceMode === "github";
  const source = github ? elements.candidateGithubUrl.value.trim() : state.candidateLocalPath;
  if (!source) {
    showToast(github ? "请输入公开 GitHub 地址。" : "请选择包含 SKILL.md 的文件夹。", true);
    return;
  }
  let manifest = null;
  elements.stageCandidate.disabled = true;
  elements.stageCandidate.querySelector("span").textContent = "正在暂存";
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
    showToast(error.message, true);
  } finally {
    elements.stageCandidate.disabled = false;
    elements.stageCandidate.querySelector("span").textContent = "暂存并审查";
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
    showToast(error.message, true);
  }
}

function clearConnectionTestResult() {
  state.providerTestSequence += 1;
  elements.deepConnectionStatus.hidden = true;
  elements.deepConnectionStatus.textContent = "";
  elements.deepConnectionStatus.className = "connection-status";
}

async function openSettings() {
  try {
    if (elements.settingsDialog.open) {
      elements.deepEndpoint.focus();
      return;
    }
    const settings = await desktop.getDeepAuditSettings();
    state.deepAuditSettings = settings;
    clearConnectionTestResult();
    elements.deepApiMode.value = settings.apiMode || "chatCompletions";
    elements.deepEndpoint.value = settings.endpoint;
    elements.deepModel.value = settings.model;
    elements.deepApiKey.value = "";
    elements.deepApiKey.placeholder = settings.hasApiKey ? "已存储；留空可继续使用" : "输入 API key";
    elements.deepCredentialState.textContent = settings.hasApiKey
      ? "API key 已存储在 macOS Keychain，应用不会回显。"
      : "API key 将存储在 macOS Keychain，应用不会回显。";
    elements.clearDeepSettings.disabled = !settings.hasApiKey && !settings.endpoint && !settings.model;
    elements.settingsDialog.showModal();
    elements.deepEndpoint.focus();
    refreshIcons();
  } catch (error) {
    showToast(error.message, true);
  }
}

async function testDeepAuditConnection() {
  const sequence = ++state.providerTestSequence;
  const apiKey = elements.deepApiKey.value.trim() || null;
  elements.testDeepConnection.disabled = true;
  elements.testDeepConnection.querySelector("span").textContent = "测试中";
  elements.deepConnectionStatus.hidden = false;
  elements.deepConnectionStatus.className = "connection-status";
  elements.deepConnectionStatus.textContent = "正在连接...";
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
    elements.deepConnectionStatus.textContent = `连接成功 · ${apiMode} · ${result.endpoint}`;
  } catch (error) {
    if (sequence !== state.providerTestSequence) return;
    elements.deepConnectionStatus.classList.add("is-error");
    elements.deepConnectionStatus.textContent = `连接失败：${error.message}`;
  } finally {
    elements.testDeepConnection.disabled = false;
    elements.testDeepConnection.querySelector("span").textContent = "测试连接";
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
    showToast("深度审查配置已更新。API key 保存在 Keychain。");
  } catch (error) {
    showToast(error.message, true);
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
  elements.deepConsentRequests.textContent = `${preview.requestCount} 次（威胁判断 + 误报复核）`;
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
    metadata.textContent = `${formatBytes(file.size)} · SHA-256 ${file.sha256.slice(0, 12)}…${file.required ? " · 必选" : ""}`;
    copy.append(path, metadata);
    label.append(checkbox, copy);
    return label;
  });
  elements.deepConsentFiles.replaceChildren(...fileRows);
  elements.deepSkippedFiles.hidden = preview.skippedFiles.length === 0;
  elements.deepSkippedSummary.textContent = `${preview.skippedFiles.length} 个文件不会上传`;
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
      showToast("尚未配置深度审查模型。请先在“设置”中填写 API 模式、Base URL、模型和 API key。", true);
      return;
    }
    const preview = await desktop.previewDeepAudit(deepAuditEditorId(), state.editor.draftMarkdown);
    state.deepAuditContext = { kind: "editor", editor: state.editor };
    state.deepAuditPreview = preview;
    renderDeepAuditConsent(preview);
    elements.deepConsentDialog.showModal();
    refreshIcons();
  } catch (error) {
    showToast(error.message, true);
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
  elements.runDeepAudit.querySelector("span").textContent = "正在审查";
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
    showToast("深度审查完成。");
  } catch (error) {
    elements.deepConsentDialog.close();
    showToast(error.message, true);
  } finally {
    elements.runDeepAudit.disabled = false;
    elements.runDeepAudit.querySelector("span").textContent = "确认发送并审查";
    state.deepAuditPreview = null;
    state.deepAuditContext = null;
  }
}

function renderDraftAudit(audit) {
  const verdicts = {
    clear: {
      title: "基础检查未发现已知阻断模式",
      badge: "可继续",
      summary: "仅表示内置规则没有命中；不代表 Skill 已通过完整安全审计。"
    },
    review: {
      title: "需要人工复核",
      badge: "需复核",
      summary: "保存前请阅读下方证据，并确认这些行为符合预期。"
    },
    block: {
      title: "建议阻止",
      badge: "已阻止",
      summary: "草稿包含结构错误或高影响操作，解决后才能保存。"
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
    elements.diffBefore.textContent = audit.diff.before.join("\n") || "（无内容）";
    elements.diffAfter.textContent = audit.diff.after.join("\n") || "（无内容）";
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
    if (sequence === state.auditSequence) renderAuditError(error.message);
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
    showToast("这个 Skill 由系统或插件管理，不能在这里编辑。", true);
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
  elements.saveDraft.innerHTML = '<i data-lucide="save"></i><span>保存修改</span>';
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
  elements.editorTitle.textContent = "新建 Skill";
  elements.saveDraft.innerHTML = '<i data-lucide="plus"></i><span>创建 Skill</span>';
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
    showToast("修改已保存。请在新任务中使用最新版本。");
    await loadSkills();
    await selectSkill(id);
  } catch (error) {
    showToast(error.message, true);
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
    showToast("Skill 已创建。请在新任务中使用最新版本。");
    await loadSkills({ preserveSelection: false });
    await selectSkill(created.id);
  } catch (error) {
    showToast(error.message, true);
  } finally {
    if (state.editor) updateEditorStatus();
  }
}

function requestDraftSave() {
  if (!state.editor?.audit || state.editor.audit.verdict === "block") return;
  if (state.editor.isNew) {
    if (!state.editor.preview?.canCreate) return;
    presentConfirmation({
      title: "创建新 Skill",
      message: state.editor.deepAudit?.verdict === "block"
        ? `云端深度审查保留了高影响风险项。阅读证据后，仍将在 ${state.editor.preview.destination} 创建 SKILL.md。`
        : `将在 ${state.editor.preview.destination} 创建 SKILL.md。`,
      label: "确认创建",
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
    title: "保存需要复核的草稿",
    message: state.editor.deepAudit?.verdict === "block"
      ? "云端深度审查保留了高影响风险项。阅读证据并确认行为符合预期后再保存。"
      : "检查发现了需要人工复核的行为。确认这些行为符合预期后再保存。",
    label: "确认保存",
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
    title: "放弃未保存修改",
    message: "关闭后，这次草稿修改不会保留。",
    label: "放弃修改",
    action: async () => {
      elements.editorDialog.close();
      state.editor = null;
    }
  });
}

async function requestSkillLifecycle(action, skill) {
  const config = {
    disable: ["停用 Skill", `停用 ${skill.displayName}？新任务将不再加载它。`, "确认停用", "primary"],
    enable: ["重新启用 Skill", `将 ${skill.displayName} 恢复到个人 Skill？`, "确认启用", "primary"],
    archive: ["归档 Skill", `将 ${skill.displayName} 移入可恢复归档？`, "确认归档", "primary"],
    restore: ["恢复 Skill", `将 ${skill.displayName} 恢复到启用的个人 Skill？`, "确认恢复", "primary"],
    delete: ["永久删除 Skill", `永久删除 ${skill.displayName} 及其全部文件？此操作无法撤销。`, "永久删除", "danger"]
  }[action];
  try {
    const preview = await desktop.previewSkillLifecycle(skill.id, action);
    if (!preview.canApply) {
      showToast(`目标位置已有同名目录：${preview.conflict?.path || preview.destination}`, true);
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
        showToast(`${skill.displayName} 已永久删除。`);
      }
      : async () => {
        const result = await desktop.applySkillLifecycle(skill.id, action, preview.directoryRevision);
        state.selectedId = result.skill.id;
        state.detail = result.skill;
        applyCatalogState(replaceCatalogSkill(state.skills, state.counts, skill.id, result.skill));
        elements.detailPanel.classList.add("is-open");
        renderDetail();
        showToast(`${config[2].replace("确认", "")}完成。请在新任务中使用最新状态。`);
      };
    presentConfirmation({
      title: config[0],
      message: `${config[1]}${preview.destination ? `\n目标位置：${preview.destination}` : ""}`,
      label: config[2],
      action: apply,
      tone: config[3],
      requiredName: action === "delete" ? skill.name : null
    });
  } catch (error) {
    showToast(error.message, true);
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
    showToast(error.message, true);
  } finally {
    elements.refresh.classList.remove("is-spinning");
    elements.refresh.disabled = false;
  }
}

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
elements.settings.addEventListener("click", openSettings);
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
    showToast("服务器安装指令已复制。");
  } catch (error) {
    showToast(`复制失败：${error.message}`, true);
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
    title: "移除深度审查配置",
    message: "将删除已保存的服务地址、模型名称和 Keychain 中的 API key。",
    label: "确认移除",
    action: async () => {
      state.deepAuditSettings = await desktop.clearDeepAuditSettings();
      elements.settingsDialog.close();
      showToast("深度审查配置已移除。");
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
  showToast("路径已复制。");
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
    showToast(error.message, true);
  } finally {
    setConfirmationBusy(false);
    state.confirmAction = null;
    state.confirmRequiredName = null;
  }
});


document.addEventListener("keydown", (event) => {
  if (event.metaKey && !event.altKey && !event.ctrlKey && event.key === ",") {
    event.preventDefault();
    openSettings();
    return;
  }
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
loadSkills({ preserveSelection: false });
