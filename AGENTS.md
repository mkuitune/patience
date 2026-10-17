# Agent communication protocol

This file defines the baseline communication protocol for any agent working in
this repository. It applies regardless of model, editor, or agent framework.

## Shared principles

- Write for the reader's available context, not the agent's context. Include what
  the reader needs to understand a statement, decision, or action.
- Use plain, specific English and active voice. Name the actor when responsibility
  could be unclear.
- Keep one main idea per sentence and related ideas together.
- Define unfamiliar technical terms and expand unfamiliar acronyms on first use.
- State causes, effects, and tradeoffs directly.
- Distinguish verified facts, assumptions, and recommendations when the distinction
  affects the reader's understanding or decision.
- Be concise without removing necessary context or technical accuracy.
- Use a professional, neutral, natural tone. Omit hype, filler, and decorative
  jargon.
- Use sentence case for titles and headings. Use lists for steps or parallel items
  and prose when ideas depend on one another.

## Communicating with the user

- Lead with the answer, finding, or intended action.
- Give concise progress updates when there is a meaningful finding, change of
  direction, or unresolved issue. Avoid narrating routine operations.
- State assumptions that materially affect the work.
- Ask questions when the answer changes scope, correctness, or a consequential
  choice. Resolve routine choices using the available context.
- Explain consequential Rust choices and tradeoffs as they arise, supporting the
  user's goal of learning modern LLM-driven Rust development. Match the detail to
  the task; do not turn every change into a tutorial.
- When reporting completion, say what changed, how it was verified, and what
  material limitations remain. Do not claim verification that was not performed.
- Use enough structure for the task. Small tasks do not need a fixed report
  template.

## Writing documentation and comments

Before creating or editing documentation or code comments, read
[`doc/WRITING_STANDARD.md`](doc/WRITING_STANDARD.md) and follow its guidance. This
includes Rust documentation comments (`///` and `//!`). Load that standard when
the task involves writing or editing these materials; it is not required for
other tasks.
