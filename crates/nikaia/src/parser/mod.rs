// crates/nikaia/src/parser/mod.rs
use crate::ast;
use anyhow::Result;
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
    Method(Symbol, Vec<ast::Expr>, Vec<ast::ConfigArg>),
    Index(Box<ast::Expr>),
}

/// A declaration's parameters, with the config zone split into its two shapes.
pub fn params_of(
    receiver: Option<ast::Receiver>,
    args: Vec<ast::FnArg>,
    zone: Option<ast::ConfigZone>,
) -> ast::FnParams {
    let (config, spread) = match zone {
        Some(ast::ConfigZone::Options(options)) => (options, None),
        Some(ast::ConfigZone::Spread(name)) => (Vec::new(), Some(name)),
        None => (Vec::new(), None),
    };
    ast::FnParams {
        receiver,
        args,
        config,
        spread,
    }
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
        Postfix::Method(method, args, config) => ast::Expr::MethodCall {
            receiver: Box::new(recv),
            method,
            args,
            config,
        },
        Postfix::Index(index) => ast::Expr::Index {
            base: Box::new(recv),
            index,
        },
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
            items:item*
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
        // --- Top-Level Items ---
        //
        // `grammar` and `struct` come before `fn`: all three are keyword-led,
        // and the order is what keeps `grammar` from being read as an
        // identifier.
        // `@=` puts the byte range this rule matched into `_span`, which is
        // how every node below gets the place in the `.nika` file it came from.
        // Without it a diagnostic can only name generated Rust.
        // Labelled, here and below: where one of these fails at the position
        // it started at, none of its alternatives got anywhere, and listing
        // what each could have begun with says less than the word for what was
        // expected. A rule that got *further* keeps its own message - the
        // label only replaces the list (winnow-grammar `# "…"`).
        rule item -> Spanned<Item> # "item" @=
            g:grammar_item -> { Spanned::new(g, _span) }
          | s:struct_item -> { Spanned::new(s, _span) }
          | e:enum_item -> { Spanned::new(e, _span) }
          | im:impl_item -> { Spanned::new(im, _span) }
          | u:use_item -> { Spanned::new(u, _span) }
          | i:fn_item -> { Spanned::new(i, _span) }

        // Kap 4.2: behaviour lives in an `impl`, never in the struct.
        // Kap 4.2 and 4.7: `impl User` gives a type behaviour of its own,
        // `impl Summarize for User` gives it a trait's. The trait name comes
        // first and the `for` is what tells the two apart, so the grammar reads
        // a name and only then finds out which form it was in.
        rule impl_item -> Item =
            "impl" first:type_ref rest:impl_for_target?
            "{" methods:impl_method* "}"
            -> {
                let (trait_name, target) = match rest {
                    Some(t) => (Some(first.name), t),
                    None => (None, first),
                };
                Item::Impl { trait_name, target, methods }
            }

        rule impl_for_target -> Type = "for" t:type_ref -> { t }

        rule impl_method -> Spanned<Item> @= f:fn_item -> { Spanned::new(f, _span) }

        rule kw_sync -> () = "sync" -> { () }
        rule kw_pub -> () = "pub" -> { () }

        // `sync` and `throws` are accepted on either side of the return type,
        // and both sides are used: Part II writes `fn add(…) sync`, Part III
        // `pub fn read(path: Path) -> Bytes throws`, and Part I 7.1
        // `fn fetch_config() throws -> String`. The comment used to claim this
        // while the rule gave `throws` only the trailing slot, so the form the
        // error-handling chapter uses did not parse.
        rule fn_item -> Item =
            vis:kw_pub?
            "fn"
            name:NAME?
            generics:generic_list?
            params:fn_params
            sync_before:kw_sync?
            throws_before:kw_throws?
            ret:return_type_arrow?
            sync_after:kw_sync?
            throws_after:kw_throws?
            body:block
            -> {
                Item::Fn {
                    name,
                    generics: generics.unwrap_or_default(),
                    receiver: params.receiver,
                    args: params.args,
                    config: params.config,
                    spread: params.spread,
                    ret_type: ret,
                    body,
                    is_sync: sync_before.is_some() || sync_after.is_some(),
                    is_public: vis.is_some(),
                    throws: throws_before.is_some() || throws_after.is_some(),
                }
            }

        // ADR-023 D1: `throws` carries no type list. The specification itself
        // wrote one in four places, so a reader will try it, and a parse error
        // at the type name says nothing about why - it offered `->` and `sync`
        // as if either were the point. The precedent is `trailing_lambda`
        // below: a form that was in the specification deserves a sentence.
        //
        // `not(kw_sync)` because `fn f() throws sync { … }` is legal - `sync`
        // may stand on either side of the return type - and `sync` is an
        // ordinary identifier to `type_ref`.
        rule kw_throws -> () =
            "throws" not(kw_sync) type_refs fail(
                "`throws` names no error type (ADR-023 D1): write `throws` on its own. \
                 *Which* errors can leave a function follows from its body and from \
                 everything the body reaches, so it is derived rather than written: \
                 the set is inferred whole-program and recorded in `nikaia.contracts`, \
                 beside the build (Part III, 13.5). Written by hand it would go stale \
                 the first time a callee gained a failure - and a failure that comes \
                 from a resource's cleanup would make you name a type you never \
                 mentioned (ADR-006 D4)."
            ) -> { () }
          | "throws" -> { () }

        // Kap 4.2: `&mut self`, `&self`, `self` - the subject, when there is one.
        rule fn_params -> FnParams =
            "(" body:fn_params_body? ")" -> {
                body.unwrap_or_default()
            }

        rule fn_params_body -> FnParams =
            r:receiver args:fn_arg_def_tail* config:config_zone? -> {
                params_of(Some(r), args, config)
            }
          | head:fn_arg_def tail:fn_arg_def_tail* config:config_zone? -> {
                let mut args = vec![head];
                args.extend(tail);
                params_of(None, args, config)
            }
          | config:config_zone -> {
                params_of(None, Vec::new(), Some(config))
            }

        // Kap 5.1: everything after the `;` is an option. Named at the call,
        // never positional - which is what the separator buys, and why it is a
        // separator rather than a convention about where the flags go.
        //
        // ADR-007 D5 puts one more thing there: `...args: Self::dsl`, the typed
        // spread a DSL driver accepts deferred parameters with. It is tried
        // first because `...` cannot begin an option's name, so a `;` followed
        // by one is unambiguous.
        rule config_zone -> ConfigZone =
            ";" "..." name:NAME ":" "Self::dsl" -> { ConfigZone::Spread(name) }
          | ";" "..." name:NAME ":" fail(
                "a typed spread is written `...name: Self::dsl` (ADR-007 D5): \
                 `Self::dsl` is the parameter type the DSL string generates, and \
                 it is the only type this parameter can have."
            ) -> { ConfigZone::Spread(name) }
          | ";" head:config_param tail:config_param_tail* -> {
                let mut params = vec![head];
                params.extend(tail);
                ConfigZone::Options(params)
            }

        rule config_param_tail -> ConfigParam = "," p:config_param -> { p }

        // The default is required, and that is what makes this an *option*: a
        // caller may leave it out, and leaving it out is never a question about
        // what the value is. A parameter that must be passed belongs before the
        // `;`.
        rule config_param -> ConfigParam =
            name:NAME ":" ty:type_ref "=" default:literal_expr -> {
                ConfigParam { name, ty, default }
            }
          | name:NAME ":" ty:type_ref fail(
                "a configuration parameter needs a default (Kap 5.1): write \
                 `name: T = value`. Without one it has to be passed at every \
                 call, and a parameter that has to be passed belongs before the \
                 `;`."
            ) -> {
                ConfigParam { name, ty, default: Expr::LitBool(false) }
            }

        // A **literal**, and only a literal. An option's default is a constant
        // in every program anyone writes, and an arbitrary expression would
        // raise a question Stage 0 has no answer for: whether it is evaluated
        // where the function is declared or where it is called.
        rule literal_expr -> Expr =
            b:bool_lit -> { b }
          | s:str_lit -> { s }
          | c:char_lit -> { c }
          | "-" f:float_lit -> { Expr::Unary { op: UnaryOp::Neg, expr: Box::new(f) } }
          | f:float_lit -> { f }
          | "-" i:int_lit -> { Expr::Unary { op: UnaryOp::Neg, expr: Box::new(i) } }
          | i:int_lit -> { i }

        rule receiver -> Receiver =
            "&" "mut" "self" -> {
                Receiver { is_ref: true, is_mut: true }
            }
          | "&" "self" -> { Receiver { is_ref: true, is_mut: false } }
          | "self" -> { Receiver { is_ref: false, is_mut: false } }

        // Kap 9.2: use std::fs
        rule use_item -> Item =
            "use" head:NAME tail:path_segment* -> {
                let mut path = vec![head];
                path.extend(tail);
                Item::Import { path }
            }

        rule path_segment -> Symbol = "::" n:NAME -> { n }

        // --- Structs ---
        //
        // ADR-008 D6: `@borrowed` is an assertion about escape, so it is part
        // of the item, not a comment.
        rule struct_item -> Item =
            borrowed:at_borrowed?
            vis:kw_pub?
            "struct"
            name:NAME
            generics:generic_list?
            "{"
            fields:field_defs?
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

        // Kap 4.4. The three shapes the specification shows and no others: a
        // name, a name with positional types, a name with named fields.
        rule enum_item -> Item =
            vis:kw_pub?
            "enum" name:NAME
            "{" variants:enum_variants "}"
            -> {
                Item::Enum { name, variants, is_public: vis.is_some() }
            }

        rule enum_variants -> Vec<EnumVariant> =
            head:enum_variant tail:enum_variant_tail* ","? -> {
                let mut variants = vec![head];
                variants.extend(tail);
                variants
            }

        rule enum_variant_tail -> EnumVariant = "," v:enum_variant -> { v }

        rule enum_variant -> EnumVariant =
            name:NAME "(" types:type_refs ")" -> {
                EnumVariant { name, fields: VariantFields::Tuple(types) }
            }
          | name:NAME "{" fields:field_defs "}" -> {
                EnumVariant { name, fields: VariantFields::Named(fields) }
            }
          | name:NAME -> {
                EnumVariant { name, fields: VariantFields::Unit }
            }

        rule field_defs -> Vec<FieldDef> =
            head:field_def tail:field_def_tail* ","? -> {
                let mut fields = vec![head];
                fields.extend(tail);
                fields
            }

        rule field_def_tail -> FieldDef = "," f:field_def -> { f }

        // Kap 9.2: `pub` on a field, which is a different question from `pub`
        // on the struct - a public type may keep its parts to itself, and 9.3
        // says that is the point.
        rule field_def -> FieldDef =
            vis:kw_pub? name:NAME ":" ty:type_ref -> {
                FieldDef { name, ty, is_public: vis.is_some() }
            }

        // --- Argumente & Typen ---

        rule fn_arg_list -> Vec<FnArg> =
            "(" args:fn_arg_defs? ")" -> { args.unwrap_or_default() }

        rule fn_arg_defs -> Vec<FnArg> =
            head:fn_arg_def tail:fn_arg_def_tail* -> {
                let mut args = vec![head];
                args.extend(tail);
                args
            }

        rule fn_arg_def_tail -> FnArg = "," arg:fn_arg_def -> { arg }

        rule fn_arg_def -> FnArg =
            name:NAME ":" ty:type_ref -> {
                FnArg { name, ty }
            }

        rule return_type_arrow -> Type =
            "->" ty:type_ref -> { ty }

        // USING [ ] SYNTAX directly for testing
        rule generic_list -> Vec<GenericParam> =
            [ params:generic_params? ] -> { params.unwrap_or_default() }

        rule generic_params -> Vec<GenericParam> =
            head:generic_param tail:generic_param_tail* -> {
                let mut params = vec![head];
                params.extend(tail);
                params
            }

        rule generic_param_tail -> GenericParam = "," p:generic_param -> { p }

        rule generic_param -> GenericParam =
            name:NAME
            -> { GenericParam { name } }

        // `&str` is a view marker (Part II, 10.6), not a lifetime - the `&` is
        // recorded and the emitter decides what it becomes.
        rule type_ref -> Type # "type" =
            view:amp?
            name:type_name
            generics:generic_type_args?
            -> {
                Type {
                    name,
                    generics: generics.unwrap_or_default(),
                    is_view: view.is_some(),
                    is_tuple: false,
                }
            }
          // `(A, B)`. The parts go where a named type's arguments go, so
          // everything that walks a type's arguments walks a tuple's parts.
          | "(" parts:type_refs ")" -> {
                Type {
                    name: _state.intern("tuple"),
                    generics: parts,
                    is_view: false,
                    is_tuple: true,
                }
            }

        rule amp -> () = "&" -> { () }

        // USING [ ] SYNTAX directly for testing
        rule generic_type_args -> Vec<Type> =
            [ args:type_refs? ] -> { args.unwrap_or_default() }

        // A type may be named by a path: `postgres::Connection`, `html::Raw`.
        //
        // The whole path is interned as one name, because that is what the name
        // *is* to a compiler that lowers name for name (ADR-011 D2) - nothing
        // here resolves a module, and a path can therefore never collide with a
        // struct this file declares, which is correct.
        rule type_name -> Symbol =
            head:NAME tail:path_segment* -> {
                if tail.is_empty() {
                    head
                } else {
                    let mut path = String::new();
                    path.push_str(_state.interner.resolve(head));
                    for segment in tail {
                        path.push_str("::");
                        path.push_str(_state.interner.resolve(segment));
                    }
                    _state.intern(&path)
                }
            }

        rule type_refs -> Vec<Type> =
            head:type_ref tail:type_ref_tail* -> {
                let mut args = vec![head];
                args.extend(tail);
                args
            }

        rule type_ref_tail -> Type = "," t:type_ref -> { t }

        // --- Part II, Kapitel 10: Grammatiken ---

        rule grammar_item -> Item =
            "grammar" name:NAME
            "{" rules:grammar_rule* "}"
            -> { Item::Grammar(GrammarDef { name, rules }) }

        rule grammar_rule -> GrammarRule @=
            frame:frame_attr?
            vis:kw_pub?
            "rule"
            name:NAME
            ret:return_type_arrow?
            label:rule_label?
            "="
            alts:g_alts
            -> {
                GrammarRule {
                    name,
                    is_public: vis.is_some(),
                    frame,
                    ret_type: ret,
                    label,
                    alts,
                    span: _span,
                }
            }

        // `rule expr -> Expr # "expression" = …`: what the rule is called when
        // it fails where it began. Spelled as the backend spells it, because
        // the lowering is name for name (ADR-011 D2) and a second spelling for
        // the same thing would be one more thing to know.
        rule rule_label -> String = "#" text:STRING -> { text }

        // ADR-009 D1: the attribute is keyed. `@frame`, `@frame(boundary: "\n")`,
        // `@frame(boundary: "\n", unchecked)`; the positional form is withdrawn,
        // so that every future cut-point key (quote, start, escape, scan) has
        // room without changing what the existing ones mean.
        rule frame_attr -> FrameAttr =
            "@frame" args:frame_args? -> {
                args.unwrap_or_default()
            }

        rule frame_args -> FrameAttr =
            "(" head:frame_arg tail:frame_arg_tail* ")" -> {
                let mut attr = head;
                for a in tail {
                    if a.boundary.is_some() { attr.boundary = a.boundary; }
                    attr.unchecked = attr.unchecked || a.unchecked;
                }
                attr
            }

        rule frame_arg_tail -> FrameAttr = "," a:frame_arg -> { a }

        rule frame_arg -> FrameAttr =
            "boundary" ":" b:STRING -> {
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

        rule g_alt_tail -> GrammarAlt = "|" a:g_alt -> { a }

        // An action block is required today (Part II, 10.1, note) - with one
        // exception that is not an omission: a `par_fold` must be the whole
        // body of its rule (ADR-009 D2), so there is nothing for an action to
        // add. The emitter supplies the binding such a rule needs.
        rule g_alt -> GrammarAlt =
            p:g_seq "->" action:block -> {
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

        rule g_elem_tail -> Spanned<Pattern> = e:g_elem -> { e }

        rule g_elem -> Spanned<Pattern> =
            c:g_cut -> { c }
          | b:g_bind -> { b }
          | p:g_postfix -> { p }

        // Part II, 10.1: the commit point. Once passed, a later failure is an
        // error rather than a reason to try the next alternative.
        rule g_cut -> Spanned<Pattern> @= "=>" -> { Spanned::new(Pattern::Cut, _span) }

        rule g_bind -> Spanned<Pattern> @=
            name:NAME ":" p:g_postfix -> {
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

        // A brace group is a bound only when its content starts with a digit,
        // which is the rule the backend states for the same ambiguity
        // (SYNTAX.md, "Braces"). Without the lookahead, `n:digit1 { n }` - an
        // action block someone forgot the `->` in front of - is read as a
        // bound and reported as `expected digits`, which is true of the parser
        // and no help to the reader.
        rule g_bounds -> Repeat =
            peek(("{" digit)) "{" b:g_bound_body "}" -> { b }

        rule g_bound_body -> Repeat =
            n:number "," m:number -> { Repeat::Between(n, m) }
          | n:number "," -> { Repeat::AtLeast(n) }
          | n:number -> { Repeat::Exactly(n) }

        rule number -> u32 =
            d:digit1 -> { d.parse().unwrap_or(0) }

        rule g_atom -> Spanned<Pattern> @=
            f:g_fold -> { f }
          | s:STRING -> { Spanned::new(Pattern::Literal(s), _span) }
          | g:g_group -> { g }
          | r:g_ref -> { r }

        rule g_group -> Spanned<Pattern> @=
            "(" p:g_choice ")" -> {
                Spanned::new(Pattern::Group(Box::new(p)), _span)
            }

        // A rule reference, a built-in (`digit`, `frame_end`), or a call to
        // either (`until(";" | frame_end)`, `list(pair, ",")`). The grammar
        // cannot tell them apart, and does not need to: what a name means is
        // the backend's question.
        rule g_ref -> Spanned<Pattern> @=
            name:NAME generics:generic_type_args? args:g_args? -> {
                Spanned::new(
                    Pattern::Ref {
                        name,
                        generics: generics.unwrap_or_default(),
                        args: args.unwrap_or_default(),
                    },
                    _span,
                )
            }

        rule g_args -> Vec<Spanned<Pattern>> =
            "(" head:g_choice tail:g_arg_tail* ")" -> {
                let mut args = vec![head];
                args.extend(tail);
                args
            }

        rule g_arg_tail -> Spanned<Pattern> = "," p:g_choice -> { p }

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

        rule g_choice_tail -> Spanned<Pattern> = "|" p:g_seq -> { p }

        // ADR-009 D2: parallel parsing is a frame plus a monoid. `fold` is the
        // accumulator; the merge is what makes it parallelisable, and asking
        // for it is how the user says a different chunk count is the same
        // answer to them.
        rule g_fold -> Spanned<Pattern> @=
            "par_fold" "("
            r:NAME ","
            init:expr ","
            step:expr ","
            merge:expr ")"
            -> {
                Spanned::new(Pattern::Fold(Box::new(FoldSpec {
                    parallel: true,
                    rule: r,
                    init,
                    step,
                    merge: Some(merge),
                })), _span)
            }
          | "fold" "("
            r:NAME ","
            init:expr ","
            step:expr ")"
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
            "{" stmts:stmt_list "}" -> { Block { stmts } }

        rule stmt_list -> Vec<Spanned<Stmt>> =
            stmts:stmt* -> { stmts }

        rule stmt -> Spanned<Stmt> # "statement" @=
            l:let_stmt -> { Spanned::new(l, _span) }
          | r:return_stmt -> { Spanned::new(r, _span) }
          | t:throw_stmt -> { Spanned::new(t, _span) }
          | w:while_stmt -> { Spanned::new(w, _span) }
          | f:for_stmt -> { Spanned::new(f, _span) }
          | a:assign_stmt -> { Spanned::new(a, _span) }
          | e:expr_stmt -> { Spanned::new(e, _span) }

        rule return_stmt -> Stmt =
            "return" value:expr? ";"? -> {
                Stmt::Return(value)
            }

        // Kap 7.1: `throw` is the only way an error originates. Without it a
        // program could propagate what `std` produced and never produce one of
        // its own (ADR-023 D2).
        rule throw_stmt -> Stmt =
            "throw" value:expr ";"? -> {
                Stmt::Expr(Expr::Throw(Box::new(value)))
            }

        rule kw_mut -> () = "mut" -> { () }

        rule let_stmt -> Stmt =
            "let"
            mutable:kw_mut?
            name:NAME
            ty:type_annotation?
            "="
            val:expr
            ";"?
            -> {
                Stmt::Let {
                    name,
                    mutable: mutable.is_some(),
                    ty,
                    value: val
                }
            }

        rule type_annotation -> Type =
            ":" ty:type_ref -> { ty }

        // Kap 3.3. The head is parsed with the brace-free expression grammar:
        // in `for d in whole { ... }` the brace opens the body, never a struct
        // literal - the same restriction Rust puts on this position.
        // Kap 3.3. Before `expr_stmt` in `stmt`, or `while` would be read as an
        // identifier and the condition as a statement of its own - which is
        // exactly what happened before this rule existed: three statements, no
        // error, and `rustc` complaining about a file nobody wrote.
        rule while_stmt -> Stmt =
            "while" cond:head_expr body:block ";"?
            -> {
                Stmt::While { cond, body }
            }

        rule for_stmt -> Stmt =
            "for" bindings:for_bindings "in"
            iter:head_expr body:block ";"?
            -> {
                Stmt::For { bindings, iter, body }
            }

        rule for_bindings -> Vec<Symbol> =
            "(" head:NAME tail:ident_tail* ")" -> {
                let mut names = vec![head];
                names.extend(tail);
                names
            }
          | n:NAME -> { vec![n] }

        rule ident_tail -> Symbol = "," n:NAME -> { n }

        rule assign_stmt -> Stmt =
            target:postfix_expr op:assign_op value:expr ";"?
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
            e:expr ";"? -> { Stmt::Expr(e) }

        // --- Expressions ---

        pub rule expr -> Expr # "expression" =
            c:closure_expr -> { c }
          | e:catch_expr -> { e }

        // Kap 7.1: `fs::map(path) catch { … }` - the error is `error` inside.
        rule catch_expr -> Expr =
            value:coalesce_expr handler:catch_tail? -> {
                match handler {
                    Some(handler) => Expr::TryCatch { expr: Box::new(value), handler },
                    None => value,
                }
            }

        rule catch_tail -> Block =
            "catch" b:block -> { b }

        // Kap 3.5: `value ?? fallback`.
        rule coalesce_expr -> Expr =
            value:range_expr fallback:coalesce_tail? -> {
                match fallback {
                    Some(fallback) => Expr::Coalesce {
                        value: Box::new(value),
                        fallback: Box::new(fallback),
                    },
                    None => value,
                }
            }

        rule coalesce_tail -> Expr =
            "??" e:or_expr -> { e }

        // Kap 5.2/5.3: a lambda, with its arguments named or implicit.
        rule closure_expr -> Expr =
            "fn" "(" params:closure_params? ")" body:block
            -> {
                Expr::Closure {
                    params: params.unwrap_or_default(),
                    implicit: false,
                    body,
                }
            }
          | "fn" body:block -> {
                Expr::Closure { params: Vec::new(), implicit: true, body }
            }

        rule closure_params -> Vec<Symbol> =
            head:NAME tail:closure_param_tail* -> {
                let mut params = vec![head];
                params.extend(tail);
                params
            }

        rule closure_param_tail -> Symbol = "," p:NAME -> { p }

        // Kap 3.3. It binds looser than every operator below it, so `0..n - 1`
        // is a range ending at `n - 1` rather than a range subtracted from -
        // which is the reading a `for` head wants and the only one that is ever
        // useful. `..=` is tried first, or its `=` would be read as the start
        // of a comparison.
        rule range_expr -> Expr =
            start:or_expr end:range_tail? -> {
                match end {
                    Some((inclusive, end)) => Expr::Range {
                        start: Box::new(start),
                        end: Box::new(end),
                        inclusive,
                    },
                    None => start,
                }
            }

        rule range_tail -> (bool, Expr) =
            "..=" e:or_expr -> { (true, e) }
          | ".." e:or_expr -> { (false, e) }

        rule or_expr -> Expr =
            head:and_expr tail:or_tail* -> { fold_binary(head, tail) }

        rule or_tail -> (BinaryOp, Expr) = "||" e:and_expr -> { (BinaryOp::Or, e) }

        rule and_expr -> Expr =
            head:cmp_expr tail:and_tail* -> { fold_binary(head, tail) }

        rule and_tail -> (BinaryOp, Expr) = "&&" e:cmp_expr -> { (BinaryOp::And, e) }

        rule cmp_expr -> Expr =
            head:add_expr tail:cmp_tail? -> {
                fold_binary(head, tail.into_iter().collect::<Vec<_>>())
            }

        rule cmp_tail -> (BinaryOp, Expr) = op:cmp_op e:add_expr -> { (op, e) }

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

        rule add_tail -> (BinaryOp, Expr) = op:add_op e:mul_expr -> { (op, e) }

        rule add_op -> BinaryOp =
            "+" -> { BinaryOp::Add }
          | "-" -> { BinaryOp::Sub }

        rule mul_expr -> Expr =
            head:cast_expr tail:mul_tail* -> { fold_binary(head, tail) }

        rule mul_tail -> (BinaryOp, Expr) = op:mul_op e:cast_expr -> { (op, e) }

        rule cast_expr -> Expr =
            head:unary_expr casts:cast_tail* -> {
                casts.into_iter().fold(head, |expr, ty| Expr::Cast {
                    expr: Box::new(expr),
                    ty,
                })
            }

        rule cast_tail -> Type = "as" ty:type_ref -> { ty }

        rule mul_op -> BinaryOp =
            "*" -> { BinaryOp::Mul }
          | "/" -> { BinaryOp::Div }
          | "%" -> { BinaryOp::Rem }

        // Labelled as well as `expr`, and for the operand rather than for the
        // whole: `1 + ` fails inside `add_tail`, whose operand is a
        // `mul_expr`, so the label on `expr` never sees it. Every operand
        // chain bottoms out here at the position the operand should have
        // started.
        rule unary_expr -> Expr # "expression" =
            op:unary_op e:unary_expr -> {
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
            // `.route("/x") fn { … }`: arguments *and* a trailing lambda. It
            // has to be tried before the plain call, or the call matches and
            // the lambda is left over - which is the parse error this form did
            // not have a grammar for until ADR-022.
            "." name:NAME args:call_arg_list lambda:trailing_lambda -> {
                let (mut positional, config) = args;
                positional.push(lambda);
                Postfix::Method(name, positional, config)
            }
          | "." name:NAME lambda:trailing_lambda -> {
                Postfix::Method(name, vec![lambda], Vec::new())
            }
          | "." name:NAME args:call_arg_list? -> {
                match args {
                    // Kap 5.1's `;` reaches a method call too, and what stands
                    // after it is kept: ADR-007 D5's deferred parameters arrive
                    // exactly here, and dropping them was why a `dsl` statement
                    // could not be given any.
                    Some((args, config)) => Postfix::Method(name, args, config),
                    None => Postfix::Field(name),
                }
            }
          | "[" index:expr "]" -> {
                Postfix::Index(Box::new(index))
            }
          // `not("?")`: `??` is the null-coalescing operator (Kap 3.5), and a
          // `t.0` - a tuple's parts are numbered, and the number is a field
          // name like any other, so nothing downstream has to know.
          | "." index:digits -> { Postfix::Field(_state.intern(&index)) }


        // `fn: expr` and `fn { … }` - the arguments are implicit (`a`, `b`),
        // and which of them the body actually uses is settled when it is
        // emitted rather than guessed here.
        rule trailing_lambda -> Expr =
            "fn" body:block -> {
                Expr::Closure { params: Vec::new(), implicit: true, body }
            }
            // ADR-022: `fn: expr` was removed, and a form that was in the
            // specification deserves a sentence rather than a parse error at
            // the colon. `fail` beats the alternatives at this position, so
            // this is what a reader gets.
          | "fn" ":" fail("the `fn: …` form was removed (ADR-022): write `fn { … }`. \
                           Its body ran to the end of the expression, so a `.method()` \
                           after it landed *inside* the lambda - silently") -> {
                Expr::Closure {
                    params: Vec::new(),
                    implicit: true,
                    body: Block { stmts: Vec::new() },
                }
            }

        // Kap 5.1: subjects, then a `;`, then options by name. The separator
        // is the whole protocol - what is before it is data and may be
        // positional, what is after it is configuration and may not.
        rule call_arg_list -> (Vec<Expr>, Vec<ConfigArg>) =
            "(" args:call_args? config:config_args? ")" -> {
                (args.unwrap_or_default(), config.unwrap_or_default())
            }

        rule call_args -> Vec<Expr> =
            head:expr tail:call_args_tail* -> {
                let mut args = vec![head];
                args.extend(tail);
                args
            }

        rule call_args_tail -> Expr = "," e:expr -> { e }

        rule config_args -> Vec<ConfigArg> =
            ";" head:config_arg tail:config_arg_tail* -> {
                let mut args = vec![head];
                args.extend(tail);
                args
            }

        rule config_arg_tail -> ConfigArg = "," a:config_arg -> { a }

        rule config_arg -> ConfigArg =
            name:NAME ":" value:expr -> { ConfigArg { name, value } }

        // Keyword-led forms first, then the struct literal, then a plain path:
        // `Reading { .. }` must be tried before `Reading` on its own, because a
        // PEG keeps the first alternative that matches.
        rule primary_expr -> Expr =
            sp:spawn_expr -> { sp }
          | d:dsl_block_expr -> { d }
          | d:dsl_from_expr -> { d }
          | i:if_expr -> { i }
          | m:match_expr -> { m }
          // Before `struct_lit` and `path_expr`, and both orders matter: a PEG
          // keeps the first alternative that matches, so `seq { … }` would
          // otherwise be read as a struct literal called `seq` - its statements
          // taken for shorthand fields - or as a variable followed by a block
          // of its own, which is the trap the `while` rule records.
          | s:seq_expr -> { s }
          | s:struct_lit -> { s }
          | c:ctor_lit -> { c }
          | b:bool_lit -> { b }
          // Before `path_expr`: a PEG keeps the first alternative that matches,
          // and `f"…"` starts with what `NAME` reads as the variable `f`.
          | s:f_str_lit -> { s }
          | p:path_expr -> { p }
          | s:str_lit -> { s }
          | c:char_lit -> { c }
          | f:float_lit -> { f }
          | i:int_lit -> { i }
          | b:block_expr -> { b }
          | t:tuple_expr -> { t }
          | p:paren_expr -> { p }

        // Kap 4.2: `Stats(min: first, max: first)` builds the struct, while
        // `Stats(first)` calls its anonymous constructor. The named form is
        // told apart by requiring the first field to carry a value - otherwise
        // `Stats(x)` would read as a struct with one shorthand field.
        rule ctor_lit -> Expr =
            name:NAME "(" head:named_field_init
            tail:field_init_tail* ","? ")"
            -> {
                let mut fields = vec![head];
                fields.extend(tail);
                Expr::StructLit { name, fields }
            }

        rule named_field_init -> FieldInit =
            name:NAME ":" value:expr -> {
                FieldInit { name, value: Some(value) }
            }

        rule float_lit -> Expr =
            f:FLOAT -> { Expr::LitFloat(f) }

        // Uppercase, so this is lexical: `1 . 5` is not a number, and neither
        // is `1.0 e5`. The exponent is what a program about physical
        // quantities is written in - `9.54791938424326609e-04` beside a `1.0`
        // is the mass of Jupiter, and spelling it out in zeroes is how a digit
        // gets lost. The text is kept as written and handed to the language
        // below, which spells a float literal the same way.
        rule FLOAT -> String =
            w:digit1 "." f:digit1 e:EXPONENT? -> {
                format!("{w}.{f}{}", e.unwrap_or_default())
            }
          | w:digit1 e:EXPONENT -> { format!("{w}{e}") }

        rule EXPONENT -> String =
            "e" "-" d:digit1 -> { format!("e-{d}") }
          | "e" "+" d:digit1 -> { format!("e+{d}") }
          | "e" d:digit1 -> { format!("e{d}") }
          | "E" "-" d:digit1 -> { format!("E-{d}") }
          | "E" "+" d:digit1 -> { format!("E+{d}") }
          | "E" d:digit1 -> { format!("E{d}") }

        // The same set without the two brace-led forms, for the head of an
        // `if` or a `for`, where a `{` is the body.
        rule head_expr -> Expr =
            start:head_cmp end:head_range_tail? -> {
                match end {
                    Some((inclusive, end)) => Expr::Range {
                        start: Box::new(start),
                        end: Box::new(end),
                        inclusive,
                    },
                    None => start,
                }
            }

        rule head_range_tail -> (bool, Expr) =
            "..=" e:head_cmp -> { (true, e) }
          | ".." e:head_cmp -> { (false, e) }

        rule head_cmp -> Expr =
            head:head_add tail:cmp_head_tail? -> {
                fold_binary(head, tail.into_iter().collect::<Vec<_>>())
            }

        rule cmp_head_tail -> (BinaryOp, Expr) = op:cmp_op e:head_add -> { (op, e) }

        rule head_add -> Expr =
            head:head_mul tail:head_add_tail* -> { fold_binary(head, tail) }

        rule head_add_tail -> (BinaryOp, Expr) = op:add_op e:head_mul -> { (op, e) }

        rule head_mul -> Expr =
            head:head_unary tail:head_mul_tail* -> { fold_binary(head, tail) }

        rule head_mul_tail -> (BinaryOp, Expr) = op:mul_op e:head_unary -> { (op, e) }

        rule head_unary -> Expr =
            op:unary_op e:head_unary -> {
                Expr::Unary { op, expr: Box::new(e) }
            }
          | e:head_postfix -> { e }

        rule head_postfix -> Expr =
            base:head_primary tail:postfix_tail* -> { fold_postfix(base, tail) }

        rule head_primary -> Expr =
            c:ctor_lit -> { c }
          | b:bool_lit -> { b }
          // Before `path_expr`, for the reason `primary_expr` gives.
          | s:f_str_lit -> { s }
          | p:path_expr -> { p }
          | s:str_lit -> { s }
          | c:char_lit -> { c }
          | f:float_lit -> { f }
          | i:int_lit -> { i }
          | p:paren_expr -> { p }

        // `(a, b)` before `(a)`: a PEG keeps the first alternative that
        // matches, and a tuple is a parenthesised expression until the comma.
        rule tuple_expr -> Expr =
            "(" head:expr tail:call_args_tail+ ","? ")" -> {
                let mut parts = vec![head];
                parts.extend(tail);
                Expr::Tuple(parts)
            }

        rule paren_expr -> Expr =
            "(" e:expr ")" -> { e }

        // Part I, 8.2: spawn takes a block lambda - `spawn({ ... })`.
        rule spawn_expr -> Expr =
            "spawn" "(" body:expr ")" -> {
                Expr::Spawn { body: Box::new(body), is_move: false }
            }

        // Part II, 10.2/10.5: `dsl Json from input` - a named grammar run over
        // Part II, 10.5: a DSL block ends with `} eod`, and the end cannot be
        // found by counting braces - the body is foreign syntax where a `}` may
        // be a string character or absent entirely. So the body is what lies
        // before the marker, taken verbatim; what it *means* is the target
        // grammar's business and is decided when it is lowered.
        rule dsl_block_expr -> Expr =
            "dsl" name:NAME "{" body:until("} eod") "} eod" -> {
                Expr::Dsl { target: name, context: None, content: body.to_string() }
            }

        // a value. The other `dsl` form takes a foreign-syntax block.
        rule dsl_from_expr -> Expr =
            // The binding is `source`, not `input`: the generated parser's own
            // closure takes a parameter called `input`, and a binding of that
            // name shadows it for the rest of the action.
            "dsl" name:NAME "from" source:head_expr -> {
                Expr::DslFrom { grammar: name, input: Box::new(source) }
            }

        // Kap 3.4. The value is a `head_expr` for the reason `if`'s condition is
        // one: `match value {` would otherwise read `value { … }` as a struct
        // literal and take the arms for fields.
        rule match_expr -> Expr =
            "match" value:head_expr "{" arms:match_arm+ "}" -> {
                Expr::Match { value: Box::new(value), arms }
            }

        rule match_arm -> MatchArm =
            pattern:match_pattern "=>" body:match_arm_body ","? -> {
                MatchArm { pattern, body }
            }

        rule match_arm_body -> Expr =
            b:block -> { Expr::Block(b) }
          | e:expr -> { e }

        // `_` first, and only where no name follows it: `_name` is a name.
        rule match_pattern -> MatchPattern =
            "_" not(raw_ident) -> { MatchPattern::Wildcard }
          | l:pattern_lit -> { MatchPattern::Literal(l) }
          | path:pattern_path "(" bindings:ident_list ")" -> {
                MatchPattern::Tuple { path, bindings }
            }
          | path:pattern_path "{" bindings:ident_list "}" -> {
                MatchPattern::Named { path, bindings }
            }
          | path:pattern_path -> { MatchPattern::Path(path) }

        // The compiler's identifier.
        //
        // The backend's `ident` accepts a **leading digit** - `1` is an
        // identifier to it, and a grammar that wants otherwise says so, which
        // is what this rule is. Without it `1.5` parses as the field `5` of a
        // variable called `1`: harmless while the emitted text happens to read
        // back the same, and wrong the moment there is more after it -
        // `1.5e-4` came out as `1.5e - 4`, and `(2.0).sqrt()` would have been
        // a field access too.
        //
        // `not(digit)` consumes nothing and demands nothing, so it costs a
        // character comparison at the start of every name.
        rule NAME -> Symbol = not(digit) n:ident -> { n }

        rule pattern_lit -> Expr =
            b:bool_lit -> { b }
          | s:str_lit -> { s }
          | c:char_lit -> { c }
          | n:int_lit -> { n }

        rule pattern_path -> Vec<Symbol> =
            head:NAME tail:path_segment* -> {
                let mut path = vec![head];
                path.extend(tail);
                path
            }

        rule ident_list -> Vec<Symbol> =
            head:NAME tail:ident_tail* -> {
                let mut names = vec![head];
                names.extend(tail);
                names
            }

        rule if_expr -> Expr =
            "if" cond:head_expr then_branch:block otherwise:else_branch?
            -> {
                Expr::If {
                    cond: Box::new(cond),
                    then_branch,
                    else_branch: otherwise,
                }
            }

        rule else_branch -> Block =
            "else" b:block -> { b }

        // Blocks are expressions (Part I, 3.1).
        rule block_expr -> Expr =
            b:block -> { Expr::Block(b) }

        // Part I 8.1.1: `seq { … }` states an order the compiler cannot see
        // (ADR-033 D7). The statements inside keep the order they were written
        // in, whatever their touch sets say - which is why it is a block and
        // not an attribute on a statement: what it constrains is a *sequence*.
        //
        // The keyword is provisional (D7). It has to read as "in this order,
        // whatever you think", and `seq` is a placeholder for a word chosen
        // later; changing it is this rule, the AST variant's doc, and the two
        // spec sections that name it.
        rule seq_expr -> Expr =
            "seq" b:block -> { Expr::Seq(b) }

        rule struct_lit -> Expr =
            name:NAME "{" fields:field_inits "}" -> {
                Expr::StructLit { name, fields }
            }

        rule field_inits -> Vec<FieldInit> =
            head:field_init tail:field_init_tail* ","? -> {
                let mut fields = vec![head];
                fields.extend(tail);
                fields
            }

        rule field_init_tail -> FieldInit = "," f:field_init -> { f }

        rule field_init -> FieldInit =
            name:NAME ":" value:expr -> {
                FieldInit { name, value: Some(value) }
            }
          | name:NAME -> { FieldInit { name, value: None } }

        // `Summary::new` is a path; `println(...)` a call; `acc` a variable.
        rule path_expr -> Expr =
            head:NAME tail:path_segment* args:call_arg_list? -> {
                let mut segments = vec![head];
                segments.extend(tail);
                let base = if segments.len() == 1 {
                    Expr::Variable(segments[0])
                } else {
                    Expr::Path(segments)
                };
                match args {
                    Some((args, config)) => Expr::Call {
                        func: Box::new(base),
                        args,
                        config,
                    },
                    None => base,
                }
            }

        rule bool_lit -> Expr =
            "true" -> { Expr::LitBool(true) }
          | "false" -> { Expr::LitBool(false) }

        // Kap 2.5. `f` before the quote is what makes a string *code* - without
        // it the braces are braces (ADR-035). UPPERCASE, so the `f` and the
        // quote are one token: `f "x"` with a space is the variable `f`
        // followed by a string, and reading it as an interpolation would make
        // whitespace change what a program means.
        rule FSTRING -> String =
            "f\"" parts:STR_CHAR* "\"" -> { parts.concat() }

        rule str_lit -> Expr =
            s:STRING -> { Expr::LitStr(s) }

        // Its own rule rather than an alternative inside `str_lit`, and the
        // reason is the error message: a rule whose body is one sequence
        // reports what it can start with, so `"` stays in the "also possible
        // here" list and `f` joins it. Folded into `str_lit` both disappear,
        // which Part III C.2 would not forgive.
        //
        // **Not reachable from `literal_expr` or `pattern_lit`**, and
        // deliberately: a Kap 5.1 default and a `match` pattern are constants,
        // and `f"…"` is a call to `format!`. The grammar is where that is said.
        rule f_str_lit -> Expr =
            s:FSTRING -> { Expr::LitInterpolated(s) }

        // Kap 2.2. Lexical, and the body is kept as written - a `'\n'` is two
        // characters here and one in the value, and the language below reads
        // the same two. Deciding what they mean would be decoding done twice.
        rule CHAR -> String =
            "'" c:CHAR_BODY "'" -> { c }

        rule CHAR_BODY -> String =
            "\\" c:any -> {
                let mut s = String::from("\\");
                s.push(c);
                s
            }
          | not("'") c:any -> { c.to_string() }

        rule char_lit -> Expr =
            c:CHAR -> { Expr::LitChar(c) }

        rule int_lit -> Expr =
            d:digits -> {
                Expr::LitInt(d.parse().unwrap())
            }

        rule digits -> String =
            d:digit1 -> { d.to_string() }
    }
}
