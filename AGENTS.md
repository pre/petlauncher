# petlauncher

- petlauncher is an application launcher, calculator and snippet manager for GNOME.
- Keep `README.md`, the READMEs of the directories and `docs/` accurate when the behaviour or the commands change.
- Use conventional commits. Types: `feat`, `fix`, `docs`, `chore`. After the type, start with a capitalized verb in present tense (`feat: Remove X from Y`, `fix: Prevent X from doing Y`). Title at most 72 characters, body lines at most 80. A commit that changes only tests is `chore`. For a breaking change, use `feat!` or `fix!` and start the body with `BREAKING CHANGE: <what breaks and why>`.

## Architecture decision records

`docs/adr/` records the decisions that later agents must know, with the reason for each. Read the ADRs before you change the architecture.

Write an ADR only when the decision has all three properties:

1. It is hard to reverse.
2. A reader of the code finds it surprising without the context.
3. It is the result of a real trade-off between alternatives.

A decision that a later change can easily reverse does not get an ADR.

### Format

File name: `docs/adr/NNNN-slug.md`. `NNNN` is the highest number in `docs/adr/` plus one. The first ADR is `0001`.

```md
# Short title of the decision

Status: accepted.

One to three sentences: the context, the decision and the reason.
```

Add these sections only when they give the reader information that the code does not give:

- `## Decision`: a list, when the decision has several parts.
- `## Considered Options`: the rejected alternatives and the reason for each rejection.
- `## Consequences`: effects of the decision that are not obvious.

### Change a decision

Write a new ADR. Do not change the decision text of the old ADR. Write the relation in the `Status` line of the two ADRs. In these examples, ADR 0013 is the new ADR and ADR 0009 is the old ADR:

- New ADR: `Status: accepted. Supersedes ADR 0009 for the storage form.` Use `Extends` or `Changes` when the old decision stays partly true.
- Old ADR: `Status: superseded by ADR 0013 for the storage form.` Or, when the old decision stays true: `Status: accepted. ADR 0013 changes the storage form.`

When a README or code comment depends on a decision, refer to it as `ADR NNNN`.
