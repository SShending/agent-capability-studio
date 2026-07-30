import test from "node:test";
import assert from "node:assert/strict";
import { parseSkillDocument, updateSkillDocument } from "../src/skill-document.js";

const markdown = `---
name: demo
description: >-
  Use when the user asks for a demo.
---

# Demo

Opening text.

## Workflow

1. Read the request.
2. Return the result.

### Guardrails

\`\`\`md
# This is code, not a heading
\`\`\`
`;

test("derives nested sections and ignores headings inside fenced code", () => {
  const document = parseSkillDocument(markdown);
  assert.equal(document.name, "demo");
  assert.deepEqual(document.sections.map(({ level, title }) => [level, title]), [
    [1, "Demo"],
    [2, "Workflow"],
    [3, "Guardrails"]
  ]);
  assert.match(document.sections[2].content, /# This is code, not a heading/);
});

test("updates one section without changing its neighbors", () => {
  const beforeWorkflow = parseSkillDocument(markdown).sections[0].content;
  const updated = updateSkillDocument(markdown, {
    type: "section-content",
    index: 1,
    value: "1. Inspect the input.\n2. Return the evidence."
  });
  const document = parseSkillDocument(updated);
  assert.equal(document.sections[0].content, beforeWorkflow);
  assert.equal(document.sections[1].content, "1. Inspect the input.\n2. Return the evidence.");
  assert.match(updated, /# This is code, not a heading/);
});

test("updates a heading through the same document interface", () => {
  const updated = updateSkillDocument(markdown, { type: "section-title", index: 1, value: "Runbook" });
  assert.match(updated, /## Runbook/);
  assert.doesNotMatch(updated, /## Workflow/);
});

test("updates the frontmatter name without changing the document body", () => {
  const updated = updateSkillDocument(markdown, { type: "name", value: "project-plan" });
  assert.equal(parseSkillDocument(updated).name, "project-plan");
  assert.match(updated, /description: >-/);
  assert.match(updated, /## Workflow/);
  assert.doesNotMatch(updated, /name: demo/);
});

test("keeps heading-free documents available as a body fallback", () => {
  const source = "---\nname: plain\ndescription: Plain Skill.\n---\n\nDo this carefully.\n";
  const document = parseSkillDocument(source);
  assert.equal(document.sections.length, 1);
  assert.equal(document.sections[0].kind, "preamble");
  const updated = updateSkillDocument(source, { type: "body", value: "Do that carefully." });
  assert.match(updated, /Do that carefully\./);
});
