use nikaia::ast::{Expr, Item, Stmt};
use nikaia::parser::parse_to_ast;

#[test]
fn test_advanced_hello_world_compilation() {
    // Advanced Hello World with Async Spawn
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
            Stmt::Expr(Expr::Call { func, args }) => {
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
