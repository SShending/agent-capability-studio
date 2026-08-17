# Agent Skill Studio

Agent Skill Studio 是一个 macOS 桌面工作台，面向不想手动管理 Skill
文件的用户，用于理解、编辑、审查、比较和迁移 Codex Skill。

[English README](README.md)

## 功能

- 发现个人、停用、归档、系统和插件管理的 Codex Skill。
- 编辑完整 Skill 包，包括 `SKILL.md`、`references/`、`scripts/`、
  `assets/` 和其他包文件。
- 在保存、安装或生命周期操作前，以易懂的方式展示触发条件、破坏性命令、
  网络访问、敏感数据、执行、持久化、依赖和编码内容等证据。
- 使用 Collections、来源信息、包校验和 GitHub 比较整理 Skill。
- 导入本地 Skill 或公开 GitHub 仓库；对于包含多个 Skill 的仓库，列出候选
  并允许分别选择。
- 将选定的个人 Skill 导出为经过校验的 Bundle，用于可信的 Mac 到 Linux
  迁移。内容完全相同的 Skill 会跳过，不同的同名 Skill 必须确认后才替换。
- 通过明确确认停用、恢复、归档和永久删除个人 Skill。系统和插件管理的
  Skill 始终只读。

## 审查与隐私

基础审查只在本机离线执行。它是有边界的证据检查，不是安全证书，也不保证
Skill 没有风险。

深度审查是可选功能。开始前，应用会显示 API 模式、实际请求地址、模型以及
将离开本机的确切文件。支持 OpenAI 兼容的 Chat Completions 和 Responses
模式，不会静默切换协议。确认的文件会发送两次：一次进行威胁审查，一次由
独立步骤复核误报。提供商不会获得工具权限，应用只接受能够在提交文件中核对
的证据。

提供商、地址和模型配置与 API 密钥分开保存。API 密钥保存在应用私有的本地
文件中；Unix 系统会使用受限权限（目录 `0700`、文件 `0600`），并进行原子写入、
符号链接和权限检查。打开设置只检查是否存在有效密钥，不读取密钥，也不要求
输入密码。这种方式不能防御已经以同一 macOS 账户运行的恶意软件，因此不应
理解为等同于 Keychain。密钥不会写入 Skill、Bundle、项目文件、日志或审查证据。

## 安装与运行

正式签名版本发布后，可从 GitHub Releases 下载 DMG，打开后将
**Agent Skill Studio** 拖入“应用程序”。正式发布前必须通过 Developer ID
签名、公证、票据固定和干净机器安装检查。

开发或本地未签名构建需要：

```bash
npm ci
npm run desktop:dev
npm run desktop:build
```

用户安装的应用不会启动 Node HTTP 服务，也不要求用户安装 Node.js 或 Rust。
Node.js 和 Rust 只用于开发；仓库固定 Node `22.23.1` 和 Rust `1.88.0` 以便
可重复构建。

## 可信迁移

对于由本机所有者导出的 Skill Bundle：

1. 在 Studio 中导出要迁移的 Skill。
2. 使用 SFTP 或服务器提供商的文件传输界面等可信方式传输 `.skillbundle`。
3. 在包含 Bundle 的目录中打开 Codex，粘贴导出回执里的安装指令。
4. 只确认确实要替换服务器版本的不同同名 Skill。

这种自迁移不重复进行语义审查，但仍会校验 Bundle、防止路径穿越、跳过重复的
相同 Skill，并且不会静默覆盖不同内容。详见
[`docs/server-migration.md`](docs/server-migration.md)。

## 产品边界

Studio 当前以 Codex 和 Skill 为核心。它不替代 CC Switch 的跨 Agent 分发与
同步、MCP Inspector 的协议调试，也不替代维护中的 Skill/MCP 安全扫描器。
新增 Agent 适配器或能力类型前，需要单独的兼容性契约和验证。

## 参与和安全问题

项目约束和路线图见 [`AGENTS.md`](AGENTS.md)、[`INIT.md`](INIT.md)、
[`CONTEXT.md`](CONTEXT.md) 和 [`PLAN.md`](PLAN.md)。安全问题请按照
[`SECURITY.md`](SECURITY.md) 私下报告，不要在公开 Issue 中发布漏洞细节。

## 许可证

MIT，详见 [`LICENSE`](LICENSE)。
