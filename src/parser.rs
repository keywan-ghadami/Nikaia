use crate::ast::*;
use syn_grammar::grammar;

grammar! {
    pub grammar CompilerGrammar {
        rule program -> Program = items:item* -> { 
            Program { items } 
        }

        rule item -> Item = 
            | "fn" name:ident "(" args:fn_args? ")" body:block -> {
                Item::Fn {
                    name,
                    generics: vec![],
                    args: args.unwrap_or_default(),
                    ret_type: None,
                    body,
                    is_sync: false,
                }
            }

        rule fn_args -> Vec<FnArg> =
            | first:fn_arg rest:("," arg:fn_arg -> { arg })* -> {
                let mut args = vec![first];
                args.extend(rest);
                args
            }

        rule fn_arg -> FnArg =
            | name:ident ":" ty:type_ref -> { FnArg { name, ty } }

        rule type_ref -> Type =
            | name:ident -> { Type { name, generics: vec![] } }

        rule block -> Block = 
            | "{" stmts:stmt* "}" -> { Block { stmts } }

        rule stmt -> Stmt = 
            | e:expr ";"? -> { Stmt::Expr(e) }

        rule expr -> Expr = 
            | spawn_expr
            | call_expr
            | block_expr
            | lit_str

        rule call_expr -> Expr = 
            | func:ident "(" args:call_args? ")" -> {
                Expr::Call {
                    func: Box::new(Expr::Variable(func)),
                    args: args.unwrap_or_default(),
                }
            }

        rule call_args -> Vec<Expr> =
            | first:expr rest:("," e:expr -> { e })* -> {
                let mut args = vec![first];
                args.extend(rest);
                args
            }

        rule spawn_expr -> Expr =
            | "spawn" "(" body:expr ")" -> {
                Expr::Spawn { body: Box::new(body), is_move: false }
            }

        rule block_expr -> Expr =
            | b:block -> { Expr::Block(b) }

        rule lit_str -> Expr = 
            | s:string_lit -> { Expr::LitStr(s.value()) }
    }
}
