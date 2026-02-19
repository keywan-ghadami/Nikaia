// crates/rustc-executor/src/lib.rs
use anyhow::Result;
use bridge_ir::{BridgeExpr, BridgeFunction, BridgeItem, BridgeLiteral, BridgeModule, BridgeStmt};
use std::fs::File;
use std::io::Write;
use std::process::Command;

// Include our mock AST (ADR-004 Unified AST Lowering)
mod ast;
use ast::{Block, Call, Crate, Expr, Function, Item, ItemKind, Lit, Local, Pat, Stmt};

pub fn execute(bridge_module: &BridgeModule, output_path: &str) -> Result<()> {
    println!("Executing Bridge Module: {}", bridge_module.name);

    // 1. Unified Lowering (Bridge -> AST)
    let krate = lower_module(bridge_module)?;

    // 2. Transpilation (AST -> Rust Source)
    let rust_code = ast::print_crate(&krate);

    // 3. Write to temporary file
    let temp_file_path = format!("{}.rs", output_path);
    let mut file = File::create(&temp_file_path)?;
    file.write_all(rust_code.as_bytes())?;

    println!("Generated Rust source at: {}", temp_file_path);

    // 4. Invoke rustc (Driver Logic)
    let status = Command::new("rustc")
        .arg(&temp_file_path)
        .arg("-o")
        .arg(output_path)
        .status()?;

    if !status.success() {
        return Err(anyhow::anyhow!("rustc failed with status: {}", status));
    }

    println!("Successfully compiled to executable: {}", output_path);

    Ok(())
}

fn lower_module(module: &BridgeModule) -> Result<Crate> {
    let mut items = Vec::new();
    for item in &module.items {
        if let Some(lowered) = lower_item(item)? {
            items.push(lowered);
        }
    }
    Ok(Crate { items })
}

fn lower_item(item: &BridgeItem) -> Result<Option<Item>> {
    match item {
        BridgeItem::Function(func) => Ok(Some(Item {
            name: func.name.clone(),
            kind: ItemKind::Fn(lower_fn(func)?),
        })),
        _ => Ok(None),
    }
}

fn lower_fn(func: &BridgeFunction) -> Result<Function> {
    Ok(Function {
        body: lower_block(&func.body)?,
    })
}

fn lower_block(block: &bridge_ir::BridgeBlock) -> Result<Block> {
    let mut stmts = Vec::new();
    for stmt in &block.stmts {
        stmts.push(lower_stmt(stmt)?);
    }
    Ok(Block { stmts })
}

fn lower_stmt(stmt: &BridgeStmt) -> Result<Stmt> {
    match stmt {
        BridgeStmt::Let(let_stmt) => Ok(Stmt::Local(Local {
            pat: Pat::Ident(let_stmt.name.clone()),
            init: if let Some(init) = &let_stmt.init {
                Some(lower_expr(init)?)
            } else {
                None
            },
        })),
        BridgeStmt::Expr(expr) => Ok(Stmt::Semi(lower_expr(expr)?)),
    }
}

fn lower_expr(expr: &BridgeExpr) -> Result<Expr> {
    match expr {
        BridgeExpr::Literal(lit) => Ok(Expr::Lit(lower_lit(lit)?)),
        BridgeExpr::Variable(name) => Ok(Expr::Path(name.clone())),
        BridgeExpr::Call(call) => {
            let mut args = Vec::new();
            for arg in &call.args {
                args.push(lower_expr(arg)?);
            }
            Ok(Expr::Call(Call {
                func: Box::new(lower_expr(&call.func)?),
                args,
            }))
        }
    }
}

fn lower_lit(lit: &BridgeLiteral) -> Result<Lit> {
    match lit {
        BridgeLiteral::Int(i) => Ok(Lit::Int(*i)),
        BridgeLiteral::String(s) => Ok(Lit::Str(s.clone())),
        BridgeLiteral::Bool(b) => Ok(Lit::Bool(*b)),
    }
}
