import { createServer } from "node:http";
import { promises as fs } from "node:fs";
import { createReadStream } from "node:fs";
import { homedir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { randomBytes } from "node:crypto";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import {
  auditSkillDraft,
  hashSkillContent,
  isExplicitTriggerCompliant,
  parseFrontmatter,
  parseSkillDocument
} from "./skill-workspace.mjs";

export { auditSkillDraft, isExplicitTriggerCompliant, parseFrontmatter } from "./skill-workspace.mjs";

const execFileAsync = promisify(execFile);
const projectRoot = path.dirname(fileURLToPath(import.meta.url));
const publicRoot = path.join(projectRoot, "public");
const defaultCodexHome = process.env.CODEX_HOME || path.join(homedir(), ".codex");

const CONTENT_TYPES = {
  ".css": "text/css; charset=utf-8",
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".png": "image/png",
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg",
  ".svg": "image/svg+xml",
  ".webp": "image/webp"
};

const SOURCE_ORDER = { personal: 0, disabled: 1, system: 2, plugin: 3, archive: 4 };
const ICON_EXTENSIONS = new Set([".png", ".jpg", ".jpeg", ".svg", ".webp"]);

function requiredTriggerPrefix(name) {
  return `Use only when the user's request explicitly contains the full skill name \`${name}\` or \`$${name}\`; never trigger from task intent, synonyms, former trigger phrases, or conversational context.`;
}

function capabilitySummary(name, description) {
  const required = requiredTriggerPrefix(name);
  const trimmed = description.trim();
  return trimmed.startsWith(required) ? trimmed.slice(required.length).trim() || trimmed : trimmed;
}

function parseAgentConfig(yaml = "") {
  const value = (key) => {
    const match = yaml.match(new RegExp(`^\\s*${key}:\\s*["']?([^"'\\n]+)["']?\\s*$`, "m"));
    return match?.[1]?.trim() || "";
  };
  return {
    displayName: value("display_name"),
    shortDescription: value("short_description"),
    iconSmall: value("icon_small"),
    brandColor: value("brand_color")
  };
}

function encodeId(source, skillPath) {
  return Buffer.from(`${source}\0${skillPath}`).toString("base64url");
}

function safeDirectoryName(value) {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, "-")
    .replace(/^-+|-+$/g, "") || "skill";
}

function within(parent, candidate) {
  const relative = path.relative(parent, candidate);
  return relative === "" || (!relative.startsWith("..") && !path.isAbsolute(relative));
}

async function exists(filePath) {
  try {
    await fs.access(filePath);
    return true;
  } catch {
    return false;
  }
}

async function countFiles(directory, depth = 0) {
  if (depth > 5) return 0;
  let entries;
  try {
    entries = await fs.readdir(directory, { withFileTypes: true });
  } catch {
    return 0;
  }
  let total = 0;
  for (const entry of entries) {
    if (entry.name === "node_modules" || entry.name === ".git") continue;
    if (entry.isFile()) total += 1;
    if (entry.isDirectory()) total += await countFiles(path.join(directory, entry.name), depth + 1);
  }
  return total;
}

async function readSkill(skillPath, source, root) {
  const skillFile = path.join(skillPath, "SKILL.md");
  let markdown;
  try {
    markdown = await fs.readFile(skillFile, "utf8");
  } catch {
    return null;
  }

  const frontmatter = parseFrontmatter(markdown);
  const agentFile = path.join(skillPath, "agents", "openai.yaml");
  const agentYaml = (await exists(agentFile)) ? await fs.readFile(agentFile, "utf8") : "";
  const agent = parseAgentConfig(agentYaml);
  const stat = await fs.stat(skillFile);
  const name = frontmatter.name || path.basename(skillPath);
  const description = frontmatter.description || agent.shortDescription || "No description provided.";
  const triggerMode = isExplicitTriggerCompliant(name, description) ? "explicit" : "contextual";
  const baselineAudit = auditSkillDraft({ markdown, originalMarkdown: markdown, expectedName: name });
  const iconCandidate = agent.iconSmall ? path.resolve(skillPath, agent.iconSmall) : "";
  const iconPath =
    iconCandidate &&
    within(skillPath, iconCandidate) &&
    ICON_EXTENSIONS.has(path.extname(iconCandidate).toLowerCase()) &&
    (await exists(iconCandidate))
      ? iconCandidate
      : null;

  return {
    id: encodeId(source, skillPath),
    name,
    displayName: agent.displayName || name,
    description,
    summary: capabilitySummary(name, description),
    source,
    state: source === "disabled" ? "disabled" : source === "archive" ? "archived" : "active",
    path: skillPath,
    root,
    directoryName: path.basename(skillPath),
    modifiedAt: stat.mtime.toISOString(),
    fileCount: await countFiles(skillPath),
    triggerCompliant: triggerMode === "explicit",
    triggerMode,
    hasBlockingFindings: baselineAudit.verdict === "block",
    hasIcon: Boolean(iconPath),
    iconPath,
    brandColor: /^#[0-9a-f]{6}$/i.test(agent.brandColor) ? agent.brandColor : null,
    markdown,
    contentHash: hashSkillContent(markdown),
    document: parseSkillDocument(markdown)
  };
}

async function scanImmediate(root, source, { includeHidden = false } = {}) {
  let entries;
  try {
    entries = await fs.readdir(root, { withFileTypes: true });
  } catch {
    return [];
  }

  const skills = [];
  for (const entry of entries) {
    if (!entry.isDirectory() || (!includeHidden && entry.name.startsWith("."))) continue;
    const skill = await readSkill(path.join(root, entry.name), source, root);
    if (skill) skills.push(skill);
  }
  return skills;
}

async function scanRecursive(root, source, depth = 0) {
  if (depth > 8) return [];
  let entries;
  try {
    entries = await fs.readdir(root, { withFileTypes: true });
  } catch {
    return [];
  }

  if (entries.some((entry) => entry.isFile() && entry.name === "SKILL.md")) {
    const skill = await readSkill(root, source, root);
    return skill ? [skill] : [];
  }

  const nested = [];
  for (const entry of entries) {
    if (!entry.isDirectory() || entry.name === "node_modules" || entry.name === ".git") continue;
    nested.push(...(await scanRecursive(path.join(root, entry.name), source, depth + 1)));
  }
  return nested;
}

function publicSkill(skill) {
  const { iconPath, markdown, document, root, ...safe } = skill;
  return safe;
}

export async function scanSkills(options = {}) {
  const codexHome = options.codexHome || defaultCodexHome;
  const personalRoot = options.personalRoot || path.join(codexHome, "skills");
  const systemRoot = options.systemRoot || path.join(personalRoot, ".system");
  const pluginRoot = options.pluginRoot || path.join(codexHome, "plugins", "cache");
  const disabledRoot = options.disabledRoot || path.join(codexHome, "skills-disabled");
  const archiveRoot = options.archiveRoot || path.join(codexHome, "skill-archive");

  const [personal, system, plugins, disabled, archive] = await Promise.all([
    scanImmediate(personalRoot, "personal"),
    scanImmediate(systemRoot, "system"),
    scanRecursive(pluginRoot, "plugin"),
    scanImmediate(disabledRoot, "disabled"),
    scanImmediate(archiveRoot, "archive")
  ]);

  const pluginLatest = new Map();
  for (const skill of plugins) {
    const current = pluginLatest.get(skill.name);
    if (!current || current.modifiedAt < skill.modifiedAt) pluginLatest.set(skill.name, skill);
  }

  const skills = [...personal, ...disabled, ...system, ...pluginLatest.values(), ...archive].sort((a, b) => {
    const sourceDifference = SOURCE_ORDER[a.source] - SOURCE_ORDER[b.source];
    return sourceDifference || a.displayName.localeCompare(b.displayName);
  });

  return {
    codexHome,
    roots: { personalRoot, systemRoot, pluginRoot, disabledRoot, archiveRoot },
    skills,
    counts: skills.reduce(
      (counts, skill) => {
        counts.total += 1;
        counts[skill.source] += 1;
        if (skill.source === "personal" && skill.hasBlockingFindings) counts.needsAttention += 1;
        return counts;
      },
      { total: 0, personal: 0, disabled: 0, system: 0, plugin: 0, archive: 0, needsAttention: 0 }
    )
  };
}

async function readJson(req) {
  const chunks = [];
  let size = 0;
  for await (const chunk of req) {
    size += chunk.length;
    if (size > 64 * 1024) throw Object.assign(new Error("Request body is too large."), { status: 413 });
    chunks.push(chunk);
  }
  if (!chunks.length) return {};
  try {
    return JSON.parse(Buffer.concat(chunks).toString("utf8"));
  } catch {
    throw Object.assign(new Error("Invalid JSON body."), { status: 400 });
  }
}

function sendJson(res, status, payload) {
  const body = JSON.stringify(payload);
  res.writeHead(status, {
    "Content-Type": CONTENT_TYPES[".json"],
    "Content-Length": Buffer.byteLength(body),
    "Cache-Control": "no-store"
  });
  res.end(body);
}

function sendError(res, error) {
  const status = error.status || 500;
  sendJson(res, status, { error: error.message || "Unexpected server error." });
}

function secureHeaders(res) {
  res.setHeader("X-Content-Type-Options", "nosniff");
  res.setHeader("X-Frame-Options", "DENY");
  res.setHeader("Referrer-Policy", "no-referrer");
  res.setHeader(
    "Content-Security-Policy",
    "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'"
  );
}

async function serveFile(res, filePath, transform) {
  const extension = path.extname(filePath).toLowerCase();
  if (!CONTENT_TYPES[extension]) throw Object.assign(new Error("Unsupported file type."), { status: 404 });
  const stat = await fs.stat(filePath);
  const headers = {
    "Content-Type": CONTENT_TYPES[extension],
    "Cache-Control": "no-store"
  };
  if (!transform) headers["Content-Length"] = stat.size;
  res.writeHead(200, headers);
  if (transform) {
    const content = await fs.readFile(filePath, "utf8");
    res.end(transform(content));
    return;
  }
  createReadStream(filePath).pipe(res);
}

function validateInstallInput(body) {
  const repo = String(body.repo || "").replace(/^\/+/, "").trim();
  const ref = String(body.ref || "main").trim();
  const skillPath = String(body.path || "").trim().replace(/^\/+/, "");
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repo)) {
    throw Object.assign(new Error("Repository must use owner/repo format."), { status: 400 });
  }
  if (!/^[A-Za-z0-9_./-]+$/.test(ref) || ref.includes("..")) {
    throw Object.assign(new Error("Invalid Git reference."), { status: 400 });
  }
  if (!skillPath || path.isAbsolute(skillPath) || skillPath.split("/").includes("..")) {
    throw Object.assign(new Error("Enter a safe path to the skill directory."), { status: 400 });
  }
  return { repo, ref, skillPath };
}

async function runValidator(codexHome) {
  const validator = path.join(codexHome, "bin", "validate-skill-triggers");
  if (!(await exists(validator))) {
    throw Object.assign(new Error("Trigger validator was not found."), { status: 404 });
  }
  const result = await execFileAsync(validator, ["--check"], {
    timeout: 60_000,
    maxBuffer: 1024 * 1024
  });
  return result.stdout.trim();
}

export function createApp(options = {}) {
  const codexHome = options.codexHome || defaultCodexHome;
  const csrfToken = options.csrfToken || randomBytes(24).toString("base64url");
  const roots = {
    personalRoot: options.personalRoot || path.join(codexHome, "skills"),
    systemRoot: options.systemRoot || path.join(codexHome, "skills", ".system"),
    pluginRoot: options.pluginRoot || path.join(codexHome, "plugins", "cache"),
    disabledRoot: options.disabledRoot || path.join(codexHome, "skills-disabled"),
    archiveRoot: options.archiveRoot || path.join(codexHome, "skill-archive")
  };

  const catalogOptions = { codexHome, ...roots };

  async function catalog() {
    return scanSkills(catalogOptions);
  }

  async function findSkill(id) {
    const current = await catalog();
    const skill = current.skills.find((item) => item.id === id);
    if (!skill) throw Object.assign(new Error("Skill was not found. Refresh and try again."), { status: 404 });
    return skill;
  }

  async function moveSkill(skill, destinationRoot, directoryName) {
    if (!["personal", "disabled", "archive"].includes(skill.source)) {
      throw Object.assign(new Error("System and plugin skills are read-only."), { status: 403 });
    }
    await fs.mkdir(destinationRoot, { recursive: true });
    const destination = path.join(destinationRoot, directoryName);
    if (!within(destinationRoot, destination)) {
      throw Object.assign(new Error("Unsafe destination path."), { status: 400 });
    }
    if (await exists(destination)) {
      throw Object.assign(new Error(`A skill folder named ${directoryName} already exists at the destination.`), {
        status: 409
      });
    }
    await fs.rename(skill.path, destination);
    return destination;
  }

  function editableSkill(skill) {
    if (!["personal", "disabled"].includes(skill.source)) {
      throw Object.assign(new Error("Only personal skills can be edited."), { status: 403 });
    }
    return skill;
  }

  function draftMarkdown(body) {
    if (typeof body.markdown !== "string") {
      throw Object.assign(new Error("Draft markdown is required."), { status: 400 });
    }
    if (Buffer.byteLength(body.markdown, "utf8") > 64 * 1024) {
      throw Object.assign(new Error("Draft markdown is too large."), { status: 413 });
    }
    return body.markdown;
  }

  async function saveDraft(skill, markdown, expectedHash) {
    editableSkill(skill);
    if (!expectedHash || expectedHash !== skill.contentHash) {
      throw Object.assign(new Error("This Skill changed after the draft was opened. Refresh before saving."), {
        status: 409
      });
    }

    const audit = auditSkillDraft({ markdown, originalMarkdown: skill.markdown, expectedName: skill.name });
    if (audit.verdict === "block") {
      const error = Object.assign(new Error("Resolve blocking findings before saving."), { status: 422, audit });
      throw error;
    }

    const skillFile = path.join(skill.path, "SKILL.md");
    const fileStat = await fs.lstat(skillFile);
    if (fileStat.isSymbolicLink()) {
      throw Object.assign(new Error("Editing a linked SKILL.md is not supported."), { status: 409 });
    }

    const temporaryFile = path.join(skill.path, `.SKILL.md.${randomBytes(8).toString("hex")}.tmp`);
    try {
      await fs.writeFile(temporaryFile, markdown, { encoding: "utf8", mode: fileStat.mode });
      await fs.rename(temporaryFile, skillFile);
    } catch (error) {
      await fs.rm(temporaryFile, { force: true }).catch(() => {});
      throw error;
    }
    return audit;
  }

  return createServer(async (req, res) => {
    secureHeaders(res);
    const host = String(req.headers.host || "").split(":")[0];
    if (host && !["127.0.0.1", "localhost", "[::1]"].includes(host)) {
      sendJson(res, 403, { error: "This service only accepts localhost requests." });
      return;
    }

    const url = new URL(req.url, "http://127.0.0.1");
    const isMutation = req.method !== "GET" && req.method !== "HEAD";
    if (isMutation && req.headers["x-skill-center-token"] !== csrfToken) {
      sendJson(res, 403, { error: "Request token is missing or invalid." });
      return;
    }

    try {
      if (req.method === "GET" && url.pathname === "/api/health") {
        sendJson(res, 200, { ok: true, localOnly: true });
        return;
      }

      if (req.method === "GET" && url.pathname === "/api/skills") {
        const current = await catalog();
        sendJson(res, 200, { ...current, skills: current.skills.map(publicSkill) });
        return;
      }

      const detailMatch = url.pathname.match(/^\/api\/skills\/([^/]+)$/);
      if (req.method === "GET" && detailMatch) {
        const skill = await findSkill(detailMatch[1]);
        sendJson(res, 200, {
          ...publicSkill(skill),
          markdown: skill.markdown,
          document: skill.document,
          editable: ["personal", "disabled"].includes(skill.source)
        });
        return;
      }

      const auditMatch = url.pathname.match(/^\/api\/skills\/([^/]+)\/audit$/);
      if (req.method === "POST" && auditMatch) {
        const skill = editableSkill(await findSkill(auditMatch[1]));
        const body = await readJson(req);
        const markdown = draftMarkdown(body);
        sendJson(
          res,
          200,
          auditSkillDraft({ markdown, originalMarkdown: skill.markdown, expectedName: skill.name })
        );
        return;
      }

      if (req.method === "PUT" && detailMatch) {
        const skill = editableSkill(await findSkill(detailMatch[1]));
        const body = await readJson(req);
        const markdown = draftMarkdown(body);
        const audit = await saveDraft(skill, markdown, String(body.expectedHash || ""));
        sendJson(res, 200, {
          ok: true,
          audit,
          contentHash: hashSkillContent(markdown),
          restartRecommended: true
        });
        return;
      }

      const iconMatch = url.pathname.match(/^\/api\/skills\/([^/]+)\/icon$/);
      if (req.method === "GET" && iconMatch) {
        const skill = await findSkill(iconMatch[1]);
        if (!skill.iconPath) throw Object.assign(new Error("No icon is available."), { status: 404 });
        await serveFile(res, skill.iconPath);
        return;
      }

      const actionMatch = url.pathname.match(/^\/api\/skills\/([^/]+)\/(toggle|archive|restore)$/);
      if (req.method === "POST" && actionMatch) {
        const [, id, action] = actionMatch;
        const skill = await findSkill(id);
        let destination;
        if (action === "toggle") {
          if (skill.source === "personal") {
            destination = await moveSkill(skill, roots.disabledRoot, skill.directoryName);
          } else if (skill.source === "disabled") {
            destination = await moveSkill(skill, roots.personalRoot, skill.directoryName);
          } else {
            throw Object.assign(new Error("Only personal skills can be enabled or disabled."), { status: 403 });
          }
        } else if (action === "archive") {
          if (!["personal", "disabled"].includes(skill.source)) {
            throw Object.assign(new Error("Only personal skills can be archived."), { status: 403 });
          }
          const stamp = new Date().toISOString().replace(/[-:.]/g, "").slice(0, 15);
          destination = await moveSkill(skill, roots.archiveRoot, `${stamp}-${skill.directoryName}`);
        } else {
          if (skill.source !== "archive") {
            throw Object.assign(new Error("Only archived skills can be restored."), { status: 403 });
          }
          destination = await moveSkill(skill, roots.personalRoot, safeDirectoryName(skill.name));
        }
        sendJson(res, 200, { ok: true, destination, restartRecommended: true });
        return;
      }

      if (req.method === "POST" && url.pathname === "/api/validate") {
        const output = await runValidator(codexHome);
        sendJson(res, 200, { ok: true, output, restartRecommended: false });
        return;
      }

      if (req.method === "POST" && url.pathname === "/api/install") {
        const body = await readJson(req);
        const { repo, ref, skillPath } = validateInstallInput(body);
        const installer = path.join(
          codexHome,
          "skills",
          ".system",
          "skill-installer",
          "scripts",
          "install-skill-from-github.py"
        );
        if (!(await exists(installer))) {
          throw Object.assign(new Error("The official Codex skill installer was not found."), { status: 404 });
        }
        const install = await execFileAsync(
          process.env.PYTHON || "/usr/bin/python3",
          [installer, "--repo", repo, "--ref", ref, "--path", skillPath],
          { timeout: 120_000, maxBuffer: 2 * 1024 * 1024, env: { ...process.env, CODEX_HOME: codexHome } }
        );
        sendJson(res, 200, {
          ok: true,
          output: install.stdout.trim(),
          restartRecommended: true
        });
        return;
      }

      if (req.method === "GET" && url.pathname === "/vendor/lucide.js") {
        await serveFile(res, path.join(projectRoot, "node_modules", "lucide", "dist", "umd", "lucide.js"));
        return;
      }

      if (req.method === "GET" && (url.pathname === "/" || url.pathname === "/index.html")) {
        await serveFile(res, path.join(publicRoot, "index.html"), (content) =>
          content.replace("__SKILL_CENTER_TOKEN__", csrfToken)
        );
        return;
      }

      if (req.method === "GET") {
        const relative = decodeURIComponent(url.pathname).replace(/^\/+/, "");
        const filePath = path.resolve(publicRoot, relative);
        if (!within(publicRoot, filePath)) throw Object.assign(new Error("Not found."), { status: 404 });
        await serveFile(res, filePath);
        return;
      }

      throw Object.assign(new Error("Not found."), { status: 404 });
    } catch (error) {
      if (error.code === "ENOENT") error.status = 404;
      if (error.stderr && !error.message.includes(error.stderr)) {
        error.message = `${error.message}\n${String(error.stderr).trim()}`;
      }
      sendError(res, error);
    }
  });
}

if (path.resolve(process.argv[1] || "") === fileURLToPath(import.meta.url)) {
  const host = "127.0.0.1";
  const port = Number(process.env.PORT || 4177);
  const server = createApp();
  server.listen(port, host, () => {
    console.log(`Agent Skill Studio is running at http://${host}:${port}`);
  });
}
