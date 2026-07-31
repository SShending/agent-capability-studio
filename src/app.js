import { createIcons, icons } from "lucide";
import { desktop } from "./desktop-bridge.js";
import {
  personalSkillsNeedingAttention,
  removeCatalogSkill,
  replaceCatalogSkill
} from "./catalog-state.js";
import { parseSkillDocument, updateSkillDocument } from "./skill-document.js";

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
  confirmAction: null,
  confirmRequiredName: null,
  providerTestSequence: 0,
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
  settings: document.querySelector("#settings-button"),
  auditStatus: document.querySelector("#audit-status"),
  auditLabel: document.querySelector("#audit-label"),
  auditIssueList: document.querySelector("#audit-issue-list"),
  confirmDialog: document.querySelector("#confirm-dialog"),
  confirmForm: document.querySelector("#confirm-form"),
  confirmTitle: document.querySelector("#confirm-title"),
  confirmMessage: document.querySelector("#confirm-message"),
  confirmSubmit: document.querySelector("#confirm-submit"),
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
  const confidence = document.createElement("span");
  confidence.className = "confidence-label";
  confidence.textContent = item.disposition === "dismissed"
    ? "复核后排除"
    : { high: "高置信", medium: "中置信", low: "低置信" }[item.confidence] || item.confidence;
  heading.append(marker, title, confidence);

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

function renderDeepAuditResult(result) {
  if (state.editor) state.editor.deepAudit = result;
  elements.deepResultSection.hidden = !result;
  if (!result) {
    elements.deepFindingList.replaceChildren();
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
  elements.deepResultVerdict.textContent = title;
  elements.deepResultBadge.textContent = badge;
  elements.deepResultBadge.className = `verdict-badge is-${result.verdict}`;
  elements.deepResultSummary.textContent = summary;
  const apiMode = deepAuditApiModeLabels[result.apiMode] || result.apiMode;
  elements.deepResultMeta.textContent = `${apiMode} · ${result.model} · ${result.files.length} 个文件 · 2 次请求${dismissed ? ` · 排除 ${dismissed} 项` : ""}`;
  elements.deepFindingList.replaceChildren(...result.findings.map(renderFinding));
  refreshIcons();
}

function formatBytes(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  return `${(bytes / 1024).toFixed(bytes < 10240 ? 1 : 0)} KB`;
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
  if (!state.editor || !state.deepAuditPreview) return;
  const selectedPaths = [...elements.deepConsentFiles.querySelectorAll("input")]
    .filter((input) => input.checked)
    .map((input) => input.value);
  elements.runDeepAudit.disabled = true;
  elements.runDeepAudit.querySelector("span").textContent = "正在审查";
  try {
    const result = await desktop.runDeepAudit(
      deepAuditEditorId(),
      state.editor.draftMarkdown,
      selectedPaths,
      state.deepAuditPreview.candidateHash,
      state.deepAuditPreview.providerHash
    );
    elements.deepConsentDialog.close();
    renderDeepAuditResult(result);
    showToast("深度审查完成。");
  } catch (error) {
    elements.deepConsentDialog.close();
    showToast(error.message, true);
  } finally {
    elements.runDeepAudit.disabled = false;
    elements.runDeepAudit.querySelector("span").textContent = "确认发送并审查";
    state.deepAuditPreview = null;
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
  renderDeepAuditResult(null);
  elements.editorTitle.textContent = detail.displayName;
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
  renderDeepAuditResult(null);
  elements.editorTitle.textContent = "新建 Skill";
  elements.saveDraft.innerHTML = '<i data-lucide="plus"></i><span>创建 Skill</span>';
  elements.draftSource.value = markdown;
  syncGuidedFields();
  setEditorMode("guided");
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
  const toastHost = elements.deepConsentDialog.open
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
elements.settings.addEventListener("click", openSettings);
elements.closeDetail.addEventListener("click", () => elements.detailPanel.classList.remove("is-open"));
elements.closeEditor.addEventListener("click", requestCloseEditor);
elements.guidedMode.addEventListener("click", () => setEditorMode("guided"));
elements.sourceMode.addEventListener("click", () => setEditorMode("source"));
elements.auditDraft.addEventListener("click", runDraftAudit);
elements.deepAudit.addEventListener("click", requestDeepAudit);
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
  elements.deepConsentDialog.close();
});
document.querySelector("#cancel-deep-audit-consent").addEventListener("click", () => {
  state.deepAuditPreview = null;
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
document.querySelector("#cancel-confirm-button").addEventListener("click", () => {
  state.confirmAction = null;
  state.confirmRequiredName = null;
  elements.confirmDialog.close();
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
  elements.confirmSubmit.disabled = true;
  try {
    await state.confirmAction();
    elements.confirmDialog.close();
  } catch (error) {
    elements.confirmDialog.close();
    showToast(error.message, true);
  } finally {
    elements.confirmSubmit.disabled = false;
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
