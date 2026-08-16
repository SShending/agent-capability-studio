import test from "node:test";
import assert from "node:assert/strict";
import {
  buildPackageChangePresentation,
  deletePackageMutations,
  renamePackageMutations,
  singleSkillExportIssue
} from "../src/package-workflow-state.js";

const snapshot = [
  { path: "references/guide.md", kind: "file" },
  { path: "references", kind: "directory" }
];

test("editing then renaming applies the move before writing the new content", () => {
  const result = renamePackageMutations(
    [{ action: "write", path: "references/guide.md", content: "new" }],
    snapshot,
    { path: "references/guide.md" },
    "references/guide.md",
    "references/review.md"
  );
  assert.deepEqual(result, [
    { action: "move", path: "references/guide.md", destination: "references/review.md" },
    { action: "write", path: "references/review.md", content: "new" }
  ]);
});

test("consecutive renames collapse to one replayable move", () => {
  const result = renamePackageMutations(
    [
      { action: "move", path: "references/guide.md", destination: "references/review.md" },
      { action: "write", path: "references/review.md", content: "new" }
    ],
    snapshot,
    { path: "references/review.md", originalPath: "references/guide.md" },
    "references/review.md",
    "references/final.md"
  );
  assert.deepEqual(result, [
    { action: "move", path: "references/guide.md", destination: "references/final.md" },
    { action: "write", path: "references/final.md", content: "new" }
  ]);
});

test("deleting a renamed existing file removes the original instead", () => {
  const result = deletePackageMutations(
    [
      { action: "move", path: "references/guide.md", destination: "references/review.md" },
      { action: "write", path: "references/review.md", content: "new" }
    ],
    snapshot,
    { path: "references/review.md", originalPath: "references/guide.md" },
    "references/review.md"
  );
  assert.deepEqual(result, [{ action: "delete", path: "references/guide.md" }]);
});

test("deleting a newly created file cancels its pending write", () => {
  const result = deletePackageMutations(
    [{ action: "write", path: "assets/new.txt", content: "draft" }],
    snapshot,
    { path: "assets/new.txt" },
    "assets/new.txt"
  );
  assert.deepEqual(result, []);
});

test("presents bounded line changes for modified Package text", () => {
  const result = buildPackageChangePresentation({
    kind: "modified",
    path: "references/guide.md",
    beforeText: "alpha\nlocal\nomega\n",
    afterText: "alpha\nupdated\nomega\n"
  });

  assert.equal(result.hasTextDiff, true);
  assert.deepEqual(
    result.diff.rows.filter((row) => row.kind !== "context"),
    [
      { kind: "remove", oldLine: 2, newLine: null, text: "local" },
      { kind: "add", oldLine: null, newLine: 2, text: "updated" }
    ]
  );
});

test("keeps binary Package changes as metadata-only rows", () => {
  const result = buildPackageChangePresentation({
    kind: "modified",
    path: "assets/icon.png",
    beforeText: null,
    afterText: null
  });

  assert.equal(result.hasTextDiff, false);
  assert.equal(result.diff, null);
});

test("single-Skill export accepts only the requested Skill plan", () => {
  assert.equal(singleSkillExportIssue({
    canExport: true,
    blocked: [],
    skills: [{ id: "personal:demo" }]
  }, "personal:demo"), null);

  assert.deepEqual(singleSkillExportIssue({
    canExport: true,
    blocked: [],
    skills: [{ id: "personal:demo" }, { id: "personal:other" }]
  }, "personal:demo"), { ruleId: "default", relativePath: null });

  const blocked = { ruleId: "credential-path", relativePath: ".env" };
  assert.equal(singleSkillExportIssue({ canExport: false, blocked: [blocked], skills: [] }, "personal:demo"), blocked);
});
