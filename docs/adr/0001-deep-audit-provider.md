# ADR 0001: Deep Audit Provider And Credential Storage

- Status: accepted
- Date: 2026-07-30
- Last updated: 2026-07-31

## Context

The local Baseline Audit is deliberately deterministic and limited. Detecting
subtle intent requires optional semantic analysis, but the Studio must not
silently upload Skill content, depend on one model vendor's response schema, or
reuse an Agent's login credentials.

## Decision

The first Deep Audit interface accepts one user-configured OpenAI-compatible API
Base URL, explicit API mode, model name, and API key. The supported modes are
Chat Completions and Responses.

- Store the API key only in macOS Keychain under the Studio's own service name.
- Store non-secret API mode, endpoint, and model preferences in the app
  configuration directory, never in a project or Skill directory. Existing
  profiles without a mode remain Chat Completions profiles.
- Keep provider transport behind a Rust adapter. Audit consumes a provider-
  neutral structured review rather than a vendor response object.
- Show the selected mode, exact endpoint, model, and eligible file list before
  every run. `SKILL.md` is required; the user may deselect other eligible text
  files.
- Use mode-specific request and response parsers. Never automatically retry or
  fall back through the other mode after failure because doing so could upload
  the confirmed files again through a different endpoint.
- Exclude likely secret files, symlinks, binary files, unsupported files, and
  content above fixed per-file, total, and file-count limits.
- Recompute the candidate-set fingerprint immediately before sending. A changed
  draft or supporting file invalidates consent and requires a new preview.
- Bind consent to a fingerprint of the selected mode, derived endpoint, and
  model. A changed provider profile also requires a new preview.
- Make two requests without tools: a source-grounded threat review followed by
  an independent false-positive review. Only evidence whose file and line range
  can be verified against the submitted content enters the result.
- Never label a successful or empty Deep Audit result as safe or secure.

## Consequences

Compatible providers may differ in how strictly they implement structured JSON;
the Studio must fail clearly on malformed or ungrounded output. Each completed
Deep Audit uses two model requests. Supporting another provider protocol later
requires a mode-specific adapter implementation, not changes to the Audit
evidence interface.
