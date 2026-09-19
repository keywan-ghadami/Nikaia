# The Nikaia Specification

What the language **is**. Normative: if the compiler disagrees with a rule
here, one of the two is a bug.

| | covers |
| :--- | :--- |
| [Part I — The Language Core](10-nikaia-light.md) | the language a person learns first: values, ownership, errors, the two build switches |
| [Part II — Concurrency and Metaprogramming](20-nikaia-advance.md) | concurrency, parallelism, macros, the grammar protocol |
| [Part III — Tooling](30-nikaia-tooling.md) | the CLI, the manifest, the contract ledger, `std`, the diagnostics contract |

The three parts are written to one **language standard**,
[`STYLE.md`](STYLE.md): a section states the rule, then its consequence, then
at most one short *Design rationale* that ends in the record holding the
argument. **Why** the language is this way lives next door, one record per
decision: [the ADRs](adr/README.md). The specification does not argue and does
not remember; it points.

A rule the compiler does not fully carry out yet is followed by a status note
in one form, `> **Implementation status:** <value>.`, with one of six values —
Implemented, Partially implemented, Not implemented, Reserved, Withdrawn,
Unspecified — and one to three factual sentences after it: what is built, which
diagnostic fires today, and the record whose §5 carries the work. **Unspecified**
marks a construct the page shows and no record decides
([ADR-141](adr/adr-141.md) D3); no page carries it today, since `select`, the
channel and the duration were decided ([ADR-148](adr/adr-148.md),
[ADR-149](adr/adr-149.md), [ADR-150](adr/adr-150.md)).

`scripts/check-spec-blocks.py` guards an editorial pass: every ```` ```nika ````
block of the three parts is a test, and the script refuses a pass that changed
one.

For what belongs in this directory and what belongs in the notes, see
[`docs/README.md`](../README.md).
