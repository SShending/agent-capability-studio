# Skill Scanner Options

Research date: 2026-07-30.

## Decision

Keep Agent Skill Studio's built-in audit deterministic, local, side-effect free,
and deliberately limited. Do not recreate a general security scanner. Preserve
the shared evidence model so maintained scanners can be optional adapters in
Phase 3.

Cisco AI Defense Skill Scanner is the stronger first adapter candidate because
it scans Skill directories locally, has static analyzers that require no API
key, and offers JSON and SARIF output. Snyk Agent Scan remains a useful optional
integration, but its README says CLI fields and issue codes are experimental,
requires a Snyk token, and sends Skill content and metadata to Snyk for
analysis. Those privacy and stability properties prevent a hard dependency.

## Current Evidence

### Cisco AI Defense Skill Scanner

- Source: `cisco-ai-defense/skill-scanner`; active on 2026-07-30, about 2,384
  GitHub stars, Apache-2.0 according to its README.
- Supports Codex and Agent Skills specification directories.
- Local engines include YAML/YARA static rules, Shell pipeline taint analysis,
  Python AST dataflow, and bytecode checks.
- Optional engines add LLM analysis, VirusTotal, and Cisco cloud analysis, with
  corresponding API keys or data-sharing implications.
- Exposes JSON, SARIF, Markdown, table, summary, and HTML output plus a Python
  SDK and plugin architecture.
- Explicitly describes results as best-effort detection, not certification.

### Snyk Agent Scan

- Source: `snyk/agent-scan`; active on 2026-07-30, about 2,835 GitHub stars,
  Apache-2.0.
- Discovers Codex and other Agent Skills and reports prompt injection, malware
  payloads, untrusted content, credential handling, and hardcoded secrets.
- Requires a Snyk token. Its README states that Skill contents and Agent
  metadata are shared with Snyk for analysis.
- Its README also states that raw CLI output, field names, issue codes, severity
  labels, and response structure are experimental and may change without notice.
- MCP scanning can execute configured MCP server commands after consent; that
  behavior is outside the Skill-only adapter and must never be invoked by a
  passive Studio audit.

## Product Boundary

The Studio owns immediate authoring feedback, plain-language evidence, exact
draft comparison, and mutation gates. Mature scanners own broad signature,
dataflow, semantic, malware, and supply-chain coverage. An external result must
be labeled with scanner name, version, configuration, exact Skill revision, and
whether content left the machine.

## Sources

- https://github.com/cisco-ai-defense/skill-scanner
- https://cisco-ai-defense.github.io/docs/skill-scanner
- https://github.com/snyk/agent-scan
