const countedSources = ["personal", "disabled", "system", "plugin", "archive"];

function adjustCounts(counts, skill, direction) {
  const next = { ...counts };
  next.total = Math.max(0, (next.total || 0) + direction);
  if (countedSources.includes(skill.source)) {
    next[skill.source] = Math.max(0, (next[skill.source] || 0) + direction);
  }
  if (skill.source === "personal" && skill.hasBlockingFindings) {
    next.needsAttention = Math.max(0, (next.needsAttention || 0) + direction);
  }
  return next;
}

export function replaceCatalogSkill(skills, counts, previousId, nextSkill) {
  const previous = skills.find((skill) => skill.id === previousId);
  let nextCounts = previous ? adjustCounts(counts, previous, -1) : { ...counts };
  nextCounts = adjustCounts(nextCounts, nextSkill, 1);
  return {
    skills: [...skills.filter((skill) => skill.id !== previousId), nextSkill],
    counts: nextCounts
  };
}

export function removeCatalogSkill(skills, counts, id) {
  const removed = skills.find((skill) => skill.id === id);
  return {
    skills: skills.filter((skill) => skill.id !== id),
    counts: removed ? adjustCounts(counts, removed, -1) : { ...counts }
  };
}
