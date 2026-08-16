import { createLineDiff } from "./line-diff.js";

function within(path, parent) {
  return path === parent || path.startsWith(`${parent}/`);
}

export function buildPackageChangePresentation(change) {
  const hasTextDiff = typeof change.beforeText === "string"
    && typeof change.afterText === "string";
  return {
    ...change,
    hasTextDiff,
    diff: hasTextDiff ? createLineDiff(change.beforeText, change.afterText) : null
  };
}

export function singleSkillExportIssue(plan, skillId) {
  if (plan?.blocked?.length) return plan.blocked[0];
  if (plan?.canExport && plan.skills?.length === 1 && plan.skills[0].id === skillId) return null;
  return { ruleId: "default", relativePath: null };
}

function remap(path, from, to) {
  return path === from ? to : `${to}${path.slice(from.length)}`;
}

export function renamePackageMutations(mutations, snapshotEntries, draftEntry, path, destination) {
  const next = mutations.map((item) => ({ ...item }));
  const affectedWrites = next.filter((item) => ["write", "copyFile"].includes(item.action) && within(item.path, path));
  let result = next.filter((item) => !affectedWrites.includes(item));
  const existedWhenOpened = snapshotEntries.some((entry) => entry.path === path);
  const priorMove = result.findLast?.((item) => item.action === "move" && item.destination === path)
    || [...result].reverse().find((item) => item.action === "move" && item.destination === path);

  if (priorMove) {
    priorMove.destination = destination;
  } else if (existedWhenOpened || draftEntry?.originalPath) {
    result.push({ action: "move", path, destination });
  } else {
    const createdDirectory = result.find((item) => item.action === "createDirectory" && item.path === path);
    if (createdDirectory) createdDirectory.path = destination;
  }

  result.push(...affectedWrites.map((item) => ({
    ...item,
    path: remap(item.path, path, destination)
  })));
  return result;
}

export function deletePackageMutations(mutations, snapshotEntries, draftEntry, path) {
  const originalPath = draftEntry?.originalPath || path;
  const existedWhenOpened = snapshotEntries.some((entry) => within(entry.path, originalPath));
  const result = mutations.filter((item) => {
    if (item.action === "move") {
      return !within(item.path, originalPath) && !within(item.destination, path);
    }
    return !within(item.path, path) && !within(item.path, originalPath);
  });
  if (existedWhenOpened) result.push({ action: "delete", path: originalPath });
  return result;
}
