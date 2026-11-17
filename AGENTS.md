# AGENTS.md

Instructions for AI coding agents working on EDR. Human contributors follow the same rules.

## Requirements

EDR's expected behavior is documented as individual requirements in [`requirements/`](requirements/README.md).

- Before changing behavior, read the requirements for the surface you touch (for example `requirements/rpc/<method>.md`) and the format in [`requirements/README.md`](requirements/README.md).
- Every PR that introduces, changes, fixes or validates behavior adds, amends or removes requirements in the same PR.
- When a test checks a requirement, name the requirement's ID in a `Requirement:` doc comment on the test (see [`requirements/README.md`](requirements/README.md)).
- Treat a conflict between the code and a requirement as a bug in one of them, and say which in the PR.

## Pull request descriptions

Every PR description has these two sections:

- **Requirements:** each affected requirement ID and its status (see [`requirements/README.md`](requirements/README.md)).
- **Human review:** the parts that need a human reviewer and why. If nothing does, say so; an AI-assisted review is then enough.

## Other conventions

- Changesets: [`.changeset/README.md`](.changeset/README.md).
- Commit messages and PR titles: Conventional Commits, see [`book/src/99_contributing.md`](book/src/99_contributing.md).
