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

For what belongs in this directory and what belongs in the notes, see
[`docs/README.md`](../README.md).
