// crates/nikaia/src/parser/mod.rs
use crate::ast;
use anyhow::Result;
use bridge_ir::{
    BridgeBlock, BridgeExpr, BridgeFunction, BridgeItem, BridgeLetStmt, BridgeLiteral,
    BridgeModule, BridgeStmt,
};
use winnow::ascii::{digit1, space0};
use winnow::combinator::terminated;
use winnow::prelude::*;
use winnow::token::literal;

pub fn parse_to_bridge(input: &str) -> Result<BridgeModule> {
    // A simplified parser for demonstration purposes
    // Parse "let x = 5;"

    let mut parser = terminated(parse_let, space0);

    match parser.parse(input) {
        Ok(stmt) => Ok(BridgeModule {
            name: "main".to_string(),
            items: vec![BridgeItem::Function(BridgeFunction {
                name: "main".to_string(),
                args: vec![],
                ret_type: None,
                body: BridgeBlock {
                    stmts: vec![BridgeStmt::Let(stmt)],
                    span: 0..input.len(),
                },
                span: 0..input.len(),
            })],
        }),
        Err(e) => Err(anyhow::anyhow!("Parse error: {}", e)),
    }
}

pub fn parse_to_ast(input: &str) -> Result<ast::Program> {
    // A parser that converts input to AST for the interpreter.
    // This is distinct from the BridgeIR parser, although they should ideally share logic.
    // For this demonstration, we'll parse the same "let x = 5;" but to AST nodes.

    let mut parser = terminated(parse_let_ast, space0);

    match parser.parse(input) {
        Ok(stmt) => Ok(ast::Program {
            items: vec![ast::Item::Fn {
                name: syn::Ident::new("main", proc_macro2::Span::call_site()),
                generics: vec![],
                args: vec![],
                ret_type: None,
                body: ast::Block { stmts: vec![stmt] },
                is_sync: true,
            }],
        }),
        Err(e) => Err(anyhow::anyhow!("Parse AST error: {}", e)),
    }
}

fn parse_let(input: &mut &str) -> ModalResult<BridgeLetStmt> {
    let _ = space0.parse_next(input)?;
    let _ = literal("let").parse_next(input)?;
    let _ = space0.parse_next(input)?;
    let name = parse_identifier.parse_next(input)?;
    let _ = space0.parse_next(input)?;
    let _ = literal("=").parse_next(input)?;
    let _ = space0.parse_next(input)?;
    let value = parse_expr.parse_next(input)?;
    let _ = space0.parse_next(input)?;
    let _ = literal(";").parse_next(input)?;

    Ok(BridgeLetStmt {
        name: name.to_string(),
        ty: None,
        init: Some(value),
        span: 0..0, // Simplified span for now
    })
}

fn parse_let_ast(input: &mut &str) -> ModalResult<ast::Stmt> {
    let _ = space0.parse_next(input)?;
    let _ = literal("let").parse_next(input)?;
    let _ = space0.parse_next(input)?;
    let name = parse_identifier.parse_next(input)?;
    let _ = space0.parse_next(input)?;
    let _ = literal("=").parse_next(input)?;
    let _ = space0.parse_next(input)?;
    let value = parse_expr_ast.parse_next(input)?;
    let _ = space0.parse_next(input)?;
    let _ = literal(";").parse_next(input)?;

    Ok(ast::Stmt::Let {
        name: syn::Ident::new(name, proc_macro2::Span::call_site()),
        mutable: false,
        ty: None,
        value: value,
    })
}

fn parse_identifier<'a>(input: &mut &'a str) -> ModalResult<&'a str> {
    winnow::ascii::alpha1.parse_next(input)
}

fn parse_expr(input: &mut &str) -> ModalResult<BridgeExpr> {
    let num_str = digit1.parse_next(input)?;
    let num = num_str.parse::<i64>().unwrap();
    Ok(BridgeExpr::Literal(BridgeLiteral::Int(num)))
}

fn parse_expr_ast(input: &mut &str) -> ModalResult<ast::Expr> {
    let num_str = digit1.parse_next(input)?;
    let num = num_str.parse::<i64>().unwrap();
    Ok(ast::Expr::LitInt(num))
}
