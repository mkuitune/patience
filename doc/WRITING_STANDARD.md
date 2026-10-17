# Writing standard

This standard applies to documentation and code comments in this repository. It
builds on the shared communication principles in [`AGENTS.md`](../AGENTS.md).

## Write for the intended reader

Assume the reader has the document or source file open but has not seen the agent's
conversation. Provide the context needed to understand the subject on the first
pass.

- Start with what the subject does and why it matters.
- Name the relevant component, constraint, or decision. Avoid references such as
  "the earlier approach" or "as we discussed" without an explicit reference.
- Make pronoun references clear.
- Keep technical terms when they are accurate; explain unfamiliar terms where
  readers first need them.
- Put procedural steps in execution order. Say what changes and why.
- Use a short example when an idea is abstract.
- Link to supporting context rather than copying it into every document.
- Include edge cases, limitations, and tradeoffs that affect use or maintenance.
  Make the scope of production-readiness claims explicit.
- If a detail is uncertain, state what is unknown and what would verify it.

Necessary context and technical accuracy take priority over brevity. These rules
aim for clear technical English; they do not imply formal ASD-STE100 compliance.

## Code comments

Ordinary code comments should mainly explain reasons and purpose. Use them for
constraints, invariants, and surprising choices that the code does not explain.
Do not narrate obvious operations. A short algorithm name or description is useful
when it helps the reader identify the approach.

For example, a comment such as this adds little:

```rust
// Retry the request.
```

When the underlying behavior is verified, explain the reason instead:

```rust
// The service may reject requests briefly after a leader change.
```

Do not invent a reason to justify existing code. Verify the reason or identify the
uncertainty. Update or remove comments when their explanation no longer applies.

## Rust API documentation

Rust documentation comments (`///` and `//!`) describe the contract readers need
to use an API. Describing observable behavior is appropriate here.

- Explain the item's purpose and relevant observable behavior.
- Describe inputs, results, and constraints when names and types are insufficient.
- Document errors and panic conditions where relevant.
- For unsafe APIs, state the caller's safety obligations in a `# Safety` section.
- Include short usage examples when they clarify the contract. Keep examples
  valid and check them with Rust documentation tests when applicable.
- Explain implementation details only when they affect correct use or explain an
  important constraint.

Choose sections according to the API. Do not add empty sections or repeat the
function signature in prose.

## Project documents

Give each document a clear purpose and enough context to stand alone for its
intended audience.

- Architecture documents explain component responsibilities, relationships,
  constraints, and tradeoffs.
- Decision records explain the problem, the decision, the reasons, and the
  consequences. Include alternatives when they explain a meaningful tradeoff.
- Instructions state prerequisites, ordered actions, and the expected result.
- Specifications distinguish requirements from proposals and unresolved questions.

Use sentence case for headings. Prefer concrete names and direct statements over
vague claims such as "robust," "simple," or "production-ready" without evidence.

## Review before finishing

Check silently before delivering documentation or comments:

1. Can the intended reader understand this without the conversation?
2. Are actors, names, pronouns, and technical terms clear?
3. Does each ordinary comment add information the code does not express?
4. Are claims supported and material uncertainties identified?
5. Do examples and links work, where applicable?
6. Can any words be removed without losing meaning or accuracy?
