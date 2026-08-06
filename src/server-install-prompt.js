export function bundleFileName(path) {
  const normalized = String(path || "").replaceAll("\\", "/");
  const name = normalized.split("/").filter(Boolean).at(-1) || "codex-skills.skillbundle";
  return name.replace(/[\r\n"“”]/g, "");
}

export function serverInstallPrompt(path) {
  const fileName = bundleFileName(path);
  return `请安装当前目录中的“${fileName}”到这台服务器的 Codex 个人 Skills 目录。

这是我从自己 Mac 上的 Agent Skill Studio 导出的可信 Bundle，不需要重新做语义安全审查。

- 解压到临时目录，不要执行其中的脚本。
- 将 skills/ 下的 Skill 安装到当前 Codex home 的 skills/。
- 已存在且内容相同的 Skill 直接跳过。
- 同名但内容不同的 Skill 先列出名称并询问我，不要覆盖。
- 完成后报告已安装、已跳过和等待确认的项目。`;
}
