#[derive(Debug, Clone)]
pub enum Stmt {
    Let {
        name: String,
        value: Expr,
    },
    Expr(Expr),
    // DSL-Support (z.B. dsl js { ... })
    Dsl {
        target: String,
        content: String,
    }
}

#[derive(Debug, Clone)]
pub enum Expr {
    LiteralInt(i64),
    LiteralStr(String),
    Variable(String),
    
    // Block: { ... }
    Block(Vec<Stmt>),
    
    // Spawn-Aufruf: spawn({ ... })
    Spawn(Box<Expr>),
    
    // Call: println("...")
    Call {
        func: String,
        args: Vec<Expr>,
    }
}

