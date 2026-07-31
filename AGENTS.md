# Agent Skill Studio Project Instructions

## Product

- Build a local desktop Skill Studio for non-programmers to understand, create,
  edit, audit, compare, and migrate Agent Skills.
- Treat the project owner using Codex on macOS as the first user.
- Use `INIT.md` as the canonical product brief and `CONTEXT.md` as the canonical
  domain glossary.
- Keep v0.1 focused on the Codex Skill workflow. MCP, rules, plugins, hooks,
  automations, runtime traces, other Agents, and other operating systems follow
  only after the Codex desktop workflow is stable.
- Treat Skills as the first capability module, not a permanent product limit.
  Add a new capability type only after current alternatives are re-evaluated and
  it proves reuse of the Studio's ownership, evidence, diff, and mutation model.
- Keep the `Agent Skill Studio` name until a second capability type is implemented
  and validated with the first user.

## Product Validation

When evaluating a product, feature, or expansion, investigate current products
and open-source implementations before recommending or implementing it.

- Compare feature coverage, maturity, maintenance, extensibility, and the first
  user's real workflow.
- Identify a defensible difference before building a standalone implementation.
- Prefer using an existing product, integrating it, or contributing upstream
  when that meets the requirement.
- State uncertainty when evidence is incomplete or stale.
- Do not rebuild CC Switch distribution/synchronization, MCP Inspector protocol
  debugging, or maintained Skill/MCP security scanning engines.

## Architecture

- Deliver a Tauri 2 desktop application, macOS first.
- Keep filesystem discovery, path containment, hashing, atomic writes, bundle
  parsing, and Agent-specific placement in the Rust desktop core.
- Keep guided authoring, presentation, evidence explanation, and comparison in
  the WebView frontend.
- Expose a small typed desktop-command interface between frontend and Rust. The
  frontend must not access arbitrary filesystem paths directly.
- Do not ship a hidden Node HTTP server or require Node.js on the user's machine.
- Implement Codex as the first Agent Adapter. Add another adapter only with a
  concrete compatibility contract and tests.
- Keep one evidence model for built-in checks and optional external scanner
  adapters: finding, evidence, severity, confidence, and verdict.
- Keep cloud semantic analysis behind a provider adapter; do not couple Audit to
  one vendor's response schema.
- Require an explicit cloud API mode. Never auto-fallback or retry through a
  different protocol because that could upload confirmed content again through
  an endpoint the user did not approve.

## Technology

- Use Rust and Tauri 2 for the desktop core and packaging.
- Reuse the existing HTML/CSS/JavaScript interaction model during the first
  vertical slice. Do not add a frontend framework without demonstrated need.
- Local development builds may be unsigned. Public v0.1 artifacts must be signed
  and notarized.
- Target macOS 13 or later provisionally and validate the final minimum before
  release.
- Release under the MIT License.

## Model Routing

- When the environment supports model routing, use `sol-high` for product and UX
  design, architecture, technical planning, threat modeling, plan review, code
  review, specification review, and milestone acceptance review.
- When the environment supports model routing, use `terra-high` for implementation,
  refactoring, test construction, debugging, build work, and executing an accepted
  phase plan.
- For work spanning both categories, use this sequence: `sol-high` produces or
  reviews the design and acceptance criteria; the human accepts the relevant
  `INIT.md`, `PLAN.md`, or phase plan; `terra-high` implements and verifies it;
  `sol-high` performs the final review against the accepted artifacts.
- Pass decisions through repository artifacts rather than relying on hidden model
  context. Implementation must trace back to the accepted plan and constraints.
- Model routing does not grant authority to bypass human approval, mutate external
  systems, publish releases, or expand scope.
- If a requested route is unavailable, state that limitation explicitly and use
  the current capable model without claiming the preferred route was used. Do not
  block safe progress solely because a route name is unavailable.

## Domain And Safety

- Keep Audit and Installation Confirmation as separate actions. Audit must not
  install, enable, delete, execute, or otherwise mutate a candidate.
- Treat explicit-name and contextual-intent triggers as valid strategies. Never
  batch-rewrite trigger policy or present one strategy as universally required.
- Never label a Skill or Audit Result as "safe" or "secure". Present evidence,
  severity, confidence, and exact versions or hashes.
- Keep system and plugin-managed Skills read-only and exclude them from export.
- Restrict lifecycle mutation to user-controlled Skills. Permanent deletion is
  allowed only from archive after exact destructive confirmation; disable,
  enable, archive, restore, and delete must never overwrite a destination.
- Bundle Import verifies and stages content; it does not install content.
- Exclude credentials and secrets from bundles. v0.1 bundles are not encrypted.
- Reject path traversal, unsafe archive entries, containment escapes, and
  unsupported symlink writes.
- Use content hashes for draft concurrency, complete-directory revisions for
  lifecycle concurrency, and atomic replacement or same-filesystem rename for
  writes and state moves.
- Performance work may cache catalog discovery and unchanged audit results, but
  must never reuse a preview revision as the apply-time mutation check. Recheck
  containment, destination conflicts, and the affected directory revision
  immediately before every lifecycle mutation.
- Do not execute untrusted Skill scripts during acquisition, import, or audit.
- Baseline Audit must be local and offline. Deep Audit may send only the files
  explicitly confirmed for that scan to the user's configured cloud provider.
- Treat content submitted to a cloud model as untrusted data: provide no tools,
  accept only structured evidence grounded in submitted files, and independently
  review likely false positives before producing the aggregated verdict.
- Never store cloud credentials in project files, Skill directories, logs, or
  audit evidence, and never reuse Codex authentication credentials.

## User Experience

- Make the desktop GUI the primary interface; keep CLI surfaces secondary.
- Design for people who do not use a terminal or edit configuration files.
- Use plain language for the common path while keeping exact source, paths,
  hashes, and evidence available one level deeper.
- Follow familiar macOS hierarchy, interaction, accessibility, reduced-motion,
  reduced-transparency, and dark-mode behavior.
- Keep provider and credential preferences in global Settings, reachable through
  a visible control and `Command+,`. Skill editors should contain actions for
  the current draft, not application-level provider configuration.
- Require explicit confirmation for installation, lifecycle mutations,
  permanent deletion, overwrites, conflicts, and findings that require manual
  review.
- Before every cloud Deep Audit, show the selected API mode, provider endpoint,
  and exact files that will leave the machine; cancellation performs no network
  request.
- Bind cloud consent to the selected mode, derived endpoint, model, and candidate
  files; recheck both provider and candidate fingerprints before sending.
- Do not generate persistent HTML reports unless the user explicitly exports one.

## Engineering

- Preserve unrelated user changes and inspect the working tree before editing.
- Keep mutations behind narrow interfaces and test the same interfaces callers
  use.
- After correctness-first code adds preview, apply, refresh, and post-action
  selection layers, inspect the complete I/O call graph before accepting the
  workflow. Locally reasonable safety checks must not silently multiply the
  same full-source scan, parse, audit, or directory hash.
- Separate discovery from mutation authority. Cache catalog discovery, parsing,
  and unchanged audit evidence; immediately before mutation, perform targeted
  current-filesystem checks for ownership, containment, destination conflicts,
  and the affected directory revision.
- After a Studio-owned save, create, move, or delete, update only the affected
  backend index and frontend state. Use full-source discovery for initial
  indexing and the user's explicit Refresh path for external filesystem changes;
  an accepted mutation contract may additionally require one fresh cross-source
  conflict scan at final confirmation, as creation and installation do.
- Verify performance with deterministic invocation-count tests on synthetic
  large catalogs plus timings on the first user's real catalog. Do not rely only
  on subjective responsiveness, elapsed-time thresholds, or small fixtures.
- Add focused tests for traversal, symlinks, conflicts, hash mismatches, atomic
  writes, read-only sources, evidence semantics, and mutation gates.
- Verify each desktop milestone with Rust tests, frontend checks, a real macOS
  window, desktop/mobile-size screenshots where relevant, and a human workflow
  pass.
- Port behavior before deleting the Node prototype; remove duplicate engines once
  parity is proven.
- Keep `AGENTS.md` durable. Put task status and sequencing in `PLAN.md`, detailed
  milestone design in `docs/phases/`, and hard-to-reverse trade-offs in ADRs.

## Prohibited Actions

- Do not add one-click bulk policy rewrites.
- Do not mutate system or plugin-managed Skills.
- Do not conflate audit, import, save, and installation confirmations.
- Do not silently upload Skill content or make cloud Deep Audit a prerequisite
  for local editing and creation.
- Do not claim universal cross-Agent compatibility without adapter evidence.
- Do not expand v0.1 into a general Agent configuration manager.
