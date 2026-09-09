// crates/nikaia/src/emit/template.rs
//
// The `html` template DSL (ADR-017), compiled where the template is written.
//
// A template is not parsed at run time: the body is known when the program is
// compiled, so it is split into literal text and holes here, and what comes out
// is the string building a hand-written renderer would do. That is what makes
// the escaping contract a *compile-time* property rather than a call somebody
// has to remember.
//
// Two of ADR-017's three decisions are enforced here and the third is
// enforced by the type system:
//
// * **D1, every hole escaped, unconditionally.** There is no flag at a hole and
//   no way to ask for the other behaviour - the emitted code always goes
//   through `html::Render`.
// * **D3, a hole is only legal in a position the grammar can escape *for*.**
//   "Escaped" is not one operation: a `<script>` body, a URL, an unquoted
//   attribute and a CSS block each need something else. A hole in one of those
//   is an error naming the position, which is what lets D1 be unconditional -
//   a promise that held only in some positions would have to be qualified
//   everywhere.
// * **D2, the one way to say "already markup" is a type.** `html::Render` is
//   that decision put where `rustc` can act on it: `Raw` renders itself, text
//   renders escaped, and a type with no impl cannot be put in a template at
//   all. The Nikaia compiler does not need to know which it is - and, having no
//   type checker, could not.

use anyhow::{anyhow, Result};

/// Where in the markup a hole sits, which decides whether it may be there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    /// Between tags. Escaping the five characters is the whole answer.
    Text,
    /// Inside `"…"` or `'…'` in a tag, on an attribute that is not a URL.
    QuotedAttribute,
    /// Inside a quoted attribute whose name takes a URL.
    Url,
    /// In a tag but not inside quotes.
    UnquotedAttribute,
    /// Inside `<script>…</script>`.
    Script,
    /// Inside `<style>…</style>`.
    Style,
    /// Inside `<!-- … -->`.
    Comment,
}

impl Position {
    /// Whether escaping for this position is something `html::escape` can do.
    fn escapable(self) -> bool {
        matches!(self, Position::Text | Position::QuotedAttribute)
    }

    /// What the error says, and what it suggests instead.
    fn why(self) -> &'static str {
        match self {
            Position::Url => {
                "a URL needs percent-encoding and a scheme check, not HTML escaping: \
                 `javascript:` survives every one of the five characters"
            }
            Position::UnquotedAttribute => {
                "an unquoted attribute value ends at the first blank, so a space in \
                 the value starts a new attribute - quote it and the hole is legal"
            }
            Position::Script => {
                "a `<script>` body is JavaScript, where `&lt;` is not `<` and \
                 `</script>` inside a string still ends the element"
            }
            Position::Style => "a CSS block needs CSS escaping, which is a different table",
            Position::Comment => {
                "a comment ends at the first `-->`, and escaping does not \
                 produce or remove one"
            }
            Position::Text | Position::QuotedAttribute => "",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Position::Text => "a text node",
            Position::QuotedAttribute => "a quoted attribute value",
            Position::Url => "a URL attribute",
            Position::UnquotedAttribute => "an unquoted attribute value",
            Position::Script => "a `<script>` body",
            Position::Style => "a `<style>` block",
            Position::Comment => "an HTML comment",
        }
    }
}

/// The attributes whose value is a URL, where HTML escaping is not the escaping
/// that is needed.
const URL_ATTRIBUTES: [&str; 7] = [
    "href",
    "src",
    "action",
    "formaction",
    "data",
    "poster",
    "xlink:href",
];

/// One piece of a compiled template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    /// Markup the template wrote itself, which is markup by definition.
    Text(String),
    /// `{ … }`: a Nikaia expression, and where it sits.
    Hole { expr: String, at: Position },
}

/// Split a template body into text and holes, deciding each hole's position.
///
/// The scan is a small HTML state machine over the *literal* text only: a hole
/// contributes nothing to it, because what a hole expands to is escaped and
/// therefore cannot open a tag, close a string, or start a comment. That is
/// what makes reading the position off the literal text sound rather than an
/// approximation.
pub fn split(body: &str) -> Result<Vec<Segment>> {
    let mut segments = Vec::new();
    let mut text = String::new();
    let mut scan = Scan::default();
    let mut chars = body.char_indices().peekable();

    while let Some((i, c)) = chars.next() {
        match c {
            '{' if chars.peek().map(|(_, c)| *c) == Some('{') => {
                chars.next();
                text.push('{');
            }
            '}' if chars.peek().map(|(_, c)| *c) == Some('}') => {
                chars.next();
                text.push('}');
            }
            '{' => {
                let at = scan.position();
                let mut expr = String::new();
                let mut closed = false;
                for (_, c) in chars.by_ref() {
                    if c == '}' {
                        closed = true;
                        break;
                    }
                    expr.push(c);
                }
                if !closed {
                    return Err(anyhow!("unclosed `{{` in the template, at byte {i}"));
                }
                if !text.is_empty() {
                    segments.push(Segment::Text(std::mem::take(&mut text)));
                }
                segments.push(Segment::Hole {
                    expr: expr.trim().to_string(),
                    at,
                });
            }
            _ => {
                scan.feed(c);
                text.push(c);
            }
        }
    }

    if !text.is_empty() {
        segments.push(Segment::Text(text));
    }
    Ok(segments)
}

/// Every hole that is somewhere escaping cannot make safe, with what to say.
pub fn illegal(segments: &[Segment]) -> Vec<(String, Position)> {
    segments
        .iter()
        .filter_map(|segment| match segment {
            Segment::Hole { expr, at } if !at.escapable() => Some((expr.clone(), *at)),
            _ => None,
        })
        .collect()
}

/// The message for one of them.
pub fn illegal_message(expr: &str, at: Position) -> String {
    format!(
        "`{{{expr}}}` is in {}, which this template cannot escape for\n\
         \x20    = {}\n\
         \x20    = every hole is escaped, every time, and a position where that \
         is not enough is an error rather than a promise that quietly does not \
         hold (ADR-017 D1, D3)",
        at.name(),
        at.why()
    )
}

/// The HTML state machine, over literal text.
#[derive(Default)]
struct Scan {
    state: State,
    /// The last few characters, for the markers that are longer than one.
    tail: String,
    /// The attribute name currently being given a value, lower-cased.
    attribute: String,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum State {
    #[default]
    Text,
    Tag,
    /// After `=` in a tag, before the value starts.
    Equals,
    Quoted(char),
    Unquoted,
    Script,
    Style,
    Comment,
}

impl Scan {
    fn position(&self) -> Position {
        match self.state {
            State::Text => Position::Text,
            State::Script => Position::Script,
            State::Style => Position::Style,
            State::Comment => Position::Comment,
            State::Tag | State::Equals | State::Unquoted => Position::UnquotedAttribute,
            State::Quoted(_) => {
                if URL_ATTRIBUTES.contains(&self.attribute.as_str()) {
                    Position::Url
                } else {
                    Position::QuotedAttribute
                }
            }
        }
    }

    fn feed(&mut self, c: char) {
        self.tail.push(c.to_ascii_lowercase());
        if self.tail.len() > 9 {
            self.tail.drain(..self.tail.len() - 9);
        }

        match self.state {
            State::Text => {
                if self.tail.ends_with("<!--") {
                    self.state = State::Comment;
                } else if c == '<' {
                    self.state = State::Tag;
                    self.attribute.clear();
                }
            }
            State::Comment => {
                if self.tail.ends_with("-->") {
                    self.state = State::Text;
                }
            }
            State::Tag | State::Equals | State::Unquoted => {
                // `<!--` looks like the start of a tag until its fourth
                // character, so the comment is recognised from in here rather
                // than by looking ahead from the `<`.
                if self.tail.ends_with("<!--") {
                    self.state = State::Comment;
                    self.attribute.clear();
                } else if c == '>' {
                    // Which element was opened decides whether what follows is
                    // markup or a foreign language.
                    self.state = if self.tail.contains("<script") {
                        State::Script
                    } else if self.tail.contains("<style") {
                        State::Style
                    } else {
                        State::Text
                    };
                    self.attribute.clear();
                } else if c == '=' && self.state == State::Tag {
                    self.state = State::Equals;
                } else if self.state == State::Equals {
                    match c {
                        '"' | '\'' => self.state = State::Quoted(c),
                        c if c.is_whitespace() => {}
                        _ => self.state = State::Unquoted,
                    }
                } else if self.state == State::Unquoted && c.is_whitespace() {
                    self.state = State::Tag;
                    self.attribute.clear();
                } else if self.state == State::Tag {
                    // An attribute name is what runs up to the `=`.
                    if c.is_whitespace() || c == '<' {
                        self.attribute.clear();
                    } else {
                        self.attribute.push(c.to_ascii_lowercase());
                    }
                }
            }
            State::Quoted(quote) => {
                if c == quote {
                    self.state = State::Tag;
                    self.attribute.clear();
                }
            }
            State::Script => {
                if self.tail.ends_with("</script") {
                    self.state = State::Tag;
                }
            }
            State::Style => {
                if self.tail.ends_with("</style") {
                    self.state = State::Tag;
                }
            }
        }
    }
}
