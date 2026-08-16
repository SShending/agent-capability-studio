export const REPOSITORY_INTAKE_STORAGE_KEY = "agent-skill-studio.repository-intake-v1";

const SHA_PATTERN = /^[a-f0-9]{40}$/i;

export function createRepositoryReviewQueue({
  sourceUrl,
  requestedRef,
  resolvedSha,
  selectedPaths,
  currentPosition = 0
}) {
  const queue = {
    sourceUrl: typeof sourceUrl === "string" ? sourceUrl.trim() : "",
    requestedRef: typeof requestedRef === "string" ? requestedRef : "",
    resolvedSha: typeof resolvedSha === "string" ? resolvedSha : "",
    selectedPaths: Object.freeze(Array.isArray(selectedPaths) ? [...selectedPaths] : []),
    currentPosition
  };
  return validateRepositoryReviewQueue(queue) ? Object.freeze(queue) : null;
}

export function validateRepositoryReviewQueue(queue) {
  if (!queue || typeof queue !== "object") return false;
  if (!isPublicGithubRepositoryUrl(queue.sourceUrl)) return false;
  if (typeof queue.requestedRef !== "string"
    || queue.requestedRef.length > 255
    || !isSafeGithubRef(queue.requestedRef)) {
    return false;
  }
  if (typeof queue.resolvedSha !== "string" || !SHA_PATTERN.test(queue.resolvedSha)) return false;
  if (!Array.isArray(queue.selectedPaths) || queue.selectedPaths.length === 0 || queue.selectedPaths.length > 256) {
    return false;
  }
  const unique = new Set();
  for (const path of queue.selectedPaths) {
    if (typeof path !== "string" || !isConventionalSkillPath(path)) return false;
    const key = path.toLowerCase();
    if (unique.has(key)) return false;
    unique.add(key);
  }
  return Number.isInteger(queue.currentPosition)
    && queue.currentPosition >= 0
    && queue.currentPosition < queue.selectedPaths.length;
}

export function persistRepositoryReviewQueue(storage, queue) {
  if (!validateRepositoryReviewQueue(queue)) {
    storage.removeItem(REPOSITORY_INTAKE_STORAGE_KEY);
    return false;
  }
  storage.setItem(REPOSITORY_INTAKE_STORAGE_KEY, JSON.stringify(queue));
  return true;
}

export function restoreRepositoryReviewQueue(storage) {
  try {
    const raw = storage.getItem(REPOSITORY_INTAKE_STORAGE_KEY);
    if (!raw) return null;
    const queue = createRepositoryReviewQueue(JSON.parse(raw));
    if (!queue) storage.removeItem(REPOSITORY_INTAKE_STORAGE_KEY);
    return queue;
  } catch {
    storage.removeItem(REPOSITORY_INTAKE_STORAGE_KEY);
    return null;
  }
}

export function clearRepositoryReviewQueue(storage) {
  storage.removeItem(REPOSITORY_INTAKE_STORAGE_KEY);
}

export function currentRepositoryQueuePath(queue) {
  return validateRepositoryReviewQueue(queue)
    ? queue.selectedPaths[queue.currentPosition]
    : null;
}

export function removeCurrentRepositoryQueuePath(queue) {
  if (!validateRepositoryReviewQueue(queue)) return null;
  const selectedPaths = queue.selectedPaths.filter((_, index) => index !== queue.currentPosition);
  if (selectedPaths.length === 0) return null;
  return createRepositoryReviewQueue({
    ...queue,
    selectedPaths,
    currentPosition: Math.min(queue.currentPosition, selectedPaths.length - 1)
  });
}

export function setRepositoryQueuePosition(queue, currentPosition) {
  if (!validateRepositoryReviewQueue(queue)
    || !Number.isInteger(currentPosition)
    || currentPosition < 0
    || currentPosition >= queue.selectedPaths.length) {
    return null;
  }
  return createRepositoryReviewQueue({ ...queue, currentPosition });
}

export function adjacentRepositoryQueuePosition(queue, offset) {
  if (!validateRepositoryReviewQueue(queue) || !Number.isInteger(offset) || offset === 0) {
    return null;
  }
  const position = queue.currentPosition + offset;
  return position >= 0 && position < queue.selectedPaths.length ? position : null;
}

export function createRepositorySessionCache() {
  return new Map();
}

export async function getOrStageRepositorySession(cache, skillPath, stage) {
  if (!(cache instanceof Map) || !isConventionalSkillPath(skillPath) || typeof stage !== "function") {
    throw new TypeError("Invalid repository staging request");
  }
  if (cache.has(skillPath)) {
    return { session: cache.get(skillPath), reused: true };
  }
  const session = await stage();
  cache.set(skillPath, session);
  return { session, reused: false };
}

export function removeRepositorySession(cache, skillPath) {
  if (!(cache instanceof Map)) return null;
  const session = cache.get(skillPath) ?? null;
  cache.delete(skillPath);
  return session;
}

export function drainRepositorySessions(cache) {
  if (!(cache instanceof Map)) return [];
  const sessions = [...cache.values()];
  cache.clear();
  return sessions;
}

export function isConventionalSkillPath(path) {
  if (path === "") return true;
  const parts = path.split("/");
  return parts[0] === "skills"
    && ((parts.length === 2 && isSafePathAtom(parts[1]))
      || (parts.length === 3
        && isSafePathAtom(parts[1])
        && isSafePathAtom(parts[2])));
}

export function filterUninstalledRepositoryCandidates(listing, installedSkills) {
  const repository = normalizeRepository(listing?.repository);
  const candidates = Array.isArray(listing?.candidates) ? listing.candidates : [];
  if (!repository) return { candidates: [...candidates], installedCount: 0 };

  const installedPaths = new Set();
  for (const skill of Array.isArray(installedSkills) ? installedSkills : []) {
    const acquisition = skill?.acquisition;
    if (acquisition?.kind !== "github"
      || !["recorded", "confirmed"].includes(acquisition.confidence)
      || normalizeRepository(acquisition.repository) !== repository
      || typeof acquisition.skillPath !== "string") {
      continue;
    }
    installedPaths.add(acquisition.skillPath);
  }

  const available = candidates.filter((candidate) => !installedPaths.has(candidate.skillPath));
  return {
    candidates: available,
    installedCount: candidates.length - available.length
  };
}

function isPublicGithubRepositoryUrl(value) {
  try {
    const url = new URL(value);
    const segments = url.pathname.split("/").filter(Boolean).map(decodeURIComponent);
    const repositoryRoot = segments.length === 2;
    const explicitRefRoot = segments.length === 4
      && segments[2] === "tree"
      && isSafeGithubRef(segments[3]);
    return url.protocol === "https:"
      && url.hostname === "github.com"
      && !url.port
      && !url.username
      && !url.password
      && !url.search
      && !url.hash
      && (repositoryRoot || explicitRefRoot)
      && isGithubAtom(segments[0])
      && isGithubAtom(segments[1].replace(/\.git$/, ""));
  } catch {
    return false;
  }
}

function isGithubAtom(value) {
  return Boolean(value)
    && value.length <= 100
    && value !== "."
    && value !== ".."
    && /^[A-Za-z0-9._-]+$/.test(value);
}

function normalizeRepository(value) {
  return typeof value === "string"
    ? value.trim().replace(/\.git$/i, "").toLocaleLowerCase()
    : "";
}

function isSafePathAtom(value) {
  return Boolean(value)
    && value !== "."
    && value !== ".."
    && !/[\\/\0]/.test(value);
}

function isSafeGithubRef(value) {
  return Boolean(value)
    && value.length <= 255
    && !value.includes("\0")
    && value.split("/").every(isSafePathAtom);
}
