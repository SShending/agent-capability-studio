import test from "node:test";
import assert from "node:assert/strict";
import { removeCatalogSkill, replaceCatalogSkill } from "../src/catalog-state.js";

const personal = {
  id: "personal-id",
  source: "personal",
  hasBlockingFindings: true
};

const counts = {
  total: 2,
  personal: 1,
  disabled: 0,
  system: 0,
  plugin: 1,
  archive: 0,
  needsAttention: 1
};

test("replaces a moved Skill and updates source and attention counts", () => {
  const archived = {
    ...personal,
    id: "archive-id",
    source: "archive",
    hasBlockingFindings: true
  };
  const result = replaceCatalogSkill(
    [personal, { id: "plugin-id", source: "plugin" }],
    counts,
    personal.id,
    archived
  );
  assert.deepEqual(result.skills.map((skill) => skill.id), ["plugin-id", "archive-id"]);
  assert.deepEqual(result.counts, {
    ...counts,
    personal: 0,
    archive: 1,
    needsAttention: 0
  });
});

test("removes a deleted Skill without allowing negative counters", () => {
  const archived = { id: "archive-id", source: "archive", hasBlockingFindings: false };
  const result = removeCatalogSkill(
    [archived],
    { total: 1, archive: 1, needsAttention: 0 },
    archived.id
  );
  assert.deepEqual(result.skills, []);
  assert.deepEqual(result.counts, { total: 0, archive: 0, needsAttention: 0 });
});

test("unknown removals leave the catalog unchanged", () => {
  const result = removeCatalogSkill([personal], counts, "missing");
  assert.deepEqual(result.skills, [personal]);
  assert.deepEqual(result.counts, counts);
});
