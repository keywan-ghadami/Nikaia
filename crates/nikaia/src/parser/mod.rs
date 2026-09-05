// crates/nikaia/src/parser/mod.rs
use crate::ast;
use anyhow::Result;
use bridge_ir::{
    BridgeBlock, BridgeCall, BridgeExpr, BridgeFunction, BridgeItem, BridgeLetStmt, BridgeLiteral,
    BridgeModule, BridgeStmt,
};
use winnow::stream::LocatingSlice;
use winnow::Parser;
use winnow_grammar::{grammar, InternerContext, ParseContext, ParseInput, Symbol};

// --- Public API ---

/// A parsed program together with the interner that produced its identifiers.
///
/// Identifiers in the AST are `Symbol` handles, which are only meaningful with
/// the `InternerContext` they were interned into - so the two travel together.
///
/// Note the cost of interning: the derived `Debug` prints identifiers as
/// `Symbol(3)` rather than their text. Use [`Parsed::text`] when a name needs to
/// be read.
#[derive(Debug)]
pub struct Parsed {
    pub program: ast::Program,
    pub interner: InternerContext,
}

impl Parsed {
    /// Resolve an identifier back to its text.
    pub fn text(&self, sym: Symbol) -> &str {
        self.interner.resolve(sym)
    }
}

pub fn parse_to_bridge(input: &str) -> Result<BridgeModule> {
    lower_program(&parse_to_ast(input)?)
}

pub fn parse_to_ast(input: &str) -> Result<Parsed> {
    // Generated parsers run on a `Stateful` stream: `LocatingSlice` supplies the
    // spans, `ParseContext` carries the shared parser state including the
    // interner. Cloning the interner out shares it (it is an `Arc` inside), so
    // the handles in the AST stay resolvable after parsing.
    let context = ParseContext::<()>::default();
    let interner = context.interner.clone();

    let mut stream = ParseInput::<()> {
        state: context,
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

    Ok(Parsed { program, interner })
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
                    name,
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
                FnArg { name, ty }
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
            name:ident
            -> { GenericParam { name } }

        rule type_ref -> Type =
            name:ident
            generics:generic_type_args?
            -> {
                Type { name, generics: generics.unwrap_or_default() }
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
                    name,
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
            func:ident _sp:skip_ws "(" _sp2:skip_ws args:call_args? _sp3:skip_ws ")" -> {
                Expr::Call {
                    func: Box::new(Expr::Variable(func)),
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
            n:ident -> { Expr::Variable(n) }

        rule digits -> String =
            d:digit1 -> { d.to_string() }
    }
}

// --- Lowering (AST -> Bridge) ---

fn lower_program(parsed: &Parsed) -> Result<BridgeModule> {
    let mut items = Vec::new();
    for item in &parsed.program.items {
        if let Some(bridge_item) = lower_item(parsed, item)? {
            items.push(bridge_item);
        }
    }

    Ok(BridgeModule {
        name: "main".to_string(),
        items,
    })
}

fn lower_item(parsed: &Parsed, item: &ast::Item) -> Result<Option<BridgeItem>> {
    match item {
        ast::Item::Fn { name, body, .. } => Ok(Some(BridgeItem::Function(BridgeFunction {
            name: parsed.text(*name).to_string(),
            args: vec![],
            ret_type: None,
            body: lower_block(parsed, body)?,
            span: 0..0,
        }))),
        _ => Ok(None),
    }
}

fn lower_block(parsed: &Parsed, block: &ast::Block) -> Result<BridgeBlock> {
    let mut stmts = Vec::new();
    for stmt in &block.stmts {
        stmts.push(lower_stmt(parsed, stmt)?);
    }
    Ok(BridgeBlock { stmts, span: 0..0 })
}

fn lower_stmt(parsed: &Parsed, stmt: &ast::Stmt) -> Result<BridgeStmt> {
    match stmt {
        ast::Stmt::Let { name, value, .. } => Ok(BridgeStmt::Let(BridgeLetStmt {
            name: parsed.text(*name).to_string(),
            ty: None,
            init: Some(lower_expr(parsed, value)?),
            span: 0..0,
        })),
        ast::Stmt::Expr(expr) => Ok(BridgeStmt::Expr(lower_expr(parsed, expr)?)),
        _ => Err(anyhow::anyhow!("Unsupported statement type")),
    }
}

fn lower_expr(parsed: &Parsed, expr: &ast::Expr) -> Result<BridgeExpr> {
    match expr {
        ast::Expr::LitInt(i) => Ok(BridgeExpr::Literal(BridgeLiteral::Int(*i))),
        ast::Expr::LitStr(s) => Ok(BridgeExpr::Literal(BridgeLiteral::String(s.clone()))),
        ast::Expr::Variable(id) => Ok(BridgeExpr::Variable(parsed.text(*id).to_string())),
        ast::Expr::Call { func, args } => {
            let mut bridge_args = Vec::new();
            for arg in args {
                bridge_args.push(lower_expr(parsed, arg)?);
            }
            Ok(BridgeExpr::Call(BridgeCall {
                func: Box::new(lower_expr(parsed, func)?),
                args: bridge_args,
                span: 0..0,
            }))
        }
        _ => Err(anyhow::anyhow!("Unsupported expression type: {:?}", expr)),
    }
}
