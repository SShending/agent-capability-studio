const countedSources = ["personal", "disabled", "system", "plugin", "archive"];

export function personalSkillsNeedingAttention(skills) {
  return skills
    .filter((skill) => skill.source === "personal" && skill.hasBlockingFindings)
    .sort((left, right) => left.displayName.localeCompare(right.displayName));
}

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

export function addCatalogSkill(skills, counts, skill) {
  if (skills.some((item) => item.id === skill.id)) {
    return { skills, counts: { ...counts } };
  }
  return {
    skills: [...skills, skill],
    counts: adjustCounts(counts, skill, 1)
  };
}

export function applyInstallOutcome(skills, counts, outcome) {
  if (!outcome?.skill) return { skills, counts: { ...counts } };
  return outcome.priorSkillId
    ? replaceCatalogSkill(skills, counts, outcome.priorSkillId, outcome.skill)
    : addCatalogSkill(skills, counts, outcome.skill);
}

export function removeCatalogSkill(skills, counts, id) {
  const removed = skills.find((skill) => skill.id === id);
  return {
    skills: skills.filter((skill) => skill.id !== id),
    counts: removed ? adjustCounts(counts, removed, -1) : { ...counts }
  };
}
