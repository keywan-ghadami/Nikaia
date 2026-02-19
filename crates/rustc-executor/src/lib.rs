#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_ast_pretty;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_session;
extern crate rustc_span;

use std::fs::File;
use std::io::Write;
use std::process::Command;

use anyhow::{anyhow, Result};
use bridge_ir::{
    BridgeCall, BridgeExpr, BridgeFunction, BridgeItem, BridgeLetStmt, BridgeLiteral, BridgeModule,
    BridgeStmt,
};

use rustc_ast::{
    self as ast, Block, BlockCheckMode, Crate, Expr, ExprKind, Fn, FnHeader, FnRetTy, FnSig,
    Generics, ItemKind, Local, LocalKind, MacCall, NodeId, Pat, PatKind, Path, Stmt, StmtKind,
    StrStyle, Ty, TyKind, Visibility, VisibilityKind,
};

use rustc_ast::token::{self, Lit as TokenLit, Token, TokenKind};
use rustc_ast::tokenstream::{DelimSpan, TokenStream, TokenTree};
use rustc_data_structures::thin_vec::{thin_vec, ThinVec};
use rustc_span::symbol::{Ident, Symbol};
use rustc_span::{Span, DUMMY_SP};

pub fn execute(bridge_module: &BridgeModule, output_path: &str) -> Result<()> {
    let krate = lower_module(bridge_module)?;

    let mut rust_code = String::new();
    for item in &krate.items {
        rust_code.push_str(&rustc_ast_pretty::pprust::item_to_string(item));
        rust_code.push('\n');
    }

    let temp_file_path = format!("{}.rs", output_path);
    let mut file = File::create(&temp_file_path)?;
    file.write_all(rust_code.as_bytes())?;

    println!("Generated Rust source at: {}", temp_file_path);

    let status = Command::new("rustc")
        .arg(&temp_file_path)
        .arg("-o")
        .arg(output_path)
        .status()?;

    if !status.success() {
        return Err(anyhow::anyhow!("rustc failed with status: {}", status));
    }

    Ok(())
}

fn lower_module(module: &BridgeModule) -> Result<Crate> {
    let mut items = ThinVec::new();
    for item in &module.items {
        if let Some(ast_item) = lower_item(item)? {
            items.push(Box::new(ast_item)); // P -> Box
        }
    }

    Ok(Crate {
        attrs: ThinVec::new(),
        items,
        spans: ast::ModSpans {
            inner_span: DUMMY_SP,
            inject_use_span: DUMMY_SP,
        },
        id: NodeId::from_u32(0),
        is_placeholder: false,
    })
}

fn lower_item(item: &BridgeItem) -> Result<Option<ast::Item>> {
    match item {
        BridgeItem::Function(func) => {
            let ident = Ident::from_str(&func.name);
            let kind = ItemKind::Fn(Box::new(lower_fn(func, ident)?));
            Ok(Some(ast::Item {
                attrs: ThinVec::new(),
                id: NodeId::from_u32(0),
                kind,
                vis: Visibility {
                    kind: VisibilityKind::Public,
                    span: DUMMY_SP,
                    tokens: None,
                },
                span: DUMMY_SP,
                tokens: None,
            }))
        }
        _ => Ok(None),
    }
}

fn lower_fn(func: &BridgeFunction, ident: Ident) -> Result<Fn> {
    let sig = FnSig {
        header: FnHeader::default(),
        decl: Box::new(ast::FnDecl {
            // P -> Box
            inputs: ThinVec::new(),
            output: FnRetTy::Default(DUMMY_SP),
        }),
        span: DUMMY_SP,
    };

    Ok(Fn {
        defaultness: ast::Defaultness::Final,
        generics: Generics::default(),
        sig,
        body: Some(Box::new(lower_block(&func.body)?)), // P -> Box
        contract: None,
        define_opaque: None,
        eii_impls: ThinVec::new(),
        ident,
    })
}

fn lower_block(block: &bridge_ir::BridgeBlock) -> Result<Block> {
    let mut stmts = ThinVec::new();
    for stmt in &block.stmts {
        stmts.push(lower_stmt(stmt)?);
    }

    Ok(Block {
        stmts,
        id: NodeId::from_u32(0),
        rules: BlockCheckMode::Default,
        span: DUMMY_SP,
        tokens: None,
    })
}

fn lower_stmt(stmt: &BridgeStmt) -> Result<Stmt> {
    match stmt {
        BridgeStmt::Let(let_stmt) => {
            let local = Local {
                pat: Box::new(Pat {
                    // P -> Box
                    id: NodeId::from_u32(0),
                    kind: PatKind::Ident(
                        ast::BindingMode::NONE,
                        Ident::from_str(&let_stmt.name),
                        None,
                    ),
                    span: DUMMY_SP,
                    tokens: None,
                }),
                ty: None,
                kind: if let Some(init) = &let_stmt.init {
                    LocalKind::Init(Box::new(lower_expr(init)?)) // P -> Box
                } else {
                    LocalKind::Decl
                },
                id: NodeId::from_u32(0),
                span: DUMMY_SP,
                attrs: ThinVec::new(),
                tokens: None,
                colon_sp: Some(DUMMY_SP),
                super_: None,
            };

            Ok(Stmt {
                id: NodeId::from_u32(0),
                kind: StmtKind::Let(Box::new(local)), // P -> Box
                span: DUMMY_SP,
            })
        }
        BridgeStmt::Expr(expr) => {
            Ok(Stmt {
                id: NodeId::from_u32(0),
                kind: StmtKind::Semi(Box::new(lower_expr(expr)?)), // P -> Box
                span: DUMMY_SP,
            })
        }
    }
}

fn lower_expr(expr: &BridgeExpr) -> Result<Expr> {
    let kind = match expr {
        BridgeExpr::Literal(lit) => lower_lit_expr(lit)?,
        BridgeExpr::Variable(name) => ExprKind::Path(None, Path::from_ident(Ident::from_str(name))),
        BridgeExpr::Call(call) => {
            let func_name = match &*call.func {
                BridgeExpr::Variable(name) => name.as_str(),
                _ => "",
            };

            if func_name == "println" {
                return lower_println(call);
            } else {
                let func = lower_expr(&call.func)?;
                let mut args = ThinVec::new();
                for arg in &call.args {
                    args.push(Box::new(lower_expr(arg)?)); // P -> Box
                }
                ExprKind::Call(Box::new(func), args) // P -> Box
            }
        }
    };

    Ok(Expr {
        id: NodeId::from_u32(0),
        kind,
        span: DUMMY_SP,
        attrs: ThinVec::new(),
        tokens: None,
    })
}

fn lower_lit_expr(lit: &BridgeLiteral) -> Result<ExprKind> {
    let kind = match lit {
        // ExprKind::Lit takes token::Lit, not LitKind
        // wait, I need token::Lit here.
        BridgeLiteral::Int(i) => {
            TokenLit::new(token::Integer, Symbol::intern(&i.to_string()), None)
        }
        BridgeLiteral::String(s) => TokenLit::new(token::Str, Symbol::intern(s), None),
        BridgeLiteral::Bool(b) => TokenLit::new(token::Bool, Symbol::intern(&b.to_string()), None),
    };

    Ok(ExprKind::Lit(kind))
}

fn lower_println(call: &BridgeCall) -> Result<Expr> {
    let mut trees = Vec::new();

    for (i, arg) in call.args.iter().enumerate() {
        if i > 0 {
            trees.push(TokenTree::Token(
                Token::new(TokenKind::Comma, DUMMY_SP),
                ast::tokenstream::Spacing::Alone,
            ));
        }

        match arg {
            BridgeExpr::Literal(BridgeLiteral::String(s)) => {
                let token = Token::new(
                    TokenKind::Literal(TokenLit::new(ast::token::Str, Symbol::intern(s), None)),
                    DUMMY_SP,
                );
                trees.push(TokenTree::Token(token, ast::tokenstream::Spacing::Alone));
            }
            _ => return Err(anyhow!("Only string literals supported in println for now")),
        }
    }

    let mac = MacCall {
        path: Path::from_ident(Ident::from_str("println")),
        args: Box::new(ast::DelimArgs {
            // P -> Box
            dspan: DelimSpan::dummy(),
            delim: ast::token::Delimiter::Parenthesis,
            tokens: TokenStream::new(trees),
        }),
    };

    Ok(Expr {
        id: NodeId::from_u32(0),
        kind: ExprKind::MacCall(Box::new(mac)), // P -> Box
        span: DUMMY_SP,
        attrs: ThinVec::new(),
        tokens: None,
    })
}
