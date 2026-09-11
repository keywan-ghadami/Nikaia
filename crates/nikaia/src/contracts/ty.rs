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
