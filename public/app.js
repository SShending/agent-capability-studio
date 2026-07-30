const token = document.querySelector('meta[name="skill-center-token"]').content;

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
  confirmAction: null,
  toastTimer: null
};

const elements = {
  list: document.querySelector("#skill-list"),
  empty: document.querySelector("#empty-state"),
  resultSummary: document.querySelector("#result-summary"),
  search: document.querySelector("#search-input"),
  sort: document.querySelector("#sort-select"),
  refresh: document.querySelector("#refresh-button"),
  install: document.querySelector("#install-button"),
  installDialog: document.querySelector("#install-dialog"),
  installForm: document.querySelector("#install-form"),
  installSubmit: document.querySelector("#install-submit"),
  confirmDialog: document.querySelector("#confirm-dialog"),
  confirmForm: document.querySelector("#confirm-form"),
  confirmTitle: document.querySelector("#confirm-title"),
  confirmMessage: document.querySelector("#confirm-message"),
  confirmSubmit: document.querySelector("#confirm-submit"),
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
  draftBody: document.querySelector("#draft-body"),
  draftSource: document.querySelector("#draft-source"),
  auditDraft: document.querySelector("#audit-draft-button"),
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

function refreshIcons() {
  window.lucide?.createIcons({ attrs: { "aria-hidden": "true" } });
}

async function api(url, options = {}) {
  const response = await fetch(url, {
    ...options,
    headers: {
      "Content-Type": "application/json",
      "X-Skill-Center-Token": token,
      ...(options.headers || {})
    }
  });
  let payload;
  try {
    payload = await response.json();
  } catch {
    payload = {};
  }
  if (!response.ok) throw new Error(payload.error || `Request failed (${response.status})`);
  return payload;
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

function parseDraftDocument(markdown) {
  const match = markdown.match(/^---\s*\r?\n([\s\S]*?)\r?\n---(?:\s*\r?\n|\s*$)/);
  if (!match) return { name: "", description: "", body: markdown };
  const values = {};
  const lines = match[1].split(/\r?\n/);
  for (let index = 0; index < lines.length; index += 1) {
    const keyMatch = lines[index].match(/^([A-Za-z0-9_-]+):(?:\s*(.*))?$/);
    if (!keyMatch) continue;
    const [, key, rawValue = ""] = keyMatch;
    if (/^[>|][+-]?$/.test(rawValue.trim())) {
      const folded = [];
      while (index + 1 < lines.length && /^\s+/.test(lines[index + 1])) {
        index += 1;
        folded.push(lines[index].trim());
      }
      values[key] = rawValue.startsWith(">") ? folded.join(" ") : folded.join("\n");
    } else {
      values[key] = rawValue.trim().replace(/^(['"])([\s\S]*)\1$/, "$2");
    }
  }
  return {
    name: values.name || "",
    description: values.description || "",
    body: markdown.slice(match[0].length).replace(/^\s+/, "")
  };
}

function updateDescription(markdown, description) {
  const match = markdown.match(/^---\s*\r?\n([\s\S]*?)\r?\n---/);
  if (!match) return markdown;
  const lines = match[1].split(/\r?\n/);
  const start = lines.findIndex((line) => /^description:/.test(line));
  let end = start;
  if (start >= 0) {
    while (end + 1 < lines.length && /^\s+/.test(lines[end + 1])) end += 1;
  }
  const normalized = description
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
  const replacement = ["description: >-", ...(normalized.length ? normalized : [""]).map((line) => `  ${line}`)];
  if (start >= 0) lines.splice(start, end - start + 1, ...replacement);
  else lines.push(...replacement);
  return `---\n${lines.join("\n")}\n---${markdown.slice(match[0].length)}`;
}

function updateGuidedDraft(markdown, description, body) {
  const withDescription = updateDescription(markdown, description);
  const match = withDescription.match(/^---\s*\r?\n[\s\S]*?\r?\n---/);
  if (!match) return withDescription;
  return `${match[0]}\n\n${body.replace(/^\s+|\s+$/g, "")}\n`;
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

function updateCounts() {
  const counts = state.counts;
  document.querySelector("#count-all").textContent = counts.total || 0;
  for (const source of ["personal", "disabled", "system", "plugin", "archive"]) {
    document.querySelector(`#count-${source}`).textContent = counts[source] || 0;
  }
  document.querySelector("#codex-home").textContent = state.codexHome || "~/.codex";

  const audit = document.querySelector("#audit-status");
  const label = document.querySelector("#audit-label");
  audit.classList.toggle("is-good", !counts.needsAttention);
  audit.classList.toggle("has-issues", Boolean(counts.needsAttention));
  label.textContent = counts.needsAttention ? `${counts.needsAttention} 项存在阻断问题` : "个人 Skill 状态正常";
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
      actionButton("停用", "circle-pause", "secondary-button", () => confirmSkillAction("toggle", skill)),
      actionButton("归档", "archive", "secondary-button", () => confirmSkillAction("archive", skill))
    );
  } else if (skill.source === "disabled") {
    actions.append(
      actionButton("编辑", "square-pen", "primary-button", () => openEditor(skill.id)),
      actionButton("启用", "circle-play", "secondary-button", () => confirmSkillAction("toggle", skill)),
      actionButton("归档", "archive", "secondary-button", () => confirmSkillAction("archive", skill))
    );
  } else if (skill.source === "archive") {
    actions.append(actionButton("恢复", "archive-restore", "primary-button", () => confirmSkillAction("restore", skill)));
  } else {
    const readonly = document.createElement("span");
    readonly.className = "trigger-badge";
    readonly.textContent = "只读管理";
    actions.append(readonly);
  }
  refreshIcons();
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
  const document = parseDraftDocument(state.editor.draftMarkdown);
  elements.draftName.value = document.name;
  elements.draftDescription.value = document.description;
  elements.draftBody.value = document.body;
}

function editorChanged() {
  return Boolean(state.editor && state.editor.draftMarkdown !== state.editor.originalMarkdown);
}

function updateEditorStatus() {
  const changed = editorChanged();
  elements.draftStatus.textContent = changed ? "有未保存修改" : "未修改";
  elements.draftStatus.classList.toggle("is-dirty", changed);
  elements.saveDraft.disabled =
    !changed || !state.editor?.audit || state.editor.audit.verdict === "block" || state.editor.auditLoading;
}

function setDraftMarkdown(markdown, { syncSource = true } = {}) {
  if (!state.editor) return;
  state.editor.draftMarkdown = markdown;
  if (syncSource) elements.draftSource.value = markdown;
  state.editor.audit = null;
  updateEditorStatus();
  scheduleDraftAudit();
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
  row.className = `finding-row ${item.severity}`;

  const heading = document.createElement("div");
  heading.className = "finding-heading";
  const marker = document.createElement("span");
  marker.className = "finding-marker";
  const title = document.createElement("strong");
  title.textContent = item.title;
  const confidence = document.createElement("span");
  confidence.className = "confidence-label";
  confidence.textContent = { high: "高置信", medium: "中置信", low: "低置信" }[item.confidence] || item.confidence;
  heading.append(marker, title, confidence);

  const explanation = document.createElement("p");
  explanation.textContent = item.explanation;
  const evidence = document.createElement("details");
  const summary = document.createElement("summary");
  summary.textContent = "查看证据";
  const evidenceText = document.createElement("p");
  evidenceText.textContent = item.evidence;
  evidence.append(summary, evidenceText);
  row.append(heading, explanation, evidence);
  return row;
}

function renderDraftAudit(audit) {
  const verdicts = {
    clear: {
      title: "未发现阻断项",
      badge: "可继续",
      summary: "基础规则未命中问题；这不是绝对安全保证。"
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
    const audit = await api(`/api/skills/${encodeURIComponent(editorId)}/audit`, {
      method: "POST",
      body: JSON.stringify({ markdown })
    });
    if (!state.editor || state.editor.id !== editorId || sequence !== state.auditSequence) return;
    state.editor.audit = audit;
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
    auditLoading: false,
    auditTimer: null
  };
  elements.editorTitle.textContent = detail.displayName;
  elements.draftSource.value = detail.markdown;
  syncGuidedFields();
  setEditorMode("guided");
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

function requestDraftSave() {
  if (!state.editor?.audit || state.editor.audit.verdict === "block") return;
  if (state.editor.audit.verdict === "clear") {
    performDraftSave();
    return;
  }
  state.confirmAction = performDraftSave;
  elements.confirmTitle.textContent = "保存需要复核的草稿";
  elements.confirmMessage.textContent = "检查发现了需要人工复核的行为。确认这些行为符合预期后再保存。";
  elements.confirmSubmit.textContent = "确认保存";
  elements.confirmDialog.showModal();
}

function requestCloseEditor() {
  if (!state.editor || !editorChanged()) {
    elements.editorDialog.close();
    state.editor = null;
    return;
  }
  state.confirmAction = async () => {
    elements.editorDialog.close();
    state.editor = null;
  };
  elements.confirmTitle.textContent = "放弃未保存修改";
  elements.confirmMessage.textContent = "关闭后，这次草稿修改不会保留。";
  elements.confirmSubmit.textContent = "放弃修改";
  elements.confirmDialog.showModal();
}

function confirmSkillAction(action, skill) {
  const config = {
    toggle: skill.source === "personal"
      ? ["停用 Skill", `停用 ${skill.displayName}？新任务将不再加载它。`, "停用"]
      : ["启用 Skill", `重新启用 ${skill.displayName}？`, "启用"],
    archive: ["归档 Skill", `将 ${skill.displayName} 移入可恢复归档？`, "归档"],
    restore: ["恢复 Skill", `将 ${skill.displayName} 恢复到个人 Skill？`, "恢复"]
  }[action];
  state.confirmAction = async () => {
    await api(`/api/skills/${encodeURIComponent(skill.id)}/${action}`, { method: "POST", body: "{}" });
    showToast(`${config[2]}完成。请在新任务中使用最新状态。`);
    state.selectedId = null;
    state.detail = null;
    elements.detailPanel.classList.remove("is-open");
    await loadSkills();
  };
  elements.confirmTitle.textContent = config[0];
  elements.confirmMessage.textContent = config[1];
  elements.confirmSubmit.textContent = config[2];
  elements.confirmDialog.showModal();
}

function showToast(message, isError = false) {
  clearTimeout(state.toastTimer);
  const toastHost = elements.editorDialog.open ? elements.editorDialog : document.body;
  if (elements.toast.parentElement !== toastHost) toastHost.append(elements.toast);
  elements.toast.classList.toggle("is-error", isError);
  elements.toastMessage.textContent = message;
  elements.toast.hidden = false;
  state.toastTimer = setTimeout(() => {
    elements.toast.hidden = true;
    if (elements.toast.parentElement !== document.body) document.body.append(elements.toast);
  }, isError ? 6500 : 4200);
}

async function loadSkills({ preserveSelection = true } = {}) {
  elements.refresh.classList.add("is-spinning");
  elements.refresh.disabled = true;
  try {
    const data = await api("/api/skills");
    state.skills = data.skills;
    state.counts = data.counts;
    state.roots = data.roots;
    state.codexHome = data.codexHome;
    if (!preserveSelection || !state.skills.some((skill) => skill.id === state.selectedId)) {
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
  state.source = button.dataset.source;
  document.querySelectorAll("[data-source]").forEach((item) => {
    item.classList.toggle("is-active", item === button);
    item.setAttribute("aria-pressed", String(item === button));
  });
  renderList();
});

elements.search.addEventListener("input", () => {
  state.query = elements.search.value;
  renderList();
});

elements.sort.addEventListener("change", () => {
  state.sort = elements.sort.value;
  renderList();
});

elements.refresh.addEventListener("click", () => loadSkills());
elements.install.addEventListener("click", () => elements.installDialog.showModal());
elements.closeDetail.addEventListener("click", () => elements.detailPanel.classList.remove("is-open"));
elements.closeEditor.addEventListener("click", requestCloseEditor);
elements.guidedMode.addEventListener("click", () => setEditorMode("guided"));
elements.sourceMode.addEventListener("click", () => setEditorMode("source"));
elements.auditDraft.addEventListener("click", runDraftAudit);
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
  setDraftMarkdown(
    updateGuidedDraft(state.editor.draftMarkdown, elements.draftDescription.value, elements.draftBody.value)
  );
});
elements.draftBody.addEventListener("input", () => {
  if (!state.editor) return;
  setDraftMarkdown(
    updateGuidedDraft(state.editor.draftMarkdown, elements.draftDescription.value, elements.draftBody.value)
  );
});
elements.draftSource.addEventListener("input", () => {
  setDraftMarkdown(elements.draftSource.value, { syncSource: false });
});
document.querySelector("#close-install-button").addEventListener("click", () => elements.installDialog.close());
document.querySelector("#cancel-install-button").addEventListener("click", () => elements.installDialog.close());
document.querySelector("#cancel-confirm-button").addEventListener("click", () => {
  state.confirmAction = null;
  elements.confirmDialog.close();
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
    showToast(error.message, true);
  } finally {
    elements.confirmSubmit.disabled = false;
    state.confirmAction = null;
  }
});

elements.installForm.addEventListener("submit", async (event) => {
  if (event.submitter?.value !== "default") return;
  event.preventDefault();
  const formData = new FormData(elements.installForm);
  elements.installSubmit.disabled = true;
  try {
    await api("/api/install", {
      method: "POST",
      body: JSON.stringify({ repo: formData.get("repo"), ref: formData.get("ref"), path: formData.get("path") })
    });
    elements.installDialog.close();
    elements.installForm.reset();
    elements.installForm.elements.ref.value = "main";
    showToast("Skill 已安装。请在列表中检查后，于新任务中使用。");
    await loadSkills({ preserveSelection: false });
  } catch (error) {
    showToast(error.message, true);
  } finally {
    elements.installSubmit.disabled = false;
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
loadSkills({ preserveSelection: false });
