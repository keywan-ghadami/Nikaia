// crates/nikaia/src/contracts/touch.rs
//
// What an operation reaches, and whether it changes it (ADR-033).
//
// Part I 8.1.1's rule is one sentence - *two operations whose touch sets are
// disjoint have no order between them* - and everything hard about it is in
// what a "touch set" is allowed to say:
//
//     touches = ["file(path) read"]      the file named by the `path` parameter
//     touches = ["stdout write"]         a resource with no parameter
//
// **An absent `touches` means it touches everything.** That is not a default
// chosen for convenience; it is the same fail-closed polarity ADR-010 D1 set
// for provenance and ADR-027 D2 for `sync`, and it is what makes this
// adoptable: a program built against libraries that describe nothing keeps
// exactly the order it has today, and gets faster only where somebody wrote
// enough down for the compiler to prove it may.
//
// **The kinds are a closed list, and that is a soundness property rather than
// tidiness.** A kind this file did not know used to parse into a resource
// nothing else could name - so `stdoutt write` was disjoint from `stdout
// write`, and a typo in a hand-maintained ledger bought an overlap instead of
// failing to buy one. A vocabulary whose *mistakes* are permissions is a
// vocabulary in the wrong polarity. [`Touch::kind_is_known`] is what says
// which names mean something, and `contracts::order` answers a `touches` entry
// naming anything else the way ADR-033 D4 answers every other thing it cannot
// read: it reaches everything, and stays where it was written.

use anyhow::{anyhow, Result};

/// Every resource a `touches` entry may name (ADR-033 D2).
///
/// It grows when a function needs it and never before - ADR-028 D5's rule,
/// which is the same rule the rest of the ledger lives under. What is here is
/// what `std`'s own entries reach:
///
/// | kind | the resource | who asked |
/// | :--- | :--- | :--- |
/// | `file(path)` | the file that parameter names | `fs::read`, `fs::write`, `fs::map` |
/// | `stdout` | the program's standard output | `print`, `println` |
/// | `stderr` | the program's standard error | `eprint`, `eprintln` |
/// | `args` | the arguments the program was started with | `cli::args` |
///
/// **What is deliberately not here**, and each for the same reason. ADR-033
/// D2's table names `endpoint(url)` for `net::post` and a lock for
/// `counter.access fn { … }`, and §3 rests Part II 12.2's
/// no-pausing-while-locked rule on the second - neither function exists. And
/// **standard input**, which is the near miss: `io::read`, `io::lines` and
/// `io::read_to_string` are all in `std`'s ledger already and none of them
/// says what it reaches, so all three order against everything. Nothing in
/// `examples/` asks - the two programs that read a pipe do it inside a `for`
/// or behind a handler that `return`s, so no pair reaches the question - and
/// ADR-028 D5 is the rule that keeps a word out until a program asks for it.
///
/// When one does, the entry is `stdin` **write** and not `stdin read`, and the
/// reason is worth leaving here rather than being rediscovered. `write` in a
/// touch set means *it changes the resource*, and reading a stream consumes
/// it: two `io::read_to_string()` calls do not both get the bytes, so the
/// read/read rule that lets two file reads overlap would be exactly wrong.
///
/// **And the clock, which is the one that is asked about before it is needed.**
/// `std` has no function that reads one - `Instant::now` appears in the runtime's
/// own executor and nowhere a program can reach - so the rule above keeps it out,
/// and the rule is the same one that let `lock` in the day the doors existed.
///
/// What to write when the day comes: **`clock read`**, and the read is right,
/// because two calls genuinely conflict over nothing and neither changes
/// anything - the opposite of `stdin`, whose read is a write. The reason it is
/// worth a paragraph anyway is the **second consumer**
/// ([ADR-067](../../../../docs/specification/adr/adr-067.md) §3): to
/// `contracts::order` an empty touch set is a *speed*, and to a repetition it
/// would be a *permission*. A clock left out of an entry would read as "repeat me
/// freely" and the repetition would see a different time. Naming it at all is
/// enough to stop that - any named resource makes the set non-empty - which is
/// why the note is here and not a special case somewhere.
/// | `lock` | **a** lock, never which one | `get`, `set`, `access`, `update`, and the two doors over several |
///
/// **`lock` joined the list the day the doors existed**
/// ([ADR-067](../../../../docs/specification/adr/adr-067.md) D3). It was named in
/// [ADR-033](../../../../docs/specification/adr/adr-033.md) D2's own table and
/// kept out of this one under the rule the paragraph above states - a word waits
/// until a program asks for it - and until [ADR-064](../../../../docs/specification/adr/adr-064.md)
/// and [ADR-065](../../../../docs/specification/adr/adr-065.md) no program could.
///
/// **It names no parameter, and that is [ADR-039](../../../../docs/specification/adr/adr-039.md)
/// D4's decision rather than a limit here**: the property says *a lock* and never
/// *which* lock, because telling two handles apart would make whether a program
/// compiles depend on whether that proof happened to succeed. Two `access`
/// calls are two **reads** and do not conflict; two doors that write do, whether
/// or not they are the same lock.
pub const KINDS: &[&str] = &["file", "stdout", "stderr", "args", "lock"];

/// Which kinds may turn out to be **one** resource however differently they are
/// named.
///
/// `stdout` and `stderr` are two handles and one destination the moment anybody
/// types `2>&1`, which is not an exotic invocation. Recording them as separate
/// kinds and stopping there would have made
///
/// ```nika
/// println("summe: 12")
/// eprintln("eine zeile ohne wert")
/// println("fertig")
/// ```
///
/// a group of three whose output interleaves differently on every run of a
/// program redirected that way - D1's guarantee broken by a resource nobody
/// checked the identity of, which is the same shape of mistake as the `catch`
/// handler whose effects nobody counted (ADR-033 §8.3).
///
/// So [`Reached::might_be_same`] compares *families* and not kinds. It is the
/// doctrine that function already states about names - "everything else might
/// be, and might be is the answer that keeps the order" - applied to the
/// question of whether two differently-named resources are one. The kinds stay
/// separate in the ledger because `eprintln` genuinely does not write standard
/// output, and a contract should say what a function does.
///
/// **It costs almost nothing.** Two console writes are microsecond work, and
/// §8.4 measured that the overlap cannot pay for anything below roughly a
/// quarter megabyte whatever carries it. The refusal buys a guarantee and
/// spends nothing anyone could measure.
///
/// It is deliberately *not* the general claim that any two resources might
/// coincide. `prog > a.txt` while the program writes `a.txt` is the same
/// question one level out, and D2's table answers it by naming the file the
/// program named: what a program says it reaches is what the analysis compares.
/// Where that is not enough, D7's `seq` is the escape the language has for
/// exactly this - "resources that look disjoint and are not".
const FAMILIES: &[&[&str]] = &[&["stdout", "stderr"]];

/// The family a kind belongs to, which is what two resources are compared on.
fn family(kind: &str) -> &str {
    FAMILIES
        .iter()
        .find(|family| family.contains(&kind))
        .and_then(|family| family.first())
        .copied()
        .unwrap_or(kind)
}

/// One resource an operation reaches.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Touch {
    /// What kind of thing - one of [`KINDS`], or a word this compiler does not
    /// know.
    ///
    /// Two touches of *different* kinds never conflict, which is the cheap half
    /// of the rule and the half that does most of the work. It is also why an
    /// unknown kind may not simply be carried along: it would be *different*
    /// from every kind there is, and therefore disjoint from all of them.
    /// [`Touch::kind_is_known`] is the question, and `contracts::order` refuses
    /// the whole entry where the answer is no (ADR-033 D4).
    pub kind: String,
    /// The parameter that names which one - `file(path)` is the file named by
    /// the argument passed for `path`.
    ///
    /// `None` where the kind names the resource on its own: there is one
    /// `stdout`, so `stdout` needs no argument to say which.
    pub parameter: Option<String>,
    /// Whether it changes the resource. Two reads never conflict - the same
    /// rule a processor applies to two loads.
    pub write: bool,
}

impl Touch {
    /// Whether this touch is a **lock**
    /// ([ADR-111](../../../../docs/specification/adr/adr-111.md) D2).
    ///
    /// What hangs on it is whether a callee could write a stamped value into
    /// the lock it reaches: one that touches none cannot, so a `Seen` passes
    /// through it and comes out stamped.
    pub fn names_a_lock(&self) -> bool {
        self.kind == "lock"
    }

    /// Read one back from the text a ledger writes.
    pub fn parse(text: &str) -> Result<Touch> {
        let text = text.trim();
        let (resource, access) = text.rsplit_once(' ').ok_or_else(|| {
            anyhow!("a touch is `resource read` or `resource write`, found `{text}`")
        })?;
        let write = match access.trim() {
            "read" => false,
            "write" => true,
            other => {
                return Err(anyhow!(
                    "a touch is `read` or `write`, not `{other}` (in `{text}`)"
                ))
            }
        };

        let resource = resource.trim();
        let (kind, parameter) = match resource.split_once('(') {
            Some((kind, rest)) => {
                let parameter = rest.strip_suffix(')').ok_or_else(|| {
                    anyhow!("a resource is `kind(parameter)`, and `{resource}` has no `)`")
                })?;
                (kind.trim(), Some(parameter.trim().to_string()))
            }
            None => (resource, None),
        };
        if kind.is_empty() {
            return Err(anyhow!("a touch needs a resource kind, found `{text}`"));
        }

        Ok(Touch {
            kind: kind.to_string(),
            parameter,
            write,
        })
    }

    /// Whether this compiler knows what the named resource *is* (ADR-033 D2).
    ///
    /// Kept separate from [`Touch::parse`] on purpose. A ledger that names a
    /// kind from a newer vocabulary is not malformed - it is a file this
    /// compiler is too old to read, and refusing to parse it would turn a
    /// library's forward step into a build failure. D4 already says what to do
    /// with an effect that cannot be read, and it is the same answer here as
    /// everywhere else: it reaches everything, so the statement stays put.
    pub fn kind_is_known(&self) -> bool {
        KINDS.contains(&self.kind.as_str())
    }

    pub fn text(&self) -> String {
        let access = if self.write { "write" } else { "read" };
        match &self.parameter {
            Some(parameter) => format!("{}({parameter}) {access}", self.kind),
            None => format!("{} {access}", self.kind),
        }
    }
}

/// Which resource a *call* reaches, with the parameter filled in.
///
/// A ledger says `file(path)`; a call site says `file` of `"measurements.txt"`.
/// This is the second, and it is what two calls are actually compared on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reached {
    pub kind: String,
    /// The argument that named it, where the compiler could see one.
    ///
    /// `None` means the resource is the kind's only one (`stdout`) **or** the
    /// argument was not something this compiler can evaluate. Those two are
    /// deliberately not distinguished here - `same_resource` treats an unknown
    /// argument as "might be any of them", which is the answer both want.
    pub named: Option<String>,
    /// `true` where the argument was not readable, so `named` is not a name.
    pub unknown: bool,
    pub write: bool,
}

impl Reached {
    /// Whether two reached resources might be the same one.
    ///
    /// Different *families* never are - see [`FAMILIES`], and note that it is
    /// families rather than kinds, because `stdout` and `stderr` are one
    /// destination as soon as somebody redirects one onto the other. The same
    /// family with two different *known* names never are. Everything else might
    /// be, and "might be" is the answer that keeps the order.
    pub fn might_be_same(&self, other: &Reached) -> bool {
        if family(&self.kind) != family(&other.kind) {
            return false;
        }
        match (&self.named, &other.named) {
            (Some(a), Some(b)) if !self.unknown && !other.unknown => a == b,
            // One of them is a resource this compiler could not name, so it
            // could be the other one.
            _ => true,
        }
    }

    /// Whether two reached resources force an order between their operations.
    ///
    /// Two reads never do, however much they overlap: reading does not change
    /// what the other one sees.
    pub fn conflicts_with(&self, other: &Reached) -> bool {
        (self.write || other.write) && self.might_be_same(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reached(kind: &str, named: Option<&str>, write: bool) -> Reached {
        Reached {
            kind: kind.to_string(),
            named: named.map(str::to_string),
            unknown: false,
            write,
        }
    }

    #[test]
    fn a_touch_round_trips_through_its_text() {
        for text in [
            "file(path) read",
            "file(path) write",
            "stdout write",
            "stderr write",
            "args read",
        ] {
            assert_eq!(Touch::parse(text).expect("parses").text(), text, "{text}");
        }
    }

    /// Every kind [`KINDS`] names is known, and nothing else is.
    ///
    /// The half that matters is the second. An unknown kind differs from every
    /// kind there is, so `might_be_same` says no to all of them - which would
    /// make a misspelled resource *disjoint from everything* and buy an
    /// overlap. `contracts::order` refuses an entry that holds one (D4), and
    /// this is the question it asks.
    #[test]
    fn a_kind_outside_the_vocabulary_is_not_known() {
        for kind in KINDS {
            let touch = Touch::parse(&format!("{kind} read")).expect("parses");
            assert!(touch.kind_is_known(), "{kind}");
        }
        for text in ["stdoutt write", "stdin write", "endpoint(url) write"] {
            let touch = Touch::parse(text).expect("parses");
            assert!(!touch.kind_is_known(), "{text}");
        }
    }

    /// … and an unknown kind is exactly the hole that check is there for.
    ///
    /// Written as an assertion about the wrong answer, so that anyone who ever
    /// makes `conflicts_with` the guard instead sees why it cannot be.
    #[test]
    fn an_unknown_kind_would_be_disjoint_from_everything() {
        let typo = reached("stdoutt", None, true);
        assert!(!typo.conflicts_with(&reached("stdout", None, true)));
        assert!(!typo.conflicts_with(&reached("file", Some("a.txt"), true)));
    }

    #[test]
    fn a_touch_names_its_kind_and_parameter() {
        let touch = Touch::parse("file(path) read").expect("parses");
        assert_eq!(touch.kind, "file");
        assert_eq!(touch.parameter.as_deref(), Some("path"));
        assert!(!touch.write);

        let out = Touch::parse("stdout write").expect("parses");
        assert_eq!(out.kind, "stdout");
        assert_eq!(out.parameter, None);
        assert!(out.write);
    }

    #[test]
    fn a_malformed_touch_says_what_is_wrong() {
        for text in [
            "file(path)",
            "file(path) maybe",
            "file(path read",
            "(x) read",
        ] {
            assert!(Touch::parse(text).is_err(), "`{text}` should not parse");
        }
    }

    /// Different kinds never meet. The cheap half of the rule.
    #[test]
    fn different_kinds_never_conflict() {
        let file = reached("file", Some("a.txt"), true);
        let out = reached("stdout", None, true);
        assert!(!file.conflicts_with(&out));
    }

    /// … but the two console handles are one destination under `2>&1`.
    ///
    /// The exception [`FAMILIES`] exists for, and the reason it is not an
    /// exception to the rule so much as the rule about *names* applied one
    /// level up: two resources this compiler cannot prove distinct are ordered.
    /// Without it `println` / `eprintln` / `println` is a group of three whose
    /// output interleaves on a redirected program, and D1's guarantee is gone
    /// for the commonest shape a program has.
    #[test]
    fn the_two_console_handles_may_be_one_destination() {
        let out = reached("stdout", None, true);
        let err = reached("stderr", None, true);
        assert!(out.conflicts_with(&err));
        assert!(err.conflicts_with(&out));
        // … and neither of them meets a file the program named.
        assert!(!out.conflicts_with(&reached("file", Some("a.txt"), true)));
        assert!(!err.conflicts_with(&reached("args", None, false)));
    }

    /// Two reads never conflict - a processor's rule for two loads.
    #[test]
    fn two_reads_never_conflict() {
        let one = reached("file", Some("a.txt"), false);
        let same = reached("file", Some("a.txt"), false);
        assert!(!one.conflicts_with(&same));

        // … and a write against the same file does.
        let written = reached("file", Some("a.txt"), true);
        assert!(one.conflicts_with(&written));
        assert!(written.conflicts_with(&one));
    }

    /// Two different files do not conflict even when both are written.
    #[test]
    fn two_named_resources_are_compared_by_name() {
        let a = reached("file", Some("a.txt"), true);
        let b = reached("file", Some("b.txt"), true);
        assert!(!a.conflicts_with(&b));
    }

    /// A resource the compiler could not name might be any of them.
    ///
    /// This is the fail-closed half: `fs::write(pfad, …)` where `pfad` is
    /// computed keeps its order against every other file operation, because the
    /// alternative is a program that is wrong on some inputs and not others.
    #[test]
    fn an_unnameable_resource_conflicts_with_its_whole_kind() {
        let unknown = Reached {
            kind: "file".to_string(),
            named: None,
            unknown: true,
            write: true,
        };
        assert!(unknown.conflicts_with(&reached("file", Some("a.txt"), false)));
        assert!(reached("file", Some("a.txt"), false).conflicts_with(&unknown));
        // Still not a conflict with another kind entirely.
        assert!(!unknown.conflicts_with(&reached("stdout", None, true)));
    }
}

// --- what a body reaches (ADR-067 D2) ---------------------------------------

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::Item;
use crate::check::MethodCalls;
// `Reached` is a name this file already has for something else, so the one
// `sync` uses for a call site comes in as `Call`.
use crate::contracts::sync::{reached, visit_stmt, visit_stmt_blocks, Reached as Call};
use crate::contracts::Ledger;
use crate::parser::Parsed;

/// What one function's body reaches, before the fixpoint joins it up.
#[derive(Default)]
struct Reach {
    /// Something it calls is not accounted for: a name no ledger knows, a
    /// construct that runs something, or a described callee whose own touch set
    /// is *"nobody said"*. The claim is off and no fixpoint brings it back.
    unknown: bool,
    /// What it reaches directly, through callees a library describes.
    outside: BTreeSet<Touch>,
    /// The functions in this **package** it calls — every unit of it, since
    /// [ADR-100](../../../docs/specification/adr/adr-100.md) D2. Its claim holds
    /// only while theirs do.
    calls: BTreeSet<String>,
}

/// Give every function in the ledger the `touches` its body earns
/// ([ADR-067](../../../../docs/specification/adr/adr-067.md) D2).
///
/// **The fourth derived column, and the one that was specified without one.**
/// `sync`, `throws` and `sharing` are each read off a body over the call graph;
/// `touches` was written with the same fail-closed polarity
/// ([ADR-033](../../../../docs/specification/adr/adr-033.md) D4) and only ever
/// hand-written in `std`'s ledger — so every function a `.nika` file declared
/// said *"nobody said"*, which means *"it touches everything"*. Safe, and
/// useless: the walk stopped at the first call out of `std`.
///
/// The fixpoint is the greatest one, for `sync::infer`'s reason: start from
/// "every function touches nothing", and take the claim away from anything that
/// reaches one without it. Mutual recursion between two functions that touch
/// nothing keeps the claim, which is right.
///
/// **A resource named by a parameter does not travel.** `fs::read` touches
/// `file(path)`, and `path` is *its* parameter: a caller's argument may be a
/// literal, or a parameter of its own under another name, and mapping one to the
/// other is a piece of work of its own. Until it is done, a callee whose touch
/// names a parameter leaves the caller unknown — conservative in the direction
/// this column is conservative in.
pub fn infer(
    ledger: &mut Ledger,
    units: &[&Parsed],
    library: &Ledger,
    resolved: &BTreeMap<String, MethodCalls>,
) {
    let mut graph: BTreeMap<String, Reach> = BTreeMap::new();
    for parsed in units.iter().copied() {
        for item in &parsed.program.items {
            match &item.node {
                Item::Fn { .. } => {
                    if let Some((name, reach)) =
                        reach_of(parsed, &item.node, None, ledger, library, resolved)
                    {
                        graph.insert(name, reach);
                    }
                }
                Item::Impl {
                    target, methods, ..
                } => {
                    let target = parsed.text(target.name).to_string();
                    for method in methods {
                        if let Some((name, reach)) = reach_of(
                            parsed,
                            &method.node,
                            Some(&target),
                            ledger,
                            library,
                            resolved,
                        ) {
                            graph.insert(name, reach);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // Start optimistic, then take the claim away until nothing changes.
    let mut known: BTreeMap<&str, bool> = graph
        .iter()
        .map(|(name, reach)| (name.as_str(), !reach.unknown))
        .collect();
    loop {
        let mut changed = false;
        for (name, reach) in &graph {
            if !known[name.as_str()] {
                continue;
            }
            let reaches_unknown = reach
                .calls
                .iter()
                .any(|callee| !known.get(callee.as_str()).copied().unwrap_or(false));
            if reaches_unknown {
                known.insert(name.as_str(), false);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    // And the set, once every claim that holds is settled. A union over the
    // whole reachable graph rather than one step, because what a caller reaches
    // is what everything it calls reaches.
    for (name, holds) in &known {
        if !holds {
            continue;
        }
        let mut found: BTreeSet<Touch> = BTreeSet::new();
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        let mut todo = vec![*name];
        while let Some(here) = todo.pop() {
            if !seen.insert(here) {
                continue;
            }
            let Some(reach) = graph.get(here) else {
                continue;
            };
            found.extend(reach.outside.iter().cloned());
            todo.extend(reach.calls.iter().map(|c| c.as_str()));
        }
        if let Some(contract) = ledger.functions.get_mut(*name) {
            // **Only where nobody said.** A hand-written entry is what its
            // author wrote, the way `sync::infer` leaves an assertion alone.
            if !contract.touches_known {
                contract.touches = found.into_iter().collect();
                contract.touches_known = true;
            }
        }
    }
}

/// One function's reach, by the key the ledger records it under.
fn reach_of(
    parsed: &Parsed,
    item: &Item,
    target: Option<&str>,
    own: &Ledger,
    library: &Ledger,
    resolved: &BTreeMap<String, MethodCalls>,
) -> Option<(String, Reach)> {
    let Item::Fn { name, body, .. } = item else {
        return None;
    };
    let own_name = match name {
        Some(name) => parsed.text(*name).to_string(),
        None => "new".to_string(),
    };
    let key = match target {
        Some(target) => format!("{target}::{own_name}"),
        None => own_name,
    };

    let mut reach = Reach::default();
    collect(parsed, body, own, library, &mut reach);

    // The method calls the type checker resolved, which this walk cannot
    // (ADR-028) - the same hand-over `sync::infer` takes.
    if let Some(methods) = resolved.get(&key) {
        reach.unknown |= methods.unresolved;
        for callee in &methods.resolved {
            if own.functions.contains_key(callee) {
                reach.calls.insert(callee.clone());
            } else {
                absorb(library.functions.get(callee), &mut reach);
            }
        }
    }

    Some((key, reach))
}

fn collect(
    parsed: &Parsed,
    block: &crate::ast::Block,
    own: &Ledger,
    library: &Ledger,
    reach: &mut Reach,
) {
    for stmt in &block.stmts {
        visit_stmt(parsed, &stmt.node, &mut |expr| {
            match reached(parsed, expr, own, library) {
                Some(Call::Own(name)) => {
                    reach.calls.insert(name);
                }
                Some(Call::Library { key, .. }) => absorb(library.functions.get(&key), reach),
                // Answered per function by the type checker, merged in above.
                Some(Call::Method) => {}
                // A name nobody knows, or a construct that runs something.
                Some(Call::Opaque(_)) => reach.unknown = true,
                None => {}
            }
        });
        visit_stmt_blocks(&stmt.node, &mut |inner| {
            collect(parsed, inner, own, library, reach)
        });
    }
}

/// Take a described callee's touch set into a caller's, or give up.
fn absorb(contract: Option<&crate::contracts::FnContract>, reach: &mut Reach) {
    let Some(contract) = contract.filter(|c| c.touches_known) else {
        reach.unknown = true;
        return;
    };
    for touch in &contract.touches {
        // A resource named by a **parameter** is named in the callee's words.
        // Until a caller's argument can be mapped onto it, inheriting the name
        // would be claiming something about the wrong resource.
        match touch.parameter {
            Some(_) => reach.unknown = true,
            None => {
                reach.outside.insert(touch.clone());
            }
        }
    }
}
