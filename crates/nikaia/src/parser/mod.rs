// crates/nikaia/src/parser/mod.rs
use crate::ast;
use anyhow::Result;
use bridge_ir::{
    BridgeBlock, BridgeCall, BridgeExpr, BridgeFunction, BridgeItem, BridgeLetStmt, BridgeLiteral,
    BridgeModule, BridgeStmt,
};
use winnow::stream::LocatingSlice;
use winnow::Parser;
use winnow_grammar::{grammar, ParseContext, ParseInput};

// --- Public API ---

pub fn parse_to_bridge(input: &str) -> Result<BridgeModule> {
    lower_program(&parse_to_ast(input)?)
}

pub fn parse_to_ast(input: &str) -> Result<ast::Program> {
    // Generated parsers run on a `Stateful` stream: `LocatingSlice` supplies the
    // spans, `ParseContext` carries the shared parser state.
    let mut stream = ParseInput::<()> {
        state: ParseContext::default(),
        input: LocatingSlice::new(input),
    };

    // `parse_<rule>()` is a factory: calling it builds the parser, which we then
    // drive with `.parse_next()`.
    let program = CompilerGrammar::parse_program()
        .parse_next(&mut stream)
        .map_err(|e| anyhow::anyhow!("Parse error:\n{}", e.render(input)))?;

    // The generated entry point already refuses leftover input; this is a
    // backstop so a partial parse can never be reported as a success.
    if !stream.input.is_empty() {
        return Err(anyhow::anyhow!(
            "Parse error: unexpected trailing input at byte {}",
            input.len() - stream.input.len()
        ));
    }

    Ok(program)
}

// --- Grammar Definition ---

grammar! {
    grammar CompilerGrammar {
        use crate::ast::*;
        use winnow::ascii::{digit1, multispace0};

        // --- Entry Point ---
        // Rule 'program' -> generates 'parse_program'
        pub rule program -> Program =
            _start:skip_ws
            items:item*
            _end:skip_ws
            -> {
                Program { items }
            }

        rule skip_ws -> () = multispace0 -> { () }

        // --- Top-Level Items ---
        rule item -> Item =
            i:fn_item -> { i }

        rule kw_sync -> () = "sync" -> { () }

        rule fn_item -> Item =
            "fn"
            _sp:skip_ws
            name:raw_ident
            _sp2:skip_ws
            generics:generic_list?
            args:fn_arg_list
            _sp3:skip_ws
            is_sync:kw_sync?
            _sp4:skip_ws
            ret:return_type_arrow?
            _sp5:skip_ws
            body:block
            -> {
                Item::Fn {
                    name: name.to_string(),
                    generics: generics.unwrap_or_default(),
                    args,
                    ret_type: ret,
                    body,
                    is_sync: is_sync.is_some()
                }
            }

        // --- Argumente & Typen ---

        rule fn_arg_list -> Vec<FnArg> =
            "(" _sp:skip_ws args:fn_arg_defs? _sp2:skip_ws ")" -> { args.unwrap_or_default() }

        rule fn_arg_defs -> Vec<FnArg> =
            head:fn_arg_def tail:fn_arg_def_tail* -> {
                let mut args = vec![head];
                args.extend(tail);
                args
            }

        rule fn_arg_def_tail -> FnArg =
            _sp:skip_ws "," _sp2:skip_ws arg:fn_arg_def -> { arg }

        rule fn_arg_def -> FnArg =
            name:raw_ident _sp:skip_ws ":" _sp2:skip_ws ty:type_ref -> {
                FnArg { name: name.to_string(), ty }
            }

        rule return_type_arrow -> Type =
            "->" _sp:skip_ws ty:type_ref -> { ty }

        // USING [ ] SYNTAX directly for testing
        rule generic_list -> Vec<GenericParam> =
            [ _sp:skip_ws params:generic_params? _sp2:skip_ws ] -> { params.unwrap_or_default() }

        rule generic_params -> Vec<GenericParam> =
            head:generic_param tail:generic_param_tail* -> {
                let mut params = vec![head];
                params.extend(tail);
                params
            }

        rule generic_param_tail -> GenericParam =
            _sp:skip_ws "," _sp2:skip_ws p:generic_param -> { p }

        rule generic_param -> GenericParam =
            name:raw_ident
            -> { GenericParam { name: name.to_string() } }

        rule type_ref -> Type =
            name:raw_ident
            generics:generic_type_args?
            -> {
                Type { name: name.to_string(), generics: generics.unwrap_or_default() }
            }

        // USING [ ] SYNTAX directly for testing
        rule generic_type_args -> Vec<Type> =
            [ _sp:skip_ws args:type_refs? _sp2:skip_ws ] -> { args.unwrap_or_default() }

        rule type_refs -> Vec<Type> =
            head:type_ref tail:type_ref_tail* -> {
                let mut args = vec![head];
                args.extend(tail);
                args
            }

        rule type_ref_tail -> Type =
            _sp:skip_ws "," _sp2:skip_ws t:type_ref -> { t }

        // --- Statements & Blocks ---

        rule block -> Block =
            "{" _sp:skip_ws stmts:stmt_list _sp2:skip_ws "}" -> { Block { stmts } }

        rule stmt_list -> Vec<Stmt> =
            stmts:stmt* -> { stmts }

        rule stmt -> Stmt =
            l:let_stmt -> { l }
          | e:expr_stmt -> { e }

        rule kw_mut -> () = "mut" -> { () }

        rule let_stmt -> Stmt =
            "let"
            _sp:skip_ws
            mutable:kw_mut?
            _sp2:skip_ws
            name:raw_ident
            _sp3:skip_ws
            ty:type_annotation?
            _sp4:skip_ws
            "="
            _sp5:skip_ws
            val:expr
            _sp6:skip_ws
            ";"?
            _sp7:skip_ws
            -> {
                Stmt::Let {
                    name: name.to_string(),
                    mutable: mutable.is_some(),
                    ty,
                    value: val
                }
            }

        rule type_annotation -> Type =
            ":" _sp:skip_ws ty:type_ref -> { ty }

        rule expr_stmt -> Stmt =
            e:expr _sp:skip_ws ";"? _sp2:skip_ws -> { Stmt::Expr(e) }

        // --- Expressions ---

        // `spawn` before `call_expr`: otherwise `spawn(...)` parses as an
        // ordinary call and never reaches `Expr::Spawn`.
        rule expr -> Expr =
            sp:spawn_expr -> { sp }
          | c:call_expr -> { c }
          | b:block_expr -> { b }
          | s:str_lit -> { s }
          | i:int_lit -> { i }
          | v:var_expr -> { v }

        // Part I, 8.2: spawn takes a block lambda - `spawn({ ... })`.
        rule spawn_expr -> Expr =
            "spawn" _sp:skip_ws "(" _sp2:skip_ws body:expr _sp3:skip_ws ")" -> {
                Expr::Spawn { body: Box::new(body), is_move: false }
            }

        // Blocks are expressions (Part I, 3.1).
        rule block_expr -> Expr =
            b:block -> { Expr::Block(b) }

        rule call_expr -> Expr =
            func:raw_ident _sp:skip_ws "(" _sp2:skip_ws args:call_args? _sp3:skip_ws ")" -> {
                Expr::Call {
                    func: Box::new(Expr::Variable(func.to_string())),
                    args: args.unwrap_or_default(),
                }
            }

        rule call_args -> Vec<Expr> =
            head:expr tail:call_args_tail* -> {
                let mut args = vec![head];
                args.extend(tail);
                args
            }

        rule call_args_tail -> Expr =
            _sp:skip_ws "," _sp2:skip_ws e:expr -> { e }

        rule str_lit -> Expr =
            s:string -> {
                Expr::LitStr(s.to_string())
            }

        rule int_lit -> Expr =
            d:digits -> {
                Expr::LitInt(d.parse().unwrap())
            }

        rule var_expr -> Expr =
            n:raw_ident -> { Expr::Variable(n.to_string()) }

        rule digits -> String =
            d:digit1 -> { d.to_string() }
    }
}

// --- Lowering (AST -> Bridge) ---

fn lower_program(prog: &ast::Program) -> Result<BridgeModule> {
    let mut items = Vec::new();
    for item in &prog.items {
        if let Some(bridge_item) = lower_item(item)? {
            items.push(bridge_item);
        }
    }

    Ok(BridgeModule {
        name: "main".to_string(),
        items,
    })
}

fn lower_item(item: &ast::Item) -> Result<Option<BridgeItem>> {
    match item {
        ast::Item::Fn { name, body, .. } => Ok(Some(BridgeItem::Function(BridgeFunction {
            name: name.clone(),
            args: vec![],
            ret_type: None,
            body: lower_block(body)?,
            span: 0..0,
        }))),
        _ => Ok(None),
    }
}

fn lower_block(block: &ast::Block) -> Result<BridgeBlock> {
    let mut stmts = Vec::new();
    for stmt in &block.stmts {
        stmts.push(lower_stmt(stmt)?);
    }
    Ok(BridgeBlock { stmts, span: 0..0 })
}

fn lower_stmt(stmt: &ast::Stmt) -> Result<BridgeStmt> {
    match stmt {
        ast::Stmt::Let { name, value, .. } => Ok(BridgeStmt::Let(BridgeLetStmt {
            name: name.clone(),
            ty: None,
            init: Some(lower_expr(value)?),
            span: 0..0,
        })),
        ast::Stmt::Expr(expr) => Ok(BridgeStmt::Expr(lower_expr(expr)?)),
        _ => Err(anyhow::anyhow!("Unsupported statement type")),
    }
}

fn lower_expr(expr: &ast::Expr) -> Result<BridgeExpr> {
    match expr {
        ast::Expr::LitInt(i) => Ok(BridgeExpr::Literal(BridgeLiteral::Int(*i))),
        ast::Expr::LitStr(s) => Ok(BridgeExpr::Literal(BridgeLiteral::String(s.clone()))),
        ast::Expr::Variable(id) => Ok(BridgeExpr::Variable(id.clone())),
        ast::Expr::Call { func, args } => {
            let mut bridge_args = Vec::new();
            for arg in args {
                bridge_args.push(lower_expr(arg)?);
            }
            Ok(BridgeExpr::Call(BridgeCall {
                func: Box::new(lower_expr(func)?),
                args: bridge_args,
                span: 0..0,
            }))
        }
        _ => Err(anyhow::anyhow!("Unsupported expression type: {:?}", expr)),
    }
}
