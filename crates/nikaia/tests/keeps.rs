//! Which parameters a body **keeps**
//! ([ADR-094](../../../docs/specification/adr/adr-094.md) D2), the first of
//! that record's five steps.
//!
//! The column alone: inferred, recorded, and read by nothing. Every call site
//! in the repository still writes its own `&`, and the point of landing this
//! half on its own is that the answer can be diffed against the corpus before
//! one of them changes.
//!
//! **Half of these tests are about the claim being withheld**, because the
//! polarity is the whole design. `keeps` is a restriction, so doubt adds it —
//! the opposite of `sync`, which is a promise and is taken away on doubt. An
//! analysis that answered *keeps* everywhere would be safe and useless, and
//! nothing in the column itself says which of the two it is; only a test that a
//! read stays a read does.

use nikaia::contracts::Ledger;
use nikaia::parser::parse_to_ast;

/// What the ledger says one function keeps, after the inference.
fn keeps(source: &str, name: &str) -> Vec<String> {
    let parsed = parse_to_ast(source).expect("the source parses");
    let ledger = Ledger::infer(&parsed);
    ledger.functions[name].keeps.clone()
}

/// **A parameter that is only looked at is lent**, which is the answer the
/// whole record exists for: `page(entries)` rather than `page(&entries)`.
#[test]
fn a_parameter_that_is_only_read_is_not_kept() {
    assert!(keeps(
        "fn width(text: String) -> i64 { return text.len() as i64 }",
        "width"
    )
    .is_empty());

    // A field read is a read of the field and not of the parameter.
    assert!(keeps(
        "struct Row { total: i64 }\n\
         fn of(row: Row) -> i64 { return row.total }",
        "of"
    )
    .is_empty());
}

/// **Handed back by value, it is kept** — the value outlives the call by
/// definition.
#[test]
fn a_parameter_returned_by_value_is_kept() {
    assert_eq!(
        keeps("fn label(text: String) -> String { return text }", "label"),
        ["text"]
    );
}

/// **And handed back as a view, it is not.** `-> &str` points into the
/// argument rather than moving it, which is what `returns = "borrows(…)"`
/// already records; `-> String` is the one that takes it away.
#[test]
fn a_parameter_returned_as_a_view_is_not_kept() {
    let source = "fn same(text: ref String) -> ref String { return text }";
    assert!(keeps(source, "same").is_empty());

    let parsed = parse_to_ast(source).expect("the source parses");
    let ledger = Ledger::infer(&parsed);
    assert_eq!(
        ledger.functions["same"].borrows,
        ["text"],
        "the view is recorded where views are recorded"
    );
}

/// **Put into a struct, it is kept.** Where that struct then goes is not this
/// expression's question — it outlives the call either way.
#[test]
fn a_parameter_put_into_a_struct_is_kept() {
    assert_eq!(
        keeps(
            "struct Row { name: String }\n\
             fn wrap(name: String) -> Row { return Row { name: name } }",
            "wrap"
        ),
        ["name"]
    );

    // `Row { name }` is the field and the name in one, and the name may be a
    // parameter — the shorthand must not be a hole in the walk.
    assert_eq!(
        keeps(
            "struct Row { name: String }\n\
             fn wrap(name: String) -> Row { return Row { name } }",
            "wrap"
        ),
        ["name"]
    );
}

/// **Assigned into a place, it is kept**, whatever the place is.
#[test]
fn a_parameter_assigned_into_a_place_is_kept() {
    assert_eq!(
        keeps(
            "struct Stats { min: i64 }\n\
             impl Stats {\n\
             \x20   fn add(ref mut self, temp: i64) { self.min = temp }\n\
             }",
            "Stats::add"
        ),
        ["temp"]
    );
}

/// **And it travels up the call graph**, which is what makes the column an
/// answer about a *program* rather than about one body: `shout` keeps `text`
/// only because `label` does.
#[test]
fn keeping_travels_up_the_call_graph() {
    let source = "fn label(text: String) -> String { return text }\n\
                  fn shout(text: String) -> String { return label(text) }\n\
                  fn width(text: String) -> i64 { return text.len() as i64 }\n\
                  fn measure(text: String) -> i64 { return width(text) }";

    assert_eq!(keeps(source, "shout"), ["text"]);
    // And a callee that only reads leaves its caller lending, which is the half
    // that says the walk is not simply answering yes.
    assert!(keeps(source, "measure").is_empty());
}

/// **Two functions that pass a parameter neither stores keep neither.**
///
/// The least fixpoint, in the one shape that tells it from a greatest one.
/// `sync` starts from *everything is sync* and takes the claim away, so mutual
/// recursion keeps it; this starts from *nothing is kept* and adds, so mutual
/// recursion keeps nothing. Both are right, and they are right for opposite
/// reasons.
#[test]
fn mutual_recursion_that_only_reads_keeps_nothing() {
    let source = "fn ping(text: String) -> i64 {\n\
                  \x20   if text.len() == 0 { return 0 }\n\
                  \x20   return pong(text)\n\
                  }\n\
                  fn pong(text: String) -> i64 { return ping(text) }";

    assert!(
        keeps(source, "ping").is_empty(),
        "{:?}",
        keeps(source, "ping")
    );
    assert!(keeps(source, "pong").is_empty());
}

/// **A task keeps what it names** ([ADR-040](../../../docs/specification/adr/adr-040.md)
/// D1): its body may outlive the statement, so it takes what it names by value.
#[test]
fn a_parameter_a_task_names_is_kept() {
    assert_eq!(
        keeps(
            "fn start(text: String, n: i64) -> i64 {\n\
             \x20   let h = spawn fn { println(f\"{text}\") }\n\
             \x20   return n + 1\n\
             }",
            "start"
        ),
        ["text"],
        "the one the task names, and not the one beside it"
    );

    // **And `return n` would keep `n`**, which is not a quirk of copy types
    // being missed — it is the rule being applied: the value leaves the call.
    // What a *copy* type costs when it is kept is the emitter's question and is
    // step 3's; this column says what the body does.
    assert_eq!(
        keeps(
            "fn start(text: String, n: i64) -> i64 {\n\
             \x20   let h = spawn fn { println(f\"{text}\") }\n\
             \x20   return n\n\
             }",
            "start"
        ),
        ["n", "text"]
    );
}

/// **A callee nothing describes keeps.** D2's fail-closed case, and the reason
/// the polarity is written down: the wrong answer this way costs a caller an
/// owned argument, which is where every caller already is, and the wrong answer
/// the other way is a `&T` parameter whose body moves the value — `rustc`'s
/// error about a file nobody wrote.
#[test]
fn a_call_nothing_describes_keeps_what_it_is_given() {
    assert_eq!(
        keeps(
            "struct Sink { n: i64 }\n\
             fn hand(text: String, sink: Sink) { sink.swallow(text) }",
            "hand"
        ),
        ["sink", "text"],
        "a method no ledger has an entry for is not one to assume about — and \
         that goes for the receiver as much as for the argument"
    );
}

/// **A receiver a method takes by value is moved out of**, so a parameter
/// standing there is kept.
///
/// And which entry the call goes to is the type checker's answer
/// ([ADR-028](../../../docs/specification/adr/adr-028.md)), so where **any**
/// method call in a body went to an entry no ledger has, the candidate list is
/// not the whole list and may not be believed. `crates/nikaia/tests/lambdas.rs`
/// is where that was met: its `Account::access` is a Rust stand-in taking
/// `self`, and the two `::access` entries `std` carries both take `&self`.
#[test]
fn a_receiver_is_kept_where_the_method_might_consume_it() {
    // Every method here resolves, and every one of them reads its receiver.
    assert!(keeps(
        "fn width(text: String) -> i64 { return text.len() as i64 }",
        "width"
    )
    .is_empty());

    // One unresolvable call in the body, and every receiver in it is kept —
    // including the one whose own method resolves perfectly well.
    assert_eq!(
        keeps(
            "struct Sink { n: i64 }\n\
             fn hand(text: String, sink: Sink) -> i64 {\n\
             \x20   sink.swallow()\n\
             \x20   return text.len() as i64\n\
             }",
            "hand"
        ),
        ["sink", "text"]
    );
}

/// **`std` says which of its own keep**, and the answer is six entries out of
/// ninety-four.
///
/// That ratio is the column's whole value. An absent `keeps` on a *present*
/// entry means *keeps nothing* — `std.contracts`' own convention for `sync`,
/// said once more — so a caller that hands a name to `str::len` may lend it. A
/// file that recorded nothing and made every caller assume the worst would be
/// safe and would leave the language exactly where it is.
#[test]
fn std_says_which_of_its_own_functions_keep() {
    let source = "struct Row { n: i64 }\n\
                  fn collect(rows: Vec[Row], row: Row) -> Vec[Row] {\n\
                  \x20   rows.push(row)\n\
                  \x20   return rows\n\
                  }\n\
                  fn sized(rows: Vec[Row]) -> i64 { return rows.len() as i64 }";

    assert_eq!(keeps(source, "collect"), ["row", "rows"]);
    assert!(
        keeps(source, "sized").is_empty(),
        "reading a length is reading"
    );
}

/// The column survives the round trip, which `--locked` needs: it compares
/// bytes, so a column that rendered differently than it parsed would fail a
/// build that changed nothing.
#[test]
fn the_column_renders_and_parses_back() {
    let parsed = parse_to_ast(
        "struct Row { name: String }\n\
         fn wrap(name: String) -> Row { return Row { name: name } }",
    )
    .expect("the source parses");
    let ledger = Ledger::infer(&parsed);
    let rendered = ledger.render();
    assert!(rendered.contains("keeps = [\"name\"]"), "{rendered}");

    let read = Ledger::parse(&rendered).expect("its own output parses");
    assert_eq!(read.functions["wrap"].keeps, ["name"]);
    assert_eq!(read.render(), rendered);

    // And a function that keeps nothing writes no line, so no existing ledger
    // grows a column of falses.
    let plain = parse_to_ast("fn width(text: String) -> i64 { return text.len() as i64 }")
        .expect("the source parses");
    assert!(!Ledger::infer(&plain).render().contains("keeps"));
}
