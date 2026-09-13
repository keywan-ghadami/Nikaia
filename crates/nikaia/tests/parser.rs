use nikaia::ast::{Expr, Item, Stmt};
use nikaia::parser::parse_to_ast;

#[test]
fn test_advanced_hello_world_compilation() {
    // Hello World with an async spawn
    let source_code = r#"
        fn main() {
            println("Hello Nikaia");
            spawn({
                log("Async World")
            })
        }
    "#;

    let parsed = parse_to_ast(source_code).expect("parse failed");
    let program = &parsed.program;

    // 2. Execute Verification (Inspect AST)
    assert_eq!(program.items.len(), 1, "Should have 1 main function");

    if let Item::Fn { name, body, .. } = &program.items[0].node {
        assert_eq!(parsed.text(name.expect("a named function")), "main");
        assert_eq!(body.stmts.len(), 2, "Main should have 2 statements");

        // Verify println("Hello Nikaia")
        match &body.stmts[0].node {
            Stmt::Expr(Expr::Call { func, args, .. }) => {
                if let Expr::Variable(fname) = &**func {
                    assert_eq!(parsed.text(*fname), "println");
                } else {
                    panic!("Expected function name");
                }

                assert_eq!(args.len(), 1);
                if let Expr::LitStr(s) = &args[0] {
                    assert_eq!(s, "Hello Nikaia");
                } else {
                    panic!("Expected string literal");
                }
            }
            _ => panic!("First statement should be a call"),
        }

        // Verify spawn({ ... })
        match &body.stmts[1].node {
            Stmt::Expr(Expr::Spawn {
                body: spawn_body,
                is_move,
            }) => {
                assert!(!is_move, "Should not be move by default");
                // spawn body is a Block expression
                if let Expr::Block(inner_block) = &**spawn_body {
                    assert_eq!(inner_block.stmts.len(), 1);
                } else {
                    panic!("Spawn body should be a block");
                }
            }
            _ => panic!("Second statement should be spawn"),
        }
    } else {
        panic!("Top level item is not a function");
    }
}

/// **A word keyword does not match the beginning of a longer word.**
///
/// The grammar is scannerless (ADR-001 D2): there is no lexer deciding where a
/// word ends, so `"as"` matched the first two characters of `assert`. Measured
/// before the `KW_*` boundary rules, and every one of them silent:
///
/// ```text
/// true asfoo      ->  `true as foo`
/// returnx         ->  `return x`
/// forx in 0..3    ->  `for x in 0..3`
/// ```
///
/// The last is the one that matters most, because it is a **valid program with a
/// different meaning**: it binds `x` where the source says `forx`.
#[test]
fn a_keyword_does_not_swallow_the_start_of_a_longer_word() {
    // `asfoo` is a name, so there is no cast and no type called `foo`.
    let parsed = parse_to_ast("fn main() {\n    let x = true asfoo\n}").expect("it parses");
    let Item::Fn { body, .. } = &parsed.program.items[0].node else {
        panic!("a function");
    };
    assert!(
        matches!(&body.stmts[1].node, Stmt::Expr(Expr::Variable(name))
            if parsed.text(*name) == "asfoo"),
        "`asfoo` is one name: {:?}",
        body.stmts[1].node
    );

    // `returnx` is a name and not a `return`.
    let parsed = parse_to_ast("fn f() -> i64 {\n    returnx\n}").expect("it parses");
    let Item::Fn { body, .. } = &parsed.program.items[0].node else {
        panic!("a function");
    };
    assert!(
        matches!(&body.stmts[0].node, Stmt::Expr(Expr::Variable(name))
            if parsed.text(*name) == "returnx"),
        "`returnx` is one name: {:?}",
        body.stmts[0].node
    );

    // `forx` does not begin a loop, so nothing binds `x`.
    let parsed =
        parse_to_ast("fn main() {\n    forx in 0..3 { println(\"x\") }\n}").expect("it parses");
    let Item::Fn { body, .. } = &parsed.program.items[0].node else {
        panic!("a function");
    };
    assert!(
        !matches!(&body.stmts[0].node, Stmt::For { .. }),
        "`forx` is not a `for`: {:?}",
        body.stmts[0].node
    );
}

/// …and the keywords themselves still work, which is the half a boundary could
/// break: a lowercase boundary rule would have let the implicit whitespace in
/// between, and `as i32` would then be refused for having a space in it.
#[test]
fn the_keywords_themselves_are_untouched() {
    for source in [
        "fn main() {\n    let x = 3 as i64\n}",
        "fn f() -> i64 {\n    return 7\n}",
        "fn main() {\n    for n in 0..3 { println(f\"{n}\") }\n}",
        "fn main() {\n    let mut n = 1\n    n = 2\n}",
        "pub fn f() sync {\n    return\n}",
        "fn f() throws {\n    throw oops()\n}",
        "fn main() {\n    if true { } else { }\n}",
        "fn main() {\n    let b = false\n    let c = true\n}",
        "struct S { id: i64 }\nenum E { A }\nimpl S {\n    fn id(&self) -> i64 { return self.id }\n}",
        "use utils\n\nfn main() {\n    utils::f()\n}",
        "fn main() {\n    let s = seq { println(\"a\") }\n}",
        "fn main() {\n    spawn({ println(\"a\") })\n}",
        "fn main() {\n    match 1 {\n        _ => println(\"x\")\n    }\n}",
        "fn main() {\n    while true { }\n}",
    ] {
        parse_to_ast(source).unwrap_or_else(|e| panic!("{source}\n{e:#}"));
    }
}
