grammar CompilerGrammar {
    rule program -> Program = items:item* -> { 
        Program { items } 
    }

    rule item -> Item = 
        "fn" name:ident paren(args:fn_args?) body:block -> {
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
        first:fn_arg rest:fn_arg_rest* -> {
            let mut args = vec![first];
            args.extend(rest);
            args
        }

    rule fn_arg_rest -> FnArg =
        "," arg:fn_arg -> { arg }

    rule fn_arg -> FnArg =
        name:ident ":" ty:type_ref -> { FnArg { name, ty } }

    rule type_ref -> Type =
        name:ident -> { Type { name, generics: vec![] } }

    rule block -> Block = 
        brace(stmts:stmts) -> { Block { stmts } }

    rule stmts -> Vec<Stmt> = 
        s:stmt* -> { s }

    rule stmt -> Stmt = 
        e:expr ";"? -> { Stmt::Expr(e) }

    rule expr -> Expr = 
        e:spawn_expr -> { e }
        | e:call_expr -> { e }
        | e:block_expr -> { e }
        | e:lit_str -> { e }

    rule call_expr -> Expr = 
        func:ident paren(args:call_args?) -> {
            Expr::Call {
                func: Box::new(Expr::Variable(func)),
                args: args.unwrap_or_default(),
            }
        }

    rule call_args -> Vec<Expr> =
        first:expr rest:call_arg_rest* -> {
            let mut args = vec![first];
            args.extend(rest);
            args
        }

    rule call_arg_rest -> Expr =
        "," e:expr -> { e }

    rule spawn_expr -> Expr =
        "spawn" paren(body:expr) -> {
            Expr::Spawn { body: Box::new(body), is_move: false }
        }

    rule block_expr -> Expr =
        b:block -> { Expr::Block(b) }

    rule lit_str -> Expr = 
        s:string_lit -> { Expr::LitStr(s.value()) }
}
