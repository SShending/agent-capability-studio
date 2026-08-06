import test from "node:test";
import assert from "node:assert/strict";
import {
  beginExportOperation,
  beginImportPreview,
  clearImportReview,
  createBundleWorkflowState,
  finishExportCommit,
  invalidateExportOperations,
  isCurrentExportOperation,
  isCurrentImportPreview,
  setImportReview
} from "../src/bundle-workflow-state.js";

test("stale export operations cannot publish into a newer dialog state", () => {
  const workflow = createBundleWorkflowState();
  const preview = beginExportOperation(workflow, "preview", "skill-a");
  const commit = beginExportOperation(workflow, "commit", "plan-a");

  assert.equal(isCurrentExportOperation(workflow, preview), false);
  assert.equal(isCurrentExportOperation(workflow, commit), true);

  invalidateExportOperations(workflow);
  assert.equal(isCurrentExportOperation(workflow, commit), false);
  assert.equal(finishExportCommit(workflow, commit), false);
});

test("import file previews remain bound to their staged session and revision", () => {
  const workflow = createBundleWorkflowState();
  setImportReview(workflow, { sessionId: "bundle-a", bundleRevision: "revision-a" });
  const previewA = beginImportPreview(workflow);
  assert.equal(isCurrentImportPreview(workflow, previewA), true);

  clearImportReview(workflow);
  setImportReview(workflow, { sessionId: "bundle-b", bundleRevision: "revision-b" });
  const previewB = beginImportPreview(workflow);

  assert.equal(isCurrentImportPreview(workflow, previewA), false);
  assert.equal(isCurrentImportPreview(workflow, previewB), true);
});
