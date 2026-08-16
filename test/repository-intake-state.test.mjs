import test from "node:test";
import assert from "node:assert/strict";
import {
  adjacentRepositoryQueuePosition,
  clearRepositoryReviewQueue,
  createRepositoryReviewQueue,
  createRepositorySessionCache,
  currentRepositoryQueuePath,
  drainRepositorySessions,
  filterUninstalledRepositoryCandidates,
  getOrStageRepositorySession,
  persistRepositoryReviewQueue,
  removeCurrentRepositoryQueuePath,
  removeRepositorySession,
  restoreRepositoryReviewQueue,
  setRepositoryQueuePosition
} from "../src/repository-intake-state.js";

function storage() {
  const values = new Map();
  return {
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
    removeItem: (key) => values.delete(key)
  };
}

test("repository review queues persist only public revision metadata", () => {
  const queue = createRepositoryReviewQueue({
    sourceUrl: "https://github.com/owner/repo",
    requestedRef: "main",
    resolvedSha: "a".repeat(40),
    selectedPaths: ["", "skills/research"],
    currentPosition: 1,
    sessionId: "must-not-persist"
  });
  const store = storage();

  assert.ok(queue);
  assert.equal(persistRepositoryReviewQueue(store, queue), true);
  assert.deepEqual(restoreRepositoryReviewQueue(store), queue);
  assert.equal(store.getItem("agent-skill-studio.repository-intake-v1").includes("sessionId"), false);
  assert.equal(currentRepositoryQueuePath(queue), "skills/research");
});

test("repository review queues reject unsafe paths and bad persisted state", () => {
  const store = storage();
  assert.equal(createRepositoryReviewQueue({
    sourceUrl: "https://github.com/owner/repo",
    requestedRef: "main",
    resolvedSha: "a".repeat(40),
    selectedPaths: ["skills/../escape"]
  }), null);

  store.setItem("agent-skill-studio.repository-intake-v1", JSON.stringify({ selectedPaths: [] }));
  assert.equal(restoreRepositoryReviewQueue(store), null);
  assert.equal(store.getItem("agent-skill-studio.repository-intake-v1"), null);
  clearRepositoryReviewQueue(store);
});

test("installing one queue entry never removes another selected path", () => {
  const queue = createRepositoryReviewQueue({
    sourceUrl: "https://github.com/owner/repo",
    requestedRef: "main",
    resolvedSha: "a".repeat(40),
    selectedPaths: ["", "skills/research", "skills/writing"],
    currentPosition: 1
  });
  const remaining = removeCurrentRepositoryQueuePath(queue);

  assert.deepEqual(remaining.selectedPaths, ["", "skills/writing"]);
  assert.equal(remaining.currentPosition, 1);
  assert.equal(currentRepositoryQueuePath(remaining), "skills/writing");
});

test("repository queues support explicit refs and selecting another review item", () => {
  const queue = createRepositoryReviewQueue({
    sourceUrl: "https://github.com/owner/repo/tree/main",
    requestedRef: "main",
    resolvedSha: "a".repeat(40),
    selectedPaths: ["", "skills/research"]
  });

  assert.ok(queue);
  const selected = setRepositoryQueuePosition(queue, 1);
  assert.equal(currentRepositoryQueuePath(selected), "skills/research");
  assert.equal(setRepositoryQueuePosition(queue, 2), null);
  assert.equal(adjacentRepositoryQueuePosition(queue, 1), 1);
  assert.equal(adjacentRepositoryQueuePosition(queue, -1), null);
  assert.equal(adjacentRepositoryQueuePosition(selected, -1), 0);
  assert.equal(adjacentRepositoryQueuePosition(selected, 1), null);
});

test("repository queues preserve safe non-ASCII Skill directory names", () => {
  const queue = createRepositoryReviewQueue({
    sourceUrl: "https://github.com/owner/repo",
    requestedRef: "main",
    resolvedSha: "a".repeat(40),
    selectedPaths: ["skills/科研/论文 助手"]
  });

  assert.ok(queue);
  assert.equal(currentRepositoryQueuePath(queue), "skills/科研/论文 助手");
});

test("repository queues preserve default and encoded explicit refs containing slashes", () => {
  const defaultRef = createRepositoryReviewQueue({
    sourceUrl: "https://github.com/owner/repo",
    requestedRef: "feature/intake",
    resolvedSha: "a".repeat(40),
    selectedPaths: ["skills/research"]
  });
  const explicitRef = createRepositoryReviewQueue({
    sourceUrl: "https://github.com/owner/repo/tree/feature%2Fintake",
    requestedRef: "feature/intake",
    resolvedSha: "b".repeat(40),
    selectedPaths: ["skills/research"]
  });

  assert.ok(defaultRef);
  assert.ok(explicitRef);
});

test("repository staging sessions are reused in memory while queue metadata stays public", async () => {
  const cache = createRepositorySessionCache();
  let stagingCalls = 0;
  const stage = async (path) => {
    stagingCalls += 1;
    return { path, sessionId: `session-${stagingCalls}`, candidateHash: `hash-${stagingCalls}` };
  };

  const first = await getOrStageRepositorySession(
    cache,
    "skills/research",
    () => stage("skills/research")
  );
  await getOrStageRepositorySession(cache, "skills/writing", () => stage("skills/writing"));
  const revisited = await getOrStageRepositorySession(
    cache,
    "skills/research",
    () => stage("skills/research")
  );

  assert.equal(stagingCalls, 2);
  assert.equal(first.reused, false);
  assert.equal(revisited.reused, true);
  assert.equal(revisited.session, first.session);

  const queue = createRepositoryReviewQueue({
    sourceUrl: "https://github.com/owner/repo",
    requestedRef: "main",
    resolvedSha: "a".repeat(40),
    selectedPaths: ["skills/research", "skills/writing"]
  });
  const store = storage();
  persistRepositoryReviewQueue(store, queue);
  assert.equal(store.getItem("agent-skill-studio.repository-intake-v1").includes("session-"), false);
});

test("repository staging sessions are removed per item or drained for a new intake", async () => {
  const cache = createRepositorySessionCache();
  const research = { sessionId: "research-session" };
  const writing = { sessionId: "writing-session" };
  await getOrStageRepositorySession(cache, "skills/research", async () => research);
  await getOrStageRepositorySession(cache, "skills/writing", async () => writing);

  assert.equal(removeRepositorySession(cache, "skills/research"), research);
  assert.deepEqual(drainRepositorySessions(cache), [writing]);
  assert.equal(cache.size, 0);
});

test("restoring a queue after restart restores metadata but stages a fresh session", async () => {
  const queue = createRepositoryReviewQueue({
    sourceUrl: "https://github.com/owner/repo",
    requestedRef: "main",
    resolvedSha: "a".repeat(40),
    selectedPaths: ["skills/research"]
  });
  const store = storage();
  persistRepositoryReviewQueue(store, queue);

  const restored = restoreRepositoryReviewQueue(store);
  const restartedCache = createRepositorySessionCache();
  let stagingCalls = 0;
  await getOrStageRepositorySession(
    restartedCache,
    currentRepositoryQueuePath(restored),
    async () => {
      stagingCalls += 1;
      return { sessionId: "fresh-session" };
    }
  );

  assert.equal(stagingCalls, 1);
});

test("repository listings hide only candidates with matching installed provenance", () => {
  const listing = {
    repository: "MattPocock/skills",
    candidates: [
      { skillPath: "", directoryName: "root" },
      { skillPath: "skills/engineering/review", directoryName: "review" },
      { skillPath: "skills/writing/draft", directoryName: "draft" },
      { skillPath: "skills/research/search", directoryName: "search" }
    ]
  };
  const result = filterUninstalledRepositoryCandidates(listing, [
    {
      name: "different-local-name",
      source: "personal",
      acquisition: {
        kind: "github",
        confidence: "recorded",
        repository: "mattpocock/skills.git",
        skillPath: "skills/engineering/review"
      }
    },
    {
      source: "archive",
      acquisition: {
        kind: "github",
        confidence: "confirmed",
        repository: "mattpocock/skills",
        skillPath: "skills/writing/draft"
      }
    },
    {
      name: "search",
      directoryName: "search",
      acquisition: { kind: "unknown", confidence: "unknown" }
    },
    {
      acquisition: {
        kind: "github",
        confidence: "recorded",
        repository: "someone/else",
        skillPath: "skills/research/search"
      }
    }
  ]);

  assert.equal(result.installedCount, 2);
  assert.deepEqual(
    result.candidates.map((candidate) => candidate.skillPath),
    ["", "skills/research/search"]
  );
});

test("repository listing provenance requires an exact Skill path", () => {
  const listing = {
    repository: "owner/repo",
    candidates: [{ skillPath: "skills/demo", directoryName: "demo" }]
  };
  const result = filterUninstalledRepositoryCandidates(listing, [
    {
      directoryName: "demo",
      acquisition: {
        kind: "github",
        confidence: "confirmed",
        repository: "owner/repo",
        skillPath: null
      }
    }
  ]);

  assert.equal(result.installedCount, 0);
  assert.equal(result.candidates.length, 1);
});
