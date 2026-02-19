// crates/rustc-executor/src/ast.rs

#[derive(Debug)]
pub struct Crate {
    pub items: Vec<Item>,
}

#[derive(Debug)]
pub struct Item {
    pub name: String,
    pub kind: ItemKind,
}

#[derive(Debug)]
pub enum ItemKind {
    Fn(Function),
}

#[derive(Debug)]
pub struct Function {
    pub body: Block,
}

#[derive(Debug)]
pub struct Block {
    pub stmts: Vec<Stmt>,
}

#[derive(Debug)]
pub enum Stmt {
    Local(Local),
    Expr(Expr),
    Semi(Expr),
}

#[derive(Debug)]
pub struct Local {
    pub pat: Pat,
    pub init: Option<Expr>,
}

#[derive(Debug)]
pub enum Pat {
    Ident(String),
}

#[derive(Debug)]
pub enum Expr {
    Lit(Lit),
    Call(Call),
    Path(String),
}

#[derive(Debug)]
pub enum Lit {
    Int(i64),
    Str(String),
    Bool(bool),
}

#[derive(Debug)]
pub struct Call {
    pub func: Box<Expr>,
    pub args: Vec<Expr>,
}

pub fn print_crate(krate: &Crate) -> String {
    let mut out = String::new();
    for item in &krate.items {
        out.push_str(&print_item(item));
        out.push('\n');
    }
    out
}

fn print_item(item: &Item) -> String {
    match &item.kind {
        ItemKind::Fn(f) => {
            format!("fn {}() {}", item.name, print_block(&f.body))
        }
    }
}

fn print_block(block: &Block) -> String {
    let mut out = String::from("{\n");
    for stmt in &block.stmts {
        out.push_str("    ");
        out.push_str(&print_stmt(stmt));
        out.push('\n');
    }
    out.push_str("}\n");
    out
}

fn print_stmt(stmt: &Stmt) -> String {
    match stmt {
        Stmt::Local(local) => {
            let mut out = String::from("let ");
            out.push_str(&print_pat(&local.pat));
            if let Some(init) = &local.init {
                out.push_str(" = ");
                out.push_str(&print_expr(init));
            }
            out.push(';');
            out
        }
        Stmt::Expr(expr) => print_expr(expr),
        Stmt::Semi(expr) => {
            let mut out = print_expr(expr);
            out.push(';');
            out
        }
    }
}

fn print_pat(pat: &Pat) -> String {
    match pat {
        Pat::Ident(s) => s.clone(),
    }
}

fn print_expr(expr: &Expr) -> String {
    match expr {
        Expr::Lit(lit) => print_lit(lit),
        Expr::Path(p) => p.clone(),
        Expr::Call(call) => {
            // Unwrapping the function name from the expression
            // This is a simplification for the mock implementation
            let func_name = match &*call.func {
                Expr::Path(p) => p.as_str(),
                _ => "unknown",
            };

            let mut out = String::new();
            if func_name == "println" {
                out.push_str("println!");
            } else {
                out.push_str(func_name);
            }

            out.push('(');
            for (i, arg) in call.args.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&print_expr(arg));
            }
            out.push(')');
            out
        }
    }
}

fn print_lit(lit: &Lit) -> String {
    match lit {
        Lit::Int(i) => i.to_string(),
        Lit::Str(s) => format!("\"{}\"", s),
        Lit::Bool(b) => b.to_string(),
    }
}
