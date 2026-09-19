# The Nikaia Specification

What the language **is**. Normative: if the compiler disagrees with a rule
here, one of the two is a bug.

| | covers |
| :--- | :--- |
| [Part I — The Language Core](10-nikaia-light.md) | the language a person learns first: values, ownership, errors, the two build switches |
| [Part II — Concurrency and Metaprogramming](20-nikaia-advance.md) | concurrency, parallelism, macros, the grammar protocol |
| [Part III — Tooling](30-nikaia-tooling.md) | the CLI, the manifest, the contract ledger, `std`, the diagnostics contract |

A section states the rule and, where the rule is surprising, why it is the rule
in a sentence or two — then links the decision. **Why** the language is this way
lives next door, one record per decision: [the ADRs](adr/README.md).

A rule specified ahead of the compiler carries a short **Status** note saying
what is built and what is not, because a reader cannot otherwise tell a plan
from a promise.

An example that is ahead of the *language* carries a different mark:

> **Unspecified.** This writes a construct the language does not have and no
> record decides. It is here for the shape of the example; the decision is
> named where it will be taken.

The two are not the same state ([ADR-141](adr/adr-141.md) D3). A **Status**
note stands under a rule that *is* decided — a record says what happens and the
compiler has not caught up. An **Unspecified** mark stands under a construct
nobody has decided at all, kept on the page because the shape it was written
for is worth keeping and removing it would lose the argument with it.

For what belongs in this directory and what belongs in the notes, see
[`docs/README.md`](../README.md).
