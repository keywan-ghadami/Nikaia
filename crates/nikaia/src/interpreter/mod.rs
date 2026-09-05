// crates/nikaia/src/interpreter/mod.rs
use crate::ast::{Block, Expr, Item, Stmt};
use crate::parser::Parsed;
use winnow_grammar::{InternerContext, Symbol};

pub struct Interpreter {
    /// Identifiers in the AST are interned handles; the interner turns them
    /// back into text.
    interner: InternerContext,
}

impl Interpreter {
    pub fn new(interner: InternerContext) -> Self {
        Self { interner }
    }

    fn text(&self, sym: &Symbol) -> &str {
        self.interner.resolve(*sym)
    }

    pub fn run(&self, parsed: &Parsed) {
        let program = &parsed.program;
        println!("[Nikaia Kernel] Interpreter Init...");
        // Entry point lookup: find 'main' function
        for item in &program.items {
            if let Item::Fn { name, body, .. } = item {
                if self.text(name) == "main" {
                    println!("[Nikaia Kernel] Executing 'main'...");
                    self.eval_block(body);
                    return;
                }
            }
        }
        println!("[Nikaia Kernel] No main function found.");
    }

    fn eval_block(&self, block: &Block) {
        for stmt in &block.stmts {
            self.eval_stmt(stmt);
        }
    }

    fn eval_stmt(&self, stmt: &Stmt) {
        match stmt {
            Stmt::Let { name, value, .. } => {
                // In a real implementation, we would store the result in a scope map.
                // For now, we just print the binding.
                println!("[Nikaia Runtime] Bind: {} = <evaluated>", self.text(name));
                self.eval_expr(value);
            }
            Stmt::Expr(expr) => {
                self.eval_expr(expr);
            }
            Stmt::Assign { .. } => {
                println!("[Nikaia Runtime] Assignment (Skipped)");
            }
        }
    }

    fn eval_expr(&self, expr: &Expr) {
        match expr {
            Expr::Call { func, args } => {
                // Simplified function resolution
                if let Expr::Variable(name) = &**func {
                    let name_str = self.text(name);
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
                println!(
                    "[Nikaia Runtime] DSL Block '{}' (Skipped)",
                    self.text(target)
                );
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
