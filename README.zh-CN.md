# Agent Skill Studio

一个在 macOS 本机理解、编辑、审查、整理和迁移 Codex 技能的桌面工作台。

[English](README.md)

![Agent Skill Studio 技能库](docs/images/skill-library.png)

一个技能往往不只有一份 Markdown，还可能包含脚本、参考资料和资源文件。仅靠
文件夹很难看清它来自哪里、哪些内容发生了变化，以及安装后会影响什么。
Agent Skill Studio 把这些信息放进一个桌面应用：先查看证据，再决定是否编辑、
安装、移动、归档或删除。

它面向不想手动管理技能目录的用户。目前以 Codex 为首个适配目标，仅运行在
macOS 本机。

## 你可以用它做什么

### 看清本机技能

在一个目录中浏览个人、停用、归档、系统和插件管理的技能。来源仓库和分组能
帮助整理列表，但不会移动原始目录。系统和插件管理的技能始终只读。

### 编辑完整技能包

在同一编辑器中处理 `SKILL.md`、`references/`、`scripts/`、`assets/` 和其他
文件。保存前会检查路径和包结构、展示准确差异，并确认技能没有在其他地方被
修改。

![完整技能包编辑器](docs/images/package-editor.png)

### 安装前先审查陌生技能

粘贴公开 GitHub 仓库地址，或者选择本机目录。对于包含多个常规技能的仓库，
应用会固定到一个明确提交，列出其中的技能，并让你逐个审查。获取、审查和安装
确认是三个独立动作，审查过程中不会运行候选技能里的脚本。

### 与来源仓库比较

如果技能记录了 GitHub 来源，应用会比较完整的本机技能包和指定的远端版本。
新增、删除和修改会区分来自本机还是远端。同步文件必须由用户明确选择，不会
静默覆盖不同的技能包。

### 将可信技能迁移到服务器

把选中的个人技能导出为带版本和哈希校验的技能迁移包。由本机导出的可信迁移包
可以传到 Linux 服务器，直接交给 Codex CLI。完全相同的技能会跳过，不同的同名
技能需要确认。服务器不需要安装 Agent Skill Studio、Node.js 或 Rust。详见
[服务器迁移说明](docs/server-migration.md)。

## 审查与隐私

基础审查只在本机离线执行。它会展示触发条件、破坏性命令、网络访问、敏感数据、
执行、持久化、依赖和编码内容等有限证据。没有发现问题不等于安全证书，也不保证
技能没有风险。

深度审查是可选功能。开始前，应用会显示 API 模式、请求地址、模型以及将离开
本机的确切文件。它支持 OpenAI 兼容的 Chat Completions 和 Responses 接口，
不会静默切换协议。连接测试只发送固定的合成提示，不会读取技能文件。

API 密钥与提供商配置分开保存在应用私有的本地文件中。Unix 系统上的目录权限为
`0700`，文件权限为 `0600`。这能阻止其他普通本机账户读取，但不能防御已经以同一
macOS 用户运行的恶意软件，因此不等同于 Keychain。应用不会把密钥写入技能、
迁移包、项目文件、日志或审查结果。完整说明见 [PRIVACY.md](PRIVACY.md) 和
[SECURITY.md](SECURITY.md)。

## 当前发布状态

公开安装包暂缓发布。仓库已经具备可重复的持续集成、通用 macOS 打包和本地未签名
DMG 流程。Developer ID 签名、Apple 公证和干净机器验收完成前，不会发布公开
DMG。

从源码运行或构建本地未签名版本：

```bash
npm ci
npm run desktop:dev
npm run desktop:build
```

打包后的应用不会启动 Node HTTP 服务，用户也不需要安装 Node.js 或 Rust。开发
环境使用仓库固定的 Node `22.23.1` 和 Rust `1.88.0`。

## 产品边界

Agent Skill Studio 不替代 CC Switch 的跨 Agent 分发、MCP Inspector 的协议调试，
也不替代持续维护的安全扫描器。Codex 是第一个 Agent 适配器。增加其他适配器或
能力类型前，需要单独定义兼容性契约并完成验证。

产品定位、领域术语、关键决策和路线图见 [INIT.md](INIT.md)、
[CONTEXT.md](CONTEXT.md)、[AGENTS.md](AGENTS.md) 和 [PLAN.md](PLAN.md)。

## 许可证

MIT，详见 [LICENSE](LICENSE)。
