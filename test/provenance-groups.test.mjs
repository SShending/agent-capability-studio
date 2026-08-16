import test from "node:test";
import assert from "node:assert/strict";
import { groupSkillsByProvenance, skillProvenanceGroup } from "../src/provenance-groups.js";

test("groups exact GitHub repositories across catalog states and sorts unknown last", () => {
  const groups = groupSkillsByProvenance([
    { name: "unknown" },
    { name: "two", source: "disabled", acquisition: { kind: "github", repository: "zeta/tools" } },
    { name: "local", acquisition: { kind: "local", selectedPath: "/tmp/local" } },
    { name: "one", source: "personal", acquisition: { kind: "github", repository: "alpha/skills" } },
    { name: "three", source: "archive", acquisition: { kind: "github", repository: "alpha/skills" } }
  ]);

  assert.deepEqual(groups.map((group) => [group.kind, group.value, group.skills.length]), [
    ["github", "alpha/skills", 2],
    ["github", "zeta/tools", 1],
    ["local", null, 1],
    ["unknown", null, 1]
  ]);
});

test("does not infer GitHub provenance from names or descriptions", () => {
  assert.deepEqual(
    skillProvenanceGroup({
      name: "github-owner-repo",
      description: "Installed from https://github.com/owner/repo"
    }),
    { key: "unknown", kind: "unknown", value: null }
  );
});

test("groups confirmed and exact records from the same repository together", () => {
  const groups = groupSkillsByProvenance([
    {
      name: "exact",
      acquisition: { kind: "github", confidence: "recorded", repository: "owner/repo" }
    },
    {
      name: "legacy",
      acquisition: { kind: "github", confidence: "confirmed", repository: "owner/repo" }
    }
  ]);

  assert.equal(groups.length, 1);
  assert.deepEqual(groups[0].skills.map((skill) => skill.name), ["exact", "legacy"]);
});

test("Collection results use the same provenance groups for mixed managed sources", () => {
  const groups = groupSkillsByProvenance([
    {
      id: "personal",
      source: "personal",
      acquisition: { kind: "github", repository: "owner/repo" }
    },
    { id: "plugin", source: "plugin", acquisition: { kind: "unknown" } },
    { id: "system", source: "system", acquisition: { kind: "local" } }
  ]);

  assert.deepEqual(groups.map((group) => group.key), ["github:owner/repo", "local", "unknown"]);
  assert.deepEqual(groups.flatMap((group) => group.skills.map((skill) => skill.id)), [
    "personal",
    "system",
    "plugin"
  ]);
});
