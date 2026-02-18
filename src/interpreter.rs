// src/interpreter.rs
use crate::ast::{Expr, Item, Program, Stmt};

pub struct Interpreter;

impl Interpreter {
    pub fn new() -> Self {
        Self
    }

    pub fn run(&self, program: &Program) {
        println!("[Nikaia Kernel] Interpreter Init...");
        // Entry point lookup: find 'main' function
        for item in &program.items {
            if let Item::Fn { name, body, .. } = item {
                if name.to_string() == "main" {
                    println!("[Nikaia Kernel] Executing 'main'...");
                    self.eval_block(body);
                    return;
                }
            }
        }
        println!("[Nikaia Kernel] No main function found.");
    }

    fn eval_block(&self, block: &crate::ast::Block) {
        for stmt in &block.stmts {
            self.eval_stmt(stmt);
        }
    }

    fn eval_stmt(&self, stmt: &Stmt) {
        match stmt {
            Stmt::Expr(expr) => {
                self.eval_expr(expr);
            }
            Stmt::Let { name, value, .. } => {
                // In a real implementation, we would store the result in a scope map.
                // For now, we just print the binding.
                println!("[Nikaia Runtime] Bind: {} = <evaluated>", name);
                self.eval_expr(value);
            }
            _ => println!("[Nikaia Runtime] Skipping statement {:?}", stmt),
        }
    }

    fn eval_expr(&self, expr: &Expr) {
        match expr {
            Expr::Call { func, args } => {
                // Simplified function resolution
                if let Expr::Variable(name) = &**func {
                    let name_str = name.to_string();
                    if name_str == "println" {
                        self.builtin_println(args);
                        return;
                    }
                    if name_str == "log" {
                        self.builtin_log(args);
                        return;
                    }
                }
                println!("[Nikaia Runtime] Call to unknown function");
            }
            Expr::Spawn { body, .. } => {
                println!("[Nikaia Runtime] Spawning Task (Async -> Sync Simulation)...");
                // In Stage 1, this will use Tokio. For now, we execute inline.
                // The body is usually an Expr::Block because of `spawn({ ... })` syntax.
                if let Expr::Block(block) = &**body {
                    self.eval_block(block);
                } else {
                    // Fallback for single expression spawn(expr)
                    self.eval_expr(body);
                }
            }
            Expr::LitStr(_) => {
                // Literals evaluate to themselves.
            }
            Expr::Block(b) => self.eval_block(b),
            Expr::Dsl { target, .. } => {
                println!("[Nikaia Runtime] DSL Block '{}' (Skipped)", target);
            }
            _ => println!("[Nikaia Runtime] Eval: {:?}", expr),
        }
    }

    fn builtin_println(&self, args: &[Expr]) {
        for arg in args {
            if let Expr::LitStr(s) = arg {
                println!("{}", s);
            } else {
                println!("<expression>");
            }
        }
    }

    fn builtin_log(&self, args: &[Expr]) {
        for arg in args {
            if let Expr::LitStr(s) = arg {
                println!("[LOG] {}", s);
            }
        }
    }
}
