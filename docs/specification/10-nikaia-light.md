# Nikaia Language Specification
**Part I: The Language Core**
**Version:** 0.0.7 (Draft)
**Date:** September 5, 2026

---

## Chapter 1: Introduction and Philosophy

### 1.1. What is Nikaia?
Nikaia is a programming language designed to solve a specific problem in software development: the trade-off between ease of use and technical performance.

In many languages, developers must choose between:
1.  **Scripting Languages** (like Python): Easy to read and write, but often slow and prone to errors that only appear when the program is running (**Runtime Errors**).
2.  **Systems Languages** (like C++ or Rust): Extremely fast and reliable, but difficult to learn and require writing complex code to manage computer memory.

Nikaia aims to combine the readability of a scripting language with the performance and safety of a systems language. The developer writes simple code that focuses on the logic (the "Happy Path"). The **Compiler** (the program that translates your code into machine-readable instructions) automatically handles the complex technical details in the background.

### 1.2. One Language, and the Switches
There is one Nikaia. The same source compiles for every machine and at every
setting, and prints the same bytes. What you choose when you build is **how**,
never **what**.

These are the things to choose. They are named rather than counted: the count
has been wrong twice — once by leaving `ordering` out and once by leaving the
re-entrancy check out — and a number nobody maintains reads as a promise.
`ordering` was the third and is **withdrawn**
([ADR-050](adr/adr-050.md) D7): statements run in the order they are written,
so there is nothing left for it to switch, and a program that wants overlap
writes `overlap { … }` (8.1.2).

#### `target` — which machine
A 64-core server has threads and unwinds a stack when something goes wrong;
WebAssembly has neither. So the machine decides what the standard library can
offer — whether a file can be memory-mapped at all — and what happens on a
`panic`: an orderly unwind where the machine unwinds, an immediate trap where it
traps.

The default is `x86_64-linux`.

#### `user_parallelism` — may *your* code run concurrently at all?
* `no` (the default) — nothing you wrote ever runs concurrently. A web service
  that wants one event loop is built this way, and **data races are
  impossible**: two pieces of your code are never in flight together, so there
  is nothing to collide.
* `yes` — it may. This is what an image filter or a scientific calculation
  wants, and the compiler enforces the rules that keep shared data intact.

It is a permission and not a count. *How many* threads or cores serve a `yes`
belongs to the machine and the moment, so the runtime decides it; a number here
would be a promise the language cannot keep on hardware it has not seen.

**The word *your* is the whole of it.** This bounds your program, not the
compiler. Reading a file may still validate its text on four cores at
`user_parallelism = no`, and the runtime may still hand a blocking call to a
helper thread — neither runs code you wrote, and neither changes a single byte
of what your program prints. The rule is:

> The compiler may use as many threads as the machine has, for as long as no
> code **you** wrote runs concurrently.


#### the re-entrancy check — should a broken rule be noticed?
Taking a lock while a lock is held is refused when you compile (Part II, 12.3).
This switch decides whether a program *also* carries the run-time check
that notices such a nesting if one ever gets through. It is on by default, and
it can be declined.

This is the switch's own guarantee of the rule above it: **for every program
that obeys the nesting rule, both builds behave identically.** The check cannot
fire in a correct compiler, so what it controls is not error handling but
self-control — if it ever fires, the compiler has a hole, and without the check
that hole would show up as a silent hang instead. It is not a development aid to
be removed later; it is a guarantee that may be declined
([ADR-039](adr/adr-039.md) D8, D2).

> **Status:** not built. Nothing refuses the nesting of Part II 12.3, no
> re-entrancy check is emitted, and `nikaia.toml` has no key for this one — it
> is the one switch here specified ahead of the manifest that would carry it.

Each of them lives in `nikaia.toml`, and `--target` and `--user-parallelism`
override those two for a single build ([ADR-037](adr/adr-037.md),
[ADR-033](adr/adr-033.md) D8, [ADR-039](adr/adr-039.md) D8).

---

## Chapter 2: Variables and Data Types

A **comment** begins with `//` and runs to the end of the line, or begins with
`/*` and runs to the matching `*/` — across lines, anywhere whitespace may
stand, and **nested**: `/* a /* b */ c */` is one comment, so a block that
already holds one can be commented out ([ADR-134](adr/adr-134.md)). An unclosed
`/*` is reported where it opened. There is no doc comment: `///` and `/** … */`
are ordinary comments that happen to begin so.

### 2.1. Variables and Assignment
A **Variable** is a named storage location in memory that holds a value. In Nikaia, variables are declared using the `let` keyword.

**Immutability**
By default, variables are **Immutable**. This means once a value is assigned to a name, it cannot be changed. This prevents accidental modification of data.

```nika
let x = 10
// x = 20  <-- This would cause a Compiler Error
```

**Mutability**
To allow a variable to change, you must explicitly mark it as **Mutable** using the keyword `mut`.

```nika
let mut y = 10
y = 20     // This is allowed
```

**Reserved words**

These words mean one thing wherever they appear, so a name may not be one of them
([ADR-051](adr/adr-051.md) D1):

```text
as        break     catch     comptime  continue  dsl       else      enum
extern    false     fn        for       grammar   if        impl      in
let       match     mut       null      overlap   pub       return    self
spawn     struct    sync      throw     throws    trait     true      unsafe
use       while     with
```

**`comptime` is on the list because its construct exists** — a statement
inside a function body (Part II, 10.2). The word every neighbouring language
uses, `const`, would say *this one does not change*, and 2.1 already gives that
to every binding that does not say `mut`; what the declaration promises is a
**time**, and `comptime` says so ([ADR-077](adr/adr-077.md)).

**`_` is not a name; it is the ignore pattern** ([ADR-126](adr/adr-126.md)). It
stands where a name would be bound and says that the value is ignored on
purpose: a position of a destructured tuple (`let (name, _) = pair()`), a
parameter a shape dictates (`fn handle(event: Event, _: Context)`, `fn(_,
value) { … }`), and a `match` arm. `let _ = expr` is refused — a call made for
its effect is written as the call, and a resource is closed by name — and `_`
is never a value. What it ignores is not moved, so a `let` over a place stays a
view of it (6.5).

**`with` is on the list for its construct**: a copy of a value with named
fields changed, `p with { x: 1 }` (4.2, [ADR-118](adr/adr-118.md)).

**`extern` and `unsafe` are on it with their constructs**, which is the
condition rather than an accident of timing: an `extern "C"` block and the
`unsafe { … }` a call to one is written in arrived in the same change
([ADR-124](adr/adr-124.md), Part III 15.1). The number that allowed two words is
**zero** — nothing in the corpus, the tests or these pages wrote either as a
name, and nothing is released, which is [ADR-084](adr/adr-084.md)'s own
standard.

**`break` and `continue` were reserved before they were constructs and are
constructs now** (3.3, [ADR-084](adr/adr-084.md)) — which is what a reservation
is for, and why their arrival cost no program a name.

**`loop`, `const`, `macro`, `quote` and `from` are ordinary names**
([ADR-116](adr/adr-116.md), [ADR-117](adr/adr-117.md)). Each was once reserved so
that a reader arriving from another language could be told something; a name
nothing declares is told the same thing now, by the refusal every stray name
gets: `loop { … }` is answered with *write `while true`*, `const X = …` with
*write `comptime`*, `macro` and `quote` with *Nikaia has no macros* — and a
declared `loop`, `quote` or `from` is a name like any other.

**`seq` has left the list**, which is the direction a reserved word may move
without breaking anything: the construct is withdrawn
([ADR-050](adr/adr-050.md) D7), so the word means nothing and is an ordinary
name again. Reserving one narrows what parses and un-reserving one widens it,
so this direction costs no program anything.

Three things about the list, because each of them is a question a reader will
have:

* **A `grammar` block has its own vocabulary**, and it is not here: `rule`,
  `boundary`, `fold`, `par_fold` and `unchecked` are keywords *inside* one
  (Part II, 10.1) and ordinary names everywhere else (D2).
* **After a `::` or a `.`, a reserved word is a name.** A segment follows a `::`
  and a member follows a `.`, and no construct begins in either position - so
  `Self::dsl` (Part II, 10.5) and `scope.spawn fn { … }` (Part II, 12.5) are what
  they look like (D3).
* **`self` is on the list and is also a name** - the one the receiver of a method
  has. So *using* it is what every method body does; *declaring* one is refused
  (D4).

> **Status:** built. A name that is a reserved word does not parse, and the parse
> error names the word and says it is reserved.
>
> `self` is the one the grammar cannot refuse that way - `NAME` is the rule for
> declaring a name *and* for referring to one - so declaring it is `NK1119` from
> the checker: at a `let`, a `for` binding, a lambda's argument and a struct
> field. A **parameter** named `self` never parsed at all, because the receiver
> takes the word and the type beside it has nowhere to go; it says so in a
> sentence now rather than asking for a closing parenthesis
> ([ADR-051](adr/adr-051.md) D4).

### 2.2. Primitive Data Types
Nikaia provides basic types to represent simple values.

* **Integers:** Whole numbers without fractions.
    * `i64`: A large integer (64-bit). Used for most numbers, and what a
      **length** is: `xs.len()` hands back an `i64`, and an index is one
      ([ADR-048](adr/adr-048.md) D1).
    * `i32`: A standard integer (32-bit). Used where the layout matters — a
      struct that has to be small, a wire format, a C header — and a number
      that meets an `i64` there is widened where you say so, with `as i64`.
    * `u8`: One byte. What reading a file hands back a list of
      (`fs::read` → `Vec[u8]`), which is why it is named here — a type a program
      meets has to be a type the specification offers. It has the same
      conversion and arithmetic names as the others.
* **Floats:** Numbers with decimal points.
    * `f64`: Double precision floating-point number. A literal may carry an
      **exponent** — `1.5e-4`, `2e3`, `9.54791938424326609e-04` — which is how a
      program about physical quantities is written; the same number spelled out
      in zeroes is how a digit gets lost.
* **Booleans:** Logic values.
    * `bool`: Can only be `true` or `false`.
* **Text:**
    * `String`: text. Whether a value of it is a view into text that is
      already there, or text of its own, is the compiler's to pick per use
      (6.6, [ADR-107](adr/adr-107.md)); a literal is a view of the program's
      own text and allocates nothing.
    * `&str`: the same text with a promise attached — *this is a borrowed
      view, no copy and no handle* — and the compiler holds the program to
      it. Written where allocating would be a mistake.
    * `char`: One character — a Unicode scalar value, not a byte. Written
      between single quotes, with the escapes a string uses: `'a'`, `'\n'`,
      `'\''`. It is what iterating a text yields (`for c in name.chars()`) and
      what a `match` over one compares against (3.4); a text is not a list of
      `char`, and turning one into the other is a decision a program makes
      rather than something that happens to it.

**A number is written in digits, and the exponent above is the only other thing
in one.** There are no digit separators, no radix prefixes and no type suffixes,
so `1_000`, `0xFF` and `1i64` are each not a number but a number beside a name —
and the name beside it is refused, as `NK1117`, because nothing declares it
(Part III, C.3). This language has **no word it does not know**: one that stands
on its own is read as a name, so `assert c` and `unsafe { … }` are refused the
same way rather than being read as constructs that are not there.

**These four are the integer types a program writes.** The compiler accepts more
— `u32`, `u64` and the machine-width `usize` among them — and the specification
does not offer them: an entry exists because a program asked for it, and none has
([ADR-028](adr/adr-028.md) D5). A **length** used to be the one place a program
met the machine-width type, and since [ADR-048](adr/adr-048.md) D1 it does not:
`for i in 0..xs.len()` gives an `i64`, `xs[i]` takes one, and neither conversion
is written — the compiler emits both. A negative index reports as an access out
of bounds, because that is what it is (Part III, A.2).

**An integer that does not fit aborts, at every build.** An `i32` holds what an
`i32` holds; an arithmetic result that does not is an inconsistent program state,
like an index past the end of a list or a division by zero (Part III, A.2), and
the program stops rather than carrying a number nobody computed. It is the same
at both settings of `user_parallelism` and in every build, so the same program
computes the same thing wherever it is built — which is 1.2's rule, applied to
arithmetic ([ADR-043](adr/adr-043.md) D1).

Three of these are easy to miss, because the code looks harmless:

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

`wrapping_add`, `wrapping_sub`, `wrapping_mul`, `wrapping_div`, `wrapping_neg`,
`wrapping_abs`, `wrapping_shl`, `wrapping_shr`, and the same names with
`saturating_`. There is no operator for either: both are rare, both are meant to
be visible in the line that does them, and a sign for one would have to be spelt
with `&`, which is borrowing here ([ADR-043](adr/adr-043.md) D2, D3).

**A conversion is written `as`, and one that may not fit aborts too.**

```nika
let average = (total as f64) / (count as f64)     // widening, always fits
let small = big as i32                            // aborts if `big` does not fit
```

Otherwise the question would be answered at the front door and let in at the
back: whoever wants the digits thrown away says so, the same way wrapping is said
([ADR-043](adr/adr-043.md) D4).

**And where a program means to keep only the low digits, it says so by name**, as
it does for wrapping. The name carries the type it converts to, because that is
what it hands back:

```nika
let small = big.truncating_i32()                  // keep the low digits, on purpose
let floored = measurement.truncating_i32()        // a float, toward zero, clamped
let n = text.len().truncating_i32()               // a count that may not fit
```

`truncating_i32` and `truncating_i64`, out of the larger integer, out of an `f64`,
and out of the type a count has ([ADR-043](adr/adr-043.md) D7).

Three conversions are easy to miss here too:

* **An `f64` to an integer** was silent three ways: `1e20 as i32` gave the largest
  `i32`, `-1e20 as i32` the smallest, and a value that is not a number gave `0`.
  All three abort now.
* **A count** — what `len` hands back — is as wide as the machine is, so what fits
  on a large machine does not on a small one. It is checked for that reason: the
  alternative is a program whose behaviour depends on where it was built.
* **An integer to an `f64`** is the one that is **not** checked. Digits go at large
  values without anything overflowing — `9007199254740993` through an `f64` comes
  back `9007199254740992` — and there is no sensible place to stop, so this is a
  limit written down here rather than an abort.

**An `as` names one of the types above and nothing else.** `n as u128` and
`n as usize` are refused as `NK1122`, naming the type: a conversion into
something this page does not offer went to the language below unread, so a value
could have a type there is no word here for — and `-3 as usize` was
18,446,744,073,709,551,613, silently, in the middle of a rule that says a
conversion which does not fit aborts ([ADR-054](adr/adr-054.md) D1).

**Where a machine-width number is what the language below wants, the compiler
writes the conversion.** A length comes back as an `i64` and an index goes in as
one ([ADR-048](adr/adr-048.md) D1); so does a **count**, which is why
`"  ".repeat(indent)` is written with no conversion at all and a negative count
aborts saying *"a count cannot be negative"* rather than becoming an enormous one
([ADR-054](adr/adr-054.md) D2).

**A literal that does not fit its type is a compile error, not an abort.**
`let x: i32 = 3000000000` is decidable where it is written, and so is a sum of
literals that cannot fit, so neither waits for the program to run
([ADR-043](adr/adr-043.md) D5).

> **Status:** the abort is built — the generated project carries the check for
> your program and turns it off for every Rust dependency, whose own arithmetic
> is not this compiler's to be right about
> ([ADR-043](adr/adr-043.md) D6), and
> `crates/nikaia/tests/overflow.rs` compiles an overflowing program and runs it.
> The `wrapping_` and `saturating_` names are built for `i32` and `i64` — the two
> integer types named above — and `crates/nikaia/tests/overflow.rs` runs them
> beside the same arithmetic with `*`, which aborts. `saturating_shl` and
> `saturating_shr` do not exist, here or in the language below. The out-of-range constant is
> refused here now, as `NK1116`, wherever a type stands beside it: an annotated
> `let`, a `return` against a declared result, or an argument whose parameter says
> what it takes. It prevented no abort — one never happened — and what it takes
> back is the message, which was the backend's, in Rust's words, about a file
> nobody wrote.
>
> **A sum reaches further than a literal.** `let b = a + 1`, where `a` is a
> constant an annotation declared an `i32`, is folded and refused with no
> annotation on the `let` line at all: the operand's declaration is what gives the
> arithmetic a type. `+ - * / %` and a negation fold, through any number of
> immutable `let`s, in a wider number than either type so that the message can
> name what the expression comes to. Everything else stops the fold and is
> accepted — a `mut` local, a parameter, a `for` binding, a cast — and an
> expression that does not fold is never refused.
>
> And a division whose divisor is a constant zero is `NK1118`
> ([ADR-043](adr/adr-043.md) D5.5). Every other division by zero stays where
> Part III A.2 puts it: unrecoverable, at run time, naming this line.
>
> The narrowing check is built too, and `crates/nikaia/tests/overflow.rs`
> compiles the conversions with `-O` and **no** check flag — a conversion carries
> its own answer rather than taking one from the build, because `as` in the
> language below truncates by definition and has no setting to turn on. So
> `5000000000 as i32` aborts where it used to print `705032704`, and
> `big.truncating_i32()` is the same `as` it always was. The `truncating_` names
> exist for the three sources above and for `i32` and `i64` as destinations.
>
> **A literal that nothing at all constrains** — `let big = 3000000000` on its
> own — is not this section's case and is not refused: it is an `i64`, because an
> `i32` does not hold it ([ADR-060](adr/adr-060.md), and 2.4). What is still
> refused the other way (Part III, C.1) is a **sum** of literals that each fit and
> whose total does not, `let b = 2000000000 + 2000000000`: no literal there is out
> of range, so nothing widens, and the language below refuses the arithmetic.

### 2.3. Nullable Types (Null Safety)
In Nikaia, types are **non-nullable** by default. A variable of type `String` must always contain a string and cannot be `null`. To allow the absence of a value, the type must be explicitly marked with a trailing question mark `?`.

```nika
let strictly_string: String = "Hello".to_string()
// strictly_string = null // Error!

let mut maybe_string: &str? = null // Valid
maybe_string = "World"             // Valid (`mut`, as in 2.1)
```

A literal is a **view** of text the program was compiled with — see 6.6, where
an allocation happens only where you wrote that you wanted one, and [ADR-024](adr/adr-024.md) D5.
It stands wherever a `String` is wanted, because a `String` may be a view
([ADR-107](adr/adr-107.md)); `.to_owned()` is how you say you want a copy.

> **Status:** not built — `String` and `&str` are two types in the checker today,
> and a literal in a `String` slot is `NK1106` until [ADR-107](adr/adr-107.md) §5
> lands.

> **Status:** built ([ADR-052](adr/adr-052.md)). `T?` lowers to the language
> below's `Option<T>` — the mapping Part III 15.2 writes the other way round —
> and `null` is a reserved word (2.1) that lowers to `None`. The `?` comes last,
> after the type's arguments, so `Vec[i64]?` is a nullable list and `Vec[i64?]`
> is a list of nullables.
>
> **The second line of the example is the one that needed deciding.** A plain
> `String` standing where a `String?` is wanted is the one widening this language
> has, and the constructor is the **compiler's** to write: there is no `Some` in
> Nikaia and must not be, or the type's whole purpose becomes paperwork. It is
> written wherever a plain value meets a nullable slot: an annotated `let`, an
> assignment, a `return`, a struct-literal field, and a call argument.
>
> **And in all five it is written even where this compiler cannot work the
> value's type out** ([ADR-068](adr/adr-068.md)). It used to write nothing there,
> because a value that is *already* nullable must not be wrapped twice and it
> could not tell — so the program failed in the language below instead. What goes
> in that case is a conversion, which is right whichever the value turns out to
> be; the constructor stays wherever the type is known, because it says what the
> line means.
>
> `let m = null` with nothing beside it is **not** refused here, and that is on
> purpose: `let mut m = null` and then `m = "hi"` is a correct program, and this
> compiler has no inference to tell it from the one where nothing ever says. So
> the backend asks for the annotation, which is the honest answer.
>
> `(A, B)?` is deliberately absent — this section does not write it.
>
> Part I 3.5's `??` was already built and now has a type to be used on, and
> `?.` is built beside it.

### 2.4. Type Inference
Nikaia is **Statically Typed**, meaning the type of every variable is known at compile time. However, you rarely need to write types manually. The compiler uses **Type Inference** to deduce the type based on the value.

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

The use is asked first and the size second, which is why `small` may still become
an `i64` and `big` never has to be annotated to be one. A number too large for
an `i64` is refused, because nothing holds it.

**And a sum of numbers is a number** ([ADR-063](adr/adr-063.md)), so the same
rule decides it:

```nika
let c = 2000000000 + 2000000000   // an i64: nothing asks, and an i32 does not hold it
let small = 2 + 3                 // an i32, the same as any number that fits
```

**A name is where it stops, and on purpose.** A name already took a type, and
arithmetic happens in the type of its operands:

```nika
let a = 2000000000        // an i32 - the first type that holds it
let c = a + a             // refused: this comes to 4000000000, which an i32 does not hold
let b: i64 = 2000000000   // say so, and the sum is an i64
let d = b + b             // 4000000000
```

That is the answer every language with two integer widths gives, and the way out
is the one word on the third line.

> **Status:** built, both halves. The *use* half is the language below's
> inference; the size half is one rule in the emitter
> ([ADR-060](adr/adr-060.md)), a literal whose value an `i32` cannot hold written
> out as an `i64` — and a literal that fits left exactly as it was, so the use
> keeps deciding. The question is about the **value**: `-2147483648` is an `i32`
> although its digits are one too many. This compiler's own `NK1116` answers the
> case where a type **stands beside** the literal — an annotated `let`, a
> `return` against a declared result, an argument whose parameter says what it
> takes — and stays silent otherwise, because a literal whose use widens it is a
> correct program and refusing one of those is the one thing the checker may
> never do (Part III, C.4).
>
> **The sum is built too** ([ADR-063](adr/adr-063.md)), and with it every
> arrangement of a constant has an answer from this compiler rather than from the
> language below: widened where an `i64` holds it, refused as `NK1116` where no
> type does or where a name pinned a narrower one. The two positions where a
> number's type comes from **where it stands** — a sequence index and a repeat
> count, both counted by the machine — are left alone, because a type written
> into one would pin what the position is there to decide.

### 2.5. Strings, Plain and Interpolated
There are two string literals, and the difference is one character at the front.

```nika
let name = "Nikaia"

println(f"hello, {name} - {name.len()} characters")   // f"…" - the braces are code
println("hello, {name}")                              // "…"  - the braces are braces
```

**`"…"` is text.** A `{` is a brace and nothing else, so a program that writes JSON, CSS or a
regular expression says what it means:

```nika
print("{}")               // prints {}
print("\\d{3}")            // a regular expression, written as one
print("{ margin: 0 }")    // a rule, not a hole
```

**`f"…"` has code in it.** Between `{` and `}` stands an expression — not just a name, but a
field, a call, an index. What follows a `:` inside one says *how* to write the value rather than
which value: the first colon that is not inside a call or an index separates the two, so
`Point(x: 1)` in a hole keeps its own.

Inside an `f"…"`, two braces stand for one: `{{` is a literal `{` and `}}` a literal `}`. An
escape is not a hole — the `{` in `"\u{0041}"` belongs to the escape — and a `}` on its own is an
error rather than a guess. **A plain string needs none of that**: it has no holes to be told
apart from, so `"{"` is a brace and `"{{"` is two.

The `f` and the quote are **one token**. `f"x"` interpolates; `f "x"` is a variable named `f`
beside a string, and whitespace never decides what a program means.

**A newline is an ordinary character in a literal.** Nothing ends one but the
closing `"`, so a literal left unterminated runs on to the next `"` in the file
or, if there is none, to the end of it.

The type follows the syntax rather than the contents. `"…"` is a view of static text and `f"…"`
builds a `String`, whether or not anyone put a hole in it — so adding a brace to a piece of text
cannot quietly change its type.

**A template's holes need no `f`.** `dsl html { <p>{name}</p> } eod` (Part II) is already marked
as a place where code appears, and the mark belongs on the construct rather than on every brace
inside it. That is the same rule read twice: a literal that holds code says so.

---

## Chapter 3: Control Flow

Control flow determines the order in which individual statements, instructions, or function calls are executed.

### 3.1. Expressions and Blocks
Nikaia is an **Expression-Oriented Language**. This means almost every construct returns a value.
A **Block** is a group of statements surrounded by curly braces `{ ... }`. The last line in a block (without a semicolon) is the return value of that block.

```nika
let result = {
    let a = 5
    let b = 10
    a + b  // Returns 15
}
```

A block's last line is the **block's** value; `return` is the **function's**. So
a `return` written at the end of a block that is itself a value — a `match` arm
(3.4), an `if` branch whose value is taken (3.2), a `catch` handler (7.1) —
leaves the enclosing function rather than handing that block a value. The one block that is its own function is a lambda, whose
`return` leaves the lambda (5.3).

### 3.2. Conditional Logic (if / else)
The `if` expression checks a condition (a `bool`). If true, it executes the first block; otherwise, it executes the `else` block. Since `if` is an expression, it can be assigned to a variable.

```nika
let age = 18

let status = if age >= 18 {
    "Adult"
} else {
    "Minor"
}
```

After `else`, an `if` may stand where the block would — **`else if`** — and
a chain is one `if` inside another with the inner braces left out, so every
rule of `if` holds at every link ([ADR-132](adr/adr-132.md)):

```nika
let grade = if score >= 90 {
    "A"
} else if score >= 80 {
    "B"
} else {
    "C"
}
```

**A condition is an ordinary expression — every one the language has**, with the
same operators, the same precedence and the same associativity as anywhere else
([ADR-087](adr/adr-087.md) D1). `&&`, `||`, `!`, `??`, `as`, `null`, a tuple, a
range: nothing is special about this position.

**Except one thing, and it is about the brace rather than about the
expression.** The `{` after the condition opens the **body**, so an expression
that *starts* with a brace has to be parenthesised — otherwise the compiler
cannot tell where the condition ends:

```nika
if p == (P { x: 1 }) {          // the parentheses say which `{` is which
    …
}

if (match n { 1 => 10, _ => 20 }) > 15 {
    …
}
```

That is the whole of the difference between the head of an `if`, a `while` or a
`for` and any other position ([ADR-087](adr/adr-087.md) D2) — and it is a rule
about spelling, not about meaning: everything is reachable, and one character
says so.

### 3.3. Loops
Loops allow code to be repeated.

**The `while` Loop**
Repeats code as long as a condition is true.

```nika
let mut count = 0
while count < 5 {
    println(f"Count is {count}")
    count += 1
}
```

**The `for` Loop**
Iterates over a sequence (like a range of numbers or a list).

```nika
// Iterates from 0 to 4 (5 is excluded)
for i in 0..5 {
    println(f"Index: {i}")
}
```

`a..b` excludes its end and `a..=b` includes it. A range is an ordinary
expression — it may be given a name, passed, or indexed with — and it binds
**looser than every operator in it**, so `0..n - 1` is a range ending at
`n - 1` rather than a range with something subtracted from it. That is the
reading a loop head wants and the only one that is ever useful.

A `for` over a list **lends** it: the elements are looked at, and the list is
still there when the loop is over. Taking them away is written,
`for x in xs.drain()` ([ADR-094](adr/adr-094.md) D4, and 6.5 for the rule it
is part of).

> **Status:** built ([ADR-094](adr/adr-094.md) D4). A `for` over a place lends
> it and `xs.drain()` is how a loop takes the elements away; a `&` written in
> front of the list is `NK1137`, because the compiler writes that reference. See
> 6.5 for the half that is not built — the caller still writes the `&` at a
> call.

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

A `break` leaves the **loop** and a `return` leaves the **function**. That is the
difference worth keeping straight: after the `break` above, the `println` runs.

Three things about them, and each is a decision rather than an accident:

* **Neither takes a value.** A loop here is a statement and hands back nothing,
  so `break` has nothing to carry out of one. `break n` is refused rather than
  read as a `break` followed by a statement `n` — which is what it would
  otherwise mean, with the value quietly dropped
  ([ADR-084](adr/adr-084.md) D3):

  ```
  error[NK1133]: nothing after a `break` in the same block is reached
  ```

* **There is no label.** `break` acts on the loop it is written in and there is
  no way to name an outer one ([ADR-084](adr/adr-084.md) D2). To leave two loops
  at once, leave the inner one and test outside it — or `return`, where the
  function has nothing left to do.

* **A jump does not leave a function**, so a `break` whose loop is outside a
  **lambda**, a **task** (`spawn`), an **`overlap` branch** or a DSL fold's step
  is refused ([ADR-084](adr/adr-084.md) D4). Each of those is a function of its
  own, and a jump is a jump to a place in *this* one:

  ```nika
  for x in xs {
      let each = fn (n) {
          break                  // error[NK1132]: the nearest loop is
      }                          //               outside this lambda
  }
  ```

  The way out is to decide inside and act outside — hand back a `bool` and let
  the loop test it. A `catch` handler is **not** one of these: a `break` in one
  leaves the loop around it, which is usually exactly what is wanted
  ([ADR-084](adr/adr-084.md) D5):

  ```nika
  for p in paths {
      let text = fs::read_to_string(p) catch { break }
      seen += text.len()
  }
  ```

**A loop can fail.** Some things a `for` walks are read *as it goes* — standard
input's lines are the one `std` has today. Getting the next one is real work,
and real work can fail. When it does, the loop stops and **the failure leaves
the function**, exactly as a failing call would (Chapter 7), and the compiler
makes you declare it:

```nika
fn tally() -> i64 throws {           // without `throws`: error[NK2701]
    let mut n = 0
    for line in io::lines() { n += 1 }
    return n
}
```

Nothing marks the loop, for the reason nothing marks a call that can fail
([ADR-023](adr/adr-023.md) D8) — and this is the same rule 6.4 already applies
to the *end* of a block, where a resource's cleanup can fail and the function
that owns it has to say so. One rule, two places the language calls something
you did not write ([ADR-025](adr/adr-025.md) D1).

What this rule exists to prevent is the alternative: a failed read that looks
like the end of the input, so a truncated stream becomes a shorter one and the
count is quietly wrong. 6.4 calls that "a decades-old bug class in other
languages", and it is the same bug at the other end of the block.

Most loops cannot fail. A range, a list, a map: nothing is read, so nothing
about them changes.

**And there is no third form.** A loop that does not end on its own is written
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

There is no `loop` keyword, and that is a decision rather than an omission
([ADR-070](adr/adr-070.md) D1). Go is the precedent, read carefully: it has no
`while` at all and lets `for` carry every loop shape, so what it shows is that one
keyword is enough — not that the unconditional loop is unnecessary. Nikaia picked
the other word to be the general one.

The word stays reserved, against the one thing that would reopen the question: a
`break` that hands back a **value** would make `while true` read as a lie, since
the head says the loop does not end and the body would say what it ends *with*.
This `break` hands back nothing ([ADR-084](adr/adr-084.md) D7), so it does not.

### 3.4. Pattern Matching (`match`)
The `match` expression compares a value against a series of patterns. It is similar to a "switch" statement in other languages but ensures that every possible case is handled.

```nika
let value = 2

match value {
    1 => println("One"),
    2 => println("Two"),
    _ => println("Something else"), // '_' catches all other values
}
```

A pattern is one of six things, and each is read the way it is written:

| pattern | matches |
| :--- | :--- |
| `_` | anything, and binds nothing — the **ignore pattern**, which also stands in a tuple position and as a parameter ([ADR-126](adr/adr-126.md)) |
| `1`, `"text"`, `true`, `'n'` | that value |
| `Op::Times` | that variant |
| `Message::Write(text)` | that variant, binding what it carries |
| `Message::Move { x, y }` | that variant, binding its fields by name |
| `other` | anything, and **binds it** to that name |

The last two lines are one rule: a path with `::` in it names a variant, and a
bare name binds. That is the same line the enum's own syntax draws — `Quit` is
a variant *of* `Message`, never on its own — so a pattern never has to be read
twice to see which of the two it is.

An arm's body is an expression or a block:

```nika
match step.0 {
    Op::Times  => { value = value * step.1 }
    Op::Divide => { value = value / step.1 }
}
```

### 3.5. Null Safety Operators
Accessing members of a Nullable Type requires handling the potential `null` case.

* **Safe Navigation (`?.`):** Accesses a member only if the receiver is not null. If it is null, the expression short-circuits to `null`. A **field** and a **method** are both members, and a method call takes its arguments there as it does anywhere ([ADR-066](adr/adr-066.md) D1).
* **Null Coalescing (`??`):** Provides a fallback value when an expression evaluates to `null`. More than one may be written: `a ?? b ?? c` takes the first that has a value (D4).

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

> **Status:** `??` is built, and 2.3's `T?` is what it operates on
> ([ADR-052](adr/adr-052.md)): `a ?? b` lowers to `a.unwrap_or_else(|| b.into())`.
>
> `?.` is built too. It lowers to `map` over a plain field and to `and_then`
> over one that is **itself** a `T?` — the second being the whole difficulty,
> since `map` there would leave `a?.b?.c` reaching through a nullable of a
> nullable. Which of the two is a question about the declared type, so the
> compiler decides it, the way 2.3's `Some(…)` is decided
> ([ADR-052](adr/adr-052.md) D6).
>
> The result is a `T?` either way, which is what lets `??` end a chain and `?.`
> continue one.
>
> **`?.` through something that cannot be absent is refused** as `NK1121`, and
> the way out is the plain `.` — a type that is not `T?` always has a value.
>
> **`?.` takes nothing.** It reaches through a view of its receiver, so `user`
> is usable on the line after `user?.name`; what comes out is a copy where the
> member copies and a view of the receiver otherwise, as a field read is (6.6)
> ([ADR-113](adr/adr-113.md)). **Not built yet**: today the lowering takes the
> receiver, and using it again is refused below with *"use of moved value"*.
>
> **`?.` reaches a method too**, because the sentence above says *member* and a
> method is one ([ADR-066](adr/adr-066.md)): `find(1)?.greet("Hallo")` calls it
> only where there is something to call it on, arguments reach it, and the result
> is a `T?` like any other reach. It flattens for the same reason a field does,
> where the method's own result is already a `T?`.
>
> It lowers to a `match` and not to `map`, which is the one place the two members
> differ: a method may **pause** and may **fail**, and a closure is where neither
> can happen.
>
> **And `??` chains**: `a ?? b ?? c` takes the first that has a value. It used to
> be a parse error naming the second `??` (D4).
>
> **A `?.` guards its own member and no more**, which is what safe navigation
> means everywhere: if the receiver is absent the whole expression is `null` and
> what follows is never reached, but a `.` written *after* the reach is reaching
> into a `T?`. In a language where `null` inhabits every type that is a crash at
> run time; here it is refused where it is written (`NK1125`, D6), because 2.3
> makes `T?` a type of its own and a member of `T` is not a member of it. So
> `a?.b.c` is refused and `a?.b?.c` is the program — every link that may be
> absent says so.

---

## Chapter 4: Data Structures

### 4.1. Structs (Custom Data Types)
A **Struct** allows you to group related values together under a single name.

**Visibility and Encapsulation**
In Nikaia, **everything is Private by Default**. This includes Structs and their Fields.
* To make a Struct usable by other modules, you must mark it `pub`.
* Even if a Struct is public, its fields remain private unless explicitly marked `pub`.

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
Because fields are private by default, you often cannot initialize a struct directly from another module using the standard `Type(field: value)` syntax. You must provide a public **Constructor**.

**The Anonymous Constructor (`pub fn`)**
Nikaia allows you to define a special function inside an `impl` block that has no name. This function is automatically called when you invoke the Type name like a function `User(...)`.

* **Internal Access:** Inside the `users.nika` module, code can access private fields to build the object using the standard Struct Literal syntax.
* **External Access:** Outside modules utilize the public anonymous constructor.

```nika
// file: users.nika
impl User {
    // The Constructor
    // It accepts positional arguments (Subject Zone)
    pub fn(username: String, email: String) -> User {
        // We can access private fields here because we are inside the module
        return User(
            username: username,
            email: email,
            is_active: true // Default logic handled internally
        )
    }
}
```

**A copy with fields changed: `with`.** Every binding is immutable unless it
says `mut`, so the value a program wants most often is the one it has with one
field different. `with` writes that without naming the rest
([ADR-118](adr/adr-118.md)):

```nika
let moved = p with { x: p.x + 1 }
let stats = old with { count: old.count + 1, sum: old.sum + t }
```

The braces are the struct literal's, with its field list and its shorthand.
Only the top level: a field of a field is `p with { pos: p.pos with { x: 1 } }`.
The fields not named are **moved** from `p`, never copied unseen — where one of
them is a text or a list and `p` is used afterwards, the refusal names the copy
to write. Across a package, `with` names `pub` fields only, as a literal does.
**Not built yet.**

**Usage Example**
External modules see `User` as a factory function.

```nika
// file: main.nika
use users::User

fn main() {
    // ERROR: Private Fields
    // Direct struct initialization is forbidden because fields are private.
    // let u = User(username: "A", email: "a@b.com", is_active: true)

    // OK: Public Factory Constructor
    // Calls the 'pub fn' defined in 'impl User'.
    // Note: Uses positional arguments as per Function Syntax.
    let u = User("Alice", "alice@example.com")
}
```

### 4.3. Why No Classes? (Data vs. Behavior)
Nikaia does not use **Classes** (a concept from Object-Oriented Programming). Instead, Nikaia separates them:
1.  **Structs** define the **Data** (what it is).
2.  **Impl Blocks** define the **Behavior** (what it does).

```nika
// Defining behavior for the User struct
impl User {
    fn login(&self) {
        println(f"{self.username} logged in.")
    }
}
```

### 4.4. Enums (Algebraic Data Types)
An **Enum** (Enumeration) is a type that can be one of several distinct variants.

```nika
enum Message {
    Quit,                        // carries nothing
    Move { x: i32, y: i32 },     // named fields, read by name
    Write(String),               // positional, read by position
}
```

Use one where a value is **one of a fixed set of things**: two operators, four
directions, the three states a connection can be in. A string would say the
same and check nothing — `"tiems"` is a value a string type accepts and an enum
does not. Reading one back is `match` (3.4), which is where the fixed set pays:
an arm per variant and no arm for a case that cannot happen.

### 4.5. Collections
Nikaia includes built-in types for storing groups of data.

* **List (Vector):** An ordered sequence of elements.
    ```nika
    let numbers = [1, 2, 3, 4]
    ```
    A list keeps the order it was given, and that order can be changed:
    `xs.sort()` puts the elements in their natural order, `xs.sort_by_key fn { … }`
    in the order of whatever the closure returns. **Both are stable** — elements
    the key does not separate keep the order they had — which is what makes two
    passes say a compound order without a comparator:
    ```nika
    names.sort()                                  // by name
    names.sort_by_key fn (name) { -report[name].hits }   // then by hits, descending
    ```
    That matters because of the line below: a map has no order to borrow, so a
    program that prints one says which.
* **Tuple:** A fixed number of values of *different* types, with no name for
    the group and no names for the parts. Written and read by position:
    ```nika
    let pair = ("*", 3)          // (&str, i64)
    let op = pair.0
    ```
    A tuple is the answer when a pair of values belongs together for one step of
    a computation and naming it would be a struct pretending to be a type — the
    element of a grammar rule that yields an operator and its operand, the two
    halves of a map entry in `for (name, value) in map`. Where the group has a
    meaning that outlives the step, it wants a struct (4.1) and its fields want
    names.
* **Map (HashMap):** Stores key-value pairs.
    ```nika
    use std::collections::HashMap
    let mut scores = HashMap::new()
    scores["Player1"] = 100
    ```
    Reading through the brackets answers a `T?`, because a key is data and may
    be absent: `scores["Player1"] ?? 0`, or `scores[name]?.rank ?? 0`
    ([ADR-114](adr/adr-114.md)); `get` says the same. A list's `xs[i]` stays a
    `T`, because an index is the program's own arithmetic and a wrong one is a
    bug (Part III, Appendix A). **Not built yet**: today a missing key aborts.

    A map is hashed according to where its keys came from: keys derived from data a remote peer supplied are hashed with a random per-run key, so nobody can pick keys that make your program crawl, and keys from data you supplied are hashed with the fast function. You do not configure this and, in the ordinary case, you do not think about it — see Part III, 17.1, for the cases where you want the last word. One consequence is worth remembering here: **the order you get when iterating a map is not guaranteed** and may differ between runs.

> **Status:** the **list literal is not built** — `[1, 2, 3]` is a parse error at
> the `[`, and so is a list *type* written `[User]`. Built: the type `Vec[T]`,
> the tuple, indexing (`xs[0]`), indexed assignment, and `HashMap::new()`.

### 4.6. Generics (Type Parameters)
To avoid writing the same code for different data types, Nikaia uses **Generics**. You define a type parameter inside square brackets `[...]`.

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

A parameter is written, never guessed, and it means two things depending on
where you stand. **Inside the body it is a type**: `x` is a `T` and a `T` is not
an `i64`, because the body did not pick what `T` is — its caller did. **At the
call it is filled in from what you pass**: `hand(n)` where `n` is an `i64` hands
back an `i64`, and `Box { item: n }` is a `Box[i64]`.

That is also why a `T` on its own has no members. Nothing has said which types
`T` may be, so nothing can say what one can do:

```nika
fn shout[T](x: T) -> String {
    return x.to_uppercase()   // error[NK1126]: `T` stands for a type the caller
}                             // picks, and nothing says it has a method
                              // `to_uppercase`
```

> **Status:** the parameter, the call that fills it in, and `NK1126` are built,
> for a `fn`, a `struct` and the `impl` over it. **A bound — `[T: Summarize]` —
> is not**, and cannot be until 4.7's `trait` declaration is: a bound names a
> trait, and a trait can currently be implemented but not declared. Until then a
> generic body may move and pass its value and nothing else
> ([ADR-074](adr/adr-074.md)). A generic `enum` is not read.

### 4.7. Traits (Defining Behavior)
A **Trait** defines a set of behaviors (methods) that different types can share.

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

A trait's methods are **signatures**: no body, because the declaration says what
a type must have and the `impl` says what it does.

Once a trait is declared, a type parameter can be **bound** by it — and that is
what gives a generic body something it may do (4.6):

```nika
trait Summarize {
    fn summary(&self) -> String
}

fn shout[T: Summarize](x: T) -> String {
    return x.summary()     // `Summarize` says there is one
}
```

The bound answers the whole call, not just whether it is allowed: how many
arguments `summary` takes, what they have to be, what it hands back, and whether
it can fail or pause all come from the declaration. Several bounds are written
`[T: Named + Aged]`.

**A trait method reads like any signature** ([ADR-109](adr/adr-109.md)): without
`sync` it may pause, without `throws` it cannot fail, and an implementation is
checked against that — a body that pauses under a `sync` declaration is refused
by name, and a body that does less than the declaration allows is fine. So a
trait can describe I/O, `fn load(&self) -> String throws`, and a call through
its bound pauses where the declaration says it may.

> **Status:** the declaration, `[T: Bound]`, `[T: A + B]` and the lookup are
> built ([ADR-078](adr/adr-078.md)), and so is what an `impl` owes its trait:
> a method the trait does not declare, or one it declares that the `impl` leaves
> out, is `NK1130`, and a method whose implementation **pauses** where the
> declaration says `sync` is `NK1129` ([ADR-080](adr/adr-080.md)). **A method
> without the word may pause and is not lowered so yet**: the emitter still
> writes a plain `fn` in every trait ([ADR-109](adr/adr-109.md) §5). **A differing signature is not
> compared yet**, so a method with the wrong arity or result is still refused by
> the language below. A **default body** has no syntax, a trait is not a type
> (there is no `dyn` and no `fn f(x: Summarize)`). A bound takes a path —
> `[H: http::Handler]` — and the ledger records a trait and each `impl` where
> they were written ([ADR-106](adr/adr-106.md)); neither is built, so today a
> trait cannot be named from another package.

---

## Chapter 5: Functions & Argument Architecture

### 5.1. The "Subject ; Config" Protocol
Nikaia enforces a strict separation between data (subjects) and configuration options to maximize readability. This is achieved via a dedicated **Semicolon Separator (`;`)** in function signatures.

**Zone 1: Subject (Positional)**
Arguments *before* the semicolon are the data the function operates on.
* **Syntactic Rule:** Positional arguments are allowed here.

**Zone 2: Configuration (Named Only)**
Arguments *after* the semicolon are options, flags, or modifiers.
* **Syntactic Rule:** Arguments here *must* be named. Positional usage is forbidden.

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

**The `;` stands between the two zones, and where one is empty it is not
written** ([ADR-133](adr/adr-133.md)). A function whose parameters are all
options is declared `fn execute(target_age: i64 = 0)` and called
`execute(target_age: 30)`; `execute(; target_age: 30)` is refused, so there is
one spelling. A mixed call keeps its `;`, and keeps it required.

> **Status:** the options-only form is not built; today it needs the leading
> `;` ([ADR-133](adr/adr-133.md) §5).

**Every configuration parameter has a default**, and that is what makes it an
*option*: a caller may leave it out, and leaving it out is never a question
about what the value is. A parameter that has to be passed belongs before the
`;`. Writing one without a default is a parse error that says so.

**A default is a literal.** An option's default is a constant in every program
anyone writes, and an arbitrary expression would raise a question with no
obvious answer — whether it is evaluated where the function is declared or where
it is called. That is worth deciding when something needs it.

**Order is the declaration's**, not the call's: `method` written first above is
still passed second, because only the declaration knows what the order is. This
is also why the ledger records an option's name, type *and* default (Part III,
13.5) — a call that leaves an option out still passes a value, and a consumer
compiling against a library cannot work out which.

**Optional Parentheses**
For functions defined without configuration or arguments, parentheses may be omitted to match the block lambda style.

```nika
fn init { 
    // No args, no parentheses required
}
```

### 5.2. There Is One Lambda Form
A lambda is written `fn { … }`, and 5.3 is the whole of it. There is no second, shorter form for
single-line bodies.

Naming the arguments — `fn(user) { … }`, 5.3 — is that one form spelled out rather than a
second one: the body is a block either way, and both spellings go in all the same places.

**The arguments are the ones it names.** A lambda that names none takes none, so
`fn { … }` is a lambda of no arguments. There were once three automatic names —
`a`, `b`, `c`, with the count read off which of them the body mentioned — and they
are **withdrawn** ([ADR-049](adr/adr-049.md)): a body reaching for one now names
something nothing declares, and that is `NK1117`.

The short form `fn: expression` is **not** part of the language. It saved four characters and its
body ran to the end of the expression, so a `.method()` chained after it landed *inside* the
lambda — silently, and a different program. Writing it is a compile error that names the block
form, rather than a parse failure, because the form appeared in earlier versions of this
specification and readers will have it in their fingers. Full reasoning:
[ADR-022](adr/adr-022.md).

### 5.3. Lambdas (`fn { ... }`)
When logic requires multiple steps, use a Block Lambda. Its arguments are the ones
it names.

* **Syntax:** `fn(name) { ... }`, and `fn(first, second) { ... }` for more than one
* **Args:** as many as the list says, and the list is the only thing that says so
* **A lambda that names none takes none**, so `fn { ... }` is the zero-argument
  form — which is what a `.or_insert_with fn { Stats(0) }` wants

```nika
let complex = users.map fn(user) {
    let bonus = calculate_bonus(user)
    // Implicit return of the last line
    user.score + bonus
}

let ids = users.map fn(user) { user.id }
```

**The automatic `a`, `b`, `c` are withdrawn** ([ADR-049](adr/adr-049.md)). They
were a second spelling in which the arguments were not written down at all and
*how many there were* was read off which of the three names the body mentioned — so
a local called `a` inside such a lambda was not a local but an argument. That rule
could not be changed while every lambda depended on it, and a body that reaches for
one of the three is now a body naming something nothing declares:

```text
error[NK1117]: nothing declares `a`, and this statement is just that name
```

**Trailing Syntax**
A lambda that is the last argument may go *outside* the parentheses, and where there are no other
arguments the parentheses go away with it:

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

> **Status:** built, in both positions, and the automatic names are gone from the
> compiler rather than left unreachable — the warning that made their rule visible
> (`NK1114`), the arity-from-body mechanism and the one function the emitter and
> the checker shared to compute it went with them ([ADR-049](adr/adr-049.md)).
> A trailing lambda may follow a method call with or without other arguments, a
> plain call, or a path: `users.map fn(user) { … }`, `numbers.reduce(0) fn(acc, n)
> { … }`, `access_all(a, b) fn(x, y) { … }` (Part II, 12.3),
> `task::scope fn(s) { … }` (Part II, 12.7), and the chain above.
> **Not built:** an effect marker on a lambda. A parameter list is followed by the
> body and by nothing else, so `fn(info) sync { … }` (7.2) ends the lambda at the
> `sync` and the line is read as three expressions rather than one.

### 5.4. Contextual Capture (The Lifecycle Rule)
Nikaia simplifies memory management in closures by automatically inferring whether to Borrow or Move variables based on the context in which the lambda is used. This behavior is the same at either `user_parallelism`.

#### A. Immediate Context (`@immediate`)
If a function guarantees that the callback will be executed and finished before the function itself returns, it is an **Immediate Context**.
* **Behavior:** Implicit Borrow (`&T`).
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
If a function stores the callback, executes it later, or sends it to another thread/task, it is a **Detached Context**.
* **Behavior:** Implicit Move (Ownership Transfer) for ordinary data. A handle on a
  shared value is **duplicated** rather than moved, so the name outside stays usable
  ([ADR-040](adr/adr-040.md) D1, and 6.2 for the rule).
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

Which of the two a lambda is in belongs to the **function that takes it**, not to
the call: `map` is immediate for every caller and `spawn` is detached for every
caller, and that is what lets the capture be decided where the lambda is
written.

**Your own function says it with a type** ([ADR-102](adr/adr-102.md)). A
parameter that is code is written as a function type, spelled the way a
signature is: `handler: fn(Request) -> Response`, and `sync` or `throws` after
the result where the declaration would put them. Without `sync` the code may
pause; without `throws` it cannot fail — the reading every declaration has. A
lambda that does less fits a type that allows more; a pausing lambda handed to
a `fn() sync` is refused.

Which of the two contexts your parameter is in is **inferred**, not written: a
parameter the body only calls is immediate and borrows, one the body keeps —
stores, hands back, gives to a task — is detached and moves. It is the same
question 6.5 asks of every parameter, and there is no `@detached` to write.
For an immediate parameter the callee's own promises follow the lambda, as
`map`'s do; for a kept one they follow the type, so a `listen` that calls a
stored `fn(Request) -> Response` may pause.

> **Status:** not built — a parameter of function type is a parse error, and
> the eight `std` entries that take a lambda are written straight into the
> ledger. The capture the rule decides **is** reported for the one detached
> context the language has a keyword for: `NK2101` at a `spawn` (8.3).
> [ADR-102](adr/adr-102.md) §5 is the order of work.

---

## Chapter 6: Memory and Ownership

Memory management is usually either manual (hard) or automatic via Garbage Collection (slow). Nikaia uses a third way: **Ownership and Borrowing**, handled by the compiler.

### 6.1. The Concept of Scope
When a variable goes out of **Scope** (usually at the end of the block `{}` where it was created), Nikaia automatically cleans up the memory. You do not need to free memory manually.

### 6.2. Unified Types
To make coding easier, Nikaia provides smart types that handle memory logic for you.

You write `Shared[T]` yourself — it is not inferred, because sharing changes *when* a value is cleaned up (6.4), and that is something your program can observe. What the compiler decides is the machinery underneath: **which count of owners each value gets**, one a second thread may safely touch where the value may reach one, and a cheaper one where it may not ([ADR-037](adr/adr-037.md) D7). What never depends on the build is the **answer** — whether a `Shared` may be handed to a task (Part II, 11.2) is decided from the type and from where it is going, at both settings and deliberately so ([ADR-045](adr/adr-045.md) D1). The count follows that answer; it does not make it.

**Where the compiler can prove that a particular value never leaves the thread that made it, it uses a cheaper count instead** ([ADR-037](adr/adr-037.md) D7). That is an optimisation and never a change of meaning: the program does the same thing either way, and the only difference is about 9 ns each time a handle is made and dropped — nothing at all for a handle you only read. **Where nothing proves it, the safe count is what you get**, and there is no way to ask for the other one: every case where the proof fails is a place something has not been written down, and writing it down is the way to the cheaper count.

**At `user_parallelism = no` every count is the cheap one** ([ADR-061](adr/adr-061.md) D2), and nothing has to be proved for it: there is one thread of yours, the runtime's own threads run no code you wrote, and a `Shared` may not be handed to code nothing written down describes ([ADR-061](adr/adr-061.md) D1) — so there is no other thread for a proof to be about. The way out of a program is to pass what is **inside** the `Shared`, a view or a copy; a foreign library that means to keep a value puts it in a hull of its own anyway. `nikaia --input x.nika --sharing` prints which count each of your values got, why, and what would have changed it. Like `--overlaps` and `--trust`, it explains a decision rather than changing one.

* **`Shared[T]`**: for a value several parts of the program own at once and nobody changes. The memory is only cleaned up when the *last* owner is finished.
* **`SharedMut[T]`**: for a value several parts own at once and any of them may change. The lock that keeps the changes apart is part of the type — there is no second wrapper to write around it — and the changing is done through the four doors of 6.3. This is the common case, which is why it has the short name ([ADR-039](adr/adr-039.md) D9).
* **`Locked[T]`**: for individually locked fields inside a shared structure — one lock per field rather than one lock around the whole of it. It is the same lock as the one inside `SharedMut[T]`, opened by the same four doors.

**How the second owner comes about.** You do not write the step that produces
one. Where a handle on a `Shared[T]` or a `SharedMut[T]` is handed on **by
value** — passed to a function that keeps it, or used by a task (Part II, 11.2) —
the handle is **duplicated**, and each handle is
cleaned up at the end of its own block (6.1). There is no method to call, because
there would be nothing for it to do: a duplicated handle copies none of the data
and produces no second value — one value, one more owner — so there is nothing to
name ([ADR-040](adr/adr-040.md) D1). Both are shared values, so the rule is the
same for both — though a `SharedMut[T]` has nowhere to be handed *to* across a
thread, because it may not cross one (Part II, 11.2).

**Lending the inner value out duplicates nothing.** `&` on a shared value is a
view of the value *inside* it, so a function that only uses the value takes an
ordinary view and never mentions sharing:

```nika
fn serve(db: &Connection) { … }

let db = Shared(postgres::connect("…"))
serve(&db)          // a view; no handle is made, and the count is untouched
```

Whether the value is shared is the caller's decision, and `serve` has no business
knowing. A signature names the shared type only where the function **keeps** the
value past the call — puts it in a structure, gives it to a task, hangs it on
something that outlives the call — because only then does it need a handle of its
own ([ADR-042](adr/adr-042.md) D1, D2).

**With `SharedMut[T]` you get in by opening it, and then it is an ordinary view
again.** There is no `&` straight through a lock: the caller opens it with one of
the four doors of 6.3 and passes the borrowed value in, and the called function
sees a plain value and must obey the rule for a lock that is open — it may not
pause, and it may not touch a lock of its own (Part II, 12.2).

```nika
let db = SharedMut(postgres::connect("…"))
db.access fn(open) { serve(open) }    // `serve` must be `sync` and lock-free
```

**How the first handle is made: you write it.** Each of the three shared types
makes one by being **called with the value that goes in it**
([ADR-064](adr/adr-064.md) D2):

```nika
let db = Shared(postgres::connect("…"))
let counter = SharedMut(0)
let frei = Locked(0)                    // a field's lock, one per field
```

**The rule behind it is one sentence, and it decides every hull in the
language:**

> **A hull you cannot see, the compiler writes. A hull you can see, you write.**

A `T?` costs nothing and hides nothing — the same value, possibly absent — so the
compiler puts a plain value into one for you (2.3). These three change **when the
value is cleaned up**, and that is something your program can observe. So the word
stands where it happens.

**It stands wherever an expression may**, which is the whole of what that buys:

```nika
keep(Shared(connect(url)))                      // an argument
let p = Pool { db: Shared(connect(url)) }       // a field of a literal
fn connect(url: String) -> Shared[Connection] {
    return Shared(Connection(url: url))         // a result
}
```

Where a plain value stands and a shared one is wanted, the message says exactly
that:

```text
error[NK1115]: `serve` takes a shared value, and `db` is not one
  help: write `Shared(db)` - a hull you can see is one you write
```

**Returning a shared value is no longer a special case**, and neither is a number:
`SharedMut(0)` works because the hull is made by a call, and a call gives the
number its type the way any other argument does.

**This is the handle and nothing else.** Ordinary data — a string, a number, a
struct of those — is still **moved** where it is handed on to something that
keeps it (6.5, 8.3). An automatic
duplication there would copy the whole of the data, which is a different thing at
a different cost, so for data the `.clone()` stays something you write yourself
([ADR-040](adr/adr-040.md) D1).

**Each handle lives to the end of its own block whether you use it again or
not.** The duplication is not conditional on a later use
([ADR-040](adr/adr-040.md) D2), so the value is cleaned up where *your* handle
ends, which may be later than the task that holds the other one. That is the
block rule of 6.1 applied unchanged rather than a special case to learn: whoever
wants the cleanup earlier ends the block earlier ([ADR-040](adr/adr-040.md) D3).
Where a handle is duplicated is not something your source shows, so `--sharing`
names each duplication site beside the count it printed for that value — the cost
of an extra handle stays something you can look up
([ADR-040](adr/adr-040.md) D5).

> **Status:** all three are built. Each is a type the compiler knows, `std`'s
> ledger carries their entries, and the constructors above really do make the
> hulls — wherever an expression may stand. `serve(&db)` works through the `deref`
> entry and nothing else ([ADR-042](adr/adr-042.md) D2), and the call that wants a
> shared value and is given a plain one is refused by `NK1115`, whose way out is
> the constructor (Part III, C.3).
>
> **`SharedMut[T]` is one name and two hulls**, and only the backend knows that
> ([ADR-064](adr/adr-064.md) D1): what a message, a printed type or a ledger entry
> says is the name you wrote. Which shapes it becomes is decided per value, the
> same way the owner count is. **`Shared[Locked[T]]` is refused** and the message
> names `SharedMut[T]`: one type, one spelling (`NK1123`).
>
> **`SharedMut[T]` and `Locked[T]` are not.** Writing either names a type that
> does not exist, the backend has no lowering for one, and the four doors of 6.3
> wait on the same thing ([ADR-039](adr/adr-039.md) §4).
>
> **The duplication is built for a handle handed to a function**, and not for one
> used by a task. A handle handed on by value — to a call whose parameter takes one
> — is duplicated, so the name outside stays usable; lending the inner value out
> duplicates nothing; and `--sharing` names each duplication site beside the count
> it printed ([ADR-040](adr/adr-040.md) D1, D5). The task half waits on `spawn`,
> which does not lower yet (Part II, 11.2) — the analysis sees it and names it, and
> no emitted program reaches it. What is built above all of it is the reasoning:
> which owner count a `Shared` value gets is inferred per value, and `--sharing`
> prints it ([ADR-037](adr/adr-037.md) D7).

### 6.3. Changing Shared Data: The Four Doors
A value behind a lock is not changed by assignment — the lock has to be opened
first, and the shape of the change decides which door you use:

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

Each door is shaped for what it is for:

* **`get` and `set` take no block at all.** `get` copies the value out. `set`'s argument is computed *before* the call, so everything slow about producing the new value — including waiting for I/O — happens outside, and the lock is open for one store.
* **`update` is handed the value as `mut v` and changes it.** It returns nothing: the change *is* the result, and `mut` is the word every changed parameter carries (4.3). Whether `v` is a copy of a small value or the address of a large one is the compiler's to decide, and a block may be run more than once where that is free — which is why it may not do I/O or take another lock ([ADR-110](adr/adr-110.md)).
* **`access` is for where copying is too expensive** — a list of ten thousand entries is not copied to be asked its length. It hands your block the value where it lies, to **read**; it may not change it.

Because `update` and `access` run your code while the lock is open, that code
must be able to run straight through: no I/O, and no second lock. The compiler
checks both (Part II, 12.2 and 12.3).

**What you take out of a lock is stamped.** `kasse.get()` is a `Seen[i64]`,
and so is what `access` computes. A `Seen` reads like the value it carries —
print it, compare it, send it in a response, hand it to any function that
touches no lock — and the stamp goes with it through arithmetic, calls,
struct fields (declared `Seen[…]`) and time. What it may not do is go back into
a lock **blind**: `kasse.set(stand + 100)` is refused wherever `stand` was
read, and so is `if stand > 100 { kasse.set(0) }`, because the decision is
stale even where the value is not. The doors for what was seen are `update`,
which decides inside the lock, and `set(neu; after: stand)`, which stores only
if the lock still holds what was seen and throws `Overtaken` otherwise
([ADR-111](adr/adr-111.md)). There is no word that removes the stamp.

Three mistakes are refused by name. Assigning to a `SharedMut` directly —
`kasse = 0` — is refused, and the message names `set`. A `set` given a stamped
value, or standing under a stamped condition, is refused, and the message names
`update` and `after:` ([ADR-039](adr/adr-039.md) D10, [ADR-111](adr/adr-111.md)
D4). And an `update` block that assigns to `v` without reading it is a `set`
through the back door, refused as one.

> **Status:** not built. None of the four doors exists: `get`, `set`, `update`,
> `access` and `access_all` have no entry in `std`, nothing lowers them, and
> neither of the two refusals above is reported.

### 6.4. Resource Cleanup (RAII)
Since Nikaia does not use a Garbage Collector, resources must be cleaned up deterministically. Nikaia follows the **RAII** principle (Resource Acquisition Is Initialization).

**Automatic Destruction**
When a variable goes out of scope (usually at the closing brace `}`), Nikaia automatically frees its memory.

**Custom Cleanup (`impl Drop`)**
If your struct manages external resources (like File Handles, Sockets, or C-Pointers), you can implement the `Drop` trait. The `drop` method is called automatically when the object is destroyed. `drop` is **synchronous**: it runs straight through and can never pause — use it for teardown that is pure memory work or a cheap native call.

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
Some resources cannot be torn down without doing real work: a buffered file must *flush* its remaining data to disk, a database transaction must *roll back*, a TLS connection wants to say goodbye over the network. All of that is I/O — and in Nikaia, I/O means the function may pause (Chapter 8). A synchronous `drop` cannot pause. For these resources, implement `Cleanup` instead:

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

You never call `cleanup` yourself, and you cannot forget it — the compiler inserts the call at the end of the block, exactly like `drop`. The only visible difference: the end of the block becomes a place where the function may briefly pause (like any other I/O), and the truth about errors surfaces (next paragraph).

**Cleanup errors are real errors.** If closing a resource can fail, the function that owns it can fail — Nikaia does not hide this (silently losing data at close time is a decades-old bug class in other languages). If `cleanup` declares `throws`, the surrounding function needs `throws` too, and the compiler tells you precisely why:

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

Two refinements:
* If a value dies **while an error is already bubbling up**, the cleanup error does not replace it — it is attached to the original error as a *secondary error* (see 7.1's automatic debug information).
* If you want to react to the close error specifically, call **`close()`** yourself — it consumes the resource, returns the error normally, and no implicit cleanup runs afterwards.

**One restriction, told straight:** a `sync` function can never pause — so a resource with a pausable `cleanup` must not go out of scope inside one. The compiler catches this (`NK2602`) and names the ways out: return the resource to your caller, close it before the `sync` part, or use a non-buffering variant.

**When `cleanup` cannot run.** If a task is *cancelled* (it lost a `select` race, or a supervisor restarts it), nobody can wait for its I/O. The runtime then adopts the pending `cleanup` runs and finishes them in the background before the program exits ("parked cleanup" — bounded by the `cleanup-deadline`, Part III 13.3b; a cleanup the deadline cut off is a failure of the program, exit status 70 with the resource named on the panic path, [ADR-112](adr/adr-112.md)). Only a **panic** gets no pausable cleanup: during panic teardown only the synchronous `drop` fallback runs — and **on a target that traps rather than unwinds, a panic ends the process immediately, so no destructors run at all** (Part III, Appendix A). What *does* still run on every panic is the **Panic Hook** (7.2) — your registered last-moment handler for dumps and crash reports. Panics are for unrecoverable bugs; recoverable failures use `throws`, where full cleanup is guaranteed.

> **Design Note: Why no `defer`?**
> Unlike languages like Go or Zig, Nikaia does not need a `defer` keyword.
> 1.  **Scope-Bound:** Cleanup happens automatically at the end of the block via `Drop`. You cannot forget it.
> 2.  **Safety:** Patterns like `.access()` for locks guarantee that resources are released, replacing manual `lock/defer unlock` sequences.
> 3.  **Unwinding:** If an error occurs (`throws`), the stack unwinds and triggers `drop` for all variables in the scope, ensuring no resource leaks even during failures.

### 6.5. References and Borrowing

Sometimes a function only needs to *look at* a value, not own it. For this, Nikaia has **References** (written `&T`, or `&str` for text). Think of it as lending a book: the owner keeps the book, the borrower may read it, and the loan ends automatically.

**The headline guarantee:**

> **Nikaia source code contains no lifetime annotations. Ever.**
> There is no syntax for them, and there never will be. Everything described below happens inside the compiler. (See [ADR-005](adr/adr-005.md).)

Two things are guaranteed to *just work*:

1.  **Borrowing inside a function**, even across I/O. Because Nikaia is async by default, a function may pause at any I/O call — and a borrowed value simply stays valid across that pause:

    ```nika
    fn report(config: &Config) {
        let name = &config.name       // borrow
        let data = fs::read("log")    // the function pauses here (I/O)...
        println(f"{name}: {data}")     // ...and the borrow is still valid.
    }
    ```

2.  **Returning borrowed values from functions.** A function like `fn first_word(s: &str) -> &str` needs no annotations. Even when the result could come from *several* inputs, the compiler figures out the connection on its own — across function boundaries, through your whole program (see 6.7).

**The one rule you need to know:** a borrow may not outlive its owner. You will rarely be able to break this rule by accident, because of the next section.

**Who writes the `&`: the declaration, never the call** ([ADR-094](adr/adr-094.md)).
A parameter written with a plain type is a **view** unless the function's body
keeps the value — stores it, hands it back, gives it to a task, or passes it to
something that keeps it — and which of the two it is comes from the body, is
written to the ledger (6.7), and is true for every caller. So the caller writes
`serve(db)` and `fs::map(path)`, and the compiler writes the reference the
callee asked for, the way it writes the pause and the failure a call carries
(7.1, 8.1). A `&` in a parameter type is an assertion — *this is a view, hold
me to it* — the way `sync` is (Part II, 12.1). A parameter the function changes
in place says `mut` in the declaration (`fn fill(mut out: Vec[i64])`), which is
`&mut self`'s rule for every parameter; the call shows nothing, as `xs.push(1)`
shows nothing. A `for` **lends** its list, so the list is still there after the
loop; iteration that takes the elements away is written, `for x in xs.drain()`.
Nothing here inserts a copy: a value handed to a function that keeps it, and
used again afterwards, is refused with `.clone()` named as the way out (8.3).

> **Status:** partly built ([ADR-094](adr/adr-094.md) §5). **A `for` lends** and
> `xs.len()` after the loop is a program; `xs.drain()` is how a loop takes the
> elements away; a `let` over a place — `config.name`, `totals.stations[name]` —
> is a view of it where the value would otherwise have to move; the `keeps`
> column is inferred and recorded for every function; and **the compiler writes
> the `&` at the call** off that column, so `serve(db)` is the line and
> `serve(&db)` is refused (Part III, C.3, `NK1137`). Four things are not lent
> and take an owned argument as before: a parameter the body **keeps**, a value
> that **copies**, an argument that is **already a view**, and a **method's**
> argument, where this compiler cannot resolve which entry the call goes to.
> **`mut` is read too**: `fn fill(mut out: Vec[i64])` lowers to `&mut Vec<i64>`
> and `fill(xs)` gains its `&mut`, and a parameter a body *changes* without the
> word is refused (`NK1138`) rather than handed to the compiler below.
> **Not built:** the ledger diff that narrates a kept value's moved cleanup
> point. And where the type of an argument is not known — a value a
> `catch` handed back, a place inside a lambda — a `&` written at the call is
> left alone rather than refused, because there is nothing to refuse it on.

### 6.6. Escaping References Are Tethered

What happens when a borrowed value is not just used, but **escapes** — returned past the scope that owns the buffer, captured by a `@detached` lambda, or put into a collection that lives longer than the buffer? In most systems languages this is where the pain starts, because the compiler must prove the owner lives long enough.

Nikaia takes a different route. The rule is:

> **Transient = borrow. Escaping = tether. Copying = yours to ask for.**

Every view (`&str`, `&[u8]`) is in one of three states, and the compiler picks the cheapest one that works. You never write these states:

| State | What it is | Cost |
| :--- | :--- | :--- |
| **Borrowed** | a plain reference into the buffer | nothing at all |
| **Tethered** | a handle on the buffer plus a position | one shared handle per *container* — no copy, no allocation |
| **Owned** | a `String` of its own | one allocation — **only** where you wrote `.to_owned()` |

The effect: **the buffer cannot die while anything still points into it.** Instead of an error telling you "you may not use this after the buffer is gone," the language guarantees the buffer stays alive — deterministically, by reference counting, with no garbage collector involved.

Two things about this are worth knowing, because they are what make it usable on large data:

* **Storing a slice in a struct is not, by itself, an escape.** A struct that is built and consumed inside the scope that owns the buffer keeps plain references. A parser loop that builds a hundred million small records pays *nothing* for them.
* **The handle sits on the container, not on every slice.** When a map full of slices outlives its buffer, the *map* holds one handle; its keys stay positions. Filling it costs no handle traffic at all.

```nika
struct Token {
    text: &str,   // a view into someone else's buffer
}

fn tokenize(source: String) -> List[Token] {
    // The returned tokens outlive `source`'s scope, so the list is tethered:
    // it keeps `source` alive. No annotations, no copies of the text,
    // no dangling references — and one handle for the whole list.
    ...
}
```

Because of this, you will never see a "struct with lifetime parameters" in Nikaia — that concept does not exist in the language.

**When you want the guarantee in writing.** In a hot loop you may want to be *told* if a value ever starts tethering rather than borrowing. Mark the struct `@borrowed`:

```nika
@borrowed
struct Reading { name: &str, temp: i32 }
```

Nothing about the program changes — except that an escape is now a compile error that names the place where it happens. `nikaia explain --tethers` prints the state of every view without changing anything at all.

**The one case that is an error.** If a slice escapes and its buffer cannot be shared — it lives on the stack, or came from a foreign library — no tether is possible. The compiler says so and offers the ways out, `.to_owned()` among them. It never inserts that copy on your behalf: an invisible copy in a loop over a billion rows is exactly the kind of surprise Nikaia refuses to produce. (Details: [ADR-008](adr/adr-008.md).)

**A parameter written `&str` may not be kept past its call.** A view inside a struct carries the buffer it points into, because the struct's own declaration says it holds a view; a parameter written `&str` on its own says only that the call may look at one. So a function that stores such a parameter — into a field of its subject, into a struct it hands back, into a task — is refused as `NK2302` (Part III, C.3), and the way to write it is to put the view in a struct and take the struct:

```nika
@borrowed
struct Reading { name: &str, temp: i32 }

impl Summary {
    fn record(&mut self, m: Reading) { … }     // and not `name: &str`
}
```

Handing a view back out of the buffer it came from is **not** this rule: `fn count(seq: &str, k: i64) -> HashMap[&str, Tally]` returns views of `seq`, and the result points into `seq` and nothing else. Why a parameter is not given a buffer of its own to name is [ADR-005](adr/adr-005.md) D1 — the language has no syntax for one — and what a struct carries instead is [ADR-008](adr/adr-008.md) D1.

Where the thing it is stored into **already carries a buffer**, there is nothing to refuse: a method of a struct that holds a view has that struct's buffer in hand, so storing the parameter into one of its fields is accepted and the parameter is a view of *that* buffer. `fn note(&mut self, name: &str)` on a `Summary` holding `label: &str` compiles, and `name` is a view of the same buffer `label` points into — which narrows what a caller may pass and is why it is the signature rather than the body that changes.

> **Status:** built for a **method of a struct that holds a view**, which is where a buffer is already named. Three cases are still refused: a function or method whose own subject holds no view (there is no buffer to name, even when the destination is a field of a struct that carries one); a view handed back through the result; and a view given to a task. Of the accepted cases, one is accepted without being decided: where the view is handed to a call on the subject, the compiler cannot see whether the callee keeps it, so it is treated as kept and the parameter is written as a view of the subject's buffer either way.

**One honest cost.** A tether keeps the *whole* buffer alive, not just the part you pointed at. Keeping one short name out of a 13 GB memory-mapped file pins all 13 GB. Where that looks like a mistake the compiler warns and suggests `.to_owned()`.

### 6.7. The Borrow Contract Ledger

For every function whose signature involves borrows, the compiler infers a **Borrow Contract**: a note such as "the result of `longest(a, b)` borrows from `a` or `b`." You never write these contracts. They are stored in a generated file, **`nikaia.contracts`**, which is committed alongside `nikaia.lock` (details in Part III, Chapter 13.5).

The ledger has two jobs:

1.  **Cache:** if a contract did not change, none of the function's callers need to be re-checked. Builds stay fast.
2.  **Explanation:** if you edit a function *body* and that changes its contract, the compiler compares old and new contract. If a caller elsewhere breaks, the error does not point at some mysterious distant line — it tells the whole story: *what you changed, how the contract changed, and which caller is affected* — plus what to do about it.

### 6.8. When the Compiler Says No

Ownership rules occasionally reject code — usually because the code contains a genuine bug. Nikaia's promise ([ADR-005](adr/adr-005.md), D7): **every such error explains itself in plain language and tells you what to do next.** You never need Rust knowledge to read a Nikaia error; if a raw internal (Rust) error ever reaches you, that is a Nikaia bug — please report it.

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

For every known pattern of this kind, the standard library provides a safe, named method (`retain`, `drain`, `entry`, `swap(i, j)`, …) and the error message points directly at it.

---

## Chapter 7: Error Handling

Failures are a part of software. Nikaia distinguishes between two types of errors.

### 7.1. Recoverable Errors (`throws`)

These are expected problems: a file is missing, a connection drops, an input does not fit the
format. A function that can fail says so with `throws`.

```nika
fn fetch_config() throws -> String {
    let file = fs::read("config.txt")   // can fail
    return net::send(file)              // can fail too
}
```

**`throws` names no types.** What a function can fail *with* follows from its body, so the compiler
infers it whole-program and writes it to `nikaia.contracts` (Part III, 13.5). Writing it into the
signature would mean maintaining derived truth by hand — the same reason a borrow relationship is
not written in the source ([ADR-005](adr/adr-005.md) D3, [ADR-023](adr/adr-023.md) D1).

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

An error **carries what belongs to it** — "not found" without the path is a message that costs a
question before it helps. An `enum` is what the language already has for one of a fixed set of
things (4.4), and a `match` over one is checked for completeness. No marker on the type is needed:
what is thrown must implement `Error`, and the `impl` line is where that is said.

**Raising: `throw`.**

```nika
if !fs::exists(path) {
    throw ConfigError::NotFound(path)
}
```

**Propagation happens on its own, and nothing marks it.** A call that can fail, inside a function
that declares `throws` — that is all of it. No operator, no sigil:

```nika
fn load() throws -> Config {
    let text = fetch_config()   // if it fails, `load` fails
    return parse(text)
}
```

That is deliberate, and it is the same decision as in three other places. Four things can leave the
control flow of a Nikaia program without the line showing it: a call may **pause** (8.1), a block's
end may **pause and fail** (6.4), a call may **fail** (here), and a loop's step may fail
([ADR-025](adr/adr-025.md)). Marking one of them would claim the other three were absent.

**Because nothing marks it, the compiler insists on the declaration.** A call that can fail, in a
function that does not say `throws`, is **refused** — `NK2605` — rather than lowered:

```text
error[NK2605]: this function can fail because `liest` can fail
  --> app.nika:2:23
   2 | fn ruft() -> String { return liest() }
                             ^
     = `liest` carries `throws = ["?"]` in the contracts this program is built against (Part III, 13.5)
     = nothing marks a failing call, so a failure leaves at a call exactly as it leaves at a block's closing brace or a loop's step (ADR-023 D8, ADR-025 D1)
     help: declare the error: add `throws` to `ruft` - or handle it at the call, `… catch { … }` (Part I, 7.1)
```

on `fn liest() -> String throws { return fs::read_to_string("x.txt") }` one line above. The shape
is Appendix C.4's: the caret is on the statement, **the note is the contract** quoted from the
ledger the call was resolved against, and the help is one of the two things a reader can type.

Two things rest on that refusal. The signature is the only place the source says a function can
fail, so a caller that does not say it has said something false; and `nikaia.contracts` records
`throws` per function and is **committed and read by other programs** (Part III, 13.5), so
accepting the program would publish that `ruft` cannot fail.

> **Status:** built for a call **by name** — a function of this program, of another of its
> modules, or of `std`. A **method** call that can fail is refused by `NK2605` all the same, but
> the lowering does not yet propagate one: on a method, `catch` at the call is the shape that
> compiles today. A call nothing describes is neither refused nor propagated, and reaches the
> backend as before.

**Handling: `catch`.** The block supplies the replacement value — or it leaves the function.

```nika
let config = load() catch {
    eprintln(f"{error}")
    return                                // leaves the function
}

let port = read_port() catch { 8080 }     // replacement value
```

Inside the block the error is called **`error`**. To pass it on rather than handle it, throw it:
`throw error`.

**Telling failures apart.** `error` is the sum of the errors that can arrive at this point — the
compiler knows them because it inferred them. Match with the patterns of 3.4, where a path with
`::` names a variant:

```nika
let config = load() catch {
    match error {
        ConfigError::NotFound(p) => Config::default()
        ConfigError::BadSyntax { line, .. } => {
            eprintln(f"config broken at line {line}")
            return
        }
        _ => throw error
    }
}
```

> **Status:** the patterns of 3.4 are built. Two things in the example above are
> not: **`..` in a named pattern**, so a `match` cannot yet bind some fields and
> ignore the rest, and a bare `throw` as an arm's body — write the arm as a
> block, `_ => { throw error }`.

Two sets, and they are not the same one. The **variants of an error type** are closed, a `match`
over them is exhaustive, and adding one is a breaking change — correctly. The **set of error types**
arriving at a `catch` is open, and it grows when a callee gains a failure. When it does, **every
`catch` over that callee is named once in the build output** — the new error, the handler it now
reaches, and that the handler takes it as it takes everything — and under `--locked` the build
fails until the ledger is regenerated and committed. The commit is the acknowledgement; nothing is
written at the handler, and a handler that matches on `error` is told the same as one that does not
([ADR-101](adr/adr-101.md), `NK2401`).

> **Status:** not built — `std.contracts` writes `throws = ["?"]` on every entry,
> so there is no set to diff until error types are lowered.

**What an error brings without anyone attaching it.** The **site** it was raised from, and the chain
beneath it where another error joined on the way — a cleanup that failed while the stack was
unwinding is attached to the original as a *secondary* error rather than replacing it (6.4), and so
are the other failing branches of an `overlap` (8.1.2). The list is `error.secondary`, in the order
the errors joined, each with its own site; a `catch` still catches one error and chooses by its
type ([ADR-115](adr/adr-115.md)). The site costs nothing at run time: the compiler knew it and
wrote it into the binary as text.

**A stack trace is not among them.** Errors here are the *expected* kind — a missing file, a line
that does not parse — and capturing a trace for each one costs far more than raising it, so a
program would pay that per rejected line for a value almost nothing reads. `NIKAIA_TRACE=1` asks
for one; without it there is none, and the long form **says so** rather than leaving you to wonder
whether one was lost. What that costs, and why the cost decided it, is
[ADR-036](adr/adr-036.md).

**Printing it: short is the default.**

```nika
eprintln(f"{error}")           // the message, and nothing else
eprintln(f"{error.full()}")    // the site, the chain, and a trace if one was captured
```

`{error}` is the message the author wrote. An error message is for the operator, not for the visitor
of a web page — which is why a failed HTTP handler answers with a generic 500 and logs the rest
([ADR-018](adr/adr-018.md)). The form you type without thinking is the one you may show a stranger.

`full()` is an ordinary call, not a spelling the language had to invent: a hole holds an expression
(2.5), and a method call is one.

So that a generic answer stays findable anyway, every error knows the **site that raised it** and
has a short form of it a person can read out. A screenshot carrying `(NK-2C7)` leads through
`nikaia explain NK-2C7` to the line that threw it — with no log file, and also when your working
tree is two features further along ([ADR-023](adr/adr-023.md) D6).

**Failure nobody writes down.** Two places perform a call you did not type, and both can fail: the
end of a block, where a resource is cleaned up (6.4, `NK2601`), and a loop's step over a fallible
stream ([ADR-025](adr/adr-025.md), `NK2701`). In both the enclosing function gains a `throws`, and
the compiler says which resource or which loop it was.

**It is one rule with three sites**, stated generally by [ADR-025](adr/adr-025.md) D1: where a
call can fail — written by you or performed by the language — the failure fails the enclosing
function, the function must declare `throws`, and the compiler names the call that is the reason.
`NK2605` is the written call, `NK2601` the closing brace, `NK2701` the loop's step; three codes
because three messages have three different things to point at, and one rule behind them.

### 7.2. Unrecoverable Errors (`panic`)
These are logical bugs, like trying to access the 10th item in a list of 5 items. Nikaia stops the execution to prevent incorrect behavior. Where the machine cannot unwind, this ends the process safely.

**The Panic Hook (`std::panic::on_panic`)**
"Abort" does not mean *no* code runs anymore — it means no normal cleanup runs. Before the process dies, or the crashed task is isolated where the program survives, Nikaia calls one last, registered function: the **Panic Hook**. This is the place for a crash dump, a crash report, or flushing a diagnostics log — so a crash in production never has to be a mystery.

```nika
use std::panic

fn main() {
    // Global, one per application. Set it early.
    // The hook must be 'sync' — mid-panic there is nothing to pause on.
    panic::on_panic fn(info) sync {
        // 'info' carries: message, file/line, and the stack trace.
        // Pattern: open crash resources at startup, only WRITE here.
        crash_log.write_report(info)
    }

    run_app()
}
```

The rules, told straight:
* **Global, application-only.** Exactly one hook per program, set by the application — a library calling `on_panic` is a compile error (`NK2604`). A crash-reporting library instead exports a function that your hook calls.
* **It runs on every panic, on every target** — right before the trap where the machine traps, before the task is poisoned where it unwinds. Supervisors (Part II, 12.8) receive their crash information from the same `info`.
* **Blocking is allowed here — briefly.** Where the process is ending anyway it costs nothing. Where the program keeps running, keep the hook short and hand heavy reporting to something you started earlier.
* **Diagnosis, not cleanup.** Do not try to flush buffered files or finish transactions from the hook — those objects may be broken in exactly the way that caused the panic. That is why panics skip destructors, and the hook does not reopen that door. If the hook itself panics, the process aborts immediately.

> **Status:** not built. There is no `std::panic`, and the line above does not
> parse as one construct either: a lambda's parameter list is followed by its
> body and by nothing else (5.3), so the `sync` marker ends the lambda and
> `panic::on_panic fn(info) sync { … }` is read as three expressions. The
> trailing lambda itself is built, named arguments included — it is the marker
> between the parameters and the block that has no grammar.

---

## Chapter 8: Concurrency (Doing things at the same time)

Even at `user_parallelism = no`, you can perform multiple tasks concurrently, such as waiting for a download while responding to user input. This is done using **Asynchronous Programming**.

### 8.1. Async by Default
In Nikaia, functions that perform Input/Output (I/O), like reading a file or downloading a URL, automatically "pause" execution without blocking the whole program. You do not need special keywords like `await`.

Within one task the order is exactly the order you wrote: `let a = fs::read("x")` pauses, and the line after it does not run until `a` is there. What runs meanwhile is some *other* task — a pause point never forks one. Tasks exist only where you put them (`spawn`, `par_iter`, `task::scope`), and `.join()` on a handle is where two of them meet again. Nikaia did not remove the marker for waiting; it kept it exactly where something branches, and left it off where nothing does.

> **Status:** built ([ADR-055](adr/adr-055.md) §6 steps 1–4), and this is the
> section that most needed saying so.
>
> ***"Pause without blocking the whole program"* was not true until it was.**
> The compiler emitted Rust with the word `async` in it **zero** times, so a
> function that read a file blocked its thread — and at `user_parallelism = no`
> that thread is the program. Every sentence above was a promise about a
> lowering nobody had written. A function the ledger says can pause is an
> `async fn` now, a file read suspends on the kernel's completion queue or an
> I/O worker's reply, and the executor is the only place the program parks. So
> *"what runs meanwhile is some other task"* is a thing that happens.
>
> **You still do not write a keyword**, and that is checked rather than
> promised: a test reads every `.nika` file in this repository and fails on
> `async`, `await` and the runtime's type names. `.join()` is a word you write,
> because it is where two **tasks** meet — a different thing from waiting, and
> the one place something branches.
>
> Of the three ways to make a task, `spawn` is built (8.2); `par_iter` and
> `task::scope` are not.

### 8.1.1. Statement Order Is the Written Order

Two statements run in the order you wrote them, whether or not they touch
anything in common. No analysis stands between the source and the schedule, and
a program that wants two things to run together says so (8.1.2).

> **This section used to say the opposite**, and the withdrawal is
> [ADR-050](adr/adr-050.md) D1. Operations that met on nothing were *unordered*
> by the compiler, with `seq { … }` to force an order the analysis could not see
> and an `ordering` switch to turn the whole thing off.
>
> **Two escapes are an admission** — that the analysis can be incomplete, and
> that the correction is by hand, after the fact, by somebody who noticed. A rule
> that changes what a program means, resting on an analysis admitted to be
> incomplete and corrected manually, is the shape of a defect source rather than
> of a guarantee; against it stood an optimisation whose reach nobody was able to
> state. So the rule goes, and `seq { … }` and the switch go with it (D7).
>
> **What the analysis was is kept**, because D3 gives it a better use: the touch
> sets are what check an `overlap` block's claim that its branches meet on
> nothing. Checking a claim is a stronger use of them than making one, and
> `--overlaps` is now a report about the blocks a program writes.

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

**It is not a task.** The block ends before the function continues, so nothing
outlives it: nothing is moved, borrowing works as it does anywhere else, and the
crossing rules a `spawn` meets (Part II, 12.4) have nothing to do here. That is
what makes it lighter than two `spawn`s.

**The branches must meet on nothing, and the compiler checks it.** This is 8.1.1's
analysis used the other way round: not *"may I reorder these?"* but *"you said
these overlap; is that true?"* A branch pair that meets on a resource is refused,
and the message names the resource.

**A branch that fails makes the block fail**, and if two fail the first **in
written order** wins, so the result is reproducible — written order is the only
order the source has. The others are not lost: they are attached to the winner
as its `secondary` list, in written order, and a log or `nikaia explain` shows
them under it ([ADR-115](adr/adr-115.md)). Per-branch handling is `catch` inside
the branch; combining failures is `catch` on the block, where a handler has the
winner and `error.secondary`. A branch cannot see another branch's failure,
because branches meet on nothing.

**A branch is an expression.** Several steps in one branch are a block expression
inside it; where two branches would both be multi-line blocks doing the same shape
of work, write a function and call it twice.

**Same meaning at both settings of `user_parallelism`, different duration.** At
`no` a branch that computes still overlaps with another branch's *waiting*,
because the waiting is not your code. Computation beside I/O overlaps at every
setting; only computation beside computation needs `yes`. That is 1.2's rule — you
choose `how`, never `what`.

> **Status:** built ([ADR-050](adr/adr-050.md) D2–D6,
> [ADR-055](adr/adr-055.md) §6 step 5). `overlap { … }` parses, every branch is
> in flight at once, the block's value is their results in written order, a
> branch that **binds** or that **meets** another is refused as `NK2104`, and an
> uncaught failure fails the block with the first in written order winning.
>
> **D6 is visible in the emitted Rust**, which is where it has to be: the
> branches that can pause are handed to the vehicle first, and the tuple is put
> back into written order afterwards with a comment naming the record. A block
> whose branches are all of one kind pays for no permutation.
>
> **And the withdrawal it was ordered before is done too** — 8.1.1's automatic
> half, `seq { … }` and the `ordering` switch are gone (D1, D7), so this is the
> one way a program asks for overlap.
>
> **And the destructuring `let` this section's own example writes is not built
> either.** `let (user, rights, prefs) = overlap { … }` does not parse: `let`
> takes one name. Part II 12.5's `let (tx, rx) = channel::bounded(100)` writes
> the same form, so it is a gap this construct met rather than one it made —
> `docs/open-work.md` carries it. An `overlap` is reached by its tuple today:
> `let r = overlap { … }`, then `r.0`.

### 8.2. Spawning Tasks
To run a new independent task, use `spawn`. It takes a lambda containing the code to run — the
one lambda form (5.3), whose body is a block whether it holds one line or several.

```nika
spawn fn { println("I am running in the background!") }
```

### 8.3. Data Ownership in Tasks (Implicit Move)
A background task may keep running after the function that started it has already finished. It therefore cannot merely *borrow* variables — they might be gone by the time it runs. Following the Contextual Capture rules (Chapter 5.4), `spawn` is a **Detached Context**: variables used inside the task are **moved** into it automatically. There is no `move` keyword — the compiler applies the rule for you.

```nika
let message = "Hello"

// 'message' is implicitly moved into the task (spawn is @detached)
spawn fn { println(message) }

// Compiler Error: 'message' now belongs to the task.
// println(message)
```

**Two cases, and the type of the value decides which.** What the rule above is
about is **ordinary data** — a string, a number, a struct or collection of those —
and for data a move stays a move. A handle on a `Shared[T]` is the other case:
the handle is **duplicated** rather than moved (6.2), so the name outside the task
keeps working and there is nothing for you to write
([ADR-040](adr/adr-040.md) D1). `message` here is a string, so everything in the
rest of this section is the **data** case.

If you still need the value afterwards, clone it **before** the task is built and
give the task the copy. A `.clone()` written *inside* the body does not help: the
body runs after `message` has already moved into the task, so it would clone the
task's own copy and leave nothing behind for the parent.

```nika
let message = "Hello"
let copy = message.clone()   // made here, while `message` is still ours
spawn fn { println(copy) }   // the copy is what moves into the task
println(message)             // OK: `message` never left
```

The compiler error for this situation — data moved into a task and used again
afterwards — explains exactly that:

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

**`NK2101` belongs to the data case only.** For a handle on a `Shared[T]` there is
nothing to refuse: using the value again after the task is built is the very thing
the duplication of 6.2 serves, so there is no error and no `.clone()` to write
([ADR-040](adr/adr-040.md) D5).

> **Status:** built, in both cases ([ADR-055](adr/adr-055.md) §6 step 4).
> `spawn` lowers: the body becomes a future the executor owns, `.join()` is a
> suspension point, and a task nobody joins still runs — the example at the top
> of 8.2 prints what it says it prints, after `main` has gone on. `NK2101` is
> raised, and it is **narrow on purpose**: only where the type is known and a
> move takes it away. A number, a `bool`, a `char` and a **view** are copied, so
> `let message = "Hello"` is not this case and `"Hello".to_string()` is; a
> handle on a `Shared[T]` is duplicated, which is the exemption this section
> states ([ADR-040](adr/adr-040.md) D1, D5); and an **assignment** between the
> task and the later use clears it, because giving the name a value again is a
> correct program. The form above is the one spelling of a `spawn`, the trailing
> lambda of 5.3, and the parser takes it — it used to insist on parentheses
> around the body, which was a bug in the parser and not a second form; that
> spelling now says so rather than parsing. A lambda that **names** an argument
> is refused as `NK2103`: a task is handed nothing.
>
> **The multi-threaded half is built too.** At `user_parallelism = yes` a task
> goes to a pool of futures over the `user-pool` worker count and runs on a
> thread of its own; at `no` every task interleaves on the one thread, which is
> what Part II 11.2 promises there. The same two lines mean the same thing at
> both, which is that section's *uniform API*.
>
> What is not built is the **refusal**. Because a task moves between threads at
> `yes`, its future must be `Send` ([ADR-055](adr/adr-055.md) §2 D6) — and one
> that is not is refused by the backend, about the generated file, rather than
> here about the program (Part III, C.1).

### 8.4. The Runtime Sidecar Model
While `user_parallelism = no` keeps your own logic on one thread ("The Happy Path"), the Runtime employs a **Hidden Sidecar Pattern** to handle heavy I/O without blocking.

* **Separation of Concerns:** User code runs exclusively on the main thread (Event Loop). Heavy operations (like SQLite queries) are offloaded to a managed Runtime Sidecar (a background thread on Native, or a Web Worker on WASM).
* **Safety Guarantee:** Data exchange occurs via strict message passing (ownership transfer). Since user code never accesses the Sidecar memory directly, **Race Conditions** remain impossible.
* **Non-Blocking:** From the developer's perspective, a database call is simply an async yield point. The Runtime guarantees that the main loop never stalls waiting for disk I/O.

> **Status:** built, for files ([ADR-055](adr/adr-055.md) §6 step 3). The
> sidecar existed and the yield point did not; now both do.
>
> The runtime starts **before your first statement**, with one I/O thread
> always and a pool for your own code only at `user_parallelism = yes`
> ([ADR-038](adr/adr-038.md) D4). What runs on that thread is `std`'s own code
> and nothing else — the boundary is a closed list of operations rather than a
> queue of closures, which is what makes "user code never accesses the Sidecar
> memory" a property of the compiler rather than a promise
> ([ADR-037](adr/adr-037.md) D2, ADR-038 §4.2). A file read is handed to the
> **kernel** where the machine can complete it, so the commonest heavy
> operation involves no sidecar thread at all (ADR-038 D3).
>
> **The last bullet is what changed.** A file operation used to be a direct call
> that blocked the calling thread while the kernel worked: no state machine, so
> no other task for the main loop to run meanwhile. It is a *slot* now — on the
> ring, or an I/O worker's reply — and asking whether it has finished never
> blocks, so the main loop runs whatever else is ready and parks in the I/O only
> when nothing is.
>
> **A database call is not that yet**, and neither is standard input: `std::db`
> does not exist, and a stream needs the readiness half of D3's mechanism, which
> is built for sockets and not wired to stdin. Both are `async` in their
> signatures and finish on their first poll, so a caller sees what it saw
> before.

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
    let r = Row(id: helper() + 1)      // no `use`, and no prefix
    println(f"{r.id}")
}
```

Two files of one package may **not** declare the same name. There is one
namespace, so two `Row`s in it is an error rather than a rule about which of them
a line means.

**A package reaches another package by its name.** `use http` makes the package
`http` reachable and does nothing else: every name from it is written with its
prefix, at every use ([ADR-046](adr/adr-046.md) D1).

```nika
use http
use request_handling as rh

fn handle(r: http::Request) -> rh::Response {
    let body: http::Body = http::read(r)
    return rh::ok(body)
}
```

The prefix falls where a name is **written**, not where a value is used: `r.path`
and `r.header("host")` carry none, because there is a value in hand and nothing to
resolve. In a body that works with values the prefix barely appears.

**No name is brought in** — not by a glob, not by a braced list, not one at a time
([ADR-046](adr/adr-046.md) D2):

```text
error: names are not brought in; a package is reached through its name
   1 | use http::{Request, Response}
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
     = write `use http`, and `http::Request` where you need it
     = if the prefix is long, `use http as h` shortens it once, in one place
```

What this buys is that a reader of any line knows where every name in it comes
from without consulting the top of the file, and that no edit elsewhere changes
what an already-written line means. What it costs is the prefix, and `use x as y`
is what shortens one that is long — once, in one place (D3).

Two more rules come with it. **A prefix must be introduced**: `http::Request`
without `use http` is refused, so a file still lists what it depends on at the top
(D4). And **one name per file**: two packages that end up under the same name, by
alias or by collision, is an error rather than a rule about which wins (D5).

`use std::fs` is the one `use` with a path in it, and it names the library rather
than a package of yours ([ADR-030](adr/adr-030.md) D1).

> **Status:** both halves are built, one level deep. Every `.nika` beside the
> entry takes part and they are one namespace; a package is depended on by
> **path** — `http = { path = "../http" }` in `[dependencies]` (Part III, 13.3) —
> and a `use` naming one resolves. Two files declaring the same name is refused by
> name, a `use` naming a file beside this one says to remove the line, one naming
> no dependency says where to declare it, and one name twice is refused.
>
> `use http as h` works too, and a call, a type and a struct literal all reach
> through it. A diagnostic names the **package** rather than the alias — the type
> is `http::Request` whatever one file calls the package, and the `use` line that
> connects the two is at the top of the same file.
>
> The braced and glob forms get the sentence above, with the caret on the brace
> or the star ([ADR-046](adr/adr-046.md) §5).
>
> **Not built:** a package's *own* package dependencies are refused rather than
> resolved, and a version does not yet find a package — it resolves through
> Cargo under the crate name `nikaia_<name>` ([ADR-103](adr/adr-103.md)), and
> that arm is unbuilt.
>
> Outside a project a `.nika` file is compiled **on its own**: a package is a
> directory of a project, and a directory of loose examples is a directory of
> programs.

### 9.2. Visibility Rules (Privacy)
Nikaia enforces strict encapsulation to prevent tight coupling between parts of your code.

1.  **Private to its package, by default:**
    * Functions, Structs, Enums and Constants are visible inside the **package**
      that declares them — every file of that directory — and nowhere else. (A
      `comptime` binding at **item level** is decided and unbuilt: inside a
      function body it is a statement today, and `pub comptime` at the top of a
      file has nowhere to stand yet — [ADR-073](adr/adr-073.md) D2,
      [ADR-077](adr/adr-077.md) for the word.)
    * Struct fields are the same: visible throughout the package that declares
      the struct.

2.  **The `pub` Keyword:**
    * To let **another package** use an item, prefix it with `pub`.
    * To let another package reach a specific field of a struct, prefix the field
      with `pub`.
    * A **public type may keep its fields private**, and 9.3 is about why that
      is the ordinary case rather than an inconvenience.

The boundary is the package and not the file on purpose
([ADR-047](adr/adr-047.md) D1): a library's internal file layout is then not its
public surface, so moving a declaration from one file to another is housekeeping
and breaks no consumer.

Reaching a private item from another package is `NK1110`, and says which package
keeps it:

```text
error[NK1110]: `secret` is private to `utils`
   4 |     let n = utils::secret()
           ^
     = an item is private to the package that declares it unless it says `pub` (Part I, 9.2)
     help: write `pub fn secret` in `utils`, or reach it through something that is public
```

> **Status:** built, for items and for fields, and `NK1110` reaches a program now
> that a package can be depended on: *"`secret` is private to `http`"*, and
> *"`http::Request.method` is private to `http`"* both for reading such a field
> and for giving one a value in a struct literal. The ledger carries a field's
> `pub` because the language below cannot enforce it — a dependency's items are
> in the same crate ([ADR-047](adr/adr-047.md) §5).

### 9.3. Granular Control
While `pub` makes an item available generally, strict privacy forces developers to create safe interfaces (Constructors and Methods) rather than exposing raw data.

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

