# GitHub Candidate Acquisition Options

## Task 3.1

Date: 2026-08-01

Question: how should Agent Skill Studio obtain a public GitHub Skill for
read-only audit and later, separately confirmed installation without requiring
Node.js or executing candidate content?

## Requirements

- Resolve a user-supplied repository/ref/path to an immutable commit SHA.
- Inspect file type, mode, size, and path before downloading file contents.
- Reject symlinks, submodules, traversal-like paths, duplicate paths, and
  containment escapes.
- Keep acquisition, audit, and installation separate; acquisition never writes
  to a managed Skill directory or executes candidate content.
- Work for a non-programmer on a clean macOS install without requiring Git,
  Node.js, or a GitHub login for public repositories.
- Bound API responses, file count, per-file size, total staged size, and
  concurrency. Surface rate limits instead of silently retrying forever.
- Preserve repository, requested ref, resolved SHA, path, hashes, and source
  URLs as candidate evidence.

## Existing Solutions

### `npx skills`

The Vercel `skills` CLI is the closest existing distribution solution. Its
current README supports GitHub shorthand and URLs, direct paths, local paths,
temporary `skills use`, 76+ agents, listing, updating, removing, and creating
Skills. Its download path has explicit 10 MiB download, 25 MiB extracted, and
1000-file defaults. Its archive implementation rejects unsafe paths, encrypted
entries, links, excessive entries, and excessive extracted bytes. Its Git
transport tests also exercise a transport allowlist.

It is not a replacement for the Studio workflow: it is a Node-based installer
and prompt generator, can create symlinks or copies in Agent directories, does
not present a provider-neutral evidence record for a staged candidate, and does
not make audit and installation separate product actions. The Studio should
reuse its limits and threat model as references, not embed its runtime or
rebuild its distribution features.

Evidence:

- [skills README at the queried commit](https://github.com/vercel-labs/skills/blob/1164afa5f0e21ebd01e6fc11249759353f494ad1/README.md)
- [download limits and archive staging](https://github.com/vercel-labs/skills/blob/1164afa5f0e21ebd01e6fc11249759353f494ad1/src/download-source.ts)
- [ZIP path/link/size validation](https://github.com/vercel-labs/skills/blob/1164afa5f0e21ebd01e6fc11249759353f494ad1/src/archive.ts)
- [Git transport allowlist tests](https://github.com/vercel-labs/skills/blob/1164afa5f0e21ebd01e6fc11249759353f494ad1/tests/git-transport-allowlist.test.ts)

### GitHub Archive API

`GET /repos/{owner}/{repo}/zipball/{ref}` is a maintained, canonical way to
download a repository snapshot. A live request for `vercel-labs/agent-skills`
resolved to a `302` whose destination was `codeload.github.com` and whose path
contained the resolved commit SHA.

The archive route is simple and one-download, but it downloads the whole
repository even when the user wants one nested Skill. It also creates a new
archive extraction attack surface: central-directory parsing, traversal,
duplicate/case-collision handling, links, encrypted entries, decompression
bombs, and archive-size limits. It is a useful fallback for a small repository,
not the default path for a path-specific candidate.

### GitHub Git Trees plus raw file downloads (selected)

GitHub's commit and Git Trees endpoints provide the needed metadata without a
Git checkout. A live validation used `vercel-labs/agent-skills`:

- `main` resolved to commit `7c180d9044c9ae2b442b567aad4e42a28dd5ed62`.
- Traversing the root tree, `skills`, and the target Skill required three small
  tree requests; the target subtree contained only `SKILL.md`.
- The tree exposed file modes, including `120000` symlinks and normal blobs,
  before file contents were requested.
- A fixed-SHA raw request returned `SKILL.md` with `content-length: 1231` and
  no redirect.

The selected flow is therefore:

1. Parse and validate a public `github.com` repository/tree URL.
2. Resolve the requested ref through the commit endpoint and record the
   resulting SHA.
3. Traverse only the requested directory path through Git Trees. Reject
   symlink (`120000`), submodule (`160000`), and unsupported entry types before
   downloads; preserve executable mode as evidence without executing it.
4. Validate tree paths, counts, depths, and declared sizes.
5. Download accepted blobs from `raw.githubusercontent.com/{owner}/{repo}/{sha}/{path}`
   with an exact-host HTTPS allowlist, streaming byte limits, bounded
   concurrency, and no redirects.
6. Write only into a temporary contained staging directory, compute SHA-256
   hashes, and return a candidate manifest. No managed Skill path is touched.

This uses a few GitHub API requests per directory path rather than one API call
per file. Raw downloads do not consume the GitHub REST core quota in the same
way as blob API requests. The app must still handle raw-server failures and
unknown content lengths with streaming limits.

Official references:

- [Get a commit](https://docs.github.com/en/rest/commits/commits#get-a-commit)
- [Get a tree](https://docs.github.com/en/rest/git/trees#get-a-tree)
- [Get a blob](https://docs.github.com/en/rest/git/blobs#get-a-blob)
- [Download a repository archive](https://docs.github.com/en/rest/repos/contents#download-a-repository-archive-zip)
- [REST API rate limits](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api)

### System `git`

The system command is familiar and supports shallow clones, but it is not a
good first-user dependency. It requires Git/Command Line Tools, inherits user
and system Git configuration, has a large transport/configuration surface, and
adds process, environment, credential-helper, filter, submodule, and hook
behavior to a read-only acquisition path. A safe wrapper would need to control
all of those inputs and still would not remove the clean-machine dependency.

### `git2-rs` / libgit2 and gitoxide (`gix`)

`git2-rs` provides mature Rust bindings to libgit2, while gitoxide is a mature
pure-Rust Git implementation. Both are active projects, but they are full Git
implementations with substantially more surface than a public GitHub snapshot
reader. libgit2 also adds a native C dependency and packaging responsibility;
gitoxide adds a large dependency graph and clone/fetch behavior to a workflow
that only needs commit/tree metadata and immutable file bytes. Neither should
be added in v0.1 for this narrow path.

Evidence checked on 2026-08-01:

- [gitoxide](https://github.com/GitoxideLabs/gitoxide)
- [libgit2](https://github.com/libgit2/libgit2)
- [git2-rs](https://github.com/rust-lang/git2-rs)

### Maintained scanners

Cisco AI Defense Skill Scanner supports `scan-repo owner/repo` and combines
static, behavioral, and optional LLM analysis. Snyk Agent Scan inventories and
scans local agent components and can send skill data to its service; its own
documentation warns that MCP configuration scans can execute configured stdio
commands. These are scanner adapters, not candidate acquisition/staging
interfaces. The Studio should keep them optional and never execute a generic
MCP scan as part of GitHub acquisition.

Evidence:

- [Cisco AI Defense Skill Scanner](https://github.com/cisco-ai-defense/skill-scanner)
- [Snyk Agent Scan](https://github.com/snyk/agent-scan)

## Decision

For public GitHub candidates in v0.1, implement a small Rust GitHub snapshot
adapter using GitHub commit/tree metadata and fixed-SHA raw downloads. Do not
shell out to Git, add libgit2/gitoxide, invoke `npx skills`, or download a whole
repository archive by default.

The first interface accepts a public GitHub URL and returns a staged candidate
manifest. It does not install. Private GitHub hosts, GitHub tokens, arbitrary
Git URLs, submodules, symlink preservation, and archive fallback remain out of
scope until a separate compatibility and credential decision.

## Safety Contract For Task 3.2

- `github.com` API and `raw.githubusercontent.com` are the only network hosts;
  HTTPS is required and redirects are disabled.
- A repository/ref/path is resolved to one commit SHA before any candidate file
  is accepted. All raw downloads use that SHA, never a moving branch name.
- Suggested starting limits are 256 files, 25 MiB total staged bytes, 2 MiB per
  file, depth 8, and a bounded four-file download concurrency. The implementation
  must reject unknown/over-limit tree responses before content download.
- Reject symlinks, submodules, duplicate normalized paths, case-colliding paths,
  traversal, unsupported entry kinds, and containment escapes.
- Preserve executable mode as evidence but never execute scripts, Git hooks,
  package managers, MCP servers, or Agent commands.
- Staging is temporary and cleanup is required on cancel, failure, and normal
  completion. Installation is a separate explicit Task 3.4 action.
- Record requested URL/ref, resolved SHA, candidate path, per-file SHA-256,
  source URL, skipped entries, limits, and acquisition warnings.

## Acceptance

The owner accepted this GitHub snapshot adapter direction on 2026-08-01. The
choice is intentionally narrower than a generic Git client and does not replace
`npx skills` for users who only want installation.
