# Nikaia Language Specification
**Part I: The Language Core**
**Version:** 0.0.74 (Draft)
**Date:** 2026-09-20

---

## Chapter 1: Introduction and Principles

### 1.1. What Nikaia is
Nikaia is a statically typed systems programming language designed around three
properties: predictable execution, memory safety, and concise source. The
language moves complexity out of user code and into the compiler, where it is
checked, inferred or generated deterministically.

Application code expresses program logic directly. Memory management,
concurrency constraints and representation details are enforced by the
compiler. The compiler translates Nikaia source to Rust and drives the Rust
toolchain, which is called the **backend** throughout this specification
([ADR-004](adr/adr-004.md) D1).

*Design rationale:* a rule the compiler carries out can be reviewed, rebuilt
and certified against this document; a rule left to convention cannot.

### 1.2. One language, and the build options
There is one Nikaia. The same source compiles for every target and under every
build option, and the resulting program prints the same bytes. A build option
decides **how** a program is built, never **what** it computes.

> What you choose when you build is how, never what.

The build options are named below. Each lives under `[build]` in
`nikaia.toml`; `--target` and `--user-parallelism` override the first two for a
single build ([ADR-037](adr/adr-037.md), [ADR-033](adr/adr-033.md) D8,
[ADR-039](adr/adr-039.md) D8). The specification does not state their number.
A former option, `ordering`, is withdrawn: statements run in the order they
are written, and a program that wants overlap writes `overlap { … }` (8.1.2,
[ADR-050](adr/adr-050.md) D7).

#### `target` — the machine
`target` names the machine a program is built for. The target decides what the
standard library offers on it (Part III 17.2) and what a `panic` does: the
stack unwinds where the machine unwinds, and the program traps where the
machine traps (Part III, Appendix A). The default is `x86_64-linux`.

#### `user_parallelism` — whether user code may run concurrently
`user_parallelism` is a permission, not a thread count.

* `no` (the default): no two pieces of user code are ever in flight at the
  same time. A data race in user code is impossible under this option, because
  nothing user code wrote runs concurrently.
* `yes`: user code may run concurrently. The compiler enforces the rules of
  Part II chapter 12 that keep shared data intact.

The option applies to user code only. The compiler and the runtime may use
additional threads for internal work, provided that no user code executes
concurrently on them. Reading a file may validate its text on four cores at
`user_parallelism = no`, and the runtime may hand a blocking call to a helper
thread; neither runs user code, and neither changes a byte of what the program
prints.

How many threads serve a `yes` is the runtime's decision, because that number
depends on the machine the program runs on.

*Design rationale:* `user_parallelism = no` guarantees sequential execution of
user code, not single-threaded execution of the compiler or the runtime. A
thread count in the source would be a promise the language cannot keep on
hardware it has not seen ([ADR-037](adr/adr-037.md) D2).

> The compiler may be concurrent even when the program is not.

#### The re-entrancy check — whether a broken rule is noticed
Taking a lock while a lock is held is refused when the program is compiled
(Part II 12.3). The re-entrancy check is a build option that decides whether
the program also carries the runtime check that notices such a nesting if one
occurs. It is on by default and may be declined.

For every program that obeys the nesting rule, both builds behave identically.
The check cannot fire in a correct compiler. If it fires, the compiler has a
defect; without the check, the same defect would appear as a silent hang
([ADR-039](adr/adr-039.md) D8, D2).

> **Implementation status:** Not implemented. Nothing refuses the nesting of
> Part II 12.3, no re-entrancy check is emitted, and `nikaia.toml` has no key
> for this option ([ADR-039](adr/adr-039.md) §4).

### 1.3. The prelude
Some names need no `use`. They are these, and there are no others
([ADR-154](adr/adr-154.md) D1):

* the containers a program cannot do without: **`Vec`**, **`String`**,
  **`Bytes`**;
* the printing functions: **`println`**, **`print`**, **`eprintln`**;
* **`assert`** and **`panic`**;
* the numeric conversions 2.2 already offers.

Everything else in the standard library is reached the way a package is
reached: `use std::fs` at the top of the file, and `fs::read_to_string(path)`
where it is used (9.1, [ADR-140](adr/adr-140.md) D5).

**The rule the list is built from** is that nothing in it does I/O but
printing, and nothing in it pauses (D2). Reading a file is a thing a program
should say it does, and `use std::fs` is that sentence. It is also what makes
the list safe on a machine with no filesystem: the list does not have to be cut
down for one (Part III 17.2).

**`HashMap` is the first thing outside it** (D3), and it is the name that makes
this a rule rather than a tidy-up: a map is common enough to argue for, and
`use std::collections` is one line, and a list that grows by *common enough*
has no floor. So `collections::HashMap` where it is used, which is 9.1's shape
one library over.

**A name joins the list by a record and never by being needed once** (D4).
Additive is the easy direction and it is the one that ends with everything in
the list; a name in it has to be argued for, with the rule above as the bar. A
prelude is also very hard to take back: every name in it is a name some program
writes without importing, and removing one breaks that program. So the size of
the list matters more than its contents.

*Design rationale:* a reader finds out what needs no `use` by reading this page
rather than a file of the compiler's.

> **Implementation status:** Partially implemented. The rule is enforced for a
> **function** and for a **type**: a name that lives in a `std` module is
> refused without its prefix — `NK1117` for a function, `NK1135` for a type —
> and a prefix is refused without its `use`, each with the line to add.
> **`Bytes` exists and is the language's** ([ADR-156](adr/adr-156.md) D1): one
> shared buffer, written bare, and what `fs::read` hands back (D3). The
> **tether** it is the container for is still not built, so a view of a buffer
> a body owns is refused rather than compiled (`NK2303`, D4).
> **`assert` and `panic` are named by the list and do not exist**: they belong
> with the testing chapter (Part III 14), which is not built either, and
> `docs/open-work.md` carries them.

---

## Chapter 2: Variables and Data Types

A **comment** begins with `//` and runs to the end of the line, or begins with
`/*` and runs to the matching `*/`. A block comment may span lines and may
stand anywhere whitespace may stand. Block comments **nest**: `/* a /* b */ c */`
is one comment, so a block that already holds one can be commented out
([ADR-134](adr/adr-134.md)). An unclosed `/*` is reported where it opened. A
block comment is a comment wherever it stands, including `/** … */`.

**A run of `///` lines immediately before an item is that item's
documentation** ([ADR-139](adr/adr-139.md)). An item is a `fn`, a `struct`, an
`enum`, a `trait`, a field or a variant. Anywhere else `///` is an ordinary
comment. An ordinary comment standing between the run and the item does not end
it; a statement does. A doc comment is prose: the compiler reads no directive,
no `@param` and no link out of it. On a `pub` item the doc comment becomes the
`doc` column of `nikaia.contracts` (Part III, 13.5), derived there like every
other column.

> **Implementation status:** Partially implemented. A `fn`, a method, a trait
> method, a `struct`, an `enum` and a `trait` carry theirs, and a `pub` one
> reaches the ledger's `doc` column ([ADR-139](adr/adr-139.md) §5). A **field**
> and a **variant** do not: nothing reads them until `nikaia doc` exists, which
> is that record's §4.

### 2.1. Variables and Assignment
A **variable** is a named storage location that holds a value. A variable is
declared with `let`.

**Immutability**
A variable is **immutable** unless it is declared otherwise. Once a value is
assigned to an immutable name, the name cannot be assigned again.

```nika
let x = 10
// x = 20  <-- This would cause a Compiler Error
```

**Mutability**
A variable that may change is declared **mutable** with the keyword `mut`.

```nika
let mut y = 10
y = 20     // This is allowed
```

**Reserved words**

A reserved word means one thing wherever it appears. A name may not be a
reserved word ([ADR-051](adr/adr-051.md) D1). The reserved words are:

```text
as        break     catch     comptime  continue  dsl       else      enum
extern    false     fn        for       grammar   if        impl      in
let       match     mut       null      overlap   pub       return    self
spawn     struct    sync      throw     throws    trait     true      unsafe
use       while     with
```

**`comptime` is reserved for its construct**: a statement inside a function
body (Part II, 10.2).

*Design rationale:* the declaration promises a **time** of evaluation, not
constancy; constancy is what every binding without `mut` already has (2.1)
([ADR-077](adr/adr-077.md)).

**`_` is not a name; it is the ignore pattern** ([ADR-126](adr/adr-126.md)). It
stands where a name would be bound and says that the value is ignored on
purpose: a position of a destructured tuple (`let (name, _) = pair()`), a
parameter a shape dictates (`fn handle(event: Event, _: Context)`, `fn(_,
value) { … }`), and a `match` arm. `let _ = expr` is refused: a call made for
its effect is written as the call, and a resource is closed by name. `_` is
never a value. What `_` ignores is not moved, so a `let` over a place stays a
view of it (6.5).

**`with` is reserved for its construct**: a copy of a value with named fields
changed, `p with { x: 1 }` (4.2, [ADR-118](adr/adr-118.md)).

**`extern` and `unsafe` are reserved for their constructs**: an `extern "C"`
block, and the `unsafe { … }` a call into one is written in
([ADR-124](adr/adr-124.md), Part III 15.1).

**`break` and `continue` are reserved for their constructs** (3.3,
[ADR-084](adr/adr-084.md)).

**`loop`, `const`, `macro`, `quote` and `from` are ordinary names**
([ADR-116](adr/adr-116.md), [ADR-117](adr/adr-117.md)). A declared `loop`,
`quote` or `from` is a name like any other. Where nothing declares one of them,
the refusal every undeclared name meets carries a hint: `loop { … }` is
answered with *write `while true`*, `const X = …` with *write `comptime`*, and
`macro` and `quote` with *Nikaia has no macros*.

**`seq` is withdrawn** ([ADR-050](adr/adr-050.md) D7). The word is an ordinary
name.

Three further rules about the list:

* **A `grammar` block has its own vocabulary**, and it is not on the list:
  `rule`, `boundary`, `fold`, `par_fold` and `unchecked` are keywords *inside*
  a `grammar` (Part II, 10.1) and ordinary names everywhere else (D2).
* **After a `::` or a `.`, a reserved word is a name.** A segment follows a `::`
  and a member follows a `.`, and no construct begins in either position, so
  `Self::dsl` (Part II, 10.5) and `scope.spawn fn { … }` (Part II, 12.5) are
  what they look like (D3).
* **`self` is on the list and is also a name**: the receiver of a method.
  A method body uses it; user code may not declare it (D4).

> **Implementation status:** Implemented. A name that is a reserved word does
> not parse, and the parse error names the word and says it is reserved.
> Declaring `self` at a `let`, a `for` binding, a lambda's argument or a struct
> field is refused with `NK1119`; a parameter named `self` does not parse, and
> the message says so ([ADR-051](adr/adr-051.md) D4).

### 2.2. Primitive Data Types
Nikaia provides basic types to represent simple values.

* **Integers:** whole numbers without fractions.
    * `i64`: a 64-bit integer. It is the type of most numbers, and the type
      of a **length**: `xs.len()` hands back an `i64`, and an index is one
      ([ADR-048](adr/adr-048.md) D1).
    * `i32`: a 32-bit integer. It is written where the layout matters: a
      struct that has to be small, a wire format, a C header. An `i32` that
      meets an `i64` is widened where the program says so, with `as i64`.
    * `u8`: one byte. Reading a file hands back a list of them
      (`fs::read` → `Vec[u8]`). It has the same conversion and arithmetic
      names as the other integer types.
* **Floats:** numbers with decimal points.
    * `f64`: a double-precision floating-point number. A literal may carry an
      **exponent**: `1.5e-4`, `2e3`, `9.54791938424326609e-04`.
* **Booleans:** logic values.
    * `bool`: `true` or `false`.
* **Text:**
    * `String`: text. Whether a value of it is a view into text that is
      already there, or text of its own, is the compiler's decision per use
      (6.6, [ADR-107](adr/adr-107.md)). A literal is a view of the program's
      own text and allocates nothing.
    * `&str`: the same text with a promise attached: *this is a borrowed
      view, no copy and no handle*. The compiler holds the program to the
      promise. It is written where allocating would be a mistake.
    * `char`: one character, a Unicode scalar value, not a byte. It is
      written between single quotes, with the escapes a string uses: `'a'`,
      `'\n'`, `'\''`. Iterating a text yields it (`for c in name.chars()`),
      and a `match` over one compares against it (3.4). A text is not a list
      of `char`; turning one into the other is written in the program.

**A number may carry a digit separator or a radix prefix, and nothing else**
([ADR-136](adr/adr-136.md)). `1_000_000`, `0xFF`, `0b1010` and `0o17` are the
four forms, beside the exponent above. An underscore stands **between** digits
and nowhere else. A float takes the separator (`1_000.5`) and no prefix. A
digit the radix does not have (`0b1210`, `0o19`) is refused. A **leading**
underscore is not one of the four forms: `_000` is a name.

**The radix is a spelling, and a literal is a value.** `0xFF` is `255`, and it
takes the first type that holds it exactly as `255` does
([ADR-060](adr/adr-060.md)). So `let mask = 0xFF` is an `i32` and
`let mask: u8 = 0xFF` is a `u8`. The width is never read off the digits. The
separator is not part of the value: `1_000` is `1000` to the checker, to the
ledger and to a diagnostic's text.

*Design rationale:* a width read off the digits would make `0x0FF` a wider type
than `0xFF`, so the type of a number would depend on how many zeroes were typed
([ADR-136](adr/adr-136.md)).

**There is no type suffix.** `1i64` is a number beside a name, and the name
beside it is refused with `NK1117` because nothing declares it (Part III, C.3).
The language has **no word it does not know**: a word that stands on its own is
read as a name, so `assert c` is refused the same way rather than being read as
a construct that is not there.

**A number too wide for an `i64` is refused** (Part III C.1).

> **Implementation status:** Implemented. All four forms, the float's separator
> and the exponent are built. An underscore that does not stand between digits,
> a digit the radix does not have, and a number too wide for an `i64` are
> refused ([ADR-136](adr/adr-136.md) §5). `NK1117`'s help does not name the
> `1_000` case, because it is a number.

**These four are the integer types a program writes.** The compiler accepts more
(`u32`, `u64` and the machine-width `usize` among them), and the specification
does not offer them ([ADR-028](adr/adr-028.md) D5). A **length** is an `i64`
([ADR-048](adr/adr-048.md) D1): `for i in 0..<xs.len()` gives an `i64`, `xs[i]`
takes one, and neither conversion is written, because the compiler emits both.
A negative index reports as an access out of bounds (Part III, A.2).

*Design rationale:* a type has an entry because a program asked for it, and no
program has asked for the wider unsigned types ([ADR-028](adr/adr-028.md) D5).

**An integer that does not fit aborts, at every build.** An `i32` holds what an
`i32` holds. An arithmetic result that does not fit is an inconsistent program
state, like an index past the end of a list or a division by zero (Part III,
A.2), and the program stops. The rule is the same at both values of
`user_parallelism` and in every build, so the same program computes the same
thing wherever it is built ([ADR-043](adr/adr-043.md) D1). That is 1.2's rule
applied to arithmetic.

Three cases abort although the code looks harmless:

* `-x` and `x.abs()` on the **smallest** value of a signed type, because there is
  no matching positive one;
* the smallest value divided by `-1`, for the same reason;
* shifting by more places than the type is wide.

**Where a program means to wrap or to stop at the limit, it says so by name.**

```nika
let h = h.wrapping_mul(31).wrapping_add(c)        // a hash, wrapping on purpose
let volume = level.saturating_add(increase)       // stops at the maximum
let n = a + b                                     // aborts if it does not fit
```

The names are `wrapping_add`, `wrapping_sub`, `wrapping_mul`, `wrapping_div`,
`wrapping_neg`, `wrapping_abs`, `wrapping_shl`, `wrapping_shr`, and the same
names with `saturating_`. There is no operator for either.

*Design rationale:* both are rare and both are meant to be visible in the line
that does them; a sign for one would have to be spelt with `&`, which is
borrowing here ([ADR-043](adr/adr-043.md) D2, D3).

**A conversion is written `as`, and one that may not fit aborts too.**

```nika
let average = (total as f64) / (count as f64)     // widening, always fits
let small = big as i32                            // aborts if `big` does not fit
```

A program that wants the digits thrown away says so, the same way wrapping is
said ([ADR-043](adr/adr-043.md) D4).

**Where a program means to keep only the low digits, it says so by name**, as
it does for wrapping. The name carries the type it converts to, because that is
what it hands back:

```nika
let small = big.truncating_i32()                  // keep the low digits, on purpose
let floored = measurement.truncating_i32()        // a float, toward zero, clamped
let n = text.len().truncating_i32()               // a count that may not fit
```

The names are `truncating_i32` and `truncating_i64`. Each converts out of the
larger integer, out of an `f64`, and out of the type a count has
([ADR-043](adr/adr-043.md) D7).

Three conversions are checked or unchecked in a way the code does not show:

* **An `f64` to an integer** aborts where the value does not fit: `1e20 as i32`,
  `-1e20 as i32`, and a value that is not a number all abort.
* **A count**, what `len` hands back, is as wide as the machine is, so a value
  that fits on a large machine may not fit on a small one. The conversion is
  checked, so the program's behaviour does not depend on where it was built.
* **An integer to an `f64`** is **not** checked. Digits are lost at large values
  without anything overflowing: `9007199254740993` through an `f64` comes back
  `9007199254740992`. This is a limit of the language, not an abort.

**An `as` names one of the types above and nothing else.** `n as u128` and
`n as usize` are refused with `NK1122`, naming the type
([ADR-054](adr/adr-054.md) D1).

*Design rationale:* a conversion into a type this page does not offer would
give a value a type the language has no word for, and `-3 as usize` would be
18,446,744,073,709,551,613 under a rule that says a conversion which does not
fit aborts ([ADR-054](adr/adr-054.md) D1).

**Where the backend wants a machine-width number, the compiler writes the
conversion.** A length comes back as an `i64` and an index goes in as one
([ADR-048](adr/adr-048.md) D1). A **count** goes in as one too, so
`"  ".repeat(indent)` is written with no conversion. A negative count aborts
with *"a count cannot be negative"* ([ADR-054](adr/adr-054.md) D2).

**A literal that does not fit its type is a compile error, not an abort.**
`let x: i32 = 3000000000` is decidable where it is written, and so is a sum of
literals that cannot fit, so neither waits for the program to run
([ADR-043](adr/adr-043.md) D5).

The compile-time check reaches a **sum**. `let b = a + 1`, where `a` is a
constant an annotation declared an `i32`, is folded and refused with no
annotation on the `let` line: the operand's declaration gives the arithmetic
its type. `+ - * / %` and a negation fold, through any number of immutable
`let`s, in a wider number than either type, so that the message can name what
the expression comes to. A `mut` local, a parameter, a `for` binding and a cast
stop the fold. An expression that does not fold is never refused.

A division whose divisor is a constant zero is refused with `NK1118`
([ADR-043](adr/adr-043.md) D5.5). Every other division by zero is
unrecoverable at run time and names its line (Part III A.2).

A literal that nothing constrains, `let big = 3000000000` on its own, is not
this section's case and is not refused: it takes the first type that holds it
([ADR-060](adr/adr-060.md), 2.4).

> **Implementation status:** Implemented. The abort is built: the generated
> project carries the check for user code and turns it off for every Rust
> dependency ([ADR-043](adr/adr-043.md) D6). The `wrapping_` and `saturating_`
> names are built for `i32` and `i64`; `saturating_shl` and `saturating_shr` do
> not exist, in Nikaia or in the backend. An out-of-range constant is refused
> with `NK1116` wherever a type stands beside it: an annotated `let`, a `return`
> against a declared result, or an argument whose parameter says what it takes.
> The narrowing check is built and does not depend on a build option: a
> conversion carries its own check, so `5000000000 as i32` aborts in every
> build, and `big.truncating_i32()` is a plain `as` in the backend. The
> `truncating_` names exist for the three sources above and for `i32` and `i64`
> as destinations.

### 2.3. Nullable Types (Null Safety)
A type is **non-nullable** unless it says otherwise. A variable of type
`String` always holds a string and is never `null`. A type that admits the
absence of a value is written with a trailing question mark `?`.

```nika
let strictly_string: String = "Hello".to_string()
// strictly_string = null // Error!

let mut maybe_string: &str? = null // Valid
maybe_string = "World"             // Valid (`mut`, as in 2.1)
```

A literal is a **view** of text the program was compiled with (6.6,
[ADR-024](adr/adr-024.md) D5). An allocation happens only where the program
writes one. A literal stands wherever a `String` is wanted, because a `String`
may be a view ([ADR-107](adr/adr-107.md)); `.to_owned()` makes a copy.

> **Implementation status:** Not implemented. `String` and `&str` are two types
> in the checker today, and a literal in a `String` slot is refused with
> `NK1106` ([ADR-107](adr/adr-107.md) §5).

**`T?` lowers to the backend's `Option<T>`**, the mapping Part III 15.2 writes
the other way round, and `null` is a reserved word (2.1) that lowers to `None`
([ADR-052](adr/adr-052.md)). The `?` comes last, after the type's arguments:
`Vec[i64]?` is a nullable list and `Vec[i64?]` is a list of nullables. `(A, B)?`
is not written.

**A plain value stands where a nullable one is wanted.** That is the one
widening the language has, and the compiler writes the constructor. There is no
`Some` in Nikaia. The constructor is written wherever a plain value meets a
nullable slot: an annotated `let`, an assignment, a `return`, a struct-literal
field, and a call argument. Where the compiler cannot work out the value's type,
it writes a conversion instead, which is right whether or not the value is
already nullable ([ADR-068](adr/adr-068.md)).

`let m = null` with nothing beside it is not refused by the compiler:
`let mut m = null` followed by `m = "hi"` is a correct program. Where nothing
ever says the type, the backend asks for the annotation.

> **Implementation status:** Implemented. `T?`, `null`, the constructor at all
> five positions and the conversion of [ADR-068](adr/adr-068.md) are built. The
> `??` and `?.` of 3.5 are built beside them ([ADR-052](adr/adr-052.md) §5).

### 2.4. Type Inference
Nikaia is **statically typed**: the type of every variable is known at compile
time. A type is rarely written. The compiler uses **type inference** to deduce
the type from the value.

```nika
let name = "Nikaia"  // Compiler knows this is a &str - a view of static text
let count = 42       // an i32, because nothing here asks for anything else
```

**A number takes the type its use asks for. Where nothing asks, it takes the
first type that holds it** — `i32`, and `i64` where an `i32` is too small
([ADR-060](adr/adr-060.md)):

```nika
let count = 42          // an i32: nothing asks, and an i32 holds it
let big = 3000000000    // an i64: nothing asks, and an i32 does not
let m = 3000000000      // also an i64 - here because the line below asks
println(f"{wide(m)}")   // fn wide(n: i64) -> i64
let small = 42
println(f"{wide(small)}")  // an i64 here: the use decides, and 42 holds in one
```

The use is asked first and the size second. So `small` may still become an
`i64`, and `big` never has to be annotated to be one. A number too large for an
`i64` is refused, because nothing holds it.

**A sum of numbers is a number** ([ADR-063](adr/adr-063.md)), so the same rule
decides it:

```nika
let c = 2000000000 + 2000000000   // an i64: nothing asks, and an i32 does not hold it
let small = 2 + 3                 // an i32, the same as any number that fits
```

**A name is where the widening stops.** A name already took a type, and
arithmetic happens in the type of its operands:

```nika
let a = 2000000000        // an i32 - the first type that holds it
let c = a + a             // refused: this comes to 4000000000, which an i32 does not hold
let b: i64 = 2000000000   // say so, and the sum is an i64
let d = b + b             // 4000000000
```

The way out is the annotation on the third line.

The question is about the **value**, not the digits: `-2147483648` is an `i32`.
`NK1116` answers the case where a type **stands beside** the literal (an
annotated `let`, a `return` against a declared result, an argument whose
parameter says what it takes) and stays silent otherwise, because a literal
whose use widens it is a correct program (Part III, C.4). A constant is widened
where an `i64` holds it and refused with `NK1116` where no type does or where a
name pinned a narrower one. Two positions take their type from **where they
stand** and are left alone: a sequence index and a repeat count, both counted
by the machine.

> **Implementation status:** Implemented. The *use* half is the backend's
> inference; the size half is one rule in the emitter, which writes a literal an
> `i32` cannot hold as an `i64` and leaves a literal that fits as it was
> ([ADR-060](adr/adr-060.md) §5). The sum is built ([ADR-063](adr/adr-063.md)
> §5).

### 2.5. Strings, Plain and Interpolated
There are two string literals, and the difference is one character at the front.

```nika
let name = "Nikaia"

println(f"hello, {name} - {name.len()} characters")   // f"…" - the braces are code
println("hello, {name}")                              // "…"  - the braces are braces
```

**`"…"` is text.** A `{` in it is a brace and nothing else, so a program that
writes JSON, CSS or a regular expression writes the braces as they are:

```nika
print("{}")               // prints {}
print("\\d{3}")            // a regular expression, written as one
print("{ margin: 0 }")    // a rule, not a hole
```

**`f"…"` has code in it.** Between `{` and `}` stands an expression: a name, a
field, a call, an index. What follows a `:` inside a hole says *how* to write
the value rather than which value. The first colon that is not inside a call or
an index separates the two, so `move(by: 1)` in a hole keeps its own colon.

Inside an `f"…"`, two braces stand for one: `{{` is a literal `{` and `}}` a
literal `}`. An escape is not a hole: the `{` in `"\u{0041}"` belongs to the
escape. A `}` on its own is an error. **A plain string needs none of that**: it
has no holes, so `"{"` is a brace and `"{{"` is two.

The `f` and the quote are **one token**. `f"x"` interpolates; `f "x"` is a
variable named `f` beside a string. Whitespace never decides what a program
means.

**A newline is an ordinary character in a literal.** Nothing ends a literal but
the closing `"`. A literal left unterminated runs on to the next `"` in the file
or, if there is none, to the end of the file.

**The type follows the syntax, not the contents.** `"…"` is a view of static
text and `f"…"` builds a `String`, whether or not it has a hole in it. Adding a
brace to a piece of text cannot change its type.

**A template's holes need no `f`.** `dsl html { <p>{name}</p> } eod` (Part II)
is already marked as a place where code appears, and the mark belongs on the
construct rather than on every brace inside it. A literal that holds code says
so.

---

## Chapter 3: Control Flow

Control flow is the order in which statements and calls are executed.

### 3.1. Expressions and Blocks
Nikaia is an **expression-oriented language**: almost every construct has a
value. A **block** is a group of statements surrounded by curly braces
`{ ... }`. The last line of a block is the value of that block.

```nika
let result = {
    let a = 5
    let b = 10
    a + b  // Returns 15
}
```

A block's last line is the **block's** value; `return` is the **function's**.
A `return` written at the end of a block that is itself a value (a `match` arm,
3.4; an `if` branch whose value is taken, 3.2; a `catch` handler, 7.1) leaves
the enclosing function rather than handing that block a value. The one block
that is its own function is a lambda, whose `return` leaves the lambda (5.3).

### 3.2. Conditional Logic (if / else)
The `if` expression tests a condition of type `bool`. Where the condition is
true it evaluates the first block; otherwise it evaluates the `else` block.
`if` is an expression, so its value may be bound to a variable.

```nika
let age = 18

let status = if age >= 18 {
    "Adult"
} else {
    "Minor"
}
```

After `else`, an `if` may stand where the block would stand: **`else if`**. A
chain is one `if` inside another with the inner braces left out, so every rule
of `if` holds at every link ([ADR-132](adr/adr-132.md)):

```nika
let grade = if score >= 90 {
    "A"
} else if score >= 80 {
    "B"
} else {
    "C"
}
```

**A condition is an ordinary expression.** Every expression the language has
may stand there, with the same operators, the same precedence and the same
associativity as anywhere else ([ADR-087](adr/adr-087.md) D1). `&&`, `||`, `!`,
`??`, `as`, `null`, a tuple and a range are all conditions.

**One rule concerns the brace.** The `{` after the condition opens the
**body**, so an expression that *starts* with a brace is parenthesised;
otherwise the compiler cannot tell where the condition ends:

```nika
if p == (P { x: 1 }) {          // the parentheses say which `{` is which
    …
}

if (match n { 1 => 10, else => 20 }) > 15 {
    …
}
```

That is the whole of the difference between the head of an `if`, a `while` or a
`for` and any other position ([ADR-087](adr/adr-087.md) D2). It is a rule about
spelling, not about meaning: every expression is reachable.

### 3.3. Loops
A loop repeats code.

**The `while` Loop**
A `while` loop repeats its body as long as its condition is true.

```nika
let mut count = 0
while count < 5 {
    println(f"Count is {count}")
    count += 1
}
```

**The `for` Loop**
A `for` loop iterates over a sequence, such as a range of numbers or a list.

```nika
// Iterates from 0 to 4: `..<` stops before its end.
for i in 0..<5 {
    println(f"Index: {i}")
}
```

**`a..b` includes its end and `a..<b` excludes it** ([ADR-137](adr/adr-137.md)
D4). The spelling is the same in a `for`, in a slice and in a pattern. The form
`a..=b` is withdrawn (D5). A range is an ordinary expression: it may be given a
name, passed, or indexed with. A range binds **looser than every operator in
it**, so `0..<n - 1` is a range ending below `n - 1`, not a range with
something subtracted from it.

*Design rationale:* one spelling per meaning; `..<` reads *up to, not
including*, as it does in Kotlin and in Swift ([ADR-137](adr/adr-137.md) D4).

> **Implementation status:** Implemented. `..<` parses and excludes its end,
> `..` includes it, and `..=` is refused naming the form that replaces it
> ([ADR-137](adr/adr-137.md) §5).

A `for` over a list **lends** it: the elements are looked at, and the list is
still there when the loop is over. Taking the elements away is written,
`for x in xs.drain()` ([ADR-094](adr/adr-094.md) D4; the rule it is part of is
6.5).

> **Implementation status:** Implemented. A `for` over a place lends it and
> `xs.drain()` takes the elements away. A `&` written in front of the list is
> refused with `NK1137`, because the compiler writes that reference
> ([ADR-094](adr/adr-094.md) D4). The half of the rule that is not built is in
> 6.5.

**Leaving a loop early: `break` and `continue`**

`break` leaves the loop. `continue` skips the rest of this turn and starts the
next one. Both act on the **innermost** loop around them
([ADR-084](adr/adr-084.md) D1):

```nika
let mut first_even = 0 - 1
for n in numbers {
    if n % 2 != 0 {
        continue                 // not this one; take the next
    }
    first_even = n
    break                        // found it; stop looking
}
println(f"{first_even}")         // the loop is over, the program is not
```

A `break` leaves the **loop** and a `return` leaves the **function**. After the
`break` above, the `println` runs.

Three rules govern them:

* **Neither takes a value.** A loop is a statement and hands back nothing, so
  `break` has nothing to carry out of one. `break n` is refused rather than
  read as a `break` followed by a statement `n` ([ADR-084](adr/adr-084.md) D3):

  ```
  error[NK1133]: nothing after a `break` in the same block is reached
  ```

* **There is no label.** `break` acts on the loop it is written in, and there
  is no way to name an outer one ([ADR-084](adr/adr-084.md) D2). A program that
  leaves two loops at once leaves the inner one and tests outside it, or
  `return`s where the function has nothing left to do.

* **A jump does not leave a function.** A `break` whose loop is outside a
  **lambda**, a **task** (`spawn`), an **`overlap` branch** or a DSL fold's step
  is refused ([ADR-084](adr/adr-084.md) D4). Each of those is a function of its
  own, and a jump is a jump to a place in the same function:

  ```nika
  for x in xs {
      let each = fn (n) {
          break                  // error[NK1132]: the nearest loop is
      }                          //               outside this lambda
  }
  ```

  Such a lambda hands back a `bool`, and the loop tests it. A `catch` handler
  is **not** a function of its own: a `break` in one leaves the loop around it
  ([ADR-084](adr/adr-084.md) D5):

  ```nika
  for p in paths {
      let text = fs::read_to_string(p) catch { break }
      seen += text.len()
  }
  ```

**A loop can fail.** Some sequences a `for` walks are read *as it goes*;
standard input's lines are the one `std` has today. Getting the next element
can fail. When it does, the loop stops and **the failure leaves the function**,
exactly as a failing call would (Chapter 7). The function declares it:

```nika
use std::io

fn tally() -> i64 throws {           // without `throws`: error[NK2701]
    let mut n = 0
    for line in io::lines() { n += 1 }
    return n
}
```

Nothing marks the loop, as nothing marks a call that can fail
([ADR-023](adr/adr-023.md) D8). The same rule applies at the *end* of a block,
where a resource's cleanup can fail and the function that owns it declares it
(6.4). It is one rule for the two places where the language performs a call
user code did not write ([ADR-025](adr/adr-025.md) D1).

*Design rationale:* a failed read that looked like the end of the input would
turn a truncated stream into a shorter one and a count into a quietly wrong
number ([ADR-025](adr/adr-025.md) D1).

Most loops cannot fail. A range, a list, a map: nothing is read, so nothing
about them changes.

**There is no third loop form.** A loop that does not end on its own is written
`while true { … }`, and it is left by a `break`:

```nika
let mut n = 0
while true {
    n += 1
    if n == 5 {
        break
    }
}
```

There is no `loop` keyword ([ADR-070](adr/adr-070.md) D1). `break` hands back
nothing ([ADR-084](adr/adr-084.md) D7).

*Design rationale:* one keyword is enough for every loop shape, and `while` is
the general one. A `break` that handed back a value would make `while true`
read as a lie, since the head says the loop does not end and the body would say
what it ends with ([ADR-070](adr/adr-070.md) D1, [ADR-084](adr/adr-084.md) D7).

### 3.4. Pattern Matching (`match`)
The `match` expression compares a value against a series of patterns. Every
possible case is handled.

```nika
let value = 2

match value {
    1 => println("One"),
    2 => println("Two"),
    else => println("Something else"), // `else` is the arm taken when nothing above matched
}
```

A pattern is one of six things, and each is read the way it is written:

| pattern | matches |
| :--- | :--- |
| `else` | anything, and binds nothing: the arm taken when none above it matched, the word `if` uses for the same idea ([ADR-145](adr/adr-145.md)). `_` in this position is refused: it is the **ignore pattern**, which stands in a tuple position and as a parameter ([ADR-126](adr/adr-126.md)), and nothing arrives at a catch-all arm |
| `1`, `"text"`, `true`, `'n'` | that value |
| `Op::Times` | that variant |
| `Message::Write(text)` | that variant, binding what it carries |
| `Message::Move { x, y }` | that variant, binding its fields by name |
| `other` | anything, and **binds it** to that name |

The last two rows are one rule: a path with `::` in it names a variant, and a
bare name binds. That is the line the enum's own syntax draws: `Quit` is a
variant *of* `Message`, never on its own.

**Six more shapes** ([ADR-137](adr/adr-137.md) D1):

| pattern | matches |
| :--- | :--- |
| `(0, 0)`, `(0, y)` | a tuple, position by position |
| `(0, y) \| (y, 0)` | either alternative; every alternative binds the **same set of names** |
| `200..299` | a range, **inclusive at both ends** (D3). An exclusive one is written by moving the end; `..<` is never written in a pattern |
| `(x, y) if x == y` | a **guard**: the arm matches only where the condition holds, and the word is `if` (D2) |
| `Event::Click(Point { x, .. })` | a pattern inside a pattern, and `..` for the fields this one does not name |

**`..` means two different things**: in a range pattern it is the range, and
in a struct pattern it is *the rest of the fields*. A struct pattern has no
range in it and a range has no fields, so the position says which.

**Every case is covered** ([ADR-146](adr/adr-146.md) D1). A `match` over an
**enum** is complete when every variant is named, and needs no `else`. A
`match` over anything else needs an `else`, because the set of an `i64`, a
`char` or a `String` cannot be written out in arms. `bool` is the exception:
`true` and `false` are two arms and a complete `match`. An incomplete `match` is
refused with `NK1151`, which names what is missing: the variants, or `else`.

An arm's body is an expression or a block. The expression may be a `throw`, a
`return`, a `break` or a `continue` ([ADR-138](adr/adr-138.md) D1). Their type
is **never**, so an arm that throws sits beside an arm that hands back a value,
and the `match` is that value's type:

```nika
match step {
    (Op::Times, n)  => { value = value * n }
    (Op::Divide, n) => { value = value / n }
}
```

> **Implementation status:** Implemented. The six rows of the first table, the
> six shapes of the second and the completeness check are built
> ([ADR-137](adr/adr-137.md) §5). A guarded arm covers nothing, so a `match`
> whose only catch-all carries an `if` is still `NK1151`. Alternatives of an
> `|` pattern that bind different names are refused with `NK1155`. An arm's
> body may be a bare `throw`, `return`, `break` or `continue`
> ([ADR-138](adr/adr-138.md) §5).

### 3.5. Null Safety Operators
A member of a nullable type is reached through an operator that handles the
`null` case.

* **Safe navigation (`?.`):** reaches a member only where the receiver is not
  `null`. Where the receiver is `null`, the expression is `null`. A **field**
  and a **method** are both members, and a method call takes its arguments
  there as it does anywhere ([ADR-066](adr/adr-066.md) D1).
* **Null coalescing (`??`):** supplies a fallback value where an expression is
  `null`. A chain may be written: `a ?? b ?? c` takes the first that has a
  value (D4).

```nika
// If find_user returns null, 'name' becomes null.
let name = repo.find_user(id)?.full_name

// A method is a member, so it is reached the same way.
let greeting = repo.find_user(id)?.greet("Hallo")

// If 'name' is null, "Guest" is assigned.
let display_name = name ?? "Guest"

// And a fallback may have a fallback.
let shown = nickname ?? name ?? "Guest"
```

**The result of a `?.` is a `T?`.** Where the member is itself a `T?`, the
result is flattened: `a?.b?.c` never reaches through a nullable of a nullable.
Which case applies is a question about the declared type, and the compiler
decides it ([ADR-052](adr/adr-052.md) D6). That is what lets `??` end a chain
and `?.` continue one.

**`?.` through a type that cannot be absent is refused** with `NK1121`. A type
that is not `T?` always has a value, and the plain `.` reaches it.

**`?.` takes nothing.** It reaches through a view of its receiver, so `user` is
usable on the line after `user?.name`. What comes out is a copy where the
member copies and a view of the receiver otherwise, as a field read is (6.6)
([ADR-113](adr/adr-113.md)).

**`?.` reaches a method** ([ADR-066](adr/adr-066.md)). `find(1)?.greet("Hallo")`
calls the method only where there is something to call it on; the arguments
reach it, and the result is a `T?` like any other reach. It flattens where the
method's own result is already a `T?`. A method may **pause** and may **fail**,
and a `?.` on one carries both.

**A `?.` guards its own member and no more.** Where the receiver is absent, the
whole expression is `null` and what follows is never reached. A `.` written
*after* the reach is reaching into a `T?`, and is refused where it is written
with `NK1125` (D6): `T?` is a type of its own (2.3), and a member of `T` is not
a member of it. So `a?.b.c` is refused and `a?.b?.c` is the program.

> **Implementation status:** Partially implemented. `??` and its chain are
> built: `a ?? b` lowers to `a.unwrap_or_else(|| b.into())`
> ([ADR-052](adr/adr-052.md)). `?.` is built over a field and over a method,
> with `NK1121` and `NK1125`; over a method it lowers to a `match`, because a
> method may pause and may fail. The view of the receiver is not built: today
> the lowering takes the receiver, and using it again is refused by the backend
> with *"use of moved value"* ([ADR-113](adr/adr-113.md) §5).

---

## Chapter 4: Data Structures

### 4.1. Structs (Custom Data Types)
A **struct** groups related values under a single name.

**Visibility and Encapsulation**
**Everything is private unless it says `pub`.** That includes a struct and its
fields.
* A struct another package may use is marked `pub`.
* The fields of a public struct stay private unless each is marked `pub`.

```nika
// file: users.nika

// The Struct is public, but fields are private
pub struct User {
    username: String,
    email: String,
    is_active: bool,
}
```

### 4.2. Constructors and Instantiation
A struct literal `Type { field: value }` names fields, and a private field
cannot be named from another package. A type another package constructs
provides a public **constructor**.

**The Anonymous Constructor (`pub fn`)**
An `impl` block may declare one function with no name. That function is called
when the type name is invoked like a function: `User(...)`.

* **Inside the package:** code builds the value with the struct literal, because
  it may name the private fields.
* **Outside the package:** code calls the public anonymous constructor.

```nika
// file: users.nika
impl User {
    // The Constructor
    // It accepts positional arguments (Subject Zone)
    pub fn(username: String, email: String) -> User {
        // We can access private fields here because we are inside the module
        return User {
            username: username,
            email: email,
            is_active: true, // Default logic handled internally
        }
    }
}
```

**A struct literal is written with braces, and only with braces**
([ADR-140](adr/adr-140.md) D1). `User { username: name, email: address }` is the
literal. The form `User(username: name)` is withdrawn. `User(a, b)` is a
**call**, of the anonymous constructor or of anything else.

*Design rationale:* with one spelling per form, a colon does not decide
whether a type's invariants are gone through or around
([ADR-140](adr/adr-140.md) D1).

**A type is constructed by its anonymous constructor** ([ADR-140](adr/adr-140.md)
D2), in `std` as in a `.nika` file: `Vec()`, `String()`, `HashMap()`,
`Stats(first)`. A written `Type::new` is refused with `NK1149`, in a call and as
a value alike; the constructor handed over as a **value** is the same spelling,
`par_fold(M, Summary, …)`. The ledger writes `Type::new`, because that is the
name the **lowering** uses, and the lowering is name for name
([ADR-011](adr/adr-011.md) D2).

> **Implementation status:** Implemented. `Type(field: value)` does not parse as
> a literal; it is a call, and where the name is a type `NK1146` names the
> braces. `Vec()`, `String()` and `HashMap()` are the constructors, and a
> written `Type::new` is refused with `NK1149` ([ADR-140](adr/adr-140.md) §5).

**A copy with fields changed: `with`.** `with` writes a copy of a value with
named fields changed, without naming the rest ([ADR-118](adr/adr-118.md)):

```nika
let moved = p with { x: p.x + 1 }
let stats = old with { count: old.count + 1, sum: old.sum + t }
```

The braces are the struct literal's, with its field list and its shorthand.
`with` names top-level fields only: a field of a field is
`p with { pos: p.pos with { x: 1 } }`. The fields not named are **moved** from
`p`, never copied unseen. Where one of them is a text or a list and `p` is used
afterwards, the refusal names the copy to write. Across a package, `with` names
`pub` fields only, as a literal does.

> **Implementation status:** Not implemented ([ADR-118](adr/adr-118.md) §5).

**Usage Example**
Another package reaches `User` through its constructor.

```nika
// file: main.nika
use users::User

fn main() {
    // ERROR: Private Fields
    // Direct struct initialization is forbidden because fields are private.
    // let u = User { username: "A", email: "a@b.com", is_active: true }

    // OK: Public Factory Constructor
    // Calls the 'pub fn' defined in 'impl User'.
    // Note: Uses positional arguments as per Function Syntax.
    let u = User("Alice", "alice@example.com")
}
```

### 4.3. Why No Classes? (Data vs. Behavior)
Nikaia has no **classes**. Data and behavior are declared apart:
1.  A **struct** defines the **data**: what a value is.
2.  An **`impl` block** defines the **behavior**: what a value does.

```nika
// Defining behavior for the User struct
impl User {
    fn login(&self) {
        println(f"{self.username} logged in.")
    }
}
```

### 4.4. Enums (Algebraic Data Types)
An **enum** is a type whose value is one of several distinct variants.

```nika
enum Message {
    Quit,                        // carries nothing
    Move { x: i32, y: i32 },     // named fields, read by name
    Write(String),               // positional, read by position
}
```

An enum is the type for a value that is **one of a fixed set of things**: two
operators, four directions, the three states a connection can be in. A string
would carry the same value and check nothing: `"tiems"` is a value a string
type accepts and an enum does not. An enum is read back with `match` (3.4): an
arm per variant, and no arm for a case that cannot happen.

### 4.5. Collections
The standard library provides types for groups of values.

* **List (Vector):** an ordered sequence of elements.
    ```nika
    let numbers = [1, 2, 3, 4]
    ```
    A list keeps the order it was given, and that order can be changed:
    `xs.sort()` puts the elements in their natural order, and
    `xs.sort_by_key fn { … }` in the order of what the closure returns. **Both
    are stable**: elements the key does not separate keep the order they had.
    Two passes therefore express a compound order without a comparator:
    ```nika
    names.sort()                                  // by name
    names.sort_by_key fn (name) { -report[name].hits }   // then by hits, descending
    ```
    A map has no order to borrow, so a program that prints one says which.
* **Tuple:** a fixed number of values of *different* types, with no name for
    the group and no names for the parts. It is written and read by position:
    ```nika
    let pair = ("*", 3)          // (&str, i64)
    let op = pair.0
    ```
    A tuple is the type for values that belong together for one step of a
    computation: the element of a grammar rule that yields an operator and its
    operand, or the two halves of a map entry in `for (name, value) in map`.
    A group whose meaning outlives the step is a struct (4.1) with named
    fields.
* **Map (HashMap):** key-value pairs.
    ```nika
use std::collections

    let mut scores = collections::HashMap()
    scores["Player1"] = 100
    ```
    Reading a map through the brackets gives a `T?`, because a key is data and
    may be absent: `scores["Player1"] ?? 0`, or `scores[name]?.rank ?? 0`
    ([ADR-114](adr/adr-114.md)). `get` gives the same. A list's `xs[i]` stays a
    `T`, because an index is the program's own arithmetic and a wrong one is a
    bug (Part III, Appendix A).

    > **Implementation status:** Not implemented. Today a missing key aborts
    > ([ADR-114](adr/adr-114.md) §5).

    A map's hash function follows where its keys came from. Keys derived from
    data a remote peer supplied are hashed with a random per-run key, so no
    peer can choose keys that degrade the program. Keys from data the program
    supplied are hashed with the fast function. User code does not configure
    this; Part III, 17.1 names the cases where a program decides it. **The
    iteration order of a map is not guaranteed** and may differ between runs.

**A list is written `[1, 2, 3]`** ([ADR-135](adr/adr-135.md)). The elements are
expressions, a trailing comma is allowed, and the type is `Vec[T]` where `T` is
what the elements agree on. Elements that do not agree are refused with
`NK1154`. `[]` is the empty list and **takes its element type from the first
use that says one**: `let xs: Vec[i64] = []`, or a `push`. Where nothing ever
says the type, `[]` is refused with `NK1153`, asking for the type. A `[` at the
**start of a line** begins a literal and never an index of the line above it,
so an index is always written where its subject is. The list *type* is written
`Vec[T]`; there is no second spelling `[User]`.

A `[]` whose only uses cannot give it an element type (`xs.len()` and nothing
else) is not refused by the compiler, and the backend reports it
([Part III C.4](30-nikaia-tooling.md)).

**A fixed-size array is `Array[T, N]`** ([ADR-152](adr/adr-152.md)), in the
bracket generic every other parameterised type is written in — `N` is a number
rather than a type, and it is the one place a type argument is one. It is `N`
elements **inline**: in a struct it is part of the struct, as an argument it is
passed as a value, and nothing is allocated — which is what makes it the
container for a profile with no allocator. It is indexed as a list is, an index
out of range aborts (Appendix A), and `len()` is the `N` it was declared with,
known while the program is built.

```nika
struct Vector3 { parts: Array[f64, 3] }

let origin: Array[f64, 3] = [0.0, 0.0, 0.0]
```

The literal is the list literal: **it takes the array type where the use asks
for one**, and a literal with no use to constrain it is a `Vec`, exactly as `[]`
takes its element type from its use. The length is part of the type, so
`Array[f64, 3]` and `Array[f64, 4]` are different types and a literal whose
length does not match `N` is refused with `NK1157` naming both numbers.

> **Implementation status:** Implemented. A use is written in four positions —
> an annotated `let`, an argument, a declared result and a struct literal's
> field — and the answer descends with the literal, so `Vec[Array[f64, 2]]` and
> `Array[Array[i64, 2], 2]` take their shape too. A slice of an array, an
> integer parameter of anything else, and a default value for an unwritten
> element are all left open ([ADR-152](adr/adr-152.md) §4).

> **Implementation status:** Implemented. The literal, its trailing comma, the
> element type, `NK1154`, `NK1153` and the `[` that begins a line are built
> ([ADR-135](adr/adr-135.md) §5). The tuple, indexing (`xs[0]`), indexed
> assignment and `HashMap()` are built beside it. `Array[T, N]` is built; its
> own note stands with it above.

### 4.6. Generics (Type Parameters)
A **generic** declaration is written once for several types. A type parameter is
declared inside square brackets `[...]`.

```nika
struct Box[T] {
    item: T,
}

impl Box[T] {
    fn get(self) -> T {
        return self.item
    }
}

fn hand[T](x: T) -> T {
    return x
}
```

A type parameter is written, never inferred. **Inside the body it is a type**:
`x` is a `T`, and a `T` is not an `i64`, because the caller picks what `T` is.
**At the call it is filled in from the arguments**: `hand(n)` where `n` is an
`i64` hands back an `i64`, and `Box { item: n }` is a `Box[i64]`.

A `T` on its own has no members. Nothing has said which types `T` may be, so
nothing can say what one can do:

```nika
fn shout[T](x: T) -> String {
    return x.to_uppercase()   // error[NK1126]: `T` stands for a type the caller
}                             // picks, and nothing says it has a method
                              // `to_uppercase`
```

> **Implementation status:** Partially implemented. The parameter, the call that
> fills it in, and `NK1126` are built for a `fn`, a `struct` and the `impl` over
> it ([ADR-074](adr/adr-074.md) §5). A generic `enum` is not read. A bound,
> `[T: Summarize]`, is 4.7's subject.

### 4.7. Traits (Defining Behavior)
A **trait** declares a set of methods that different types can share.

```nika
trait Summarize {
    fn summary(&self) -> String
}

impl Summarize for User {
    fn summary(&self) -> String {
        return f"User: {self.username}"
    }
}
```

A trait's methods are **signatures** without a body. The declaration says what
a type must have, and the `impl` says what it does.

A type parameter may be **bound** by a trait. The bound is what gives a generic
body something it may do (4.6):

```nika
trait Summarize {
    fn summary(&self) -> String
}

fn shout[T: Summarize](x: T) -> String {
    return x.summary()     // `Summarize` says there is one
}
```

The bound answers the whole call: how many arguments `summary` takes, what
they have to be, what it hands back, and whether it can fail or pause all come
from the declaration. Several bounds are written `[T: Named + Aged]`. A bound
may take a path, `[H: http::Handler]`, and the ledger records a trait and each
`impl` where they were written ([ADR-106](adr/adr-106.md)).

**A trait method reads like any signature** ([ADR-109](adr/adr-109.md)). Without
`sync` it may pause; without `throws` it cannot fail. An implementation is
checked against the declaration: a body that pauses under a `sync` declaration
is refused, and a body that does less than the declaration allows is accepted.
A trait can therefore describe I/O, `fn load(&self) -> String throws`, and a
call through its bound pauses where the declaration says it may.

An `impl` owes its trait the declared methods and no others: a method the trait
does not declare, or one it declares that the `impl` leaves out, is refused
with `NK1130`. A method whose implementation **pauses** where the declaration
says `sync` is refused with `NK1129` ([ADR-080](adr/adr-080.md)). A trait
method has no **default body**, and a trait is not a type: there is no `dyn`
and no `fn f(x: Summarize)`.

> **Implementation status:** Partially implemented. The declaration,
> `[T: Bound]`, `[T: A + B]`, the lookup, `NK1130` and `NK1129` are built
> ([ADR-078](adr/adr-078.md), [ADR-080](adr/adr-080.md)). A method without
> `sync` is not yet lowered as one that may pause: the emitter writes a plain
> `fn` in every trait ([ADR-109](adr/adr-109.md) §5). A differing signature is
> not compared, so a method with the wrong arity or result is refused by the
> backend. A bound with a path and the ledger's record of traits are not built,
> so a trait cannot be named from another package
> ([ADR-106](adr/adr-106.md) §5).

---

## Chapter 5: Functions & Argument Architecture

### 5.1. The "Subject ; Config" Protocol
A function signature separates the data a function operates on (its subject)
from its configuration options with a **semicolon separator (`;`)**.

**Zone 1: Subject (Positional)**
Parameters *before* the semicolon are the data the function operates on.
* **Syntactic rule:** arguments here are positional.

**Zone 2: Configuration (Named Only)**
Parameters *after* the semicolon are options, flags, or modifiers.
* **Syntactic rule:** arguments here are named. A positional argument in this
  zone is refused.

```nika
// Definition
fn request(url: &str; timeout: i32 = 30, method: &str = "GET") { ... }

// Valid calls
request("https://api.com")                              // both options defaulted
request("https://api.com"; timeout: 60)                 // one of them named
request("https://api.com"; method: "POST", timeout: 5)  // in any order

// Invalid calls
// request("https://api.com", 60)      // a positional argument in the named zone
// request("https://api.com"; timout: 5)  // error[NK1109]: no option `timout`
```

**The `;` stands between the two zones, and where one zone is empty it is not
written** ([ADR-133](adr/adr-133.md)). A function whose parameters are all
options is declared `fn execute(target_age: i64 = 0)` and called
`execute(target_age: 30)`. `execute(; target_age: 30)` is refused, so there is
one spelling. A mixed call keeps its `;`, and the `;` is required there. A
**method** call takes the same form. A driver's deferred parameters (Part II
10.5) write no `;` either, because a receiver stands outside the parentheses
and leaves no zone for the separator to stand between.

> **Implementation status:** Implemented. The declaration and the call are both
> built ([ADR-133](adr/adr-133.md) §5); the call form became unambiguous when
> [ADR-140](adr/adr-140.md) D1 withdrew the named struct literal.

**Every configuration parameter has a default.** A caller may leave an option
out, and the call then takes the default. A parameter that has to be passed
belongs before the `;`. An option without a default is a parse error that says
so.

**A default is a literal.** Where a default is evaluated, at the declaration or
at the call, is therefore never a question.

**Order is the declaration's**, not the call's: `method` written first above is
still passed second. The ledger records an option's name, type *and* default
(Part III, 13.5), because a call that leaves an option out still passes a value,
and a consumer compiling against a library must know which.

**Optional Parentheses**
A function declared without parameters may omit the parentheses, matching the
block lambda style.

```nika
fn init { 
    // No args, no parentheses required
}
```

### 5.2. There Is One Lambda Form
A lambda is written `fn { … }`, and 5.3 describes it. There is no second,
shorter form for single-line bodies.

Naming the arguments, `fn(user) { … }` (5.3), is the same form spelled out: the
body is a block either way, and both spellings stand in all the same places.

**The arguments are the ones the lambda names.** A lambda that names none takes
none, so `fn { … }` is a lambda of no arguments. The three automatic names `a`,
`b` and `c` are withdrawn ([ADR-049](adr/adr-049.md)); a body that uses one
names something nothing declares, and is refused with `NK1117`.

The short form `fn: expression` is withdrawn ([ADR-022](adr/adr-022.md)).
Writing it is refused with a message that names the block form.

*Design rationale:* the short form's body ran to the end of the expression, so
a `.method()` chained after it landed inside the lambda and silently changed
the program ([ADR-022](adr/adr-022.md)).

### 5.3. Lambdas (`fn { ... }`)
A block lambda holds any number of statements. Its arguments are the ones it
names.

* **Syntax:** `fn(name) { ... }`, and `fn(first, second) { ... }` for more than one
* **Arguments:** as many as the list says; the list is the only thing that says so
* **A lambda that names none takes none**, so `fn { ... }` is the zero-argument
  form, which is what `.or_insert_with fn { Stats(0) }` takes

```nika
let complex = users.map fn(user) {
    let bonus = calculate_bonus(user)
    // Implicit return of the last line
    user.score + bonus
}

let ids = users.map fn(user) { user.id }
```

**The automatic `a`, `b`, `c` are withdrawn** ([ADR-049](adr/adr-049.md)). A
local inside a lambda may be called anything. A body that uses one of the three
names without declaring it names something nothing declares:

```text
error[NK1117]: nothing declares `a`, and this statement is just that name
```

**Trailing Syntax**
A lambda that is the last argument may stand *outside* the parentheses. Where
there are no other arguments, the parentheses are omitted with it:

```nika
let ids = users.map fn(user) { user.id }
let sum = numbers.reduce(0) fn(acc, n) { acc + n }
```

A block ends at its `}`, so a chain continues after it and means what it reads as:

```nika
Server::new()
    .route("/x") fn { handler(db) }
    .listen(":8080")
```

```nika
// The count comes from the list, so a local may be called anything
users.map fn(user) {
    if user.is_guest() {
        return "Guest"
    }
    return user.name
}
```

A trailing lambda may follow a method call with or without other arguments, a
plain call, or a path: `users.map fn(user) { … }`,
`numbers.reduce(0) fn(acc, n) { … }`, `access_all(a, b) fn(x, y) { … }`
(Part II, 12.3), `task::scope fn(s) { … }` (Part II, 12.7), and the chain
above.

**A lambda carries no effect marker.** A parameter list is followed by the body
and by nothing else; `fn(info) sync { … }` is not one construct (7.2,
[ADR-141](adr/adr-141.md) D1).

> **Implementation status:** Implemented. The lambda is built in both
> positions. The automatic names, their warning `NK1114` and the
> arity-from-body mechanism are removed from the compiler
> ([ADR-049](adr/adr-049.md) §5).

### 5.4. Contextual Capture (The Lifecycle Rule)
Whether a lambda borrows or moves the variables it uses is inferred from the
context the lambda is used in. The rule is the same at both values of
`user_parallelism`.

#### A. Immediate Context (`@immediate`)
A function that runs the callback to completion before it returns is an
**immediate context**.
* **Behavior:** implicit borrow (`&T`).
* **Examples:** `map`, `filter`, `for_each`, `sort_by`.

```nika
let prefix = "User: "
let names = ["Alice", "Bob"]

// 'map' is @immediate. It executes completely within this stack frame.
// 'prefix' is implicitly borrowed.
let formatted = names.map fn (name) { prefix + name }

// 'prefix' is still valid here because it was only borrowed.
println(prefix)
```

#### B. Detached Context (`@detached`)
A function that stores the callback, runs it later, or hands it to another
thread or task is a **detached context**.
* **Behavior:** implicit move (ownership transfer) for ordinary data. A handle
  on a shared value is **duplicated** rather than moved, so the name outside
  stays usable ([ADR-040](adr/adr-040.md) D1; the rule is 6.2).
* **Examples:** `spawn`, `defer`, `set_timeout`, `channel.on_receive`.

```nika
let prefix = "Log: "

// 'spawn' is @detached. The lambda might outlive the current function.
// 'prefix' is implicitly moved into the background task to ensure safety.
spawn fn { println(prefix + "System started") }

// Compiler Error: 'prefix' has been moved!
// println(prefix)
```

#### C. Where the Distinction Lives

The context belongs to the **function that takes the lambda**, not to the call.
`map` is immediate for every caller and `spawn` is detached for every caller,
so the capture is decided where the lambda is written.

**A function in user code declares a code parameter with a function type**
([ADR-102](adr/adr-102.md)). The type is spelled the way a signature is,
`handler: fn(Request) -> Response`, with `sync` or `throws` after the result
where a declaration puts them. Without `sync` the code may pause; without
`throws` it cannot fail. A lambda that does less fits a type that allows more.
A pausing lambda handed to a `fn() sync` is refused.

The context of such a parameter is **inferred**, not written. A parameter the
body only calls is immediate and borrows. A parameter the body keeps (stores,
hands back, gives to a task) is detached and moves. It is the question 6.5 asks
of every parameter, and there is no `@detached` to write. For an immediate
parameter the callee's own promises follow the lambda, as `map`'s do. For a
kept parameter they follow the type, so a `listen` that calls a stored
`fn(Request) -> Response` may pause.

> **Implementation status:** Not implemented. A parameter of function type is a
> parse error, and the eight `std` entries that take a lambda are written
> straight into the ledger. The capture at a `spawn` is reported with `NK2101`
> (8.3). [ADR-102](adr/adr-102.md) §5 carries the work.

---

## Chapter 6: Memory and Ownership

Memory is managed by **ownership and borrowing**, which the compiler enforces.
There is no garbage collector, and user code frees no memory.

### 6.1. The Concept of Scope
When a variable goes out of **scope**, at the end of the block `{}` where it
was created, its memory is released. User code frees no memory.

### 6.2. Unified Types
Three shared types carry a value that has more than one owner.

**`Shared[T]` is written in the source; it is not inferred.** Sharing changes
*when* a value is cleaned up (6.4), and a program can observe that. What the
compiler decides is the machinery underneath: **which owner count each value
gets**, one a second thread may safely touch where the value may reach one, and
a cheaper one where it may not ([ADR-037](adr/adr-037.md) D7). Whether a
`Shared` may be handed to a task (Part II, 11.2) is decided from the type and
from where it is going, at both values of `user_parallelism`
([ADR-045](adr/adr-045.md) D1). The count follows that answer; it does not make
it.

**Where the compiler can prove that a value never leaves the thread that made
it, it uses the cheaper count** ([ADR-037](adr/adr-037.md) D7). That is an
optimisation, never a change of meaning. The difference is about 9 ns each
time a handle is made and dropped, and nothing for a handle that is only read.
**Where nothing proves it, the value gets the safe count**, and there is no way
to ask for the other one.

**At `user_parallelism = no` every count is the cheap one**
([ADR-061](adr/adr-061.md) D2). Nothing has to be proved for it: user code runs
on one thread, the runtime's own threads run no user code, and a `Shared` may
not be handed to code nothing written down describes
([ADR-061](adr/adr-061.md) D1). What a program passes out of itself is what is
**inside** the `Shared`, a view or a copy; a foreign library that means to keep
a value puts it in a hull of its own. `nikaia --input x.nika --sharing` prints
which count each value got, why, and what would have changed it. Like
`--overlaps` and `--trust`, it explains a decision and changes none.

* **`Shared[T]`**: a value several parts of the program own at once and nobody
  changes. The memory is released when the *last* owner is finished.
* **`SharedMut[T]`**: a value several parts own at once and any of them may
  change. The lock that keeps the changes apart is part of the type; there is
  no second wrapper to write around it. The value is changed through the four
  doors of 6.3. This is the common case, which is why it has the short name
  ([ADR-039](adr/adr-039.md) D9).
* **`Locked[T]`**: an individually locked field inside a shared structure, one
  lock per field rather than one lock around the whole. It is the same lock as
  the one inside `SharedMut[T]`, opened by the same four doors.

**The second owner arises without a written step.** Where a handle on a
`Shared[T]` or a `SharedMut[T]` is handed on **by value**, passed to a function
that keeps it or used by a task (Part II, 11.2), the handle is **duplicated**.
Each handle is cleaned up at the end of its own block (6.1). There is no method
to call: a duplicated handle copies none of the data and produces no second
value, one value and one more owner ([ADR-040](adr/adr-040.md) D1). The rule is
the same for both shared types; a `SharedMut[T]` may not cross a thread
(Part II, 11.2).

**Lending the inner value out duplicates nothing.** `&` on a shared value is a
view of the value *inside* it, so a function that only uses the value takes an
ordinary view and never mentions sharing:

```nika
fn serve(db: &Connection) { … }

let db = Shared(postgres::connect("…"))
serve(&db)          // a view; no handle is made, and the count is untouched
```

Whether the value is shared is the caller's decision, and `serve` does not know
it. A signature names the shared type only where the function **keeps** the
value past the call: puts it in a structure, gives it to a task, hangs it on
something that outlives the call. Only then does it need a handle of its own
([ADR-042](adr/adr-042.md) D1, D2).

**A `SharedMut[T]` is opened, and the opened value is an ordinary view.** There
is no `&` straight through a lock. The caller opens the lock with one of the
four doors of 6.3 and passes the borrowed value in. The called function sees a
plain value and obeys the rule for an open lock: it may not pause, and it may
not touch a lock of its own (Part II, 12.2).

```nika
let db = SharedMut(postgres::connect("…"))
db.access fn(open) { serve(open) }    // `serve` must be `sync` and lock-free
```

**The first handle is written in the source.** Each of the three shared types
makes one by being **called with the value that goes in it**
([ADR-064](adr/adr-064.md) D2):

```nika
let db = Shared(postgres::connect("…"))
let counter = SharedMut(0)
let frei = Locked(0)                    // a field's lock, one per field
```

One rule decides every hull in the language:

> **A hull you cannot see, the compiler writes. A hull you can see, you write.**

A `T?` costs nothing and hides nothing (the same value, possibly absent), so the
compiler puts a plain value into one (2.3). The three shared types change
**when the value is cleaned up**, and a program can observe that, so the word
stands where it happens.

**A constructor stands wherever an expression may:**

```nika
keep(Shared(connect(url)))                      // an argument
let p = Pool { db: Shared(connect(url)) }       // a field of a literal
fn connect(url: String) -> Shared[Connection] {
    return Shared(Connection { url: url })      // a result
}
```

Where a plain value stands and a shared one is wanted, the message says exactly
that:

```text
error[NK1115]: `serve` takes a shared value, and `db` is not one
  help: write `Shared(db)` - a hull you can see is one you write
```

A shared value may be returned, and a number may be shared: `SharedMut(0)` is a
call, and the call gives the number its type as any other argument does.

**Only the handle is duplicated.** Ordinary data (a string, a number, a struct
of those) is **moved** where it is handed on to something that keeps it (6.5,
8.3). For data, the `.clone()` is written in the source
([ADR-040](adr/adr-040.md) D1).

*Design rationale:* an automatic duplication of data would copy the whole of
it, which is a different thing at a different cost ([ADR-040](adr/adr-040.md)
D1).

**Each handle lives to the end of its own block, whether or not it is used
again.** The duplication is not conditional on a later use
([ADR-040](adr/adr-040.md) D2), so the value is cleaned up where the caller's
handle ends, which may be later than the task that holds the other one. That is
the block rule of 6.1 applied unchanged: a program that wants the cleanup
earlier ends the block earlier ([ADR-040](adr/adr-040.md) D3). Where a handle
is duplicated is not visible in the source, so `--sharing` names each
duplication site beside the count it printed for that value
([ADR-040](adr/adr-040.md) D5).

**`SharedMut[T]` is one name and two hulls**, and only the backend knows that
([ADR-064](adr/adr-064.md) D1). A message, a printed type and a ledger entry
say the name the program wrote. Which shape it becomes is decided per value,
as the owner count is. **`Shared[Locked[T]]` is refused** with `NK1123`, and
the message names `SharedMut[T]`: one type, one spelling.

> **Implementation status:** Partially implemented. `Shared[T]` is built: the
> type, its ledger entry, the constructor wherever an expression may stand, and
> `serve(&db)` through the `deref` entry ([ADR-042](adr/adr-042.md) D2). A call
> that wants a shared value and is given a plain one is refused with `NK1115`
> (Part III, C.3). `SharedMut[T]` and `Locked[T]` are not built: writing either
> names a type that does not exist, the backend has no lowering for one, and
> the four doors of 6.3 wait on the same thing ([ADR-039](adr/adr-039.md) §4).
> The duplication is built for a handle handed to a function and not for one
> used by a task, because `spawn` does not lower a handle yet (Part II, 11.2);
> the owner count is inferred per value and `--sharing` prints it with each
> duplication site ([ADR-037](adr/adr-037.md) D7, [ADR-040](adr/adr-040.md)
> D1, D5).

### 6.3. Changing Shared Data: The Four Doors
A value behind a lock is not changed by assignment. The lock is opened first,
and the shape of the change decides which door opens it:

```nika
let kasse: SharedMut[i32] = ...

let stand = kasse.get()              // take a copy out
kasse.set(hole_neuen_stand())        // replace it; the new value comes from outside
kasse.update fn(mut v) { v += 100 }   // change it, under the lock

// Where the value is large, `update` changes it where it lies, and `access` reads it there.
let protokoll: SharedMut[Log] = ...
protokoll.update fn(mut log) { log.add("gebucht") }
let n = protokoll.access fn(log) { log.len() }
```

Each door is shaped for its use:

* **`get` and `set` take no block.** `get` copies the value out. `set`'s
  argument is computed *before* the call, so everything slow about producing
  the new value, including waiting for I/O, happens outside, and the lock is
  open for one store.
* **`update` is handed the value as `mut v` and changes it.** It returns
  nothing: the change *is* the result, and `mut` is the word every changed
  parameter carries (4.3). Whether `v` is a copy of a small value or the
  address of a large one is the compiler's decision. An `update` block may be
  run more than once where that is free, so it may not do I/O or take another
  lock ([ADR-110](adr/adr-110.md)).
* **`access` is for a value that is too expensive to copy**: a list of ten
  thousand entries is not copied to be asked its length. It hands the block
  the value where it lies, to **read**; the block may not change it.

`update` and `access` run user code while the lock is open, so that code runs
straight through: no I/O, and no second lock. The compiler checks both
(Part II, 12.2 and 12.3).

**A value taken out of a lock is stamped.** `kasse.get()` is a `Seen[i64]`,
and so is what `access` computes. A `Seen` reads like the value it carries: a
program prints it, compares it, sends it in a response, and hands it to any
function that touches no lock. The stamp goes with it through arithmetic,
calls, struct fields declared `Seen[…]`, and time. A stamped value may not go
back into a lock **blind**: `kasse.set(stand + 100)` is refused wherever
`stand` was read, and so is `if stand > 100 { kasse.set(0) }`, because the
decision is stale even where the value is not. The doors for what was seen are
`update`, which decides inside the lock, and `set(neu; after: stand)`, which
stores only if the lock still holds what was seen and throws `Overtaken`
otherwise ([ADR-111](adr/adr-111.md)). There is no word that removes the stamp.

Three mistakes are refused by name. Assigning to a `SharedMut` directly,
`kasse = 0`, is refused, and the message names `set`. A `set` given a stamped
value, or standing under a stamped condition, is refused, and the message names
`update` and `after:` ([ADR-039](adr/adr-039.md) D10, [ADR-111](adr/adr-111.md)
D4). An `update` block that assigns to `v` without reading it is a `set`
through the back door, and is refused as one.

> **Implementation status:** Not implemented. `get`, `set`, `update`, `access`
> and `access_all` have no entry in `std`, nothing lowers them, and none of the
> refusals above is reported ([ADR-039](adr/adr-039.md) §4,
> [ADR-111](adr/adr-111.md) §5).

### 6.4. Resource Cleanup (RAII)
Resources are cleaned up deterministically, following the **RAII** principle
(Resource Acquisition Is Initialization). There is no garbage collector.

**Automatic Destruction**
When a variable goes out of scope, at the closing brace `}`, its memory is
released.

**Custom Cleanup (`impl Drop`)**
A struct that manages an external resource (a file handle, a socket, a C
pointer) may implement the `Drop` trait. The `drop` method is called when the
value is destroyed. `drop` is **synchronous**: it runs straight through and
never pauses. It is for teardown that is pure memory work or a cheap native
call.

```nika
struct FileHandle {
    fd: i32
}

impl Drop for FileHandle {
    fn drop(&mut self) {
        println("Closing file descriptor...")
        // Native close call would go here
    }
}
```

**Cleanup That Needs I/O (`impl Cleanup`)**
Some resources cannot be torn down without I/O: a buffered file *flushes* its
remaining data to disk, a database transaction *rolls back*, a TLS connection
closes over the network. I/O means the function may pause (Chapter 8), and a
synchronous `drop` cannot pause. Such a resource implements `Cleanup`:

```nika
impl Cleanup for BufferedFile {
    // Pausable teardown. May pause, may fail.
    // The compiler calls it automatically at the end of the scope —
    // on normal exit AND while an error is bubbling up.
    fn cleanup(&mut self) throws {
        self.flush()
    }

    // Synchronous last resort. Must not pause, must not fail.
    // Runs after cleanup(), or alone if cleanup() cannot run
    // (see "When cleanup cannot run" below).
    fn drop(&mut self) {
        // release the handle — nothing that waits
    }
}
```

User code never calls `cleanup`. The compiler inserts the call at the end of
the block, exactly as it does for `drop`. The end of the block is then a place
where the function may pause, like any other I/O, and where a failure surfaces
(next paragraph).

**A cleanup error is an error.** If closing a resource can fail, the function
that owns the resource can fail. If `cleanup` declares `throws`, the enclosing
function declares `throws` too. A function that does not is refused with
`NK2601`, and the message names the resource:

```text
error[NK2601]: this function can fail because closing `f` can fail
  --> report.nika:2
   |
 2 |     let f = fs::create("report.txt")
   |         ^ `f` is a buffered file; writing its remaining data
   |           to disk at the end of this function can fail
   |
  help: declare the error:  fn save_report(text: String) throws
  help: or handle it precisely by closing explicitly:
        f.close() catch { ... }
```

*Design rationale:* silently losing data at close time is a decades-old bug
class in other languages ([ADR-025](adr/adr-025.md) D1).

Two refinements:
* If a value dies **while an error is already propagating**, the cleanup error
  does not replace it. It is attached to the original error as a *secondary
  error* (7.1).
* A program that handles the close error specifically calls **`close()`**.
  `close()` consumes the resource and returns the error normally, and no
  implicit cleanup runs afterwards.

**A `sync` function never pauses**, so a resource with a pausable `cleanup`
may not go out of scope inside one. Such a program is refused with `NK2602`,
and the message names the ways out: return the resource to the caller, close it
before the `sync` part, or use a non-buffering variant.

**When `cleanup` cannot run.** When a task is *cancelled* (it lost a `select`
race, or a supervisor restarts it), nobody can wait for its I/O. The runtime
adopts the pending `cleanup` runs and finishes them in the background before
the program exits ("parked cleanup"). The runtime configuration
`cleanup-deadline` bounds them (Part III 13.3b). A cleanup the deadline cut off
is a failure of the program: exit status 70, with the resource named on the
panic path ([ADR-112](adr/adr-112.md)). A **panic** runs no pausable cleanup:
during panic teardown only the synchronous `drop` fallback runs, and **on a
target that traps rather than unwinds, a panic ends the process immediately, so
no destructors run at all** (Part III, Appendix A). The **panic hook** (7.2)
runs on every panic. A panic is for an unrecoverable bug; a recoverable failure
uses `throws`, where full cleanup is guaranteed.

There is no `defer` keyword. Cleanup happens at the end of the block through
`Drop`, so it cannot be forgotten. A lock's `.access()` releases the lock, so
there is no manual `lock/defer unlock` pair. A `throws` unwinds the stack and
runs `drop` for every variable in scope, so no resource leaks during a failure.

### 6.5. References and Borrowing

A function that only *looks at* a value, and does not own it, takes a
**reference** (written `&T`, or `&str` for text). The owner keeps the value,
the borrower may read it, and the loan ends on its own.

**Nikaia source contains no lifetime annotations.** There is no syntax for
them. Everything described below happens inside the compiler
([ADR-005](adr/adr-005.md)).

> Nikaia source code contains no lifetime annotations. Ever.

Two things are guaranteed:

1.  **Borrowing inside a function**, across I/O. A function may pause at any
    I/O call (Chapter 8), and a borrowed value stays valid across the pause:

    ```nika
use std::fs

    fn report(config: &Config) {
        let name = &config.name       // borrow
        let data = fs::read("log")    // the function pauses here (I/O)...
        println(f"{name}: {data}")     // ...and the borrow is still valid.
    }
    ```

2.  **Returning a borrowed value from a function.** `fn first_word(s: &str) -> &str`
    needs no annotation. Where the result could come from *several* inputs,
    the compiler infers the connection, across function boundaries and through
    the whole program (6.7).

**A borrow may not outlive its owner.** The next section is why a program
rarely breaks this rule by accident.

**The declaration writes the `&`, never the call** ([ADR-094](adr/adr-094.md)).
A parameter written with a plain type is a **view** unless the function's body
keeps the value: stores it, hands it back, gives it to a task, or passes it to
something that keeps it. Which of the two it is comes from the body, is written
to the ledger (6.7), and is true for every caller. The caller writes `serve(db)`
and `fs::map(path)`, and the compiler writes the reference the callee asked
for, as it writes the pause and the failure a call carries (7.1, 8.1). A `&` in
a parameter type is an assertion, *this is a view*, as `sync` is (Part II,
12.1). A parameter the function changes in place says `mut` in the declaration,
`fn fill(mut out: Vec[i64])`; that is `&mut self`'s rule for every parameter,
and the call shows nothing, as `xs.push(1)` shows nothing. A `for` **lends**
its list, so the list is still there after the loop; iteration that takes the
elements away is written `for x in xs.drain()`. Nothing here inserts a copy: a
value handed to a function that keeps it, and used again afterwards, is refused
with `.clone()` named as the way out (8.3).

Four kinds of argument are not lent and are passed owned: a parameter the body
**keeps**, a value that **copies**, an argument that is **already a view**, and
a **method's** argument.

> **Implementation status:** Partially implemented ([ADR-094](adr/adr-094.md)
> §5). A `for` lends; `xs.drain()` takes the elements away; a `let` over a
> place (`config.name`, `totals.stations[name]`) is a view of it where the value
> would otherwise have to move; the `keeps` column is inferred and recorded for
> every function; and the compiler writes the `&` at the call off that column,
> so `serve(&db)` is refused with `NK1137` (Part III, C.3). `mut` is read:
> `fn fill(mut out: Vec[i64])` lowers to `&mut Vec<i64>`, `fill(xs)` gains its
> `&mut`, and a parameter a body changes without the word is refused with
> `NK1138`. A method's argument is passed owned because the compiler cannot yet
> resolve which entry the call goes to. Not built: the ledger diff that
> narrates a kept value's moved cleanup point, and the refusal of a `&` written
> at a call whose argument type is not known (a value a `catch` handed back, a
> place inside a lambda).

### 6.6. Escaping References Are Tethered

A borrowed value **escapes** when it is returned past the scope that owns the
buffer, captured by a `@detached` lambda, or put into a collection that lives
longer than the buffer. An escaping view is **tethered**: the buffer stays
alive as long as the view does.

> **Transient = borrow. Escaping = tether. Copying = yours to ask for.**

Every view (`&str`, `&[u8]`) is in one of three states. The compiler picks the
cheapest one that works, and the states are never written in the source:

| State | What it is | Cost |
| :--- | :--- | :--- |
| **Borrowed** | a plain reference into the buffer | nothing at all |
| **Tethered** | a handle on the buffer plus a position | one shared handle per *container* — no copy, no allocation |
| **Owned** | a `String` of its own | one allocation, **only** where the program wrote `.to_owned()` |

**The buffer cannot die while anything still points into it.** The buffer is
kept alive deterministically, by reference counting, with no garbage
collector.

Two rules keep this cheap on large data:

* **Storing a slice in a struct is not, by itself, an escape.** A struct that
  is built and consumed inside the scope that owns the buffer keeps plain
  references. A parser loop that builds a hundred million small records pays
  nothing for them.
* **The handle sits on the container, not on every slice.** When a map full of
  slices outlives its buffer, the *map* holds one handle, and its keys stay
  positions. Filling it costs no handle traffic.

```nika
struct Token {
    text: &str,   // a view into someone else's buffer
}

fn tokenize(source: String) -> Vec[Token] {
    // The returned tokens outlive `source`'s scope, so the list is tethered:
    // it keeps `source` alive. No annotations, no copies of the text,
    // no dangling references — and one handle for the whole list.
    ...
}
```

There is no struct with lifetime parameters in Nikaia; the concept does not
exist in the language.

**A struct marked `@borrowed` never tethers.** In a hot loop, a program that
wants to be told if a value ever starts tethering rather than borrowing marks
the struct:

```nika
@borrowed
struct Reading { name: &str, temp: i32 }
```

Nothing about the program changes, except that an escape is a compile error
that names the place where it happens. `nikaia explain --tethers` prints the
state of every view and changes nothing.

**An escape whose buffer cannot be shared is refused.** Where a slice escapes
and its buffer lives on the stack or came from a foreign library, no tether is
possible. The compiler refuses the program and names the ways out,
`.to_owned()` among them. The compiler never inserts that copy
([ADR-008](adr/adr-008.md)).

*Design rationale:* an invisible copy in a loop over a billion rows is the kind
of surprise the language refuses to produce ([ADR-008](adr/adr-008.md)).

**A parameter written `&str` may not be kept past its call.** A view inside a
struct carries the buffer it points into, because the struct's declaration says
it holds a view. A parameter written `&str` on its own says only that the call
may look at one. A function that stores such a parameter, into a field of its
subject, into a struct it hands back, or into a task, is refused with `NK2302`
(Part III, C.3). Such a function puts the view in a struct and takes the
struct:

```nika
@borrowed
struct Reading { name: &str, temp: i32 }

impl Summary {
    fn record(&mut self, m: Reading) { … }     // and not `name: &str`
}
```

Handing a view back out of the buffer it came from is **not** this rule:
`fn count(seq: &str, k: i64) -> HashMap[&str, Tally]` returns views of `seq`,
and the result points into `seq` and nothing else. A parameter has no buffer of
its own to name, because the language has no syntax for one
([ADR-005](adr/adr-005.md) D1); a struct carries its buffer instead
([ADR-008](adr/adr-008.md) D1).

Where the destination **already carries a buffer**, there is nothing to refuse.
A method of a struct that holds a view has that struct's buffer in hand, so
storing the parameter into one of its fields is accepted, and the parameter is
a view of *that* buffer. `fn note(&mut self, name: &str)` on a `Summary`
holding `label: &str` compiles, and `name` is a view of the buffer `label`
points into. That narrows what a caller may pass, which is why the signature
changes rather than the body.

> **Implementation status:** Partially implemented, and the table above is the
> shape of it: **Borrowed** and **Owned** are built — the first is the language
> below's own lifetime and costs nothing, the second is `.to_owned()` and is
> never inserted — and **Tethered is not**. The **analysis** is: every view in a
> signature carries its solved state in the ledger and `nikaia --tethers` prints
> it, and nothing reads it yet, because a state is a representation and only one
> of the three is emitted. Every view in `examples/` and `benches/` solves to
> **Borrowed** ([ADR-008](adr/adr-008.md) §3's worked check, measured). Where a value would tether, the
> program is **refused** instead, which is the residual hard error
> [ADR-008](adr/adr-008.md) D5 names rather than the state beside it — on the
> Nikaia line since [ADR-156](adr/adr-156.md) D4, where a function hands back a
> view of a buffer its own body made (`NK2303`, Part III C.3);
> `@borrowed` is parsed and forbids nothing, because there is no transition yet
> to forbid. `docs/open-work.md` carries what the missing state would take.
>
> The rule below is built for a
> method of a struct that holds a view. Three cases are refused with `NK2302`:
> a function or method whose own subject holds no view, even when the
> destination is a field of a struct that carries one; a view handed back
> through the result; and a view given to a task. Where the view is handed to
> a call on the subject, the compiler cannot see whether the callee keeps it,
> so it is treated as kept and the parameter is written as a view of the
> subject's buffer ([ADR-008](adr/adr-008.md) §5).

**A tether keeps the whole buffer alive**, not just the part pointed at.
Keeping one short name out of a 13 GB memory-mapped file pins all 13 GB. Where
that looks like a mistake, the compiler warns and suggests `.to_owned()`.

### 6.7. The Borrow Contract Ledger

For every function whose signature involves a borrow, the compiler infers a
**borrow contract**, such as "the result of `longest(a, b)` borrows from `a`
or `b`." A contract is never written in the source. Contracts are stored in a
generated file, **`nikaia.contracts`**, the ledger, which is committed
alongside `nikaia.lock` (Part III, 13.5).

The ledger has two jobs:

1.  **Cache:** if a contract did not change, none of the function's callers is
    re-checked.
2.  **Explanation:** if an edit to a function *body* changes its contract, the
    compiler compares the old and the new contract. If a caller elsewhere
    breaks, the error names the edit, the change in the contract, the caller
    that is affected, and the way out.

### 6.8. When the Compiler Says No

An ownership rule refuses a program that breaks it. **Every such refusal
explains itself in plain language and names the way out**
([ADR-005](adr/adr-005.md) D7). Reading a Nikaia diagnostic requires no
knowledge of Rust; a backend error that reaches user code is a defect of the
compiler.

The most common case is changing a collection while looping over it:

```nika
let mut users = load_users()
for u in users {
    if u.is_duplicate() {
        users.remove(u)   // bug: pulling the rug out from under the loop
    }
}
```

```text
error[NK2301]: cannot change `users` while looping over it
  --> main.nika:4
   |
 4 |         users.remove(u)
   |         ^^^^^^^^^^^^^^^ the loop is still reading `users`
   |
  note: removing items mid-loop would invalidate the loop's position
        (this is a crash or silent bug in most languages)
  help: use the built-in method that does this safely:
        users.retain fn (user) { !user.is_duplicate() }
```

For every known pattern of this kind, the standard library provides a safe,
named method (`retain`, `drain`, `entry`, `swap(i, j)`, …), and the diagnostic
names it.

---

## Chapter 7: Error Handling

Nikaia distinguishes two kinds of errors.

### 7.1. Recoverable Errors (`throws`)

A recoverable error is an expected problem: a file is missing, a connection
drops, an input does not fit the format. A function that can fail says so with
`throws`.

```nika
use std::fs

fn fetch_config() -> String throws {
    let file = fs::read("config.txt")   // can fail
    return net::send(file)              // can fail too
}
```

**`throws` stands after the result type**, and `sync` with it:
`fn fetch_config() -> String throws` ([ADR-140](adr/adr-140.md) D4). A function
*type* has the same order ([ADR-102](adr/adr-102.md) D1), so a declaration and
a type read the same way round. A declaration with no result type writes the
word after the parameters: `fn tick() sync { … }`. The form before the arrow
is withdrawn and does not parse; the message names the order.

> **Implementation status:** Implemented. The pre-arrow form is a parse error
> whose message names the order ([ADR-140](adr/adr-140.md) §5).

**`throws` names no types.** What a function can fail *with* follows from its
body. The compiler infers it whole-program and writes it to `nikaia.contracts`
(Part III, 13.5).

*Design rationale:* an error set in the signature would be derived truth
maintained by hand, for the same reason a borrow relationship is not written in
the source ([ADR-005](adr/adr-005.md) D3, [ADR-023](adr/adr-023.md) D1).

**An error type is an `enum`.**

```nika
enum ConfigError {
    NotFound(Path),
    Unreadable(Path),
    BadSyntax { line: i64, expected: &str },
}

impl Error for ConfigError {
    fn message(&self) -> String {
        match self {
            ConfigError::NotFound(p)   => f"no config at {p}"
            ConfigError::Unreadable(p) => f"cannot read {p}"
            ConfigError::BadSyntax { line, expected } => f"line {line}: expected {expected}"
        }
    }
}
```

An error **carries what belongs to it**: "not found" carries the path. An
`enum` is the language's type for one of a fixed set of things (4.4), and a
`match` over one is checked for completeness. The type carries no marker: what
is thrown implements `Error`, and the `impl` line says so — so a number, a
`bool`, a character and text are not errors, and a `throw` of one is refused
with `NK1161`.

**Raising: `throw`.**

```nika
if !fs::exists(path) {
    throw ConfigError::NotFound(path)
}
```

**A `throw` is an expression**, and so are `return`, `break` and `continue`
([ADR-138](adr/adr-138.md) D1). Their type is **never**
([ADR-093](adr/adr-093.md)). A `never` fits every expected type without
widening it, because the expression hands back nothing, so each may stand
wherever an expression may:

```nika
let user = find(id) ?? throw NotFound(id)
```

What they *do* is unchanged: a `throw` leaves the function and makes it
`throws`, a `break` needs a loop, and a statement after a `break` in the same
block is refused with `NK1133` (3.3).

> **Implementation status:** Implemented ([ADR-138](adr/adr-138.md) §5). A
> `??` whose fallback jumps lowers to a `match` rather than to
> `unwrap_or_else`, because a closure is a function boundary and a jump does
> not cross one ([ADR-084](adr/adr-084.md) D4). The statement forms are
> unchanged, and a bare `break` on a line of its own is the statement it always
> was, so `NK1133` still answers about what follows it.

**Propagation happens on its own, and nothing marks it.** A call that can fail
stands inside a function that declares `throws`. There is no operator and no
sigil:

```nika
fn load() -> Config throws {
    let text = fetch_config()   // if it fails, `load` fails
    return parse(text)
}
```

Four things leave the control flow of a program without the line showing it: a
call may **pause** (8.1), a block's end may **pause and fail** (6.4), a call may
**fail** (here), and a loop's step may fail ([ADR-025](adr/adr-025.md)).

*Design rationale:* marking one of the four would claim the other three were
absent ([ADR-023](adr/adr-023.md) D8).

**Because nothing marks a failing call, the declaration is required.** A call
that can fail, in a function that does not say `throws`, is refused with
`NK2605`:

```text
error[NK2605]: this function can fail because `liest` can fail
  --> app.nika:2:23
   2 | fn ruft() -> String { return liest() }
                             ^
     = `liest` carries `throws = ["?"]` in the contracts this program is built against (Part III, 13.5)
     = nothing marks a failing call, so a failure leaves at a call exactly as it leaves at a block's closing brace or a loop's step (ADR-023 D8, ADR-025 D1)
     help: declare the error: add `throws` to `ruft` - or handle it at the call, `… catch { … }` (Part I, 7.1)
```

for `fn liest() -> String throws { return fs::read_to_string("x.txt") }` one
line above. The shape is Appendix C.4's: the caret is on the statement, the
note is the contract quoted from the ledger the call was resolved against, and
the help is one of the two things user code can write.

*Design rationale:* the signature is the only place the source says a function
can fail, so a caller that does not say it has said something false; and
`nikaia.contracts` records `throws` per function and is committed and read by
other programs (Part III, 13.5), so accepting the program would publish that
`ruft` cannot fail ([ADR-023](adr/adr-023.md) D8).

> **Implementation status:** Partially implemented. The refusal and the
> propagation are built for a call **by name**: a function of this program, of
> another of its modules, or of `std`. A **method** call that can fail is
> refused with `NK2605`, but the lowering does not propagate one yet; on a
> method, `catch` at the call is the form that compiles today. A call nothing
> describes is neither refused nor propagated, and reaches the backend
> ([ADR-023](adr/adr-023.md) §5).

**Handling: `catch`.** The block supplies the replacement value, or it leaves
the function.

```nika
let config = load() catch {
    eprintln(f"{error}")
    return                                // leaves the function
}

let port = read_port() catch { 8080 }     // replacement value
```

Inside the block the error is named **`error`**. A handler that passes the
error on writes `throw error`.

**Telling failures apart.** `error` is the sum of the errors that can arrive at
this point; the compiler inferred them. A handler tells them apart with the
patterns of 3.4, where a path with `::` names a variant:

```nika
let config = load() catch {
    match error {
        ConfigError::NotFound(p) => Config::default()
        ConfigError::BadSyntax { line, .. } => {
            eprintln(f"config broken at line {line}")
            return
        }
        else => throw error
    }
}
```

> **Implementation status:** Implemented, where the compiler can **name** what
> arrives ([ADR-157](adr/adr-157.md) D1). The patterns of 3.4 are built, `..`
> in a named pattern among them ([ADR-137](adr/adr-137.md) §5), and a bare
> `throw` is an arm's body ([ADR-138](adr/adr-138.md) §5) — and since
> [ADR-157](adr/adr-157.md) the failure channel is the **error type** where the
> inferred set has exactly one member, so the block above is a program that
> runs. Where the set has two members, or a member this compiler cannot name,
> the channel is one opaque error and a `match` over its variants is not
> lowerable: `catch { 8080 }` and `f"{error}"` are what such a handler has.
> Every function in `examples/` reaching `std` is in that second case, because
> `std`'s ledger names no error types yet. `docs/open-work.md` carries both.

**Two sets differ.** The **variants of an error type** are closed: a `match`
over them is exhaustive, and adding one is a breaking change. The **set of
error types** arriving at a `catch` is open, and it grows when a callee gains a
failure. When it grows, **every `catch` over that callee is named once in the
build output**, with the new error, the handler it now reaches, and the fact
that the handler takes it as it takes everything (`NK2401`). Under `--locked`
the build fails until the ledger is regenerated and committed. The commit is the
acknowledgement; nothing is written at the handler, and a handler that matches
on `error` is told the same as one that does not ([ADR-101](adr/adr-101.md)).

> **Implementation status:** Not implemented. `std.contracts` writes
> `throws = ["?"]` on every entry, so there is no set to diff until error types
> are lowered ([ADR-101](adr/adr-101.md) §5).

**Every error carries its site and its chain.** The **site** is where the error
was raised. The chain holds every error that joined on the way: a cleanup that
failed while the stack was unwinding is attached to the original as a
*secondary* error rather than replacing it (6.4), and so are the other failing
branches of an `overlap` (8.1.2). The list is `error.secondary`, in the order
the errors joined, each with its own site. A `catch` catches one error and
chooses by its type ([ADR-115](adr/adr-115.md)). The site costs nothing at run
time: the compiler wrote it into the binary as text.

**A stack trace is not carried.** `NIKAIA_TRACE=1` asks for one; without it
there is none, and the long form says so ([ADR-036](adr/adr-036.md)).

*Design rationale:* a recoverable error is the expected kind, and capturing a
trace for each one costs far more than raising it, per rejected line, for a
value almost nothing reads ([ADR-036](adr/adr-036.md)).

**Printing it: short is the default.**

```nika
eprintln(f"{error}")           // the message, and nothing else
eprintln(f"{error.full()}")    // the site, the chain, and a trace if one was captured
```

`{error}` is the message the author wrote. An error message is for the
operator, not for the visitor of a web page, so a failed HTTP handler answers
with a generic 500 and logs the rest ([ADR-018](adr/adr-018.md)).

> The form you type without thinking is the one you may show a stranger.

`full()` is an ordinary call: a hole holds an expression (2.5), and a method
call is one.

Every error knows the **site that raised it** and has a short form of it a
person can read out. A screenshot carrying `(NK-2C7)` leads through
`nikaia explain NK-2C7` to the line that threw it, with no log file, and also
when the working tree has moved on ([ADR-023](adr/adr-023.md) D6).

**A call the language performs can fail.** Two places perform a call user code
did not write: the end of a block, where a resource is cleaned up (6.4,
`NK2601`), and a loop's step over a fallible stream ([ADR-025](adr/adr-025.md),
`NK2701`). In both the enclosing function declares `throws`, and the compiler
names the resource or the loop.

**It is one rule with three sites** ([ADR-025](adr/adr-025.md) D1). Where a
call can fail, written by user code or performed by the language, the failure
fails the enclosing function, the function declares `throws`, and the compiler
names the call that is the reason. `NK2605` is the written call, `NK2601` the
closing brace, `NK2701` the loop's step.

### 7.2. Unrecoverable Errors (`panic`)
An unrecoverable error is a logic bug, such as reading the tenth item of a
list of five. The program stops. Where the machine cannot unwind, the process
ends.

**The Panic Hook (`std::panic::on_panic`)**
An abort runs no normal cleanup. Before the process dies, or before the crashed
task is isolated where the program survives, the runtime calls one registered
function: the **panic hook**. It is the place for a crash dump, a crash report,
or flushing a diagnostics log.

```nika
use std::panic

fn main() {
    // Global, one per application. Set it early.
    // The hook may not pause — mid-panic there is nothing to pause on — and
    // that promise is on `on_panic`'s parameter type, not on the lambda
    // (ADR-102 D1): a lambda carries no promise of its own.
    panic::on_panic fn(info) {
        // 'info' carries: message, file/line, and the stack trace.
        // Pattern: open crash resources at startup, only WRITE here.
        crash_log.write_report(info)
    }

    run_app()
}
```

The rules:
* **Global, application-only.** There is exactly one hook per program, set by
  the application. A library that calls `on_panic` is refused with `NK2604`. A
  crash-reporting library exports a function that the application's hook
  calls.
* **It runs on every panic, on every target**: before the trap where the
  machine traps, before the task is poisoned where it unwinds. A supervisor
  (Part II, 12.8) receives its crash information from the same `info`.
* **The hook may block, briefly.** Where the process is ending anyway, blocking
  costs nothing. Where the program keeps running, the hook stays short and
  hands heavy reporting to something started earlier.
* **Diagnosis, not cleanup.** The hook does not flush buffered files or finish
  transactions: those objects may be broken in exactly the way that caused the
  panic, which is why a panic skips destructors. If the hook itself panics, the
  process aborts immediately.

**The hook may not pause.** That promise is on `on_panic`'s parameter type
([ADR-102](adr/adr-102.md) D1), not on the lambda; a lambda carries no promise
of its own ([ADR-141](adr/adr-141.md) D1). The form `fn(info) sync { … }` is
withdrawn (5.3).

> **Implementation status:** Not implemented. There is no `std::panic`. The
> trailing lambda itself is built, named arguments included
> ([ADR-102](adr/adr-102.md) §5).

---

## Chapter 8: Concurrency (Doing things at the same time)

A program performs several tasks concurrently at both values of
`user_parallelism`, such as waiting for a download while answering input. The
mechanism is **asynchronous execution**.

### 8.1. Async by Default
A function that performs I/O, such as reading a file or downloading a URL,
**pauses** without blocking the program. There is no `await` keyword.

Within one task, the order is the written order: `let a = fs::read("x")`
pauses, and the line after it does not run until `a` is there. What runs
meanwhile is some *other* task; a pause never forks one. A task exists only
where the program writes one (`spawn`, `par_iter`, `task::scope`), and
`.join()` on a handle is where two tasks meet again. The marker for waiting
stands where something branches, and nowhere else.

> **Implementation status:** Partially implemented. A function the ledger says
> can pause is an `async fn`; a file read suspends on the kernel's completion
> queue or an I/O worker's reply; and the executor is the only place the
> program parks ([ADR-055](adr/adr-055.md) §6 steps 1–4). No `.nika` file
> writes `async`, `await` or a runtime type name. Of the three ways to make a
> task, `spawn` is built (8.2); `par_iter` and `task::scope` are not.

### 8.1.1. Statement Order Is the Written Order

Two statements run in the order they are written, whether or not they touch
anything in common. No analysis stands between the source and the schedule. A
program that wants two things to run together says so (8.1.2).

The automatic reordering of operations that met on nothing, the `seq { … }`
block and the `ordering` build option are withdrawn
([ADR-050](adr/adr-050.md) D1, D7). The touch-set analysis is kept for one use:
it checks an `overlap` block's claim that its branches meet on nothing (D3),
and `--overlaps` reports on the blocks a program writes.

### 8.1.2. Asking for Overlap: `overlap { … }`

**Each statement in the block is a branch.** The block starts every branch, waits
for all of them, and its value is the tuple of their results **in written order**
([ADR-050](adr/adr-050.md) D2).

```nika
let (user, rights, prefs) = overlap {
    db::load_user(id)
    db::load_rights(id)
    cache::load_prefs(id)
}
```

**An `overlap` block is not a task.** The block ends before the function
continues, so nothing outlives it: nothing is moved, borrowing works as it does
anywhere else, and the crossing rules a `spawn` meets (Part II, 12.4) do not
apply. That is what makes it lighter than two `spawn`s.

**The branches meet on nothing, and the compiler checks it.** A branch pair that
meets on a resource is refused with `NK2104`, and the message names the
resource. A branch that **binds** a name is refused the same way.

**A branch that fails makes the block fail.** Where two branches fail, the
first **in written order** wins, so the result is reproducible. The other
failures are attached to the winner as its `secondary` list, in written order,
and a log or `nikaia explain` shows them under it ([ADR-115](adr/adr-115.md)).
Per-branch handling is a `catch` inside the branch. Combining failures is a
`catch` on the block, where the handler has the winner and `error.secondary`. A
branch cannot see another branch's failure, because branches meet on nothing.

**A branch is an expression.** Several steps in one branch are a block
expression inside it. Two branches that would both be multi-line blocks of the
same shape are a function called twice.

**The meaning is the same at both values of `user_parallelism`; the duration
differs.** At `no`, a branch that computes still overlaps with another branch's
*waiting*, because the waiting is not user code. Computation beside I/O
overlaps under both values; computation beside computation overlaps only under
`yes`. That is 1.2's rule.

The emitted Rust hands the branches that can pause to the vehicle first and
puts the tuple back into written order afterwards (D6). A block whose branches
are all of one kind pays for no permutation.

> **Implementation status:** Partially implemented. `overlap { … }` parses,
> every branch is in flight at once, the block's value is the results in
> written order, `NK2104` is raised, and an uncaught failure fails the block
> with the first in written order winning ([ADR-050](adr/adr-050.md) D2–D6,
> [ADR-055](adr/adr-055.md) §6 step 5). The destructuring `let` the example
> writes is not built: `let (user, rights, prefs) = overlap { … }` does not
> parse, because `let` takes one name; Part II 12.5's
> `let (tx, rx) = channel::bounded(100)` writes the same form, and
> `docs/open-work.md` carries it. Today an `overlap` is reached by its tuple:
> `let r = overlap { … }`, then `r.0`.

### 8.2. Spawning Tasks
`spawn` starts a new independent task. It takes a lambda holding the code to
run: the one lambda form (5.3), whose body is a block whether it holds one line
or several.

```nika
spawn fn { println("I am running in the background!") }
```

### 8.3. Data Ownership in Tasks (Implicit Move)
A background task may keep running after the function that started it has
finished, so it cannot *borrow* a variable that might be gone by the time it
runs. Under the contextual capture rules (5.4), `spawn` is a **detached
context**: a variable used inside the task is **moved** into it. There is no
`move` keyword; the compiler applies the rule.

```nika
let message = "Hello"

// 'message' is implicitly moved into the task (spawn is @detached)
spawn fn { println(message) }

// Compiler Error: 'message' now belongs to the task.
// println(message)
```

**The type of the value decides between two cases.** The rule above is about
**ordinary data**: a string, a number, a struct or collection of those. For data
a move is a move. A handle on a `Shared[T]` is the other case: the handle is
**duplicated** rather than moved (6.2), so the name outside the task keeps
working and nothing is written ([ADR-040](adr/adr-040.md) D1). `message` here
is a string, so the rest of this section is the **data** case.

A program that needs the value afterwards clones it **before** the task is
built and gives the task the copy. A `.clone()` written *inside* the body does
not help: the body runs after `message` has moved into the task, so it would
clone the task's own copy and leave nothing behind for the parent.

```nika
let message = "Hello"
let copy = message.clone()   // made here, while `message` is still ours
spawn fn { println(copy) }   // the copy is what moves into the task
println(message)             // OK: `message` never left
```

Data moved into a task and used again afterwards is refused with `NK2101`:

```text
error[NK2101]: this background task takes ownership of `message`
  --> main.nika:3
   |
 3 | spawn fn { println(message) }
   |                   ^^^^^^^ moved into the task here
 4 | println(message)
   | --------------- but `message` is used again afterwards
   |
  note: a task started with `spawn` may outlive this function,
        so it cannot merely borrow your variables — it takes them with it
  help: clone before the task is built, and give the task the copy:
        let copy = message.clone()
        spawn fn { println(copy) }
```

**`NK2101` belongs to the data case only.** For a handle on a `Shared[T]` there
is nothing to refuse: using the value again after the task is built is what the
duplication of 6.2 serves, so there is no error and no `.clone()` to write
([ADR-040](adr/adr-040.md) D5).

`NK2101` is raised only where the type is known and a move takes the value
away. A number, a `bool`, a `char` and a **view** are copied, so
`let message = "Hello"` is not this case and `"Hello".to_string()` is. An
**assignment** between the task and the later use clears it, because giving the
name a value again is a correct program. The form above is the one spelling of
a `spawn`, the trailing lambda of 5.3. A lambda that **names** an argument is
refused with `NK2103`: a task is handed nothing.

**A task means the same thing at both values of `user_parallelism`.** At `yes`
a task goes to a pool of futures over the `user-pool` worker count and runs on
a thread of its own. At `no` every task interleaves on the one thread
(Part II 11.2). Because a task moves between threads at `yes`, its future must
be `Send` ([ADR-055](adr/adr-055.md) §2 D6).

> **Implementation status:** Partially implemented. `spawn` lowers: the body
> becomes a future the executor owns, `.join()` is a suspension point, and a
> task nobody joins still runs, so the example at the top of 8.2 prints after
> `main` has gone on ([ADR-055](adr/adr-055.md) §6 step 4). `NK2101` and
> `NK2103` are raised, and both values of `user_parallelism` are built. A
> future that is not `Send` is refused by the backend, about the generated
> file, rather than by the compiler about the program (Part III, C.1).

### 8.4. The Runtime Sidecar Model
At `user_parallelism = no`, user code runs on one thread. The runtime uses a
**hidden sidecar** to carry out heavy I/O without blocking that thread.

* **Separation of concerns:** user code runs exclusively on the main thread
  (the event loop). A heavy operation, such as an SQLite query, is offloaded to
  a runtime sidecar: a background thread on a native target, a Web Worker on
  WebAssembly.
* **Safety guarantee:** data is exchanged by message passing (ownership
  transfer). User code never accesses the sidecar's memory, so a **race
  condition** remains impossible.
* **Non-blocking:** a database call is a pause point. The main loop never
  stalls waiting for disk I/O.

**The runtime starts before the first statement of user code**, with one I/O
thread always and a pool for user code only at `user_parallelism = yes`
([ADR-038](adr/adr-038.md) D4). What runs on the I/O thread is `std`'s own code
and nothing else: the boundary is a closed list of operations rather than a
queue of closures, which makes "user code never accesses the sidecar's memory"
a property of the compiler ([ADR-037](adr/adr-037.md) D2, ADR-038 §4.2). A file
read is handed to the **kernel** where the machine can complete it, so the
commonest heavy operation involves no sidecar thread (ADR-038 D3). A file
operation is a *slot*, on the ring or in an I/O worker's reply; asking whether
it has finished never blocks, so the main loop runs whatever else is ready and
parks in the I/O only when nothing is.

> **Implementation status:** Partially implemented. The sidecar and the pause
> point are built for files ([ADR-055](adr/adr-055.md) §6 step 3). A database
> call and standard input are not: `std::db` does not exist, and a stream needs
> the readiness half of ADR-038 D3's mechanism, which is built for sockets and
> not wired to stdin. Both are `async` in their signatures and finish on their
> first poll.

---

## Chapter 9: Project Organization and Visibility

### 9.1. Packages and Files
**A package is a directory.** The files in it see one another with no `use` at
all: they share one namespace, and a name declared in any of them can be written
in any other ([ADR-047](adr/adr-047.md) D1).

```nika
// file: src/parse.nika      (the same package as src/main.nika)
pub struct Row { pub id: i64 }

fn helper() -> i64 { return 41 }
```

```nika
// file: src/main.nika
fn main() {
    let r = Row { id: helper() + 1 }   // no `use`, and no prefix
    println(f"{r.id}")
}
```

Two files of one package may **not** declare the same name. There is one
namespace, and two `Row`s in it are refused.

**A package reaches another package by its name.** `use http` makes the package
`http` reachable and does nothing else. Every name from it is written with its
prefix, at every use ([ADR-046](adr/adr-046.md) D1).

```nika
use http
use request_handling as rh

fn handle(r: http::Request) -> rh::Response {
    let body: http::Body = http::read(r)
    return rh::ok(body)
}
```

The prefix falls where a name is **written**, not where a value is used:
`r.path` and `r.header("host")` carry none, because there is a value in hand
and nothing to resolve.

**No name is brought in**: not by a glob, not by a braced list, not one at a
time ([ADR-046](adr/adr-046.md) D2):

```text
error: names are not brought in; a package is reached through its name
   1 | use http::{Request, Response}
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
     = write `use http`, and `http::Request` where you need it
     = if the prefix is long, `use http as h` shortens it once, in one place
```

`use x as y` shortens a long prefix once, in one place (D3).

*Design rationale:* every line says where each name in it comes from without
the top of the file, and no edit elsewhere changes what an already-written line
means ([ADR-046](adr/adr-046.md) D2).

Two more rules come with it. **A prefix is introduced before it is used**:
`http::Request` without `use http` is refused, so a file lists what it depends
on at the top (D4). **One name per file**: two packages under the same name in
one file, by alias or by collision, are refused (D5).

**A name denotes one thing** ([ADR-144](adr/adr-144.md) D1). A second
declaration is refused with `NK1148`, with the caret on the one that arrived. A
`fn`, a `struct`, an `enum`, a `trait` and a `grammar` declare a name. A
**method** belongs to its type, so two types may each have a `len`; a **rule**
belongs to its grammar and is reached as `Json::value`. That is the same rule
one namespace down from *two files of a package may not declare the same name*.

`use std::fs` is the one `use` with a path in it, and it names the standard
library rather than a package ([ADR-030](adr/adr-030.md) D1). **It brings no
name in either** ([ADR-140](adr/adr-140.md) D5): it names a module and the
module's items are reached through it, exactly as a package's prefix works. A
`use` whose last segment is a **type** is refused with `NK1156`, because it
does nothing: what needs no `use` is the list in 1.3, and a type is not reached
by naming it twice. `HashMap` is **not** on that list
([ADR-154](adr/adr-154.md) D3): it is `use std::collections` and
`collections::HashMap`, the same shape this section gives a package.

A diagnostic names the **package** rather than the alias: the type is
`http::Request` whatever one file calls the package, and the `use` line that
connects the two is at the top of the same file. Outside a project a `.nika`
file is compiled **on its own**: a package is a directory of a project, and a
directory of loose examples is a directory of programs.

> **Implementation status:** Partially implemented. Every `.nika` beside the
> entry takes part in one namespace; a package is depended on by **path**,
> `http = { path = "../http" }` in `[dependencies]` (Part III, 13.3), and a
> `use` naming one resolves. Two files declaring the same name, a `use` naming
> a file beside this one, a `use` naming no dependency, one name twice
> (`NK1148`), and the braced and glob forms are refused
> ([ADR-046](adr/adr-046.md) §5, [ADR-144](adr/adr-144.md) §5). `use http as h`
> is built for a call, a type and a struct literal. A `use std::…` naming a
> type is refused with `NK1156` ([ADR-140](adr/adr-140.md) §5 step 5). Not
> built: a package's own
> package dependencies are refused rather than resolved; and a version does not
> yet find a package, which resolves through Cargo under the crate name
> `nikaia_<name>` ([ADR-103](adr/adr-103.md) §5).

### 9.2. Visibility Rules (Privacy)
Visibility is per package.

1.  **Private to its package unless it says otherwise:**
    * A function, a struct, an enum and a `comptime` binding are visible inside
      the **package** that declares them, in every file of that directory, and
      nowhere else.
    * A struct field is the same: visible throughout the package that declares
      the struct.

2.  **The `pub` keyword:**
    * An item **another package** may use is prefixed with `pub`.
    * A field another package may reach is prefixed with `pub`.
    * A **public type may keep its fields private**; 9.3 shows the ordinary
      case.

> **Implementation status:** Partially implemented. A `comptime` binding at
> **item level** is decided and not built: inside a function body it is a
> statement today, and `pub comptime` at the top of a file has nowhere to stand
> ([ADR-073](adr/adr-073.md) D2, §5; [ADR-077](adr/adr-077.md) for the word).

The boundary is the package and not the file ([ADR-047](adr/adr-047.md) D1).

*Design rationale:* a library's internal file layout is then not its public
surface, so moving a declaration from one file to another breaks no consumer
([ADR-047](adr/adr-047.md) D1).

Reaching a private item from another package is refused with `NK1110`, and the
message says which package keeps it:

```text
error[NK1110]: `secret` is private to `utils`
   4 |     let n = utils::secret()
           ^
     = an item is private to the package that declares it unless it says `pub` (Part I, 9.2)
     help: write `pub fn secret` in `utils`, or reach it through something that is public
```

> **Implementation status:** Implemented. `NK1110` is raised for an item and
> for a field, both for reading a field and for giving one a value in a struct
> literal. The ledger carries a field's `pub` because the backend cannot
> enforce it: a dependency's items are in the same crate
> ([ADR-047](adr/adr-047.md) §5).

### 9.3. Granular Control
`pub` makes an item available to every package. A private field is reached
through a constructor or a method, so a type's invariants hold outside its
package.

```nika
// file: network.nika

// Private: Only usable inside network.nika
struct Config {
    port: i32
}

// Public: Usable by anyone
pub struct Server {
    // Private field: Can only be changed by Server methods
    config: Config,
    
    // Public field: Can be read/written by anyone
    pub name: String 
}
```

