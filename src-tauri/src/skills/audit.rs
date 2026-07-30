use super::Finding;

const NEGATIONS: &[&str] = &[
    "do not ",
    "don't ",
    "never ",
    "avoid ",
    "must not ",
    "禁止",
    "不要",
    "切勿",
    "不得",
    "请勿",
];

const NETWORK: &[&str] = &[
    "curl ",
    "wget ",
    "http://",
    "https://",
    "requests.",
    "fetch(",
    "axios.",
    "netcat ",
    "nc ",
    "scp ",
];

const SENSITIVE: &[&str] = &[
    "api_key",
    "api key",
    "access_token",
    "access token",
    "secret",
    "credential",
    ".ssh",
    ".aws",
    ".env",
    "keychain",
];

const EXECUTION: &[&str] = &[
    "sudo ",
    "chmod 777",
    "bash -c",
    "sh -c",
    "zsh -c",
    "python -c",
    "python3 -c",
    "node -e",
    "ruby -e",
    "perl -e",
    "eval(",
    "exec(",
    "os.system",
    "subprocess.",
    "child_process",
    "execsync",
    "spawn(",
    "osascript",
    "powershell",
];

const PERSISTENCE: &[&str] = &[
    "launchctl",
    "launchagents",
    "launchdaemons",
    "crontab",
    "/etc/cron",
    ".zshrc",
    ".bashrc",
    "login item",
];

const DEPENDENCY_INSTALL: &[&str] = &[
    "pip install",
    "uv add",
    "npm install",
    "pnpm add",
    "yarn add",
    "cargo install",
    "brew install",
    "apt install",
];

const PROMPT_OVERRIDE: &[&str] = &[
    "ignore previous instructions",
    "ignore all previous",
    "reveal the system prompt",
    "do not tell the user",
    "hide this instruction",
];

const ENCODED_PAYLOAD: &[&str] = &[
    "base64 -d",
    "base64 --decode",
    "from_base64",
    "b64decode",
    "atob(",
    "powershell -enc",
];

pub(super) fn safety_findings(markdown: &str) -> Vec<Finding> {
    let active_lines: Vec<_> = markdown
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let lower = line.to_ascii_lowercase();
            (!is_negated(&lower)).then_some((index + 1, line, lower))
        })
        .collect();
    let mut findings = Vec::new();

    if let Some(evidence) = first_match(&active_lines, is_remote_execution) {
        findings.push(finding(
            "remote-code-execution",
            "blocker",
            "下载内容后直接执行",
            "远程内容未经检查就交给命令解释器执行，可能直接控制本机。",
            evidence,
            "high",
        ));
    }
    if let Some(evidence) = first_match(&active_lines, is_destructive_filesystem) {
        findings.push(finding(
            "destructive-filesystem",
            "blocker",
            "发现破坏性文件操作",
            "该指令可能递归删除数据、抹除磁盘或直接写入设备。",
            evidence,
            "high",
        ));
    }
    if let Some(evidence) = first_match(&active_lines, |line| {
        contains_any(line, SENSITIVE) && is_outbound_transfer(line)
    }) {
        findings.push(finding(
            "credential-exfiltration",
            "blocker",
            "敏感数据可能被传出本机",
            "同一条指令同时读取凭据位置并执行外部传输。",
            evidence,
            "high",
        ));
    }

    push_review(
        &mut findings,
        &active_lines,
        "command-execution",
        "包含命令或解释器执行",
        "Skill 可能启动高权限命令、Shell 或语言运行时，请核对参数和作用范围。",
        EXECUTION,
    );
    push_review(
        &mut findings,
        &active_lines,
        "persistence-change",
        "可能修改持久化启动配置",
        "Skill 可能在登录、启动或定时任务中持续运行。",
        PERSISTENCE,
    );
    push_review(
        &mut findings,
        &active_lines,
        "dependency-installation",
        "包含依赖或软件安装",
        "安装会引入新的可执行代码和供应链依赖，请确认来源与固定版本。",
        DEPENDENCY_INSTALL,
    );
    push_review(
        &mut findings,
        &active_lines,
        "network-access",
        "包含网络访问",
        "Skill 可能与外部地址交换数据，请确认目的地和发送内容。",
        NETWORK,
    );
    push_review(
        &mut findings,
        &active_lines,
        "sensitive-data-access",
        "可能接触凭据或敏感配置",
        "Skill 提及密钥、令牌或常见凭据位置，请确认读取和输出范围。",
        SENSITIVE,
    );
    push_review(
        &mut findings,
        &active_lines,
        "prompt-override",
        "包含覆盖或隐藏指令",
        "这类语句可能改变 Agent 的既有约束或隐藏行为，需要人工理解上下文。",
        PROMPT_OVERRIDE,
    );
    push_review(
        &mut findings,
        &active_lines,
        "encoded-payload",
        "包含编码或混淆载荷",
        "编码内容可能隐藏实际命令或数据，需要先解码并检查再决定是否使用。",
        ENCODED_PAYLOAD,
    );

    let has_download = active_lines
        .iter()
        .any(|(_, _, line)| contains_any(line, NETWORK));
    let has_execution = active_lines
        .iter()
        .any(|(_, _, line)| contains_any(line, EXECUTION));
    let has_direct_remote_execution = active_lines
        .iter()
        .any(|(_, _, line)| is_remote_execution(line));
    if has_download && has_execution && !has_direct_remote_execution {
        findings.push(finding(
            "staged-download-execution",
            "warning",
            "下载与执行可能分阶段发生",
            "文档同时包含网络获取和命令执行，但不在同一行；请追踪下载文件如何被使用。",
            "在不同指令中发现网络访问与命令执行能力。".into(),
            "medium",
        ));
    }

    findings
}

fn push_review(
    findings: &mut Vec<Finding>,
    lines: &[(usize, &str, String)],
    id: &str,
    title: &str,
    explanation: &str,
    patterns: &[&str],
) {
    if let Some(evidence) = first_match(lines, |line| contains_any(line, patterns)) {
        findings.push(finding(
            id,
            "warning",
            title,
            explanation,
            evidence,
            "medium",
        ));
    }
}

fn first_match(
    lines: &[(usize, &str, String)],
    predicate: impl Fn(&str) -> bool,
) -> Option<String> {
    lines
        .iter()
        .find(|(_, _, lower)| predicate(lower))
        .map(|(number, original, _)| format!("第 {number} 行：{}", truncate(original.trim(), 220)))
}

fn is_negated(line: &str) -> bool {
    NEGATIONS.iter().any(|marker| line.contains(marker))
}

fn is_remote_execution(line: &str) -> bool {
    let downloads = line.contains("curl ") || line.contains("wget ");
    let piped_shell = ["| sh", "|sh", "| bash", "|bash", "| zsh", "|zsh"]
        .iter()
        .any(|marker| line.contains(marker));
    let decoded_shell =
        (line.contains("base64 -d") || line.contains("base64 --decode")) && piped_shell;
    (downloads && piped_shell) || decoded_shell
}

fn is_destructive_filesystem(line: &str) -> bool {
    ["rm -rf", "rm -fr", "mkfs", "diskutil erase", "shred "]
        .iter()
        .any(|marker| line.contains(marker))
        || (line.contains("find ") && line.contains(" -delete"))
        || (line.contains("dd ") && line.contains("of=/dev/"))
}

fn is_outbound_transfer(line: &str) -> bool {
    [
        "curl ",
        "wget ",
        "requests.post",
        "fetch(",
        "axios.post",
        "netcat ",
        "nc ",
        "scp ",
        "upload",
    ]
    .iter()
    .any(|marker| line.contains(marker))
}

fn contains_any(value: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|pattern| value.contains(pattern))
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

fn finding(
    id: &str,
    severity: &str,
    title: &str,
    explanation: &str,
    evidence: String,
    confidence: &str,
) -> Finding {
    Finding {
        id: id.into(),
        severity: severity.into(),
        title: title.into(),
        explanation: explanation.into(),
        evidence,
        confidence: confidence.into(),
    }
}
