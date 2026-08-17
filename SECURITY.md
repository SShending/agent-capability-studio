# Security Policy

## Scope

Agent Skill Studio is a local macOS application. Its built-in Baseline Audit is
deliberately limited and should not be treated as a complete malware scanner,
security certificate, or guarantee of safety. The application must not execute
untrusted Skill scripts during discovery, import, audit, or update comparison.

## Reporting a vulnerability

Please do not open a public issue for an undisclosed vulnerability. Use a
private GitHub security advisory for this repository when available. If private
advisories are not enabled, contact the repository owner through the GitHub
profile and include:

- the affected version or commit;
- a concise impact description;
- reproduction steps or a minimal fixture that contains no real credentials;
- any relevant logs with API keys, tokens, personal paths, and private Skill
  content removed.

Please allow time to investigate and coordinate a fix before public disclosure.

## Security boundaries

- Candidate acquisition, audit, import staging, saving, and installation are
  separate actions. Audit alone never mutates a Skill.
- System and plugin-managed Skills are read-only.
- Archive import rejects traversal, containment escapes, unsupported symlinks,
  and resource-limit violations.
- Baseline Audit is offline. Deep Audit sends only files explicitly confirmed
  for that scan to the configured provider and shows the endpoint and file list
  first.
- Credentials are kept outside project and Skill directories. The persistent
  local file store uses restrictive permissions and atomic replacement, but it
  is not a defense against malicious software running as the same user.
- Export excludes credentials and other likely secret material. Bundles are
  not encrypted in v0.1.

## Supported versions

Until the first public release, the latest repository revision is the only
supported development version. Release support and response timelines will be
documented with the first signed release.
