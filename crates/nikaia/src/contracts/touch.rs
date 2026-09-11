// crates/nikaia/src/contracts/touch.rs
//
// What an operation reaches, and whether it changes it (ADR-033).
//
// Part I 8.1.1's rule is one sentence - *two operations whose touch sets are
// disjoint have no order between them* - and everything hard about it is in
// what a "touch set" is allowed to say. This is the smallest vocabulary that
// answers the first increment (ADR-033 §6) and nothing more:
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

use anyhow::{anyhow, Result};

/// One resource an operation reaches.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Touch {
    /// What kind of thing: `file`, `stdout`, `endpoint`.
    ///
    /// Two touches of *different* kinds never conflict, which is the cheap half
    /// of the rule and the half that does most of the work.
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
    /// Different kinds never are. The same kind with two different *known*
    /// names never are. Everything else might be, and "might be" is the answer
    /// that keeps the order.
    pub fn might_be_same(&self, other: &Reached) -> bool {
        if self.kind != other.kind {
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
        for text in ["file(path) read", "file(path) write", "stdout write"] {
            assert_eq!(Touch::parse(text).expect("parses").text(), text, "{text}");
        }
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
