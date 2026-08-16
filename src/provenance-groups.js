export function skillProvenanceGroup(skill) {
  const acquisition = skill?.acquisition;
  if (acquisition?.kind === "github" && acquisition.repository) {
    return {
      key: `github:${acquisition.repository.toLocaleLowerCase()}`,
      kind: "github",
      value: acquisition.repository
    };
  }
  if (acquisition?.kind === "local") {
    return { key: "local", kind: "local", value: null };
  }
  return { key: "unknown", kind: "unknown", value: null };
}

export function groupSkillsByProvenance(skills) {
  const groups = new Map();
  for (const skill of skills) {
    const group = skillProvenanceGroup(skill);
    if (!groups.has(group.key)) groups.set(group.key, { ...group, skills: [] });
    groups.get(group.key).skills.push(skill);
  }
  return [...groups.values()].sort((left, right) => {
    const rank = { github: 0, local: 1, unknown: 2 };
    return rank[left.kind] - rank[right.kind]
      || (left.value || "").localeCompare(right.value || "");
  });
}
