import { createHash } from "node:crypto";

const SKILL_NAME_PATTERN = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;

function unquote(value) {
  const trimmed = value.trim();
  if (
    (trimmed.startsWith('"') && trimmed.endsWith('"')) ||
    (trimmed.startsWith("'") && trimmed.endsWith("'"))
  ) {
    return trimmed.slice(1, -1).replace(/\\"/g, '"').replace(/\\'/g, "'");
  }
  return trimmed;
}

export function parseFrontmatter(markdown) {
  const match = markdown.match(/^---\s*\r?\n([\s\S]*?)\r?\n---(?:\s*\r?\n|\s*$)/);
  if (!match) return {};

  const result = {};
  const lines = match[1].split(/\r?\n/);
  for (let index = 0; index < lines.length; index += 1) {
    const keyMatch = lines[index].match(/^([A-Za-z0-9_-]+):(?:\s*(.*))?$/);
    if (!keyMatch) continue;
    const [, key, rawValue = ""] = keyMatch;

    if (/^[>|][+-]?$/.test(rawValue.trim())) {
      const values = [];
      while (index + 1 < lines.length && /^\s+/.test(lines[index + 1])) {
        index += 1;
        values.push(lines[index].trim());
      }
      result[key] = rawValue.startsWith(">") ? values.join(" ") : values.join("\n");
      continue;
    }

    result[key] = unquote(rawValue);
  }
  return result;
}

export function parseSkillDocument(markdown) {
  const match = markdown.match(/^---\s*\r?\n([\s\S]*?)\r?\n---(?:\s*\r?\n|\s*$)/);
  const frontmatter = parseFrontmatter(markdown);
  return {
    hasFrontmatter: Boolean(match),
    name: String(frontmatter.name || ""),
    description: String(frontmatter.description || ""),
    body: match ? markdown.slice(match[0].length).replace(/^\s+/, "") : markdown
  };
}

export function requiredTriggerPrefix(name) {
  return `Use only when the user's request explicitly contains the full skill name \`${name}\` or \`$${name}\`; never trigger from task intent, synonyms, former trigger phrases, or conversational context.`;
}

export function isExplicitTriggerCompliant(name, description = "") {
  return Boolean(name) && description.trim().startsWith(requiredTriggerPrefix(name));
}

export function hashSkillContent(markdown) {
  return createHash("sha256").update(markdown, "utf8").digest("hex");
}

function finding(id, severity, title, explanation, evidence, confidence = "high") {
  return { id, severity, title, explanation, evidence, confidence };
}

function summarizeDiff(originalMarkdown, draftMarkdown) {
  const before = originalMarkdown.split(/\r?\n/);
  const after = draftMarkdown.split(/\r?\n/);
  let prefix = 0;
  while (prefix < before.length && prefix < after.length && before[prefix] === after[prefix]) prefix += 1;

  let suffix = 0;
  while (
    suffix < before.length - prefix &&
    suffix < after.length - prefix &&
    before[before.length - 1 - suffix] === after[after.length - 1 - suffix]
  ) {
    suffix += 1;
  }

  const removed = before.slice(prefix, before.length - suffix || before.length);
  const added = after.slice(prefix, after.length - suffix || after.length);
  return {
    changed: originalMarkdown !== draftMarkdown,
    startLine: prefix + 1,
    addedCount: added.length,
    removedCount: removed.length,
    before: removed.slice(0, 120),
    after: added.slice(0, 120),
    truncated: removed.length > 120 || added.length > 120
  };
}

export function auditSkillDraft({ markdown, originalMarkdown = "", expectedName = "" }) {
  const document = parseSkillDocument(markdown);
  const findings = [];

  if (!document.hasFrontmatter) {
    findings.push(
      finding(
        "missing-frontmatter",
        "blocker",
        "缺少 Skill 基本信息",
        "文件开头需要包含名称和用途，Agent 才能识别这个 Skill。",
        "未找到以 --- 包围的 frontmatter。"
      )
    );
  }

  if (!document.name) {
    findings.push(
      finding("missing-name", "blocker", "缺少 Skill 名称", "名称用于识别 Skill，不能为空。", "name 字段为空。")
    );
  } else if (!SKILL_NAME_PATTERN.test(document.name)) {
    findings.push(
      finding(
        "invalid-name",
        "blocker",
        "Skill 名称格式不兼容",
        "名称只能包含小写字母、数字和单个连字符。",
        `当前名称：${document.name}`
      )
    );
  }

  if (expectedName && document.name && document.name !== expectedName) {
    findings.push(
      finding(
        "identity-change",
        "blocker",
        "名称与当前 Skill 不一致",
        "直接修改名称会让文件夹身份与 Skill 身份分离，请通过复制创建新 Skill。",
        `当前为 ${expectedName}，草稿为 ${document.name}。`
      )
    );
  }

  if (!document.description.trim()) {
    findings.push(
      finding(
        "missing-description",
        "blocker",
        "缺少使用说明",
        "Agent 需要用途和触发条件来判断何时使用这个 Skill。",
        "description 字段为空。"
      )
    );
  } else if (!isExplicitTriggerCompliant(document.name, document.description)) {
    findings.push(
      finding(
        "contextual-trigger",
        "info",
        "采用按意图触发",
        "Agent 可以在任务意图与用途匹配时加载这个 Skill；这与明确点名触发是两种合法策略。",
        "description 描述了能力和适用场景，没有使用仅点名触发前缀。",
        "high"
      )
    );
  }

  if (document.body.trim().length < 40) {
    findings.push(
      finding(
        "thin-instructions",
        "warning",
        "工作步骤过少",
        "说明太短时，Agent 可能无法稳定完成任务或处理边界情况。",
        `正文仅有 ${document.body.trim().length} 个字符。`,
        "medium"
      )
    );
  }

  const commandSignals = [
    { pattern: /\bsudo\b/i, evidence: "包含 sudo 命令。" },
    { pattern: /\brm\s+-[^\n]*r[^\n]*/i, evidence: "包含递归删除命令。" },
    { pattern: /(?:curl|wget)[^\n|]*\|\s*(?:sh|bash|zsh)\b/i, evidence: "包含下载后直接执行的命令。" },
    { pattern: /\bchmod\s+777\b/i, evidence: "包含开放式文件权限命令。" }
  ];
  const commandEvidence = commandSignals.filter(({ pattern }) => pattern.test(markdown)).map(({ evidence }) => evidence);
  if (commandEvidence.length) {
    findings.push(
      finding(
        "dangerous-command",
        "blocker",
        "发现高影响命令",
        "这些命令可能修改系统、删除文件或执行未经检查的远程内容。",
        commandEvidence.join(" ")
      )
    );
  }

  if (/\b(?:curl|wget)\b|https?:\/\//i.test(markdown)) {
    findings.push(
      finding(
        "network-access",
        "warning",
        "包含网络访问",
        "运行时可能把请求或数据发送到外部地址，请确认目的地和传输内容。",
        "发现网址或网络下载命令。",
        "medium"
      )
    );
  }

  if (/\b(?:api[_ -]?key|access[_ -]?token|secret|credential|\.ssh|\.aws)\b/i.test(markdown)) {
    findings.push(
      finding(
        "sensitive-data",
        "warning",
        "可能接触凭据或敏感配置",
        "请确认 Skill 不会记录、上传或在输出中泄露这些信息。",
        "发现密钥、令牌、凭据或常见凭据目录相关文字。",
        "medium"
      )
    );
  }

  if (!findings.length) {
    findings.push(
      finding(
        "baseline-clear",
        "info",
        "基础检查未发现阻断项",
        "这表示当前规则没有命中问题，不代表 Skill 绝对安全。",
        "结构、触发策略和高影响命令检查均未命中。",
        "medium"
      )
    );
  }

  const verdict = findings.some((item) => item.severity === "blocker")
    ? "block"
    : findings.some((item) => item.severity === "warning")
      ? "review"
      : "clear";

  return {
    verdict,
    findings,
    contentHash: hashSkillContent(markdown),
    document,
    diff: summarizeDiff(originalMarkdown, markdown)
  };
}
