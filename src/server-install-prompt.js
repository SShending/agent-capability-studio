export function bundleFileName(path) {
  const normalized = String(path || "").replaceAll("\\", "/");
  const name = normalized.split("/").filter(Boolean).at(-1) || "codex-skills.skillbundle";
  return name.replace(/[\r\n"“”]/g, "");
}

export function serverInstallPrompt(path, locale = "zh-CN") {
  const fileName = bundleFileName(path);
  if (locale === "en") {
    return `Install “${fileName}” from the current directory into this server's personal Codex Skills directory.

This is a trusted Bundle exported from my own Mac with Agent Skill Studio. Do not repeat the semantic security audit.

- Extract it into a temporary directory and do not run its scripts.
- Install the Skill directories under skills/ into the skills/ directory of the current Codex home.
- Skip any existing Skill with identical content.
- For a same-name Skill with different content, list its name and ask me before replacing anything.
- Report the installed, skipped, and unresolved items when complete.`;
  }
  return `请安装当前目录中的“${fileName}”到这台服务器的 Codex 个人 Skills 目录。

这是我从自己 Mac 上的 Agent Skill Studio 导出的可信 Bundle，不需要重新做语义安全审查。

- 解压到临时目录，不要执行其中的脚本。
- 将 skills/ 下的 Skill 安装到当前 Codex home 的 skills/。
- 已存在且内容相同的 Skill 直接跳过。
- 同名但内容不同的 Skill 先列出名称并询问我，不要覆盖。
- 完成后报告已安装、已跳过和等待确认的项目。`;
}
