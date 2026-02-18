use serde::{Deserialize, Serialize};
use std::ops::Range;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeModule {
    pub name: String,
    pub items: Vec<BridgeItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BridgeItem {
    Function(BridgeFunction),
    Struct(BridgeStruct),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeFunction {
    pub name: String,
    pub args: Vec<BridgeArg>,
    pub ret_type: Option<String>,
    pub body: BridgeBlock,
    pub span: Range<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeStruct {
    pub name: String,
    pub fields: Vec<BridgeField>,
    pub span: Range<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeArg {
    pub name: String,
    pub ty: String,
    pub span: Range<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeField {
    pub name: String,
    pub ty: String,
    pub span: Range<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeBlock {
    pub stmts: Vec<BridgeStmt>,
    pub span: Range<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BridgeStmt {
    Let(BridgeLetStmt),
    Expr(BridgeExpr),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeLetStmt {
    pub name: String,
    pub ty: Option<String>,
    pub init: Option<BridgeExpr>,
    pub span: Range<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BridgeExpr {
    Literal(BridgeLiteral),
    Variable(String),
    Call(BridgeCall),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeCall {
    pub func: Box<BridgeExpr>,
    pub args: Vec<BridgeExpr>,
    pub span: Range<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BridgeLiteral {
    Int(i64),
    String(String),
    Bool(bool),
}
