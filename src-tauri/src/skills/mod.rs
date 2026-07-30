use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    env,
    fs::{self},
    io::Write,
    path::{Path, PathBuf},
    time::SystemTime,
};
use tempfile::NamedTempFile;
use thiserror::Error;

const MAX_DRAFT_BYTES: usize = 64 * 1024;
const MAX_SCAN_DEPTH: usize = 8;

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("This Skill was not found. Refresh and try again.")]
    NotFound,
    #[error("Only personal Skills can be edited in this phase.")]
    ReadOnly,
    #[error("Draft markdown is required and must be at most 64 KiB.")]
    InvalidDraft,
    #[error("This Skill changed after the draft was opened. Refresh before saving.")]
    Conflict,
    #[error("Resolve blocking findings before saving.")]
    Blocked,
    #[error("Editing linked or escaped Skill files is not supported.")]
    UnsafePath,
    #[error("Unable to access the local Skill workspace: {0}")]
    Io(#[from] std::io::Error),
}

impl WorkspaceError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "SKILL_NOT_FOUND",
            Self::ReadOnly => "READ_ONLY_SOURCE",
            Self::InvalidDraft => "INVALID_DRAFT",
            Self::Conflict => "STALE_DRAFT",
            Self::Blocked => "BLOCKING_FINDINGS",
            Self::UnsafePath => "UNSAFE_PATH",
            Self::Io(_) => "LOCAL_IO_ERROR",
        }
    }
}

#[derive(Clone)]
pub struct Workspace {
    codex_home: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
enum Source {
    Personal,
    Disabled,
    System,
    Plugin,
    Archive,
}

impl Source {
    fn label(self) -> &'static str {
        match self {
            Self::Personal => "personal",
            Self::Disabled => "disabled",
            Self::System => "system",
            Self::Plugin => "plugin",
            Self::Archive => "archive",
        }
    }
    fn rank(self) -> usize {
        match self {
            Self::Personal => 0,
            Self::Disabled => 1,
            Self::System => 2,
            Self::Plugin => 3,
            Self::Archive => 4,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub codex_home: String,
    pub roots: Roots,
    pub skills: Vec<SkillSummary>,
    pub counts: Counts,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Roots {
    pub personal_root: String,
    pub system_root: String,
    pub plugin_root: String,
    pub disabled_root: String,
    pub archive_root: String,
}

#[derive(Debug, Serialize, Default)]
pub struct Counts {
    pub total: usize,
    pub personal: usize,
    pub disabled: usize,
    pub system: usize,
    pub plugin: usize,
    pub archive: usize,
    #[serde(rename = "needsAttention")]
    pub needs_attention: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSummary {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub summary: String,
    pub source: String,
    pub state: String,
    pub path: String,
    pub directory_name: String,
    pub modified_at: String,
    pub file_count: usize,
    pub trigger_compliant: bool,
    pub trigger_mode: String,
    pub has_blocking_findings: bool,
    pub has_icon: bool,
    pub brand_color: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillDetail {
    #[serde(flatten)]
    pub summary: SkillSummary,
    pub markdown: String,
    pub document: SkillDocument,
    pub editable: bool,
    pub content_hash: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillDocument {
    pub has_frontmatter: bool,
    pub name: String,
    pub description: String,
    pub body: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub id: String,
    pub severity: String,
    pub title: String,
    pub explanation: String,
    pub evidence: String,
    pub confidence: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diff {
    pub changed: bool,
    pub start_line: usize,
    pub added_count: usize,
    pub removed_count: usize,
    pub before: Vec<String>,
    pub after: Vec<String>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditResult {
    pub verdict: String,
    pub findings: Vec<Finding>,
    pub content_hash: String,
    pub document: SkillDocument,
    pub diff: Diff,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    pub ok: bool,
    pub audit: AuditResult,
    pub content_hash: String,
    pub restart_recommended: bool,
}

#[derive(Debug, Deserialize, Default)]
struct Frontmatter {
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
}

#[derive(Debug, Deserialize, Default)]
struct AgentFile {
    #[serde(default)]
    interface: AgentInterface,
}

#[derive(Debug, Deserialize, Default)]
struct AgentInterface {
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    short_description: String,
    #[serde(default)]
    brand_color: String,
}

#[derive(Clone)]
struct InternalSkill {
    summary: SkillSummary,
    source: Source,
    root: PathBuf,
    directory: PathBuf,
    skill_file: PathBuf,
    markdown: String,
    document: SkillDocument,
    modified: SystemTime,
}

impl Workspace {
    pub fn from_environment() -> Self {
        let codex_home = env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex")))
            .unwrap_or_else(|| PathBuf::from(".codex"));
        Self { codex_home }
    }

    pub fn list_skills(&self) -> Result<Catalog, WorkspaceError> {
        let roots = self.roots();
        let mut skills = Vec::new();
        skills.extend(self.scan_immediate(&roots.personal, Source::Personal, false)?);
        skills.extend(self.scan_immediate(&roots.disabled, Source::Disabled, false)?);
        skills.extend(self.scan_immediate(&roots.system, Source::System, true)?);
        skills.extend(self.scan_recursive(&roots.plugin, Source::Plugin, 0)?);
        skills.extend(self.scan_immediate(&roots.archive, Source::Archive, false)?);

        let mut newest_plugins: HashMap<String, InternalSkill> = HashMap::new();
        let mut non_plugins = Vec::new();
        for skill in skills {
            if skill.source == Source::Plugin {
                let replace = newest_plugins
                    .get(&skill.summary.name)
                    .map(|old| old.modified < skill.modified)
                    .unwrap_or(true);
                if replace { newest_plugins.insert(skill.summary.name.clone(), skill); }
            } else { non_plugins.push(skill); }
        }
        non_plugins.extend(newest_plugins.into_values());
        non_plugins.sort_by(|a, b| {
            a.source.rank().cmp(&b.source.rank())
                .then_with(|| a.summary.display_name.cmp(&b.summary.display_name))
        });

        let mut counts = Counts::default();
        for skill in &non_plugins {
            counts.total += 1;
            match skill.source {
                Source::Personal => { counts.personal += 1; if skill.summary.has_blocking_findings { counts.needs_attention += 1; } }
                Source::Disabled => counts.disabled += 1,
                Source::System => counts.system += 1,
                Source::Plugin => counts.plugin += 1,
                Source::Archive => counts.archive += 1,
            }
        }
        Ok(Catalog {
            codex_home: self.codex_home.display().to_string(),
            roots: Roots {
                personal_root: roots.personal.display().to_string(), system_root: roots.system.display().to_string(),
                plugin_root: roots.plugin.display().to_string(), disabled_root: roots.disabled.display().to_string(),
                archive_root: roots.archive.display().to_string(),
            },
            skills: non_plugins.into_iter().map(|skill| skill.summary).collect(),
            counts,
        })
    }

    pub fn get_skill(&self, id: &str) -> Result<SkillDetail, WorkspaceError> {
        let skill = self.find_skill(id)?;
        Ok(SkillDetail {
            content_hash: hash(&skill.markdown), summary: skill.summary,
            markdown: skill.markdown, document: skill.document,
            editable: skill.source == Source::Personal,
        })
    }

    pub fn audit_draft(&self, id: &str, markdown: &str) -> Result<AuditResult, WorkspaceError> {
        self.validate_draft(markdown)?;
        let skill = self.editable_skill(id)?;
        Ok(audit(markdown, &skill.markdown, &skill.summary.name))
    }

    pub fn save_draft(&self, id: &str, markdown: &str, expected_hash: &str) -> Result<SaveResult, WorkspaceError> {
        self.validate_draft(markdown)?;
        let skill = self.editable_skill(id)?;
        if expected_hash.is_empty() || expected_hash != hash(&skill.markdown) { return Err(WorkspaceError::Conflict); }
        let audit = audit(markdown, &skill.markdown, &skill.summary.name);
        if audit.verdict == "block" { return Err(WorkspaceError::Blocked); }
        self.atomic_save(&skill, markdown)?;
        Ok(SaveResult { ok: true, content_hash: hash(markdown), audit, restart_recommended: true })
    }

    fn validate_draft(&self, markdown: &str) -> Result<(), WorkspaceError> {
        if markdown.is_empty() || markdown.len() > MAX_DRAFT_BYTES { Err(WorkspaceError::InvalidDraft) } else { Ok(()) }
    }

    fn editable_skill(&self, id: &str) -> Result<InternalSkill, WorkspaceError> {
        let skill = self.find_skill(id)?;
        if skill.source != Source::Personal { return Err(WorkspaceError::ReadOnly); }
        Ok(skill)
    }

    fn find_skill(&self, id: &str) -> Result<InternalSkill, WorkspaceError> {
        let roots = self.roots();
        let mut candidates = Vec::new();
        candidates.extend(self.scan_immediate(&roots.personal, Source::Personal, false)?);
        candidates.extend(self.scan_immediate(&roots.disabled, Source::Disabled, false)?);
        candidates.extend(self.scan_immediate(&roots.system, Source::System, true)?);
        candidates.extend(self.scan_recursive(&roots.plugin, Source::Plugin, 0)?);
        candidates.extend(self.scan_immediate(&roots.archive, Source::Archive, false)?);
        candidates.into_iter().find(|skill| skill.summary.id == id).ok_or(WorkspaceError::NotFound)
    }

    fn roots(&self) -> WorkspaceRoots {
        WorkspaceRoots {
            personal: self.codex_home.join("skills"), system: self.codex_home.join("skills/.system"),
            plugin: self.codex_home.join("plugins/cache"), disabled: self.codex_home.join("skills-disabled"),
            archive: self.codex_home.join("skill-archive"),
        }
    }

    fn scan_immediate(&self, root: &Path, source: Source, include_hidden: bool) -> Result<Vec<InternalSkill>, WorkspaceError> {
        let Ok(entries) = fs::read_dir(root) else { return Ok(Vec::new()) };
        let mut result = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name();
            if !include_hidden && name.to_string_lossy().starts_with('.') { continue; }
            if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                if let Some(skill) = self.read_skill(&entry.path(), source, root)? { result.push(skill); }
            }
        }
        Ok(result)
    }

    fn scan_recursive(&self, root: &Path, source: Source, depth: usize) -> Result<Vec<InternalSkill>, WorkspaceError> {
        if depth > MAX_SCAN_DEPTH { return Ok(Vec::new()); }
        let Ok(entries) = fs::read_dir(root) else { return Ok(Vec::new()) };
        let entries: Vec<_> = entries.flatten().collect();
        if entries.iter().any(|entry| entry.file_name() == "SKILL.md" && entry.file_type().map(|kind| kind.is_file()).unwrap_or(false)) {
            return Ok(self.read_skill(root, source, root)?.into_iter().collect());
        }
        let mut result = Vec::new();
        for entry in entries {
            let name = entry.file_name();
            if name == "node_modules" || name == ".git" { continue; }
            if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                result.extend(self.scan_recursive(&entry.path(), source, depth + 1)?);
            }
        }
        Ok(result)
    }

    fn read_skill(&self, directory: &Path, source: Source, root: &Path) -> Result<Option<InternalSkill>, WorkspaceError> {
        let skill_file = directory.join("SKILL.md");
        let metadata = match fs::symlink_metadata(&skill_file) { Ok(value) => value, Err(_) => return Ok(None) };
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() { return Ok(None); }
        let markdown = match fs::read_to_string(&skill_file) { Ok(value) => value, Err(_) => return Ok(None) };
        let document = parse_document(&markdown);
        let agent_file = directory.join("agents/openai.yaml");
        let agent: AgentFile = fs::read_to_string(agent_file).ok()
            .and_then(|yaml| serde_yaml::from_str(&yaml).ok()).unwrap_or_default();
        let name = if document.name.is_empty() { directory.file_name().unwrap_or_default().to_string_lossy().to_string() } else { document.name.clone() };
        let description = if document.description.is_empty() { agent.interface.short_description.clone() } else { document.description.clone() };
        let explicit = explicit_trigger(&name, &description);
        let baseline = audit(&markdown, &markdown, &name);
        let id = URL_SAFE_NO_PAD.encode(format!("{}\0{}", source.label(), directory.display()));
        let brand_color = agent.interface.brand_color;
        let summary = SkillSummary {
            id, name: name.clone(), display_name: if agent.interface.display_name.is_empty() { name.clone() } else { agent.interface.display_name },
            summary: capability_summary(&name, &description), description, source: source.label().to_string(),
            state: match source { Source::Disabled => "disabled", Source::Archive => "archived", _ => "active" }.to_string(),
            path: directory.display().to_string(), directory_name: directory.file_name().unwrap_or_default().to_string_lossy().to_string(),
            modified_at: DateTime::<Utc>::from(metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH)).to_rfc3339(),
            file_count: count_files(directory, 0), trigger_compliant: explicit,
            trigger_mode: if explicit { "explicit" } else { "contextual" }.to_string(),
            has_blocking_findings: baseline.verdict == "block", has_icon: false,
            brand_color: if valid_color(&brand_color) { Some(brand_color) } else { None },
        };
        Ok(Some(InternalSkill { summary, source, root: root.to_path_buf(), directory: directory.to_path_buf(), skill_file, markdown, document, modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH) }))
    }

    fn atomic_save(&self, skill: &InternalSkill, markdown: &str) -> Result<(), WorkspaceError> {
        let metadata = fs::symlink_metadata(&skill.skill_file)?;
        if metadata.file_type().is_symlink() { return Err(WorkspaceError::UnsafePath); }
        let root = fs::canonicalize(&skill.root)?;
        let directory = fs::canonicalize(&skill.directory)?;
        if !directory.starts_with(&root) { return Err(WorkspaceError::UnsafePath); }
        let mut temporary = NamedTempFile::new_in(&directory)?;
        temporary.as_file_mut().set_permissions(metadata.permissions())?;
        temporary.write_all(markdown.as_bytes())?;
        temporary.as_file_mut().sync_all()?;
        temporary.persist(&skill.skill_file).map_err(|error| WorkspaceError::Io(error.error))?;
        Ok(())
    }
}

struct WorkspaceRoots { personal: PathBuf, system: PathBuf, plugin: PathBuf, disabled: PathBuf, archive: PathBuf }

fn parse_document(markdown: &str) -> SkillDocument {
    let normalized = markdown.replace("\r\n", "\n");
    if !normalized.starts_with("---\n") { return SkillDocument { has_frontmatter: false, name: String::new(), description: String::new(), body: markdown.to_string() }; }
    let Some(end) = normalized[4..].find("\n---") else { return SkillDocument { has_frontmatter: false, name: String::new(), description: String::new(), body: markdown.to_string() }; };
    let closing = end + 4;
    let yaml = &normalized[4..closing];
    let parsed: Frontmatter = serde_yaml::from_str(yaml).unwrap_or_default();
    let body = normalized[closing + 4..].trim_start().to_string();
    SkillDocument { has_frontmatter: true, name: parsed.name, description: parsed.description, body }
}

fn required_prefix(name: &str) -> String { format!("Use only when the user's request explicitly contains the full skill name `{name}` or `${name}`; never trigger from task intent, synonyms, former trigger phrases, or conversational context.") }
fn explicit_trigger(name: &str, description: &str) -> bool { !name.is_empty() && description.trim_start().starts_with(&required_prefix(name)) }
fn capability_summary(name: &str, description: &str) -> String { description.trim().strip_prefix(&required_prefix(name)).unwrap_or(description.trim()).trim().to_string() }
fn valid_name(name: &str) -> bool { !name.is_empty() && name.chars().enumerate().all(|(index, character)| character.is_ascii_lowercase() || character.is_ascii_digit() || (character == '-' && index > 0)) && !name.ends_with('-') && !name.contains("--") }
fn valid_color(color: &str) -> bool { color.len() == 7 && color.starts_with('#') && color[1..].chars().all(|character| character.is_ascii_hexdigit()) }
fn hash(value: &str) -> String { format!("{:x}", Sha256::digest(value.as_bytes())) }
fn count_files(root: &Path, depth: usize) -> usize {
    if depth > 5 { return 0; }
    fs::read_dir(root).ok().into_iter().flatten().flatten().map(|entry| {
        let name = entry.file_name();
        if name == "node_modules" || name == ".git" { return 0; }
        let kind = entry.file_type().ok();
        if kind.as_ref().is_some_and(|kind| kind.is_file()) { 1 } else if kind.as_ref().is_some_and(|kind| kind.is_dir()) { count_files(&entry.path(), depth + 1) } else { 0 }
    }).sum()
}

fn finding(id: &str, severity: &str, title: &str, explanation: &str, evidence: String, confidence: &str) -> Finding {
    Finding { id: id.into(), severity: severity.into(), title: title.into(), explanation: explanation.into(), evidence, confidence: confidence.into() }
}

fn audit(markdown: &str, original: &str, expected_name: &str) -> AuditResult {
    let document = parse_document(markdown);
    let mut findings = Vec::new();
    if !document.has_frontmatter { findings.push(finding("missing-frontmatter", "blocker", "缺少 Skill 基本信息", "文件开头需要包含名称和用途，Agent 才能识别这个 Skill。", "未找到以 --- 包围的 frontmatter。".into(), "high")); }
    if document.name.is_empty() { findings.push(finding("missing-name", "blocker", "缺少 Skill 名称", "名称用于识别 Skill，不能为空。", "name 字段为空。".into(), "high")); }
    else if !valid_name(&document.name) { findings.push(finding("invalid-name", "blocker", "Skill 名称格式不兼容", "名称只能包含小写字母、数字和单个连字符。", format!("当前名称：{}", document.name), "high")); }
    if !expected_name.is_empty() && !document.name.is_empty() && document.name != expected_name { findings.push(finding("identity-change", "blocker", "名称与当前 Skill 不一致", "直接修改名称会让文件夹身份与 Skill 身份分离，请通过复制创建新 Skill。", format!("当前为 {expected_name}，草稿为 {}。", document.name), "high")); }
    if document.description.trim().is_empty() { findings.push(finding("missing-description", "blocker", "缺少使用说明", "Agent 需要用途和触发条件来判断何时使用这个 Skill。", "description 字段为空。".into(), "high")); }
    else if !explicit_trigger(&document.name, &document.description) { findings.push(finding("contextual-trigger", "info", "采用按意图触发", "Agent 可以在任务意图与用途匹配时加载这个 Skill；这与明确点名触发是两种合法策略。", "description 描述了能力和适用场景，没有使用仅点名触发前缀。".into(), "high")); }
    if document.body.trim().chars().count() < 40 { findings.push(finding("thin-instructions", "warning", "工作步骤过少", "说明太短时，Agent 可能无法稳定完成任务或处理边界情况。", format!("正文仅有 {} 个字符。", document.body.trim().chars().count()), "medium")); }
    let lower = markdown.to_ascii_lowercase();
    let dangerous = lower.contains("sudo") || lower.contains("chmod 777") || lower.lines().any(|line| (line.contains("curl") || line.contains("wget")) && (line.contains("| sh") || line.contains("| bash") || line.contains("| zsh"))) || lower.lines().any(|line| line.contains("rm -") && line.contains('r'));
    if dangerous { findings.push(finding("dangerous-command", "blocker", "发现高影响命令", "这些命令可能修改系统、删除文件或执行未经检查的远程内容。", "检测到 sudo、递归删除、下载后执行或开放式权限命令。".into(), "high")); }
    if lower.contains("curl") || lower.contains("wget") || lower.contains("http://") || lower.contains("https://") { findings.push(finding("network-access", "warning", "包含网络访问", "运行时可能把请求或数据发送到外部地址，请确认目的地和传输内容。", "发现网址或网络下载命令。".into(), "medium")); }
    if ["api_key", "api key", "access_token", "access token", "secret", "credential", ".ssh", ".aws"].iter().any(|token| lower.contains(token)) { findings.push(finding("sensitive-data", "warning", "可能接触凭据或敏感配置", "请确认 Skill 不会记录、上传或在输出中泄露这些信息。", "发现密钥、令牌、凭据或常见凭据目录相关文字。".into(), "medium")); }
    if findings.is_empty() { findings.push(finding("baseline-clear", "info", "基础检查未发现阻断项", "这表示当前规则没有命中问题，不代表 Skill 绝对安全。", "结构、触发策略和高影响命令检查均未命中。".into(), "medium")); }
    let verdict = if findings.iter().any(|item| item.severity == "blocker") { "block" } else if findings.iter().any(|item| item.severity == "warning") { "review" } else { "clear" };
    AuditResult { verdict: verdict.into(), findings, content_hash: hash(markdown), document, diff: diff(original, markdown) }
}

fn diff(before: &str, after: &str) -> Diff {
    let before: Vec<String> = before.lines().map(str::to_string).collect();
    let after: Vec<String> = after.lines().map(str::to_string).collect();
    let mut prefix = 0; while prefix < before.len() && prefix < after.len() && before[prefix] == after[prefix] { prefix += 1; }
    let mut suffix = 0; while suffix < before.len() - prefix && suffix < after.len() - prefix && before[before.len() - 1 - suffix] == after[after.len() - 1 - suffix] { suffix += 1; }
    let removed = before[prefix..before.len() - suffix].to_vec(); let added = after[prefix..after.len() - suffix].to_vec();
    Diff { changed: before != after, start_line: prefix + 1, added_count: added.len(), removed_count: removed.len(), truncated: removed.len() > 120 || added.len() > 120, before: removed.into_iter().take(120).collect(), after: added.into_iter().take(120).collect() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn markdown(name: &str, description: &str, body: &str) -> String {
        format!("---\nname: {name}\ndescription: >-\n  {description}\n---\n\n# {name}\n\n{body}\n")
    }

    fn workspace() -> (tempfile::TempDir, Workspace) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let workspace = Workspace { codex_home: directory.path().to_path_buf() };
        (directory, workspace)
    }

    fn write_skill(root: &Path, name: &str, description: &str, body: &str) {
        let directory = root.join("skills").join(name);
        fs::create_dir_all(&directory).expect("skill directory");
        fs::write(directory.join("SKILL.md"), markdown(name, description, body)).expect("skill document");
    }

    #[test]
    fn parses_folded_frontmatter() {
        let document = parse_document("---\nname: demo\ndescription: >-\n  First line.\n  Second line.\n---\n\n# Demo\n");
        assert!(document.has_frontmatter);
        assert_eq!(document.name, "demo");
        assert_eq!(document.description, "First line. Second line.");
        assert_eq!(document.body, "# Demo\n");
    }

    #[test]
    fn contextual_trigger_is_information_not_a_blocker() {
        let draft = markdown("demo", "Use when the user asks to plan a project.", "1. Read the request.\n2. Return an evidence-based plan.");
        let result = audit(&draft, &draft, "demo");
        assert_eq!(result.verdict, "clear");
        assert!(result.findings.iter().any(|finding| finding.id == "contextual-trigger"));
    }

    #[test]
    fn saves_personal_skill_and_rejects_stale_hash() {
        let (directory, workspace) = workspace();
        let description = "Use when the user asks to plan a project before implementation.";
        write_skill(directory.path(), "demo", description, "1. Read the request.\n2. Return a focused plan with evidence.");
        let skill = workspace.list_skills().expect("catalog").skills.pop().expect("personal skill");
        let detail = workspace.get_skill(&skill.id).expect("detail");
        let revised = detail.markdown.replace("focused plan", "durable project plan");
        let result = workspace.save_draft(&skill.id, &revised, &detail.content_hash).expect("save");
        assert!(result.ok);
        assert_eq!(fs::read_to_string(directory.path().join("skills/demo/SKILL.md")).expect("saved file"), revised);
        assert!(matches!(workspace.save_draft(&skill.id, &revised, &detail.content_hash), Err(WorkspaceError::Conflict)));
    }

    #[test]
    fn system_skill_is_read_only() {
        let (directory, workspace) = workspace();
        let root = directory.path().join("skills/.system/system-demo");
        fs::create_dir_all(&root).expect("system directory");
        let draft = markdown("system-demo", "Built in Skill.", "1. Read.\n2. Return a detailed result for the task.");
        fs::write(root.join("SKILL.md"), &draft).expect("system skill");
        let skill = workspace.list_skills().expect("catalog").skills.into_iter().next().expect("system skill");
        assert_eq!(skill.source, "system");
        assert!(matches!(workspace.audit_draft(&skill.id, &draft), Err(WorkspaceError::ReadOnly)));
    }
}
