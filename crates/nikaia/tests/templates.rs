//! The `html` template DSL (ADR-017), compiled where the template is written.
//!
//! Three decisions, and each has a test that would fail if it were quietly
//! relaxed: every hole escaped unconditionally, "already markup" said with a
//! type, and a hole only where escaping is enough.

use nikaia::emit::template::{self, Position, Segment};
use nikaia::emit::{emit_program, Profile};
use nikaia::parser::parse_to_ast;

fn emit(source: &str) -> String {
    let parsed = parse_to_ast(source).expect("the source parses");
    emit_program(&parsed, Profile::Advanced)
        .expect("the source lowers")
        .rust
}

fn refuse(source: &str) -> String {
    let parsed = parse_to_ast(source).expect("the source parses");
    format!(
        "{:#}",
        emit_program(&parsed, Profile::Advanced).expect_err("the template is refused")
    )
}

fn template(body: &str) -> String {
    format!("use std::html\nfn page(name: &str) -> String {{ return dsl html {{ {body} }} eod }}")
}

/// D1: every hole goes through `html::Render`, and there is no other path.
#[test]
fn every_hole_is_rendered() {
    let emitted = emit(&template("<p>{name}</p>"));
    assert!(
        emitted.contains("::nikaia_std::html::Render::render(&name)"),
        "{emitted}"
    );
    // The literal text is written as it stands: it is markup the template
    // author typed.
    assert!(emitted.contains(r#"__html.push_str("<p>")"#), "{emitted}");
}

/// A hole holds an *expression* of this language, not a name.
#[test]
fn a_hole_holds_an_expression() {
    let emitted = emit(
        "use std::html\n\
         pub struct R { id: i32 }\n\
         fn page(r: R) -> String { return dsl html { <td>{r.id}</td> } eod }",
    );
    assert!(
        emitted.contains("::nikaia_std::html::Render::render(&r.id)"),
        "{emitted}"
    );
}

/// D3: the positions escaping is enough for, and the ones it is not.
#[test]
fn a_hole_is_only_legal_where_escaping_is_enough() {
    // Legal: a text node and a quoted attribute that is not a URL.
    emit(&template(r#"<p class="x{name}">{name}</p>"#));

    for (body, expected) in [
        ("<script>var x = {name}</script>", "`<script>` body"),
        (r#"<a href="{name}">x</a>"#, "URL attribute"),
        ("<div class={name}>x</div>", "unquoted attribute value"),
        ("<!-- {name} -->", "HTML comment"),
        ("<style>a {{ color: {name} }}</style>", "`<style>` block"),
    ] {
        let message = refuse(&template(body));
        assert!(
            message.contains(expected),
            "{body}\nshould name {expected}, said:\n{message}"
        );
        assert!(message.contains("escaping cannot make safe"), "{message}");
    }
}

/// The refusal says every hole it refuses, not the first: a template with three
/// of them should say so three times rather than once per build.
#[test]
fn every_illegal_hole_is_named_at_once() {
    let message = refuse(&template("<script>{name}</script><a href=\"{name}\">x</a>"));
    assert!(message.contains("`<script>` body"), "{message}");
    assert!(message.contains("URL attribute"), "{message}");
}

/// The scanner reads the position off the *literal* text, which is sound
/// because a hole's value is escaped and so cannot open a tag or close a
/// string.
#[test]
fn the_scan_tracks_where_it_is() {
    let segments = template::split(r#"<p class="a">x</p><script>y</script>"#).expect("splits");
    assert_eq!(segments.len(), 1, "{segments:?}");

    let with_holes =
        template::split(r#"<p class="{a}">{b}</p><script>{c}</script>"#).expect("splits");
    let positions: Vec<Position> = with_holes
        .iter()
        .filter_map(|s| match s {
            Segment::Hole { at, .. } => Some(*at),
            _ => None,
        })
        .collect();
    assert_eq!(
        positions,
        [Position::QuotedAttribute, Position::Text, Position::Script]
    );
}

/// A quoted attribute is safe unless it is a URL, where HTML escaping is not
/// the escaping that is needed - `javascript:` survives all five characters.
#[test]
fn a_url_attribute_is_not_a_quoted_attribute() {
    for (body, at) in [
        (r#"<a href="{x}">"#, Position::Url),
        (r#"<img src="{x}">"#, Position::Url),
        (r#"<form action="{x}">"#, Position::Url),
        (r#"<a title="{x}">"#, Position::QuotedAttribute),
        (r#"<div data-id="{x}">"#, Position::QuotedAttribute),
    ] {
        let segments = template::split(body).expect("splits");
        let found = segments
            .iter()
            .find_map(|s| match s {
                Segment::Hole { at, .. } => Some(*at),
                _ => None,
            })
            .expect("a hole");
        assert_eq!(found, at, "{body}");
    }
}

/// `{{` is a literal brace, the same rule an interpolated string follows: a
/// template shares the syntax rather than inventing a second one.
#[test]
fn a_doubled_brace_is_a_literal_one() {
    let segments = template::split("<style>a {{ color: red }}</style>").expect("splits");
    assert_eq!(
        segments,
        vec![Segment::Text("<style>a { color: red }</style>".to_string())]
    );
}

/// An unclosed hole is an error rather than a template that swallows the rest
/// of itself.
#[test]
fn an_unclosed_hole_is_refused() {
    let message = format!(
        "{:#}",
        template::split("<p>{name</p>").expect_err("refused")
    );
    assert!(message.contains("unclosed"), "{message}");
}

/// Only `html` is compiled here. A `dsl sql { … }` needs the
/// deferred-parameter binding of ADR-007 D4, and saying so is better than
/// lowering it to something that reads like a template and is not one.
#[test]
fn another_dsl_target_says_what_it_needs() {
    let message = refuse("fn q() -> String { return dsl sql { SELECT 1 } eod }");
    assert!(message.contains("deferred-parameter"), "{message}");
}
