# EDR requirements

EDR's informal specification: the behavior EDR guarantees, written as individual requirements. Every PR that touches behavior lists the requirements it adds, amends, removes, fixes or validates, and edits the files here when behavior or a guarantee changes, or when it documents existing behavior.

The requirements are not exhaustive: most of EDR's behavior isn't documented here yet, and requirements are added as PRs touch the code. A missing requirement doesn't mean the behavior is unspecified. Treat existing behavior and tests as the reference. When you rely on behavior that has no requirement, add one if you've verified it; otherwise mention the gap in the PR description.

## Layout

One file per user-facing surface or cross-cutting concept. Paths mirror how users reach the behavior:

```
requirements/
├── README.md                  # this file: format and conventions
├── concepts/<concept>.md      # behavior shared by several surfaces
└── rpc/<method>.md            # one file per JSON-RPC method
```

Add new top-level folders as needed (for example `solidity-tests/`, `napi/`, `chains/op/`). Prefer extending an existing file over creating a new one; split a file when it no longer reads as one topic.

## File format

```markdown
---
area: rpc # folder name
surface: eth_estimateGas # method, config option, cheatcode, … ; omit for concepts
---

# <surface or concept name>

One paragraph of context: what this is, from a user's point of view.

## <requirement-id>

<The requirement: one or two sentences using MUST / MUST NOT / SHOULD, or a short table when it maps cases to outcomes.>

- **Applies to:** <chains, hardforks, block tags, config: whatever narrows it; "all" if nothing>
- **Rationale:** <why, if not obvious>
```

### Requirement IDs

- Format: `<surface-or-concept>.<kebab-case-slug>`, e.g. `eth_estimateGas.minimum-follows-block-hardfork`.
- IDs are stable: don't rename them when wording changes. Slugs instead of numbers so concurrent PRs don't collide.
- PRs reference requirements by ID in their description (see below).
- To remove a requirement, delete it. The git history keeps the old wording. First search for tests tagged with its ID. If they still pass, check whether the requirement really goes away or the tests were tagged with the wrong ID. Then delete the tests that only checked the removed requirement, and retag the others.
- Cross-references point one way only: a requirement names the IDs it depends on in its text (for example a method requirement naming a concept requirement). Concepts don't list the requirements that use them; search for the ID instead.

## Writing requirements

A requirement states behavior that users can observe through EDR's interface, not how the code produces it. If a refactor that keeps behavior would force you to edit it, it's too specific.

- State the rule, not an example. Leave out measurements, specific bugs and history; they belong in the PR description, which git blame leads to.
- Keep **Rationale** to one line on what goes wrong if the rule is broken, or omit it when that's obvious.
- Don't describe algorithms or name internal functions or types.
- Scope a requirement to what you verified. Name the methods, chains or conditions you checked in the code or tests; don't write "all methods" or "any request" unless you checked every one.
- When you suspect a requirement applies more widely but haven't checked, say so in the PR description instead of widening the requirement.
- A narrow requirement that is correct beats a broad one that is wrong: readers rely on requirements instead of reading the code, so an overclaim misleads them.

## Tests

A test that checks a requirement names the requirement's ID in a doc comment, so searching for an ID finds its tests. A test can check several requirements; give each its own `Requirement:` line. A requirement with no matching test is untested. Check concept requirements through the method requirements that use them, and tag a test with a concept ID only when the concept is what the test checks.

```rust
/// Requirement: `eth_estimateGas.minimum-follows-block-hardfork`
#[test]
fn estimate_at_historical_block_follows_block_hardfork() {}
```

## In PR descriptions

In PRs that touch behavior, add a **Requirements** section that lists each affected ID with one of: `added`, `amended`, `removed`, `fixed` (code now meets an existing requirement), `validated` (a test was added for an existing requirement). If no requirement is affected, write `none`.
