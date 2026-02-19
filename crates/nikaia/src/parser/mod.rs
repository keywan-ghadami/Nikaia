// crates/nikaia/src/parser/mod.rs
use crate::ast;
use anyhow::Result;
use bridge_ir::{
    BridgeBlock, BridgeCall, BridgeExpr, BridgeFunction, BridgeItem, BridgeLetStmt, BridgeLiteral,
    BridgeModule, BridgeStmt,
};
use winnow_grammar::grammar;

// --- Public API ---

pub fn parse_to_bridge(input: &str) -> Result<BridgeModule> {
    let program = parse_to_ast(input)?;
    lower_program(program)
}

pub fn parse_to_ast(input: &str) -> Result<ast::Program> {
    use winnow::stream::LocatingSlice;
    use winnow::Parser;

    // Wrap input in LocatingSlice to provide Location trait required by the grammar
    let input = LocatingSlice::new(input);

    // The macro generates a module `CompilerGrammar`
    // The rule `program` becomes `parse_program`
    CompilerGrammar::parse_program
        .parse(input)
        .map_err(|e| anyhow::anyhow!("Parse error:\n{}", e))
}

// --- Grammar Definition ---

grammar! {
    grammar CompilerGrammar {
        use crate::ast::*;
        use syn::Ident;
        use proc_macro2::Span;
        use winnow::ascii::{multispace0, digit1};

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
            name:ident
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
                    name: Ident::new(&name, Span::call_site()),
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
            name:ident _sp:skip_ws ":" _sp2:skip_ws ty:type_ref -> {
                FnArg { name: Ident::new(&name, Span::call_site()), ty }
            }

        rule return_type_arrow -> Type =
            "->" _sp:skip_ws ty:type_ref -> { ty }

        rule generic_list -> Vec<GenericParam> =
            "[" _sp:skip_ws params:generic_params? _sp2:skip_ws "]" -> { params.unwrap_or_default() }

        rule generic_params -> Vec<GenericParam> =
            head:generic_param tail:generic_param_tail* -> {
                let mut params = vec![head];
                params.extend(tail);
                params
            }

        rule generic_param_tail -> GenericParam =
            _sp:skip_ws "," _sp2:skip_ws p:generic_param -> { p }

        rule generic_param -> GenericParam =
            name:ident
            -> { GenericParam { name: Ident::new(&name, Span::call_site()) } }

        rule type_ref -> Type =
            name:ident
            generics:generic_type_args?
            -> {
                Type { name: Ident::new(&name, Span::call_site()), generics: generics.unwrap_or_default() }
            }

        rule generic_type_args -> Vec<Type> =
            "[" _sp:skip_ws args:type_refs? _sp2:skip_ws "]" -> { args.unwrap_or_default() }

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
            name:ident
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
                    name: Ident::new(&name, Span::call_site()),
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

        rule expr -> Expr =
            c:call_expr -> { c }
          | s:str_lit -> { s }
          | i:int_lit -> { i }
          | v:var_expr -> { v }

        rule call_expr -> Expr =
            func:ident _sp:skip_ws "(" _sp2:skip_ws args:call_args? _sp3:skip_ws ")" -> {
                Expr::Call {
                    func: Box::new(Expr::Variable(Ident::new(&func, Span::call_site()))),
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
                Expr::LitStr(s)
            }

        rule int_lit -> Expr =
            d:digits -> {
                Expr::LitInt(d.parse().unwrap())
            }

        rule var_expr -> Expr =
            n:ident -> { Expr::Variable(Ident::new(&n, Span::call_site())) }

        rule digits -> String =
            d:digit1 -> { d.to_string() }
    }
}

// --- Lowering (AST -> Bridge) ---

fn lower_program(prog: ast::Program) -> Result<BridgeModule> {
    let mut items = Vec::new();
    for item in prog.items {
        if let Some(bridge_item) = lower_item(item)? {
            items.push(bridge_item);
        }
    }

    Ok(BridgeModule {
        name: "main".to_string(),
        items,
    })
}

fn lower_item(item: ast::Item) -> Result<Option<BridgeItem>> {
    match item {
        ast::Item::Fn { name, body, .. } => Ok(Some(BridgeItem::Function(BridgeFunction {
            name: name.to_string(),
            args: vec![],
            ret_type: None,
            body: lower_block(body)?,
            span: 0..0,
        }))),
        _ => Ok(None),
    }
}

fn lower_block(block: ast::Block) -> Result<BridgeBlock> {
    let mut stmts = Vec::new();
    for stmt in block.stmts {
        stmts.push(lower_stmt(stmt)?);
    }
    Ok(BridgeBlock { stmts, span: 0..0 })
}

fn lower_stmt(stmt: ast::Stmt) -> Result<BridgeStmt> {
    match stmt {
        ast::Stmt::Let { name, value, .. } => Ok(BridgeStmt::Let(BridgeLetStmt {
            name: name.to_string(),
            ty: None,
            init: Some(lower_expr(value)?),
            span: 0..0,
        })),
        ast::Stmt::Expr(expr) => Ok(BridgeStmt::Expr(lower_expr(expr)?)),
        _ => Err(anyhow::anyhow!("Unsupported statement type")),
    }
}

fn lower_expr(expr: ast::Expr) -> Result<BridgeExpr> {
    match expr {
        ast::Expr::LitInt(i) => Ok(BridgeExpr::Literal(BridgeLiteral::Int(i))),
        ast::Expr::LitStr(s) => Ok(BridgeExpr::Literal(BridgeLiteral::String(s))),
        ast::Expr::Variable(id) => Ok(BridgeExpr::Variable(id.to_string())),
        ast::Expr::Call { func, args } => {
            let mut bridge_args = Vec::new();
            for arg in args {
                bridge_args.push(lower_expr(arg)?);
            }
            Ok(BridgeExpr::Call(BridgeCall {
                func: Box::new(lower_expr(*func)?),
                args: bridge_args,
                span: 0..0,
            }))
        }
        _ => Err(anyhow::anyhow!("Unsupported expression type: {:?}", expr)),
    }
}
