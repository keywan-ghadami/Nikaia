// crates/nikaia/src/contracts/ty.rs
//
// The type language the checker reasons in, and the one the ledger records.
//
// It is `ast::Type` with one addition and one subtraction. The addition is
// `Unknown`, which is not a type but the absence of a claim - see below. The
// subtraction is the interner: a ledger is a file, so a name here is text, and
// two types are the same when they are written the same. That is name-for-name
// (ADR-011 D2) applied to types: nothing resolves a module or an alias, so
// `postgres::Connection` and `Connection` are different types, which is correct
// for a compiler that does not know they are not.
//
// **`Unknown` is the whole design.** Stage 0 has no signatures for the Rust
// half of `std` beyond what `std.contracts` writes down, and a Nikaia program
// calls `push_str`, `entry`, `map_or` and `chars` freely. A checker that had to
// answer for those would either need a second frontend for Rust or would have
// to guess - and a type checker that guesses reports errors that are not there,
// which is worse than one that says less. So the rule is:
//
//   * anything involving `Unknown` is compatible with anything;
//   * an error is reported only where **both** sides are known and disagree.
//
// The checker is therefore sound in the direction that matters for a tool
// people run: it never rejects a correct program. It does not catch every
// wrong one, and what it does not catch is a function of how much of `std` is
// written down - which is a number that goes up as ADR-014 proceeds, without
// this file changing.

use std::collections::BTreeSet;
use std::fmt;

use crate::ast;
use crate::parser::Parsed;

/// A type, or the absence of a claim about one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ty {
    /// No claim. Compatible with everything, in both directions.
    Unknown,
    /// A name, its arguments, and whether it is a view: `&str`, `Vec[Row]`,
    /// `HashMap[&str, Stats]`.
    Named {
        name: String,
        args: Vec<Ty>,
        view: bool,
    },
    /// `(A, B)` - a fixed number of parts and no name.
    Tuple(Vec<Ty>),
    /// `$V` - a name in a *library's* signature that stands for a type the
    /// receiver supplies (ADR-030).
    ///
    /// `HashMap::entry(&HashMap[$K, $V], key: ?) -> Entry[$V]` says the result
    /// holds whatever the map holds. At a call site the receiver's actual type
    /// binds the variables and they are **substituted away**; one that stays
    /// unbound becomes `Unknown`, never a name.
    ///
    /// That last sentence is the whole safety argument, and it is why this does
    /// not contradict [ADR-024] D4. D4 erases a Nikaia function's `T` to `?`
    /// because a `T` that survives into a comparison makes the checker report
    /// that `i32` is not `T` - a false positive. A variable here never survives
    /// into a comparison: it is bound and replaced, or it is `?`.
    ///
    /// The sigil is not decoration. `T`, `K`, `V` are also perfectly good type
    /// names, and a rule that guessed from capitalisation would silently turn
    /// somebody's type into a hole.
    ///
    /// [ADR-024]: ../../../docs/specification/adr/adr-024.md
    Var {
        name: String,
        /// `&$V` - the `&` belongs to the *use*, not to what the receiver
        /// bound. `and_modify` hands its lambda a reference to whatever the map
        /// holds, so the signature writes `fn(&$V)` and `$V` is still `Stats`.
        view: bool,
    },
    /// `fn(&Stats)` - a parameter that takes a lambda, and what the lambda is
    /// handed when it runs (ADR-029).
    ///
    /// **Only a ledger writes one.** Nikaia's own grammar has no syntax for a
    /// function type, so no `.nika` source can declare a parameter of this
    /// shape; these exist because `std`'s higher-order functions are Rust and
    /// something has to say what `and_modify` passes its lambda. Without that,
    /// the `a` in `fn { a.add(t) }` has no type, nothing it is called on
    /// resolves, and the function around it cannot be shown to be `sync`.
    ///
    /// What the lambda *hands back* is deliberately absent: nothing needs it
    /// yet, and a spelling is easier to add than to change.
    Fn { params: Vec<Ty> },
}

impl Ty {
    pub fn named(name: impl Into<String>) -> Ty {
        Ty::Named {
            name: name.into(),
            args: Vec::new(),
            view: false,
        }
    }

    pub fn view(name: impl Into<String>) -> Ty {
        Ty::Named {
            name: name.into(),
            args: Vec::new(),
            view: true,
        }
    }

    pub fn is_unknown(&self) -> bool {
        matches!(self, Ty::Unknown)
    }

    /// Whether a value of this type may stand where `expected` is wanted.
    ///
    /// Equality, plus the rule that makes the checker usable: **anything
    /// involving `Unknown` fits.** There is no subtyping and no implicit
    /// widening - Nikaia states that a conversion is written (ADR-013 D7), and
    /// a checker that quietly allowed `i32` where `i64` is wanted would be
    /// checking a different language from the one the specification describes.
    pub fn fits(&self, expected: &Ty) -> bool {
        match (self, expected) {
            (Ty::Unknown, _) | (_, Ty::Unknown) => true,
            (Ty::Tuple(a), Ty::Tuple(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(a, b)| a.fits(b))
            }
            // Two lambdas fit when they take the same things. A lambda never
            // fits a named type and no named type fits a lambda - which is a
            // claim, so it is only made where both sides are written down, and
            // `Unknown` above has already taken every other case.
            (Ty::Fn { params: a }, Ty::Fn { params: b }) => {
                a.len() == b.len() && a.iter().zip(b).all(|(a, b)| a.fits(b))
            }
            // A variable that reaches a comparison was never bound, and an
            // unbound variable is the absence of a claim rather than a claim
            // about a type called `$V`. `substitute` is supposed to have
            // removed it; this is the belt to that pair of braces.
            (Ty::Var { .. }, _) | (_, Ty::Var { .. }) => true,
            (
                Ty::Named {
                    name: a,
                    args: aa,
                    view: av,
                },
                Ty::Named {
                    name: b,
                    args: ba,
                    view: bv,
                },
            ) => {
                a == b
                    && av == bv
                    && aa.len() == ba.len()
                    && aa.iter().zip(ba).all(|(a, b)| a.fits(b))
            }
            _ => false,
        }
    }

    /// The type as a ledger writes it, which is how the source writes it.
    pub fn text(&self) -> String {
        self.to_string()
    }

    /// Read one back.
    pub fn parse(text: &str) -> Ty {
        let text = text.trim();
        if text.is_empty() || text == "?" {
            return Ty::Unknown;
        }
        if let Some(inner) = text.strip_prefix('(').and_then(|t| t.strip_suffix(')')) {
            return Ty::Tuple(split_args(inner).iter().map(|p| Ty::parse(p)).collect());
        }
        // `fn(&Stats)`, and `fn()` for a lambda that is handed nothing. Read
        // before the `&`, because a function type is never a view.
        if let Some(inner) = text.strip_prefix("fn(").and_then(|t| t.strip_suffix(')')) {
            return Ty::Fn {
                params: split_args(inner).iter().map(|p| Ty::parse(p)).collect(),
            };
        }
        let (view, rest) = match text.strip_prefix('&') {
            Some(rest) => (true, rest.trim()),
            None => (false, text),
        };
        // After the `&`, because `&$V` is a view of what `$V` binds to and not
        // a type whose name begins with a dollar.
        if let Some(name) = rest.strip_prefix('$').filter(|name| !name.is_empty()) {
            return Ty::Var {
                name: name.to_string(),
                view,
            };
        }
        match rest.find('[') {
            Some(at) if rest.ends_with(']') => Ty::Named {
                name: rest[..at].trim().to_string(),
                args: split_args(&rest[at + 1..rest.len() - 1])
                    .iter()
                    .map(|p| Ty::parse(p))
                    .collect(),
                view,
            },
            _ => Ty::Named {
                name: rest.to_string(),
                args: Vec::new(),
                view,
            },
        }
    }

    /// The same type with every name in `parameters` replaced by `Unknown`.
    ///
    /// A generic parameter is a name that stands for a type rather than being
    /// one, and a checker that treated `T` as a type would report that `i32` is
    /// not `T` - which is the shape of a false positive this checker exists not
    /// to produce. Erasing them says exactly as much as Stage 0 knows: a
    /// generic function's parameters are checked for *number* and not for type.
    pub fn erase(&self, parameters: &BTreeSet<String>) -> Ty {
        match self {
            Ty::Unknown => Ty::Unknown,
            Ty::Tuple(parts) => Ty::Tuple(parts.iter().map(|p| p.erase(parameters)).collect()),
            Ty::Fn { params } => Ty::Fn {
                params: params.iter().map(|p| p.erase(parameters)).collect(),
            },
            // A library's variable is not a Nikaia function's generic, and
            // erasing one is not the other's business.
            Ty::Var { name, view } => Ty::Var {
                name: name.clone(),
                view: *view,
            },
            Ty::Named { name, args, view } => {
                if args.is_empty() && parameters.contains(name) {
                    return Ty::Unknown;
                }
                Ty::Named {
                    name: name.clone(),
                    args: args.iter().map(|a| a.erase(parameters)).collect(),
                    view: *view,
                }
            }
        }
    }

    /// The type a `.nika` declaration names.
    pub fn from_ast(parsed: &Parsed, ty: &ast::Type) -> Ty {
        if ty.is_tuple {
            return Ty::Tuple(
                ty.generics
                    .iter()
                    .map(|g| Ty::from_ast(parsed, g))
                    .collect(),
            );
        }
        Ty::Named {
            name: parsed.text(ty.name).to_string(),
            args: ty
                .generics
                .iter()
                .map(|g| Ty::from_ast(parsed, g))
                .collect(),
            view: ty.is_view,
        }
    }
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::Unknown => f.write_str("?"),
            Ty::Tuple(parts) => {
                let parts: Vec<String> = parts.iter().map(|p| p.to_string()).collect();
                write!(f, "({})", parts.join(", "))
            }
            Ty::Fn { params } => {
                let params: Vec<String> = params.iter().map(|p| p.to_string()).collect();
                write!(f, "fn({})", params.join(", "))
            }
            Ty::Var { name, view } => {
                if *view {
                    f.write_str("&")?;
                }
                write!(f, "${name}")
            }
            Ty::Named { name, args, view } => {
                if *view {
                    f.write_str("&")?;
                }
                f.write_str(name)?;
                if !args.is_empty() {
                    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
                    write!(f, "[{}]", args.join(", "))?;
                }
                Ok(())
            }
        }
    }
}

/// Split `a, b[c, d], (e, f)` on the commas that are not inside a bracket.
pub fn split_args(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut current = String::new();
    for c in text.chars() {
        match c {
            '[' | '(' => {
                depth += 1;
                current.push(c);
            }
            ']' | ')' => {
                depth = depth.saturating_sub(1);
                current.push(c);
            }
            ',' if depth == 0 => parts.push(std::mem::take(&mut current)),
            c => current.push(c),
        }
    }
    if !current.trim().is_empty() {
        parts.push(current);
    }
    parts.iter().map(|p| p.trim().to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A type is written the way the source writes it, and reads back the same.
    #[test]
    fn a_type_round_trips_through_its_text() {
        for text in [
            "i32",
            "&str",
            "String",
            "Vec[Row]",
            "HashMap[&str, Stats]",
            "(Op, i64)",
            "Vec[(＆str, i64)]".replace('＆', "&").as_str(),
            "?",
        ] {
            let ty = Ty::parse(text);
            assert_eq!(ty.text(), text, "{text}");
            assert_eq!(Ty::parse(&ty.text()), ty, "{text}");
        }
    }

    /// Unknown fits everything, in both directions. That is what lets the
    /// checker say nothing about the half of `std` that is Rust.
    #[test]
    fn unknown_fits_anything() {
        let known = Ty::named("i32");
        assert!(Ty::Unknown.fits(&known));
        assert!(known.fits(&Ty::Unknown));
        assert!(Ty::Unknown.fits(&Ty::Unknown));
    }

    /// Two known types fit only when they are written the same. There is no
    /// implicit widening: Nikaia states that a conversion is written, and a
    /// checker that allowed it here would be checking a different language.
    #[test]
    fn two_known_types_fit_only_when_equal() {
        assert!(Ty::named("i32").fits(&Ty::named("i32")));
        assert!(!Ty::named("i32").fits(&Ty::named("i64")));
        assert!(!Ty::view("str").fits(&Ty::named("String")));
        assert!(Ty::parse("Vec[Row]").fits(&Ty::parse("Vec[Row]")));
        assert!(!Ty::parse("Vec[Row]").fits(&Ty::parse("Vec[Hit]")));
    }

    /// …and an unknown *part* is still no claim about the whole.
    #[test]
    fn an_unknown_argument_makes_the_whole_fit() {
        assert!(Ty::parse("Vec[?]").fits(&Ty::parse("Vec[Row]")));
        assert!(Ty::parse("(i32, ?)").fits(&Ty::parse("(i32, String)")));
        assert!(!Ty::parse("(i32, ?)").fits(&Ty::parse("(String, String)")));
    }
}

#[cfg(test)]
mod fn_type_tests {
    use super::*;

    /// A function type reads back the way it was written (ADR-029).
    #[test]
    fn a_function_type_round_trips() {
        for text in ["fn()", "fn(&Stats)", "fn(&str, i64)", "fn(?)"] {
            assert_eq!(Ty::parse(text).text(), text, "{text}");
        }
    }

    /// `fn(&Stats)` is not a type named `fn(&Stats)`.
    ///
    /// The parse is ordered so that a function type is recognised before the
    /// `&` and the `[…]` are looked for; without that it fell through to
    /// `Named` and the whole spelling became a name.
    #[test]
    fn a_function_type_is_not_a_name() {
        let parsed = Ty::parse("fn(&Stats)");
        assert!(matches!(parsed, Ty::Fn { .. }), "{parsed:?}");
        let Ty::Fn { params } = parsed else {
            unreachable!("just matched")
        };
        assert_eq!(params, vec![Ty::view("Stats")]);
    }

    /// It fits another lambda of the same shape, and nothing else that is
    /// written down.
    #[test]
    fn a_function_type_fits_its_own_shape() {
        let one = Ty::parse("fn(&Stats)");
        assert!(one.fits(&Ty::parse("fn(&Stats)")));
        assert!(!one.fits(&Ty::parse("fn(i64)")));
        assert!(!one.fits(&Ty::parse("fn()")));
        // A named type and a lambda are different claims.
        assert!(!one.fits(&Ty::named("Stats")));
        assert!(!Ty::named("Stats").fits(&one));
        // `?` is the absence of a claim, so it still fits both ways.
        assert!(one.fits(&Ty::Unknown));
        assert!(Ty::Unknown.fits(&one));
    }
}

/// Bind a library signature's type variables from the receiver's actual type
/// (ADR-031).
///
/// **One level, positional, receiver only.** `HashMap[$K, $V]` against
/// `HashMap[&str, Stats]` binds `$K` and `$V`; a pattern that is not a variable
/// is compared no further, and nothing is bound from an argument. That is not
/// an implementation shortcut - it is the decision. Binding from arguments and
/// matching nested patterns is where a signature language grows into a
/// unification algorithm, and each step of that wants its own reason.
///
/// A mismatch binds nothing rather than failing. This is not a check: the
/// question is what the receiver can *tell* the signature, and a receiver that
/// tells it nothing leaves the variables unbound, which `substitute` turns into
/// `?`.
pub fn bind(pattern: &Ty, actual: &Ty, out: &mut std::collections::BTreeMap<String, Ty>) {
    match (pattern, actual) {
        // The `&` in `&$V` says how the *method* takes it, not what the
        // receiver holds, so it is dropped when binding and reapplied when
        // substituting.
        (Ty::Var { name, .. }, actual) => {
            out.entry(name.clone()).or_insert_with(|| actual.clone());
        }
        (
            Ty::Named {
                name: pattern_name,
                args: pattern_args,
                ..
            },
            Ty::Named {
                name: actual_name,
                args: actual_args,
                ..
            },
        ) if pattern_name == actual_name && pattern_args.len() == actual_args.len() => {
            for (pattern, actual) in pattern_args.iter().zip(actual_args) {
                bind(pattern, actual, out);
            }
        }
        // The view flag is deliberately not compared: `&HashMap[$K, $V]` must
        // bind against a `HashMap[…]` held by value and the other way round,
        // because a signature writes the receiver the way the method takes it
        // and a caller holds it however it holds it.
        _ => {}
    }
}

/// Replace a signature's variables with what the receiver bound them to.
///
/// **An unbound variable becomes `Unknown`, never a name.** That is the whole
/// safety argument for [`Ty::Var`] and the reason this does not contradict
/// ADR-024 D4: a variable never survives into a comparison, so the checker is
/// never in a position to report that `i32` is not `$V`.
pub fn substitute(ty: &Ty, bound: &std::collections::BTreeMap<String, Ty>) -> Ty {
    match ty {
        Ty::Var { name, view } => match bound.get(name) {
            Some(Ty::Named { name, args, .. }) if *view => Ty::Named {
                name: name.clone(),
                args: args.clone(),
                view: true,
            },
            Some(bound) => bound.clone(),
            None => Ty::Unknown,
        },
        Ty::Named { name, args, view } => Ty::Named {
            name: name.clone(),
            args: args.iter().map(|a| substitute(a, bound)).collect(),
            view: *view,
        },
        Ty::Tuple(parts) => Ty::Tuple(parts.iter().map(|p| substitute(p, bound)).collect()),
        Ty::Fn { params } => Ty::Fn {
            params: params.iter().map(|p| substitute(p, bound)).collect(),
        },
        Ty::Unknown => Ty::Unknown,
    }
}

#[cfg(test)]
mod variable_tests {
    use super::*;
    use std::collections::BTreeMap;

    fn bound_from(pattern: &str, actual: &str) -> BTreeMap<String, Ty> {
        let mut out = BTreeMap::new();
        bind(&Ty::parse(pattern), &Ty::parse(actual), &mut out);
        out
    }

    /// A variable reads back the way it is written, and is not a name.
    #[test]
    fn a_variable_round_trips_and_is_not_a_name() {
        assert_eq!(
            Ty::parse("$V"),
            Ty::Var {
                name: "V".to_string(),
                view: false
            }
        );
        assert_eq!(Ty::parse("&$V").text(), "&$V");
        assert_eq!(Ty::parse("$V").text(), "$V");
        assert_eq!(Ty::parse("Entry[$V]").text(), "Entry[$V]");
        assert_eq!(Ty::parse("fn(&$V)").text(), "fn(&$V)");
        // A type genuinely called `V` is still a type called `V`.
        assert_eq!(Ty::parse("V"), Ty::named("V"));
    }

    /// The receiver binds the variables, one level and by position.
    #[test]
    fn the_receiver_binds_what_the_signature_names() {
        let bound = bound_from("&HashMap[$K, $V]", "HashMap[&str, Stats]");
        assert_eq!(bound["K"], Ty::view("str"));
        assert_eq!(bound["V"], Ty::named("Stats"));
    }

    /// A receiver that says nothing binds nothing, and nothing is `?`.
    ///
    /// `let m = HashMap::new()` gives `HashMap[?, ?]`, and the honest answer
    /// downstream is "no claim" rather than a guess.
    #[test]
    fn a_receiver_that_says_nothing_leaves_the_variables_unbound() {
        let bound = bound_from("&HashMap[$K, $V]", "HashMap[?, ?]");
        assert_eq!(
            substitute(&Ty::parse("Entry[$V]"), &bound),
            Ty::parse("Entry[?]")
        );

        // A different type altogether binds nothing at all.
        let none = bound_from("&HashMap[$K, $V]", "Vec[i64]");
        assert!(none.is_empty());
        assert_eq!(substitute(&Ty::parse("$V"), &none), Ty::Unknown);
    }

    /// An unbound variable becomes `?` - never a type called `$V`.
    ///
    /// This is the property ADR-024 D4 was protecting when it erased a generic
    /// to `?`, kept here by substitution rather than by erasure.
    #[test]
    fn an_unbound_variable_becomes_unknown() {
        let empty = BTreeMap::new();
        assert_eq!(substitute(&Ty::parse("$V"), &empty), Ty::Unknown);
        assert_eq!(
            substitute(&Ty::parse("fn(&$V)"), &empty),
            Ty::parse("fn(?)"),
        );
        // And it fits anything, so a leak cannot become a false rejection.
        assert!(Ty::parse("$V").fits(&Ty::named("i32")));
        assert!(Ty::named("i32").fits(&Ty::parse("$V")));
    }

    /// The chain the whole decision exists for.
    ///
    /// `HashMap[&str, Stats]` → `Entry[Stats]` → `fn(&Stats)`, which is what
    /// gives the `a` in `.and_modify fn { a.add(t) }` a type.
    #[test]
    fn the_chain_from_a_map_to_a_lambda_parameter() {
        let at_entry = bound_from("&HashMap[$K, $V]", "HashMap[&str, Stats]");
        let entry = substitute(&Ty::parse("Entry[$V]"), &at_entry);
        assert_eq!(entry, Ty::parse("Entry[Stats]"));

        let at_and_modify = bound_from("Entry[$V]", &entry.text());
        let lambda = substitute(&Ty::parse("fn(&$V)"), &at_and_modify);
        assert_eq!(lambda, Ty::parse("fn(&Stats)"));
    }

    /// The view flag does not stop a binding.
    ///
    /// A signature writes the receiver the way the method takes it, and a
    /// caller holds it however it holds it; requiring the two to agree would
    /// make every `&`-taking method fail to bind.
    #[test]
    fn a_view_binds_against_a_value() {
        let bound = bound_from("&Vec[$T]", "Vec[i64]");
        assert_eq!(bound["T"], Ty::named("i64"));
    }
}
