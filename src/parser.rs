use winnow_grammar::grammar;

grammar! {
    grammar CompilerGrammar {
        use crate::ast::*;
        use syn::Ident;
        use proc_macro2::Span;

        // --- Entry Point ---
        pub rule program -> Program =
            items:item* -> {
                Program { items }
            }

        // --- Top-Level Items ---
        rule item -> Item =
            i:fn_item -> { i }

        rule kw_sync -> () = "sync" -> { () }

        rule fn_item -> Item =
            "fn"
            name:ident
            generics:generic_list?
            args:fn_arg_list
            is_sync:kw_sync?
            ret:return_type_arrow?
            body:block
            -> {
                Item::Fn {
                    name: Ident::new(&name, Span::call_site()),
                    generics: generics.unwrap_or_default(),
                    args,
                    ret_type: ret,
                    body,
                    is_sync: is_sync.is_some()
                }
            }

        // --- Argumente & Typen ---

        rule fn_arg_list -> Vec<FnArg> =
            paren(args:fn_arg_defs?) -> { args.unwrap_or_default() }

        rule fn_arg_defs -> Vec<FnArg> =
            head:fn_arg_def tail:fn_arg_def_tail* -> {
                let mut args = vec![head];
                args.extend(tail);
                args
            }

        rule fn_arg_def_tail -> FnArg =
            "," arg:fn_arg_def -> { arg }

        rule fn_arg_def -> FnArg =
            name:ident ":" ty:type_ref -> {
                FnArg { name: Ident::new(&name, Span::call_site()), ty }
            }

        rule return_type_arrow -> Type =
            "->" ty:type_ref -> { ty }

        rule generic_list -> Vec<GenericParam> =
            [ params:generic_params? ] -> { params.unwrap_or_default() }

        rule generic_params -> Vec<GenericParam> =
            head:generic_param tail:generic_param_tail* -> {
                let mut params = vec![head];
                params.extend(tail);
                params
            }

        rule generic_param_tail -> GenericParam =
            "," p:generic_param -> { p }

        rule generic_param -> GenericParam =
            name:ident
            -> { GenericParam { name: Ident::new(&name, Span::call_site()) } }

        rule type_ref -> Type =
            name:ident
            generics:generic_type_args?
            -> {
                Type { name: Ident::new(&name, Span::call_site()), generics: generics.unwrap_or_default() }
            }

        rule generic_type_args -> Vec<Type> =
            [ args:type_refs? ] -> { args.unwrap_or_default() }

        rule type_refs -> Vec<Type> =
            head:type_ref tail:type_ref_tail* -> {
                let mut args = vec![head];
                args.extend(tail);
                args
            }

        rule type_ref_tail -> Type =
            "," t:type_ref -> { t }

        // --- Statements & Blocks ---

        rule block -> Block =
            { stmts:stmt* } -> { Block { stmts } }

        rule stmt -> Stmt =
            l:let_stmt -> { l }
          | e:expr_stmt -> { e }

        rule kw_mut -> () = "mut" -> { () }

        rule let_stmt -> Stmt =
            "let"
            mutable:kw_mut?
            name:ident
            ty:type_annotation?
            "="
            val:expr
            ";"?
            -> {
                Stmt::Let {
                    name: Ident::new(&name, Span::call_site()),
                    mutable: mutable.is_some(),
                    ty,
                    value: val
                }
            }

        rule type_annotation -> Type =
            ":" ty:type_ref -> { ty }

        rule expr_stmt -> Stmt =
            e:expr ";"? -> { Stmt::Expr(e) }

        // --- Expressions ---

        rule expr -> Expr =
            c:call_expr -> { c }
          | s:str_lit -> { s }

        rule call_expr -> Expr =
            func:ident paren(args:call_args?) -> {
                Expr::Call {
                    func: Box::new(Expr::Variable(Ident::new(&func, Span::call_site()))),
                    args: args.unwrap_or_default(),
                }
            }

        rule call_args -> Vec<Expr> =
            head:expr tail:call_args_tail* -> {
                let mut args = vec![head];
                args.extend(tail);
                args
            }

        rule call_args_tail -> Expr =
            "," e:expr -> { e }

        rule str_lit -> Expr =
            s:string -> {
                Expr::LitStr(s)
            }
    }
}
