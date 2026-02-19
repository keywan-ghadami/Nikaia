use anyhow::Result;
use bridge_ir::{BridgeExpr, BridgeFunction, BridgeItem, BridgeLiteral, BridgeModule, BridgeStmt};
use std::fs::File;
use std::io::Write;
use std::process::Command;

pub fn execute(bridge_module: &BridgeModule, output_path: &str) -> Result<()> {
    println!("Executing Bridge Module: {}", bridge_module.name);

    // 1. Generate Rust Source Code
    let rust_code = generate_rust_source(bridge_module)?;

    // 2. Write to temporary file
    let temp_file_path = format!("{}.rs", output_path);
    let mut file = File::create(&temp_file_path)?;
    file.write_all(rust_code.as_bytes())?;

    println!("Generated Rust source at: {}", temp_file_path);

    // 3. Invoke rustc
    // We assume 'rustc' is in the PATH.
    // We strictly use stable features here as this is the "Executor" translating BridgeIR -> Rust.
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

fn generate_rust_source(module: &BridgeModule) -> Result<String> {
    let mut code = String::new();

    // Add standard imports if needed
    // code.push_str("use std::io;\n\n");

    for item in &module.items {
        match item {
            BridgeItem::Function(func) => {
                code.push_str(&generate_function(func)?);
            }
            _ => {
                // Ignore structs for now in this vertical slice
            }
        }
    }

    Ok(code)
}

fn generate_function(func: &BridgeFunction) -> Result<String> {
    let mut code = String::new();
    code.push_str(&format!("fn {}() {{\n", func.name));

    for stmt in &func.body.stmts {
        code.push_str(&generate_stmt(stmt)?);
    }

    code.push_str("}\n\n");
    Ok(code)
}

fn generate_stmt(stmt: &BridgeStmt) -> Result<String> {
    match stmt {
        BridgeStmt::Let(let_stmt) => {
            let mut code = format!("    let {}", let_stmt.name);
            if let Some(init) = &let_stmt.init {
                code.push_str(" = ");
                code.push_str(&generate_expr(init)?);
            }
            code.push_str(";\n");
            Ok(code)
        }
        BridgeStmt::Expr(expr) => {
            let mut code = format!("    {}", generate_expr(expr)?);
            code.push_str(";\n");
            Ok(code)
        }
    }
}

fn generate_expr(expr: &BridgeExpr) -> Result<String> {
    match expr {
        BridgeExpr::Literal(lit) => match lit {
            BridgeLiteral::Int(i) => Ok(i.to_string()),
            BridgeLiteral::String(s) => Ok(format!("\"{}\"", s)),
            BridgeLiteral::Bool(b) => Ok(b.to_string()),
        },
        BridgeExpr::Variable(name) => Ok(name.clone()),
        BridgeExpr::Call(call) => {
            // Special handling for println (it's a macro in Rust)
            let func_name = match &*call.func {
                BridgeExpr::Variable(name) => name.as_str(),
                _ => return Err(anyhow::anyhow!("Indirect calls not supported yet")),
            };

            let mut code = String::new();
            if func_name == "println" {
                code.push_str("println!");
            } else {
                code.push_str(func_name);
            }

            code.push_str("(");
            for (i, arg) in call.args.iter().enumerate() {
                if i > 0 {
                    code.push_str(", ");
                }
                code.push_str(&generate_expr(arg)?);
            }
            code.push_str(")");
            Ok(code)
        }
    }
}
