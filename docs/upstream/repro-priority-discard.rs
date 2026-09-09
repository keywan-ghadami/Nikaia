use winnow_grammar::grammar;
use winnow_grammar::testing::WinnowTestExt;

grammar! {
    grammar G {
        // Kein eigenes WS, keine Kommentare. Nur: eine Regel mit zwei
        // Alternativen in einer Wiederholung, danach ein Pflicht-Token.
        pub rule doc -> usize = xs:item* "." -> { xs.len() }
        rule item -> u32 = n:u32 -> { n } | "#" h:hex_digit1 -> { 0 }
    }
}

fn main() {
    // Bei `x` hoert `item*` auf (erwartet: Zahl oder `#`) und dann
    // scheitert das Pflicht-`.` (erwartet: `.`).
    // Richtig waere: "expected one of: `#`, `.`, integer".
    let e = G::parse_doc().parse_test("1 2 x").inner.unwrap_err();
    println!("{e}");
}
