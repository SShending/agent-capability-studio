export function createBundleWorkflowState() {
  return {
    exportGeneration: 0,
    exportCommit: null,
    importReview: null,
    importPreviewGeneration: 0
  };
}

export function invalidateExportOperations(workflow) {
  workflow.exportGeneration += 1;
}

export function beginExportOperation(workflow, kind, identity) {
  const operation = Object.freeze({
    generation: ++workflow.exportGeneration,
    kind,
    identity
  });
  if (kind === "commit") workflow.exportCommit = operation;
  return operation;
}

export function isCurrentExportOperation(workflow, operation) {
  return operation?.generation === workflow.exportGeneration
    && (operation.kind !== "commit" || workflow.exportCommit === operation);
}

export function finishExportCommit(workflow, operation) {
  if (!isCurrentExportOperation(workflow, operation)) return false;
  workflow.exportCommit = null;
  return true;
}

export function setImportReview(workflow, review) {
  workflow.importPreviewGeneration += 1;
  workflow.importReview = review;
}

export function clearImportReview(workflow) {
  workflow.importPreviewGeneration += 1;
  const review = workflow.importReview;
  workflow.importReview = null;
  return review;
}

export function beginImportPreview(workflow) {
  const review = workflow.importReview;
  if (!review) return null;
  return Object.freeze({
    generation: ++workflow.importPreviewGeneration,
    sessionId: review.sessionId,
    bundleRevision: review.bundleRevision
  });
}

export function isCurrentImportPreview(workflow, operation) {
  const review = workflow.importReview;
  return Boolean(review)
    && operation?.generation === workflow.importPreviewGeneration
    && operation.sessionId === review.sessionId
    && operation.bundleRevision === review.bundleRevision;
}
