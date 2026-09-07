// crates/nikaia/src/parser/mod.rs
use crate::ast;
use anyhow::Result;
use bridge_ir::{
    BridgeBlock, BridgeCall, BridgeExpr, BridgeFunction, BridgeItem, BridgeLetStmt, BridgeLiteral,
    BridgeModule, BridgeStmt,
};
use winnow::stream::LocatingSlice;
use winnow::Parser;
use winnow_grammar::{grammar, InternerContext, ParseContext, ParseInput, Symbol};

// --- Public API ---

/// A parsed program together with the interner that produced its identifiers.
///
/// Identifiers in the AST are `Symbol` handles, which are only meaningful with
/// the `InternerContext` they were interned into - so the two travel together.
///
/// Note the cost of interning: the derived `Debug` prints identifiers as
/// `Symbol(3)` rather than their text. Use [`Parsed::text`] when a name needs to
/// be read.
#[derive(Debug)]
pub struct Parsed {
    pub program: ast::Program,
    pub interner: InternerContext,
}

impl Parsed {
    /// Resolve an identifier back to its text.
    pub fn text(&self, sym: Symbol) -> &str {
        self.interner.resolve(sym)
    }
}

pub fn parse_to_bridge(input: &str) -> Result<BridgeModule> {
    lower_program(&parse_to_ast(input)?)
}

/// Parse one expression, interning into an existing table.
///
/// The holes of an interpolated string (`"{a}={s.mean()}"`) are Nikaia
/// expressions inside a literal: they are not seen by the grammar that read the
/// literal, so they are parsed here, with the program's own interner so that
/// the symbols they produce mean the same as everywhere else.
pub fn parse_expression(interner: &InternerContext, input: &str) -> Result<ast::Expr> {
    let context = ParseContext::<()> {
        interner: interner.clone(),
        ..Default::default()
    };

    let mut stream = ParseInput::<()> {
        state: context,
        input: LocatingSlice::new(input),
    };

    let expr = CompilerGrammar::parse_expr()
        .parse_next(&mut stream)
        .map_err(|e| anyhow::anyhow!("{}", e.render(input)))?;

    if !stream.input.is_empty() {
        return Err(anyhow::anyhow!(
            "trailing input after expression: `{}`",
            &input[input.len() - stream.input.len()..]
        ));
    }

    Ok(expr)
}

pub fn parse_to_ast(input: &str) -> Result<Parsed> {
    // Generated parsers run on a `Stateful` stream: `LocatingSlice` supplies the
    // spans, `ParseContext` carries the shared parser state including the
    // interner. Cloning the interner out shares it (it is an `Arc` inside), so
    // the handles in the AST stay resolvable after parsing.
    let context = ParseContext::<()>::default();
    let interner = context.interner.clone();

    let mut stream = ParseInput::<()> {
        state: context,
        input: LocatingSlice::new(input),
    };

    // `parse_<rule>()` is a factory: calling it builds the parser, which we then
    // drive with `.parse_next()`.
    let program = CompilerGrammar::parse_program()
        .parse_next(&mut stream)
        .map_err(|e| anyhow::anyhow!("Parse error:\n{}", e.render(input)))?;

    // The generated entry point already refuses leftover input; this is a
    // backstop so a partial parse can never be reported as a success.
    if !stream.input.is_empty() {
        return Err(anyhow::anyhow!(
            "Parse error: unexpected trailing input at byte {}",
            input.len() - stream.input.len()
        ));
    }

    Ok(Parsed { program, interner })
}

// --- Action-block helpers ---
//
// The generated parser is a module with `use super::*`, so these are in scope
// inside the action blocks below. They exist because a PEG has no precedence
// table: the expression rules parse a head and a list of tails, and the shape
// is rebuilt here.

/// What may follow a primary expression: `.field`, `.method(..)`, `?`.
#[derive(Debug, Clone)]
pub enum Postfix {
    Field(Symbol),
    Method(Symbol, Vec<ast::Expr>),
    Index(Box<ast::Expr>),
    Try,
}

/// Left-associative: `a - b - c` is `(a - b) - c`.
pub fn fold_binary(head: ast::Expr, tail: Vec<(ast::BinaryOp, ast::Expr)>) -> ast::Expr {
    tail.into_iter()
        .fold(head, |lhs, (op, rhs)| ast::Expr::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        })
}

pub fn fold_postfix(base: ast::Expr, tail: Vec<Postfix>) -> ast::Expr {
    tail.into_iter().fold(base, |recv, step| match step {
        Postfix::Field(name) => ast::Expr::Field {
            base: Box::new(recv),
            name,
        },
        Postfix::Method(method, args) => ast::Expr::MethodCall {
            receiver: Box::new(recv),
            method,
            args,
        },
        Postfix::Index(index) => ast::Expr::Index {
            base: Box::new(recv),
            index,
        },
        Postfix::Try => ast::Expr::Try(Box::new(recv)),
    })
}

// --- Grammar Definition ---

grammar! {
    grammar CompilerGrammar {
        use crate::ast::*;
        use winnow::ascii::{digit1, multispace1};

        // --- Entry Point ---
        // Rule 'program' -> generates 'parse_program'
        pub rule program -> Program =
            _start:skip_ws
            items:item*
            _end:skip_ws
            -> {
                Program { items }
            }

        // Comments are whitespace, and `WS` is the only place that can be said:
        // the generator inserts it between the tokens of every syntactic rule.
        // All three are UPPERCASE on purpose - a lowercase `comment` would be
        // syntactic, so the generator would insert `WS` between *its* tokens,
        // and `WS` calls it. That cycle recurses until the stack is gone.
        rule WSE = multispace1
        rule WS = (WSE | COMMENT)*
        rule COMMENT = "//" until(line_ending)

        // String literals keep their escapes: the boundary of a frame is
        // written `"\n"`, and what the emitter hands to the parser backend is
        // that same text. The built-in `string` recognizes only `\\` and `\"`,
        // which is one escape short of a newline.
        //
        // UPPERCASE: a lexical rule, or the implicit whitespace would eat the
        // spaces inside the literal.
        rule STRING -> String =
            "\"" parts:STR_CHAR* "\"" -> { parts.concat() }

        rule STR_CHAR -> String =
            "\\" c:any -> {
                let mut s = String::from("\\");
                s.push(c);
                s
            }
          | not("\"") c:any -> { c.to_string() }

        // The explicit form, for the leading and trailing positions where there
        // is no preceding token for the implicit `WS` to follow.
        rule skip_ws -> () = _w:WS -> { () }

        // --- Top-Level Items ---
        //
        // `grammar` and `struct` come before `fn`: all three are keyword-led,
        // and the order is what keeps `grammar` from being read as an
        // identifier.
        // `@=` puts the byte range this rule matched into `_span`, which is
        // how every node below gets the place in the `.nika` file it came from.
        // Without it a diagnostic can only name generated Rust.
        rule item -> Spanned<Item> @=
            g:grammar_item -> { Spanned::new(g, _span) }
          | s:struct_item -> { Spanned::new(s, _span) }
          | im:impl_item -> { Spanned::new(im, _span) }
          | u:use_item -> { Spanned::new(u, _span) }
          | i:fn_item -> { Spanned::new(i, _span) }

        // Kap 4.2: behaviour lives in an `impl`, never in the struct.
        rule impl_item -> Item =
            "impl" _sp:skip_ws target:type_ref _sp2:skip_ws
            "{" _sp3:skip_ws methods:impl_method* _sp4:skip_ws "}"
            -> { Item::Impl { target, methods } }

        rule impl_method -> Spanned<Item> @=
            _sp:skip_ws f:fn_item -> { Spanned::new(f, _span) }

        rule kw_sync -> () = "sync" -> { () }
        rule kw_pub -> () = "pub" -> { () }

        // `sync` and `throws` are accepted on either side of the return type:
        // Part II writes `fn add(…) sync`, Part I `fn f(…) -> String throws`.
        rule fn_item -> Item =
            vis:kw_pub?
            _sp0:skip_ws
            "fn"
            _sp:skip_ws
            name:ident?
            _sp2:skip_ws
            generics:generic_list?
            params:fn_params
            _sp3:skip_ws
            sync_before:kw_sync?
            _sp4:skip_ws
            ret:return_type_arrow?
            _sp5:skip_ws
            sync_after:kw_sync?
            _sp6:skip_ws
            throws:kw_throws?
            _sp7:skip_ws
            body:block
            -> {
                Item::Fn {
                    name,
                    generics: generics.unwrap_or_default(),
                    receiver: params.receiver,
                    args: params.args,
                    ret_type: ret,
                    body,
                    is_sync: sync_before.is_some() || sync_after.is_some(),
                    is_public: vis.is_some(),
                    throws: throws.is_some(),
                }
            }

        rule kw_throws -> () = "throws" -> { () }

        // Kap 4.2: `&mut self`, `&self`, `self` - the subject, when there is one.
        rule fn_params -> FnParams =
            "(" _sp:skip_ws body:fn_params_body? _sp2:skip_ws ")" -> {
                body.unwrap_or_default()
            }

        rule fn_params_body -> FnParams =
            r:receiver args:fn_arg_def_tail* -> {
                FnParams { receiver: Some(r), args }
            }
          | head:fn_arg_def tail:fn_arg_def_tail* -> {
                let mut args = vec![head];
                args.extend(tail);
                FnParams { receiver: None, args }
            }

        rule receiver -> Receiver =
            "&" _sp:skip_ws "mut" _sp2:skip_ws "self" -> {
                Receiver { is_ref: true, is_mut: true }
            }
          | "&" _sp:skip_ws "self" -> { Receiver { is_ref: true, is_mut: false } }
          | "self" -> { Receiver { is_ref: false, is_mut: false } }

        // Kap 9.2: use std::fs
        rule use_item -> Item =
            "use" _sp:skip_ws head:ident tail:path_segment* -> {
                let mut path = vec![head];
                path.extend(tail);
                Item::Import { path }
            }

        rule path_segment -> Symbol =
            _sp:skip_ws "::" _sp2:skip_ws n:ident -> { n }

        // --- Structs ---
        //
        // ADR-008 D6: `@borrowed` is an assertion about escape, so it is part
        // of the item, not a comment.
        rule struct_item -> Item =
            borrowed:at_borrowed?
            _sp:skip_ws
            vis:kw_pub?
            _sp2:skip_ws
            "struct"
            _sp3:skip_ws
            name:ident
            _sp4:skip_ws
            generics:generic_list?
            _sp5:skip_ws
            "{"
            _sp6:skip_ws
            fields:field_defs?
            _sp7:skip_ws
            "}"
            -> {
                Item::Struct {
                    name,
                    generics: generics.unwrap_or_default(),
                    fields: fields.unwrap_or_default(),
                    is_public: vis.is_some(),
                    is_borrowed: borrowed.is_some(),
                }
            }

        rule at_borrowed -> () = "@borrowed" -> { () }

        rule field_defs -> Vec<FieldDef> =
            head:field_def tail:field_def_tail* _sp:skip_ws ","? -> {
                let mut fields = vec![head];
                fields.extend(tail);
                fields
            }

        rule field_def_tail -> FieldDef =
            _sp:skip_ws "," _sp2:skip_ws f:field_def -> { f }

        rule field_def -> FieldDef =
            name:ident _sp:skip_ws ":" _sp2:skip_ws ty:type_ref -> {
                FieldDef { name, ty }
            }

        // --- Argumente & Typen ---

        rule fn_arg_list -> Vec<FnArg> =
            "(" _sp:skip_ws args:fn_arg_defs? _sp2:skip_ws ")" -> { args.unwrap_or_default() }

        rule fn_arg_defs -> Vec<FnArg> =
            head:fn_arg_def tail:fn_arg_def_tail* -> {
                let mut args = vec![head];
                args.extend(tail);
                args
            }

        rule fn_arg_def_tail -> FnArg =
            _sp:skip_ws "," _sp2:skip_ws arg:fn_arg_def -> { arg }

        rule fn_arg_def -> FnArg =
            name:ident _sp:skip_ws ":" _sp2:skip_ws ty:type_ref -> {
                FnArg { name, ty }
            }

        rule return_type_arrow -> Type =
            "->" _sp:skip_ws ty:type_ref -> { ty }

        // USING [ ] SYNTAX directly for testing
        rule generic_list -> Vec<GenericParam> =
            [ _sp:skip_ws params:generic_params? _sp2:skip_ws ] -> { params.unwrap_or_default() }

        rule generic_params -> Vec<GenericParam> =
            head:generic_param tail:generic_param_tail* -> {
                let mut params = vec![head];
                params.extend(tail);
                params
            }

        rule generic_param_tail -> GenericParam =
            _sp:skip_ws "," _sp2:skip_ws p:generic_param -> { p }

        rule generic_param -> GenericParam =
            name:ident
            -> { GenericParam { name } }

        // `&str` is a view marker (Part II, 10.6), not a lifetime - the `&` is
        // recorded and the emitter decides what it becomes.
        rule type_ref -> Type =
            view:amp?
            _sp:skip_ws
            name:ident
            generics:generic_type_args?
            -> {
                Type { name, generics: generics.unwrap_or_default(), is_view: view.is_some() }
            }

        rule amp -> () = "&" -> { () }

        // USING [ ] SYNTAX directly for testing
        rule generic_type_args -> Vec<Type> =
            [ _sp:skip_ws args:type_refs? _sp2:skip_ws ] -> { args.unwrap_or_default() }

        rule type_refs -> Vec<Type> =
            head:type_ref tail:type_ref_tail* -> {
                let mut args = vec![head];
                args.extend(tail);
                args
            }

        rule type_ref_tail -> Type =
            _sp:skip_ws "," _sp2:skip_ws t:type_ref -> { t }

        // --- Part II, Kapitel 10: Grammatiken ---

        rule grammar_item -> Item =
            "grammar" _sp:skip_ws name:ident _sp2:skip_ws
            "{" _sp3:skip_ws rules:grammar_rule* _sp4:skip_ws "}"
            -> { Item::Grammar(GrammarDef { name, rules }) }

        rule grammar_rule -> GrammarRule @=
            _sp:skip_ws
            frame:frame_attr?
            _sp2:skip_ws
            vis:kw_pub?
            _sp3:skip_ws
            "rule"
            _sp4:skip_ws
            name:ident
            _sp5:skip_ws
            ret:return_type_arrow?
            _sp6:skip_ws
            "="
            _sp7:skip_ws
            alts:g_alts
            -> {
                GrammarRule {
                    name,
                    is_public: vis.is_some(),
                    frame,
                    ret_type: ret,
                    alts,
                    span: _span,
                }
            }

        // ADR-009 D1: the attribute is keyed. `@frame`, `@frame(boundary: "\n")`,
        // `@frame(boundary: "\n", unchecked)`; the positional form is withdrawn,
        // so that every future cut-point key (quote, start, escape, scan) has
        // room without changing what the existing ones mean.
        rule frame_attr -> FrameAttr =
            "@frame" _sp:skip_ws args:frame_args? -> {
                args.unwrap_or_default()
            }

        rule frame_args -> FrameAttr =
            "(" _sp:skip_ws head:frame_arg tail:frame_arg_tail* _sp2:skip_ws ")" -> {
                let mut attr = head;
                for a in tail {
                    if a.boundary.is_some() { attr.boundary = a.boundary; }
                    attr.unchecked = attr.unchecked || a.unchecked;
                }
                attr
            }

        rule frame_arg_tail -> FrameAttr =
            _sp:skip_ws "," _sp2:skip_ws a:frame_arg -> { a }

        rule frame_arg -> FrameAttr =
            "boundary" _sp:skip_ws ":" _sp2:skip_ws b:STRING -> {
                FrameAttr { boundary: Some(b), unchecked: false }
            }
          | "unchecked" -> {
                FrameAttr { boundary: None, unchecked: true }
            }

        rule g_alts -> Vec<GrammarAlt> =
            head:g_alt tail:g_alt_tail* -> {
                let mut alts = vec![head];
                alts.extend(tail);
                alts
            }

        rule g_alt_tail -> GrammarAlt =
            _sp:skip_ws "|" _sp2:skip_ws a:g_alt -> { a }

        // An action block is required today (Part II, 10.1, note) - with one
        // exception that is not an omission: a `par_fold` must be the whole
        // body of its rule (ADR-009 D2), so there is nothing for an action to
        // add. The emitter supplies the binding such a rule needs.
        rule g_alt -> GrammarAlt =
            p:g_seq _sp:skip_ws "->" _sp2:skip_ws action:block -> {
                GrammarAlt { pattern: p, action: Some(action) }
            }
          | f:g_fold -> {
                GrammarAlt { pattern: f, action: None }
            }

        rule g_seq -> Spanned<Pattern> @=
            head:g_elem tail:g_elem_tail* -> {
                if tail.is_empty() {
                    // One element is its own span, not the sequence's.
                    head
                } else {
                    let mut parts = vec![head];
                    parts.extend(tail);
                    Spanned::new(Pattern::Seq(parts), _span)
                }
            }

        rule g_elem_tail -> Spanned<Pattern> =
            _sp:skip_ws e:g_elem -> { e }

        rule g_elem -> Spanned<Pattern> =
            c:g_cut -> { c }
          | b:g_bind -> { b }
          | p:g_postfix -> { p }

        // Part II, 10.1: the commit point. Once passed, a later failure is an
        // error rather than a reason to try the next alternative.
        rule g_cut -> Spanned<Pattern> @= "=>" -> { Spanned::new(Pattern::Cut, _span) }

        rule g_bind -> Spanned<Pattern> @=
            name:ident ":" p:g_postfix -> {
                Spanned::new(Pattern::Bind { name, pat: Box::new(p) }, _span)
            }

        rule g_postfix -> Spanned<Pattern> @=
            a:g_atom rep:g_repeat? -> {
                match rep {
                    Some(r) => Spanned::new(Pattern::Repeat { pat: Box::new(a), rep: r }, _span),
                    None => a,
                }
            }

        // ADR-009 D5: a bounded repetition is how a format states a fixed
        // width. `*` and `+` say "unbounded" and mean it.
        rule g_repeat -> Repeat =
            "*" -> { Repeat::Star }
          | "+" -> { Repeat::Plus }
          | "?" -> { Repeat::Optional }
          | r:g_bounds -> { r }

        rule g_bounds -> Repeat =
            "{" _sp:skip_ws n:number _sp2:skip_ws "," _sp3:skip_ws m:number _sp4:skip_ws "}" -> {
                Repeat::Between(n, m)
            }
          | "{" _sp:skip_ws n:number _sp2:skip_ws "," _sp3:skip_ws "}" -> {
                Repeat::AtLeast(n)
            }
          | "{" _sp:skip_ws n:number _sp2:skip_ws "}" -> {
                Repeat::Exactly(n)
            }

        rule number -> u32 =
            d:digit1 -> { d.parse().unwrap_or(0) }

        rule g_atom -> Spanned<Pattern> @=
            f:g_fold -> { f }
          | s:STRING -> { Spanned::new(Pattern::Literal(s), _span) }
          | g:g_group -> { g }
          | r:g_ref -> { r }

        rule g_group -> Spanned<Pattern> @=
            "(" _sp:skip_ws p:g_choice _sp2:skip_ws ")" -> {
                Spanned::new(Pattern::Group(Box::new(p)), _span)
            }

        // A rule reference, a built-in (`digit`, `frame_end`), or a call to
        // either (`until(";" | frame_end)`, `list(pair, ",")`). The grammar
        // cannot tell them apart, and does not need to: what a name means is
        // the backend's question.
        rule g_ref -> Spanned<Pattern> @=
            name:ident args:g_args? -> {
                Spanned::new(Pattern::Ref { name, args: args.unwrap_or_default() }, _span)
            }

        rule g_args -> Vec<Spanned<Pattern>> =
            "(" _sp:skip_ws head:g_choice tail:g_arg_tail* _sp2:skip_ws ")" -> {
                let mut args = vec![head];
                args.extend(tail);
                args
            }

        rule g_arg_tail -> Spanned<Pattern> =
            _sp:skip_ws "," _sp2:skip_ws p:g_choice -> { p }

        rule g_choice -> Spanned<Pattern> @=
            head:g_seq tail:g_choice_tail* -> {
                if tail.is_empty() {
                    head
                } else {
                    let mut parts = vec![head];
                    parts.extend(tail);
                    Spanned::new(Pattern::Choice(parts), _span)
                }
            }

        rule g_choice_tail -> Spanned<Pattern> =
            _sp:skip_ws "|" _sp2:skip_ws p:g_seq -> { p }

        // ADR-009 D2: parallel parsing is a frame plus a monoid. `fold` is the
        // accumulator; the merge is what makes it parallelisable, and asking
        // for it is how the user says a different chunk count is the same
        // answer to them.
        rule g_fold -> Spanned<Pattern> @=
            "par_fold" _sp:skip_ws "(" _sp2:skip_ws
            r:ident _sp3:skip_ws "," _sp4:skip_ws
            init:expr _sp5:skip_ws "," _sp6:skip_ws
            step:expr _sp7:skip_ws "," _sp8:skip_ws
            merge:expr _sp9:skip_ws ")"
            -> {
                Spanned::new(Pattern::Fold(Box::new(FoldSpec {
                    parallel: true,
                    rule: r,
                    init,
                    step,
                    merge: Some(merge),
                })), _span)
            }
          | "fold" _sp:skip_ws "(" _sp2:skip_ws
            r:ident _sp3:skip_ws "," _sp4:skip_ws
            init:expr _sp5:skip_ws "," _sp6:skip_ws
            step:expr _sp7:skip_ws ")"
            -> {
                Spanned::new(Pattern::Fold(Box::new(FoldSpec {
                    parallel: false,
                    rule: r,
                    init,
                    step,
                    merge: None,
                })), _span)
            }

        // --- Statements & Blocks ---

        rule block -> Block =
            "{" _sp:skip_ws stmts:stmt_list _sp2:skip_ws "}" -> { Block { stmts } }

        rule stmt_list -> Vec<Spanned<Stmt>> =
            stmts:stmt* -> { stmts }

        rule stmt -> Spanned<Stmt> @=
            l:let_stmt -> { Spanned::new(l, _span) }
          | r:return_stmt -> { Spanned::new(r, _span) }
          | f:for_stmt -> { Spanned::new(f, _span) }
          | a:assign_stmt -> { Spanned::new(a, _span) }
          | e:expr_stmt -> { Spanned::new(e, _span) }

        rule return_stmt -> Stmt =
            "return" _sp:skip_ws value:expr? _sp2:skip_ws ";"? _sp3:skip_ws -> {
                Stmt::Return(value)
            }

        rule kw_mut -> () = "mut" -> { () }

        rule let_stmt -> Stmt =
            "let"
            _sp:skip_ws
            mutable:kw_mut?
            _sp2:skip_ws
            name:ident
            _sp3:skip_ws
            ty:type_annotation?
            _sp4:skip_ws
            "="
            _sp5:skip_ws
            val:expr
            _sp6:skip_ws
            ";"?
            _sp7:skip_ws
            -> {
                Stmt::Let {
                    name,
                    mutable: mutable.is_some(),
                    ty,
                    value: val
                }
            }

        rule type_annotation -> Type =
            ":" _sp:skip_ws ty:type_ref -> { ty }

        // Kap 3.3. The head is parsed with the brace-free expression grammar:
        // in `for d in whole { ... }` the brace opens the body, never a struct
        // literal - the same restriction Rust puts on this position.
        rule for_stmt -> Stmt =
            "for" _sp:skip_ws bindings:for_bindings _sp2:skip_ws "in" _sp3:skip_ws
            iter:head_expr _sp4:skip_ws body:block _sp5:skip_ws ";"? _sp6:skip_ws
            -> {
                Stmt::For { bindings, iter, body }
            }

        rule for_bindings -> Vec<Symbol> =
            "(" _sp:skip_ws head:ident tail:ident_tail* _sp2:skip_ws ")" -> {
                let mut names = vec![head];
                names.extend(tail);
                names
            }
          | n:ident -> { vec![n] }

        rule ident_tail -> Symbol =
            _sp:skip_ws "," _sp2:skip_ws n:ident -> { n }

        rule assign_stmt -> Stmt =
            target:postfix_expr _sp:skip_ws op:assign_op _sp2:skip_ws value:expr
            _sp3:skip_ws ";"? _sp4:skip_ws
            -> { Stmt::Assign { target, op, value } }

        // The compound forms first: a bare `=` would take the first character
        // of `+=` and leave an expression that cannot parse.
        rule assign_op -> Option<BinaryOp> =
            "+=" -> { Some(BinaryOp::Add) }
          | "-=" -> { Some(BinaryOp::Sub) }
          | "*=" -> { Some(BinaryOp::Mul) }
          | "/=" -> { Some(BinaryOp::Div) }
          | "=" -> { None }

        rule expr_stmt -> Stmt =
            e:expr _sp:skip_ws ";"? _sp2:skip_ws -> { Stmt::Expr(e) }

        // --- Expressions ---

        pub rule expr -> Expr =
            c:closure_expr -> { c }
          | e:catch_expr -> { e }

        // Kap 7.1: `fs::map(path) catch { … }` - the error is `error` inside.
        rule catch_expr -> Expr =
            value:coalesce_expr _sp:skip_ws handler:catch_tail? -> {
                match handler {
                    Some(handler) => Expr::TryCatch { expr: Box::new(value), handler },
                    None => value,
                }
            }

        rule catch_tail -> Block =
            "catch" _sp:skip_ws b:block -> { b }

        // Kap 3.5: `value ?? fallback`.
        rule coalesce_expr -> Expr =
            value:or_expr _sp:skip_ws fallback:coalesce_tail? -> {
                match fallback {
                    Some(fallback) => Expr::Coalesce {
                        value: Box::new(value),
                        fallback: Box::new(fallback),
                    },
                    None => value,
                }
            }

        rule coalesce_tail -> Expr =
            "??" _sp:skip_ws e:or_expr -> { e }

        // Kap 5.2/5.3: a lambda, with its arguments named or implicit.
        rule closure_expr -> Expr =
            "fn" _sp:skip_ws "(" _sp2:skip_ws params:closure_params? _sp3:skip_ws ")"
            _sp4:skip_ws body:block
            -> {
                Expr::Closure {
                    params: params.unwrap_or_default(),
                    implicit: false,
                    body,
                }
            }
          | "fn" _sp:skip_ws body:block -> {
                Expr::Closure { params: Vec::new(), implicit: true, body }
            }

        rule closure_params -> Vec<Symbol> =
            head:ident tail:closure_param_tail* -> {
                let mut params = vec![head];
                params.extend(tail);
                params
            }

        rule closure_param_tail -> Symbol =
            _sp:skip_ws "," _sp2:skip_ws p:ident -> { p }

        rule or_expr -> Expr =
            head:and_expr tail:or_tail* -> { fold_binary(head, tail) }

        rule or_tail -> (BinaryOp, Expr) =
            _sp:skip_ws "||" _sp2:skip_ws e:and_expr -> { (BinaryOp::Or, e) }

        rule and_expr -> Expr =
            head:cmp_expr tail:and_tail* -> { fold_binary(head, tail) }

        rule and_tail -> (BinaryOp, Expr) =
            _sp:skip_ws "&&" _sp2:skip_ws e:cmp_expr -> { (BinaryOp::And, e) }

        rule cmp_expr -> Expr =
            head:add_expr tail:cmp_tail? -> {
                fold_binary(head, tail.into_iter().collect::<Vec<_>>())
            }

        rule cmp_tail -> (BinaryOp, Expr) =
            _sp:skip_ws op:cmp_op _sp2:skip_ws e:add_expr -> { (op, e) }

        // `<=` before `<`: the shorter one would win otherwise and leave `=`
        // to be read as an assignment.
        rule cmp_op -> BinaryOp =
            "==" -> { BinaryOp::Eq }
          | "!=" -> { BinaryOp::Ne }
          | "<=" -> { BinaryOp::Le }
          | ">=" -> { BinaryOp::Ge }
          | "<" -> { BinaryOp::Lt }
          | ">" -> { BinaryOp::Gt }

        rule add_expr -> Expr =
            head:mul_expr tail:add_tail* -> { fold_binary(head, tail) }

        rule add_tail -> (BinaryOp, Expr) =
            _sp:skip_ws op:add_op _sp2:skip_ws e:mul_expr -> { (op, e) }

        rule add_op -> BinaryOp =
            "+" -> { BinaryOp::Add }
          | "-" -> { BinaryOp::Sub }

        rule mul_expr -> Expr =
            head:cast_expr tail:mul_tail* -> { fold_binary(head, tail) }

        rule mul_tail -> (BinaryOp, Expr) =
            _sp:skip_ws op:mul_op _sp2:skip_ws e:cast_expr -> { (op, e) }

        rule cast_expr -> Expr =
            head:unary_expr casts:cast_tail* -> {
                casts.into_iter().fold(head, |expr, ty| Expr::Cast {
                    expr: Box::new(expr),
                    ty,
                })
            }

        rule cast_tail -> Type =
            _sp:skip_ws "as" _sp2:skip_ws ty:type_ref -> { ty }

        rule mul_op -> BinaryOp =
            "*" -> { BinaryOp::Mul }
          | "/" -> { BinaryOp::Div }
          | "%" -> { BinaryOp::Rem }

        rule unary_expr -> Expr =
            op:unary_op _sp:skip_ws e:unary_expr -> {
                Expr::Unary { op, expr: Box::new(e) }
            }
          | e:postfix_expr -> { e }

        rule unary_op -> UnaryOp =
            "-" -> { UnaryOp::Neg }
          | "!" -> { UnaryOp::Not }
          | "&" -> { UnaryOp::Ref }

        rule postfix_expr -> Expr =
            base:primary_expr tail:postfix_tail* -> { fold_postfix(base, tail) }

        // The trailing-lambda form first (Kap 5.2): `.map fn: a.id` has no
        // parentheses, so the plain method rule would stop before the `fn:` and
        // leave it stranded.
        rule postfix_tail -> Postfix =
            "." name:ident _sp:skip_ws lambda:trailing_lambda -> {
                Postfix::Method(name, vec![lambda])
            }
          | "." name:ident args:call_arg_list? -> {
                match args {
                    Some(args) => Postfix::Method(name, args),
                    None => Postfix::Field(name),
                }
            }
          | "[" _sp:skip_ws index:expr _sp2:skip_ws "]" -> {
                Postfix::Index(Box::new(index))
            }
          // `not("?")`: `??` is the null-coalescing operator (Kap 3.5), and a
          // greedy `?` would take it apart into two error propagations.
          | "?" not("?") -> { Postfix::Try }

        // `fn: expr` and `fn { … }` - the arguments are implicit (`a`, `b`),
        // and which of them the body actually uses is settled when it is
        // emitted rather than guessed here.
        rule trailing_lambda -> Expr =
            "fn" _sp:skip_ws ":" _sp2:skip_ws body:expr -> {
                Expr::Closure {
                    params: Vec::new(),
                    implicit: true,
                    body: Block {
                        stmts: vec![Spanned::new(Stmt::Expr(body), 0..0)],
                    },
                }
            }
          | "fn" _sp:skip_ws body:block -> {
                Expr::Closure { params: Vec::new(), implicit: true, body }
            }

        rule call_arg_list -> Vec<Expr> =
            "(" _sp:skip_ws args:call_args? _sp2:skip_ws ")" -> { args.unwrap_or_default() }

        rule call_args -> Vec<Expr> =
            head:expr tail:call_args_tail* -> {
                let mut args = vec![head];
                args.extend(tail);
                args
            }

        rule call_args_tail -> Expr =
            _sp:skip_ws "," _sp2:skip_ws e:expr -> { e }

        // Keyword-led forms first, then the struct literal, then a plain path:
        // `Reading { .. }` must be tried before `Reading` on its own, because a
        // PEG keeps the first alternative that matches.
        rule primary_expr -> Expr =
            sp:spawn_expr -> { sp }
          | d:dsl_from_expr -> { d }
          | i:if_expr -> { i }
          | s:struct_lit -> { s }
          | c:ctor_lit -> { c }
          | b:bool_lit -> { b }
          | p:path_expr -> { p }
          | s:str_lit -> { s }
          | f:float_lit -> { f }
          | i:int_lit -> { i }
          | b:block_expr -> { b }
          | p:paren_expr -> { p }

        // Kap 4.2: `Stats(min: first, max: first)` builds the struct, while
        // `Stats(first)` calls its anonymous constructor. The named form is
        // told apart by requiring the first field to carry a value - otherwise
        // `Stats(x)` would read as a struct with one shorthand field.
        rule ctor_lit -> Expr =
            name:ident _sp:skip_ws "(" _sp2:skip_ws head:named_field_init
            tail:field_init_tail* _sp3:skip_ws ","? _sp4:skip_ws ")"
            -> {
                let mut fields = vec![head];
                fields.extend(tail);
                Expr::StructLit { name, fields }
            }

        rule named_field_init -> FieldInit =
            name:ident _sp:skip_ws ":" _sp2:skip_ws value:expr -> {
                FieldInit { name, value: Some(value) }
            }

        rule float_lit -> Expr =
            whole:digits "." frac:digits -> {
                Expr::LitFloat(format!("{whole}.{frac}"))
            }

        // The same set without the two brace-led forms, for the head of an
        // `if` or a `for`, where a `{` is the body.
        rule head_expr -> Expr =
            head:head_add tail:cmp_head_tail? -> {
                fold_binary(head, tail.into_iter().collect::<Vec<_>>())
            }

        rule cmp_head_tail -> (BinaryOp, Expr) =
            _sp:skip_ws op:cmp_op _sp2:skip_ws e:head_add -> { (op, e) }

        rule head_add -> Expr =
            head:head_mul tail:head_add_tail* -> { fold_binary(head, tail) }

        rule head_add_tail -> (BinaryOp, Expr) =
            _sp:skip_ws op:add_op _sp2:skip_ws e:head_mul -> { (op, e) }

        rule head_mul -> Expr =
            head:head_unary tail:head_mul_tail* -> { fold_binary(head, tail) }

        rule head_mul_tail -> (BinaryOp, Expr) =
            _sp:skip_ws op:mul_op _sp2:skip_ws e:head_unary -> { (op, e) }

        rule head_unary -> Expr =
            op:unary_op _sp:skip_ws e:head_unary -> {
                Expr::Unary { op, expr: Box::new(e) }
            }
          | e:head_postfix -> { e }

        rule head_postfix -> Expr =
            base:head_primary tail:postfix_tail* -> { fold_postfix(base, tail) }

        rule head_primary -> Expr =
            c:ctor_lit -> { c }
          | b:bool_lit -> { b }
          | p:path_expr -> { p }
          | s:str_lit -> { s }
          | f:float_lit -> { f }
          | i:int_lit -> { i }
          | p:paren_expr -> { p }

        rule paren_expr -> Expr =
            "(" _sp:skip_ws e:expr _sp2:skip_ws ")" -> { e }

        // Part I, 8.2: spawn takes a block lambda - `spawn({ ... })`.
        rule spawn_expr -> Expr =
            "spawn" _sp:skip_ws "(" _sp2:skip_ws body:expr _sp3:skip_ws ")" -> {
                Expr::Spawn { body: Box::new(body), is_move: false }
            }

        // Part II, 10.2/10.5: `dsl Json from input` - a named grammar run over
        // a value. The other `dsl` form takes a foreign-syntax block and is not
        // parsed here.
        rule dsl_from_expr -> Expr =
            "dsl" _sp:skip_ws name:ident _sp2:skip_ws "from" _sp3:skip_ws input:head_expr -> {
                Expr::DslFrom { grammar: name, input: Box::new(input) }
            }

        rule if_expr -> Expr =
            "if" _sp:skip_ws cond:head_expr _sp2:skip_ws then_branch:block
            _sp3:skip_ws otherwise:else_branch?
            -> {
                Expr::If {
                    cond: Box::new(cond),
                    then_branch,
                    else_branch: otherwise,
                }
            }

        rule else_branch -> Block =
            "else" _sp:skip_ws b:block -> { b }

        // Blocks are expressions (Part I, 3.1).
        rule block_expr -> Expr =
            b:block -> { Expr::Block(b) }

        rule struct_lit -> Expr =
            name:ident _sp:skip_ws "{" _sp2:skip_ws fields:field_inits _sp3:skip_ws "}" -> {
                Expr::StructLit { name, fields }
            }

        rule field_inits -> Vec<FieldInit> =
            head:field_init tail:field_init_tail* _sp:skip_ws ","? -> {
                let mut fields = vec![head];
                fields.extend(tail);
                fields
            }

        rule field_init_tail -> FieldInit =
            _sp:skip_ws "," _sp2:skip_ws f:field_init -> { f }

        rule field_init -> FieldInit =
            name:ident _sp:skip_ws ":" _sp2:skip_ws value:expr -> {
                FieldInit { name, value: Some(value) }
            }
          | name:ident -> { FieldInit { name, value: None } }

        // `Summary::new` is a path; `println(...)` a call; `acc` a variable.
        rule path_expr -> Expr =
            head:ident tail:path_segment* args:call_arg_list? -> {
                let mut segments = vec![head];
                segments.extend(tail);
                let base = if segments.len() == 1 {
                    Expr::Variable(segments[0])
                } else {
                    Expr::Path(segments)
                };
                match args {
                    Some(args) => Expr::Call { func: Box::new(base), args },
                    None => base,
                }
            }

        rule bool_lit -> Expr =
            "true" -> { Expr::LitBool(true) }
          | "false" -> { Expr::LitBool(false) }

        rule str_lit -> Expr =
            s:STRING -> { Expr::LitStr(s) }

        rule int_lit -> Expr =
            d:digits -> {
                Expr::LitInt(d.parse().unwrap())
            }

        rule digits -> String =
            d:digit1 -> { d.to_string() }
    }
}

// --- Lowering (AST -> Bridge) ---

fn lower_program(parsed: &Parsed) -> Result<BridgeModule> {
    let mut items = Vec::new();
    for item in &parsed.program.items {
        if let Some(bridge_item) = lower_item(parsed, &item.node, item.span.clone())? {
            items.push(bridge_item);
        }
    }

    Ok(BridgeModule {
        name: "main".to_string(),
        items,
    })
}

fn lower_item(parsed: &Parsed, item: &ast::Item, span: ast::Span) -> Result<Option<BridgeItem>> {
    match item {
        ast::Item::Fn {
            name: Some(name),
            body,
            ..
        } => Ok(Some(BridgeItem::Function(BridgeFunction {
            name: parsed.text(*name).to_string(),
            args: vec![],
            ret_type: None,
            body: lower_block(parsed, body)?,
            span,
        }))),
        // A grammar is not a Bridge item: it lowers onto the parser backend,
        // which is the `rust` emitter's job. Dropping it silently would compile
        // a program with its parser missing.
        ast::Item::Grammar(def) => Err(anyhow::anyhow!(
            "grammar `{}` cannot be lowered through the bridge backend; use --backend=rust",
            parsed.text(def.name)
        )),
        _ => Ok(None),
    }
}

fn lower_block(parsed: &Parsed, block: &ast::Block) -> Result<BridgeBlock> {
    let mut stmts = Vec::new();
    for stmt in &block.stmts {
        stmts.push(lower_stmt(parsed, &stmt.node, stmt.span.clone())?);
    }
    Ok(BridgeBlock { stmts, span: 0..0 })
}

fn lower_stmt(parsed: &Parsed, stmt: &ast::Stmt, span: ast::Span) -> Result<BridgeStmt> {
    match stmt {
        ast::Stmt::Let { name, value, .. } => Ok(BridgeStmt::Let(BridgeLetStmt {
            name: parsed.text(*name).to_string(),
            ty: None,
            init: Some(lower_expr(parsed, value)?),
            span,
        })),
        ast::Stmt::Expr(expr) => Ok(BridgeStmt::Expr(lower_expr(parsed, expr)?)),
        _ => Err(anyhow::anyhow!("Unsupported statement type")),
    }
}

fn lower_expr(parsed: &Parsed, expr: &ast::Expr) -> Result<BridgeExpr> {
    match expr {
        ast::Expr::LitInt(i) => Ok(BridgeExpr::Literal(BridgeLiteral::Int(*i))),
        ast::Expr::LitStr(s) => Ok(BridgeExpr::Literal(BridgeLiteral::String(s.clone()))),
        ast::Expr::Variable(id) => Ok(BridgeExpr::Variable(parsed.text(*id).to_string())),
        ast::Expr::Call { func, args } => {
            let mut bridge_args = Vec::new();
            for arg in args {
                bridge_args.push(lower_expr(parsed, arg)?);
            }
            Ok(BridgeExpr::Call(BridgeCall {
                func: Box::new(lower_expr(parsed, func)?),
                args: bridge_args,
                span: 0..0,
            }))
        }
        _ => Err(anyhow::anyhow!("Unsupported expression type: {:?}", expr)),
    }
}
