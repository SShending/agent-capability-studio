import test from "node:test";
import assert from "node:assert/strict";
import {
  addCatalogSkill,
  applyInstallOutcome,
  personalSkillsNeedingAttention,
  removeCatalogSkill,
  replaceCatalogSkill
} from "../src/catalog-state.js";

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

test("adds an installed Skill once and updates catalog counts", () => {
  const installed = { id: "new-id", source: "personal", hasBlockingFindings: false };
  const result = addCatalogSkill([personal], counts, installed);
  assert.deepEqual(result.skills.map((skill) => skill.id), ["personal-id", "new-id"]);
  assert.deepEqual(result.counts, { ...counts, total: 3, personal: 2 });

  const duplicate = addCatalogSkill(result.skills, result.counts, installed);
  assert.deepEqual(duplicate, result);
});

test("applies the flattened Skill detail returned by a Bundle install receipt", () => {
  const previous = {
    id: "personal:demo-old",
    source: "personal",
    displayName: "Old Demo",
    hasBlockingFindings: false
  };
  const installed = {
    id: "personal:demo-new",
    source: "personal",
    displayName: "Imported Demo",
    hasBlockingFindings: false
  };
  const result = applyInstallOutcome([previous], {
    total: 1,
    personal: 1,
    needsAttention: 0
  }, {
    priorSkillId: previous.id,
    skill: installed
  });

  assert.equal(result.skills.length, 1);
  assert.equal(result.skills[0].displayName, "Imported Demo");
  assert.equal(result.skills[0].id, "personal:demo-new");
  assert.equal(result.counts.personal, 1);
});

test("lists named personal Skills needing attention in display order", () => {
  const skills = [
    { id: "z", displayName: "Zulu", source: "personal", hasBlockingFindings: true },
    { id: "a", displayName: "Alpha", source: "personal", hasBlockingFindings: true },
    { id: "disabled", displayName: "Disabled", source: "disabled", hasBlockingFindings: true },
    { id: "clear", displayName: "Clear", source: "personal", hasBlockingFindings: false }
  ];
  assert.deepEqual(
    personalSkillsNeedingAttention(skills).map((skill) => skill.id),
    ["a", "z"]
  );
});
