# The Specification's Language Standard

This page is normative for the three parts of the specification. A sentence in
Part I, II or III that does not follow it is a defect of the page, as a rule
the compiler disagrees with is a defect of one of the two.

## 1. What the specification is, and what it is not

The specification states **the language contract**: what a program may write,
what it means, and what the compiler refuses, with the diagnostic it refuses
it with. Nothing else. Argument, history and implementation status live in
other files — the decision records ([the ADRs](adr/README.md)), the
[changelog](../../CHANGELOG.md) and the [roadmap](../project_status_and_roadmap.md)
— and the specification does not repeat them or point at them. Every sentence
a reader does not need to write a correct program costs every reader tokens and
time.

Two kinds of text are allowed on a page:

1. **The rule.** Indicative present, third person, one rule per sentence.
   *A configuration parameter has a default. A call that omits it takes the
   default.*
2. **The consequence.** What follows from the rule for a program, in the same
   voice. *A parameter without a default is positional.*

A page may end a rule with a pointed sentence that fixes it in memory, set as
a block quote after the rule it belongs to:

> The compiler may be concurrent even when the program is not.

## 2. What a page does not contain

* **References to decision records.** No `ADR-…` citation, link or number.
  A reference to another section of the specification (*Part I 6.6*) stays.
* **Rationale.** No *why*, no *design rationale*, no options weighed, no
  objections answered, no comparison with what another language or an
  alternative design would do — unless the comparison *is* the rule (*`..<`
  stops before its end*).
* **History.** Not how a rule came to be, not what an earlier draft or version
  said, not what was withdrawn or renamed, not which test caught what. A
  withdrawn form is refused by the compiler with a diagnostic that names the
  replacement; the page states the current form only.
* **Implementation status.** No *built*, *not yet implemented*, *today*,
  *currently* or version numbers. The page states the contract; the roadmap
  and the changelog say how much of it the compiler carries out.
* **The reader.** No *the reader will*, no *it is worth saying*, no *you* and
  *your* in a rule. A rule names its subject: *user code*, *a program*, *the
  caller*, *the compiler*, *the runtime*.
* **Compression.** One idea per sentence.
* **Capitals on common nouns.** *compiler*, *variable*, *runtime error*,
  *build*. Capitals are for names: Nikaia, Rust, WebAssembly, and for the
  language's own constructs written as they are written: `comptime`,
  `grammar`, `Shared`.

## 3. Reserved words

A word that is reserved and has no construct is listed as reserved, in one
sentence, with no status note.

## 4. Vocabulary

One word per thing, and the word is not varied.

| word | meaning |
| :--- | :--- |
| **source** | the `.nika` text a program is written in |
| **compiler** | the Nikaia compiler; the **backend** is the Rust toolchain it drives |
| **build** | one translation of a program, with its build options |
| **program** | the compiled result, or the source that becomes it where the difference does not matter |
| **runtime** | the code that runs beside user code in a program: the executor, the I/O worker, cleanup |
| **target** | the machine and whether foreign code may call in (`target`, `artifact`) |
| **build option** | a key under `[build]` in `nikaia.toml`; it decides what the compiler produces. Not *switch*, *setting*, *choice* or *configuration* |
| **runtime configuration** | a key in `nikaia-runtime.toml`; it decides how the program runs |
| **user code** | source the program's author wrote, as opposed to the runtime's and the standard library's |
| **standard library**, `std` | the Nikaia standard library |
| **ledger** | `nikaia.contracts` |
| **refused** | the compiler rejects the program; a refusal has a diagnostic code |
| **pause** | a suspension point of a function that is not `sync` |

## 5. Diagnostics

A refusal is stated with its code: *A call outside `unsafe { … }` is refused
with `NK1143`.* A page never quotes the backend's message where the compiler
has its own, and never describes a refusal without its code where one exists.

## 6. What does not change under this standard

Code blocks are the specification's examples and tests; an editorial pass
leaves the code of every ```` ```nika ```` block as it is and in its place.
Section numbers and headings are what the compiler's messages and other
documents cite and do not move.
