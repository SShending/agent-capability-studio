# Vibe Coding Pitfalls

## Surface Reuse Without Workflow Equivalence

### Symptom

An existing screen is reused because its data and controls look similar to a
new feature, but the result feels dense, confusing, or focused on the wrong
information.

### Why AI-assisted development is vulnerable

An implementation agent can easily see that a working screen already renders
the same entities and actions. Reusing it produces a visible result quickly,
while the mismatch between the two user workflows is harder to detect from code
alone. This optimizes for implementation similarity instead of task clarity.

### Example from this project

The GitHub update workflow initially reused Candidate Review. Candidate Review
answers whether unfamiliar content should be installed, so compatibility and
audit evidence dominate its hierarchy. Update Review instead needs to answer:

1. What changed?
2. Is the evidence consistent with a local edit or a remote update?
3. What can the user synchronize without overwriting work?

Sharing candidate acquisition and comparison modules was valid. Sharing the
complete information architecture was not.

### Guardrail

Before reusing a complete screen or dialog, compare these four properties:

- the question the user is trying to answer;
- the decision the user must make;
- the primary action and its consequences;
- the content's trust state and required evidence.

Reuse the complete workflow only when all four match. Otherwise reuse the data
modules, visual primitives, and interaction patterns, then design a hierarchy
for the new task.

### Review prompt

Ask: "Are we reusing this because the user workflow is equivalent, or because
the existing code is convenient?"

The durable project rule is recorded in `AGENTS.md` under User Experience.
