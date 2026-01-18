use crate::ast::{
    AsmBinding, Block, EnumVariant, Expr, FieldDef, FnArg, GenericParam, Item, MatchArm, Program,
    Stmt, Type,
};
use syn::parse::{Parse, ParseStream};
use syn::{braced, parenthesized, token, Ident, LitInt, LitStr, Result, Token};

// --- Custom Keywords definieren ---
mod kw {
    syn::custom_keyword!(spawn);
    syn::custom_keyword!(dsl);
    syn::custom_keyword!(grammar);
    syn::custom_keyword!(test);
    syn::custom_keyword!(bench);
    syn::custom_keyword!(sync);
    syn::custom_keyword!(asm);
    // Modifier für ASM
    syn::custom_keyword!(in);
    syn::custom_keyword!(out);
    syn::custom_keyword!(inout);
    syn::custom_keyword!(reg);
    syn::custom_keyword!(mem);
}

// --- Top-Level Parsing ---

impl Parse for Program {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut items = Vec::new();
        while !input.is_empty() {
            items.push(input.parse()?);
        }
        Ok(Program { items })
    }
}

impl Parse for Item {
    fn parse(input: ParseStream) -> Result<Self> {
        // Fall 1: use std::http
        if input.peek(Token![use]) {
            let _ = input.parse::<Token![use]>()?;
            // Vereinfacht: Wir lesen bis zum Semikolon oder Zeilenende
            let mut path = String::new();
            while !input.peek(Token![;]) && !input.is_empty() {
                let part: Ident = input.parse()?;
                path.push_str(&part.to_string());
                if input.peek(Token![::]) {
                    let _ = input.parse::<Token![::]>()?;
                    path.push_str("::");
                } else {
                    break;
                }
            }
            // Optionales Semikolon konsumieren
            if input.peek(Token![;]) {
                let _ = input.parse::<Token![;]>()?;
            }
            return Ok(Item::Import { path });
        }

        // Fall 2: fn name(...) { ... }
        if input.peek(Token![fn]) {
            let _ = input.parse::<Token![fn]>()?;
            let name: Ident = input.parse()?;
            
            // Generics: [T] (Optional)
            let generics = if input.peek(token::Bracket) {
                parse_generics(input)?
            } else {
                Vec::new()
            };

            // Argumente: (a: i32, b: i32)
            let content;
            let _ = parenthesized!(content in input);
            let args = content.parse_terminated(FnArg::parse, Token![,])?
                .into_iter().collect();

            // Sync Keyword (Optional)
            let is_sync = input.peek(kw::sync);
            if is_sync { let _ = input.parse::<kw::sync>()?; }

            // Return Type: -> Type (Optional)
            let ret_type = if input.peek(Token![->]) {
                let _ = input.parse::<Token![->]>()?;
                Some(input.parse()?)
            } else {
                None
            };

            let body: Block = input.parse()?;
            return Ok(Item::Fn { name, generics, args, ret_type, body, is_sync });
        }

        // Fall 3: struct Name { ... }
        if input.peek(Token![struct]) {
            let _ = input.parse::<Token![struct]>()?;
            let name: Ident = input.parse()?;
            let generics = if input.peek(token::Bracket) { parse_generics(input)? } else { Vec::new() };
            
            let content;
            let _ = braced!(content in input);
            let fields = content.parse_terminated(FieldDef::parse, Token![,])?
                .into_iter().collect();
                
            return Ok(Item::Struct { name, generics, fields });
        }

        // Fall 4: enum Name { ... }
        if input.peek(Token![enum]) {
            let _ = input.parse::<Token![enum]>()?;
            let name: Ident = input.parse()?;
            let generics = if input.peek(token::Bracket) { parse_generics(input)? } else { Vec::new() };
            
            let content;
            let _ = braced!(content in input);
            let variants = content.parse_terminated(EnumVariant::parse, Token![,])?
                .into_iter().collect();

            return Ok(Item::Enum { name, generics, variants });
        }

        // Fall 5: impl Type { ... }
        if input.peek(Token![impl]) {
            let _ = input.parse::<Token![impl]>()?;
            let target: Type = input.parse()?;
            
            let content;
            let _ = braced!(content in input);
            let mut methods = Vec::new();
            while !content.is_empty() {
                methods.push(content.parse()?);
            }
            return Ok(Item::Impl { target, methods });
        }

        // Fall 6: test "Name" { ... }
        if input.peek(kw::test) {
            let _ = input.parse::<kw::test>()?;
            let name: LitStr = input.parse()?;
            let body: Block = input.parse()?;
            return Ok(Item::Test { name: name.value(), body });
        }

        // Fall 7: grammar Name { ... }
        if input.peek(kw::grammar) {
            let _ = input.parse::<kw::grammar>()?;
            let name: Ident = input.parse()?;
            let content_buffer;
            let _ = braced!(content_buffer in input);
            let content = content_buffer.parse()?; // TokenStream
            return Ok(Item::Grammar { name, content });
        }

        Err(input.error("Unerwartetes Top-Level Item"))
    }
}

// --- Statement & Expression Parsing ---

impl Parse for Block {
    fn parse(input: ParseStream) -> Result<Self> {
        let content;
        let _ = braced!(content in input);
        let mut stmts = Vec::new();
        while !content.is_empty() {
            stmts.push(content.parse()?);
        }
        Ok(Block { stmts })
    }
}

impl Parse for Stmt {
    fn parse(input: ParseStream) -> Result<Self> {
        // let mut x = ...
        if input.peek(Token![let]) {
            let _ = input.parse::<Token![let]>()?;
            let mutable = input.peek(Token![mut]);
            if mutable { let _ = input.parse::<Token![mut]>()?; }
            
            let name: Ident = input.parse()?;
            
            // Optionaler Typ: let x: i32 = ...
            let ty = if input.peek(Token![:]) {
                let _ = input.parse::<Token![:]>()?;
                Some(input.parse()?)
            } else {
                None
            };

            let _ = input.parse::<Token![=]>()?;
            let value: Expr = input.parse()?;
            
            // Optionales Semikolon (Expression-Oriented)
            if input.peek(Token![;]) { let _ = input.parse::<Token![;]>()?; }
            
            return Ok(Stmt::Let { name, mutable, ty, value });
        }

        // Fallback: Expression Statement (evtl. Assignment)
        let expr: Expr = input.parse()?;
        
        // Check for Assignment: x = y
        if input.peek(Token![=]) {
            let _ = input.parse::<Token![=]>()?;
            let value: Expr = input.parse()?;
            if input.peek(Token![;]) { let _ = input.parse::<Token![;]>()?; }
            return Ok(Stmt::Assign { target: expr, value });
        }
        
        // Semikolon fressen falls vorhanden
        if input.peek(Token![;]) { let _ = input.parse::<Token![;]>()?; }

        Ok(Stmt::Expr(expr))
    }
}

impl Parse for Expr {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut expr = parse_atom(input)?;

        // Postfix-Operatoren behandeln (z.B. ?{...} Error Handling)
        loop {
            // ?{ ... } Syntax
            if input.peek(Token![?]) {
                let _ = input.parse::<Token![?]>()?;
                let handler: Block = input.parse()?;
                expr = Expr::TryCatch { expr: Box::new(expr), handler };
            } else {
                break;
            }
        }
        
        Ok(expr)
    }
}

// Parsen von atomaren Ausdrücken (höchste Priorität)
fn parse_atom(input: ParseStream) -> Result<Expr> {
    // 1. Block { ... }
    if input.peek(token::Brace) {
        return Ok(Expr::Block(input.parse()?));
    }

    // 2. if ...
    if input.peek(Token![if]) {
        let _ = input.parse::<Token![if]>()?;
        let cond = Box::new(input.parse()?);
        let then_branch: Block = input.parse()?;
        let else_branch = if input.peek(Token![else]) {
            let _ = input.parse::<Token![else]>()?;
            // else if ... rekursiv behandeln oder Block
            if input.peek(Token![if]) {
                let else_if: Expr = input.parse()?;
                // Wrap in Block
                Some(Block { stmts: vec![Stmt::Expr(else_if)] })
            } else {
                Some(input.parse()?)
            }
        } else {
            None
        };
        return Ok(Expr::If { cond, then_branch, else_branch });
    }

    // 3. spawn { ... } oder spawn(move { ... })
    if input.peek(kw::spawn) {
        let _ = input.parse::<kw::spawn>()?;
        // Check for parens: spawn({ ... }) -> Syntax aus Spec
        // Oder shorthand: spawn { ... }
        
        // Fall A: spawn(...)
        if input.peek(token::Paren) {
            let content;
            let _ = parenthesized!(content in input);
            // Check 'move'
            let is_move = if content.peek(Token![move]) {
                let _ = content.parse::<Token![move]>()?;
                true
            } else {
                false
            };
            let body: Expr = content.parse()?; // Erwartet Block-Expr
            return Ok(Expr::Spawn { body: Box::new(body), is_move });
        }
        
        // Fall B: spawn { ... }
        let body: Block = input.parse()?;
        return Ok(Expr::Spawn { body: Box::new(Expr::Block(body)), is_move: false });
    }

    // 4. unsafe asm { ... } { ... }
    if input.peek(Token![unsafe]) && input.peek2(kw::asm) {
        let _ = input.parse::<Token![unsafe]>()?;
        let _ = input.parse::<kw::asm>()?;
        
        let bindings_buffer;
        let _ = braced!(bindings_buffer in input);
        let bindings = bindings_buffer.parse_terminated(AsmBinding::parse, Token![,])?
            .into_iter().collect();

        let code_buffer;
        let _ = braced!(code_buffer in input);
        // Wir nehmen den Inhalt als String
        let code_stream: proc_macro2::TokenStream = code_buffer.parse()?;
        let code = code_stream.to_string(); // Vereinfacht für Stage 0

        return Ok(Expr::Asm { bindings, code });
    }

    // 5. dsl target context { ... }
    if input.peek(kw::dsl) {
        let _ = input.parse::<kw::dsl>()?;
        let target: Ident = input.parse()?;
        
        // Kontext ist optional. Wenn das nächste Token eine Brace ist, gibt es keinen Kontext.
        let context = if !input.peek(token::Brace) {
            Some(input.parse()?)
        } else {
            None
        };

        let content_buffer;
        let _ = braced!(content_buffer in input);
        let content = content_buffer.parse()?;
        
        return Ok(Expr::Dsl { target, context, content });
    }

    // 6. match expr { ... }
    if input.peek(Token![match]) {
        let _ = input.parse::<Token![match]>()?;
        let expr = Box::new(input.parse()?);
        let content;
        let _ = braced!(content in input);
        let arms = content.parse_terminated(MatchArm::parse, Token![,])?
            .into_iter().collect();
        return Ok(Expr::Match { expr, arms });
    }

    // 7. Literale & Variablen
    if input.peek(LitInt) {
        let lit: LitInt = input.parse()?;
        return Ok(Expr::LitInt(lit.base10_parse()?));
    }
    if input.peek(LitStr) {
        let lit: LitStr = input.parse()?;
        return Ok(Expr::LitStr(lit.value()));
    }
    if input.peek(Token![true]) {
        let _ = input.parse::<Token![true]>()?;
        return Ok(Expr::LitBool(true));
    }
    if input.peek(Token![false]) {
        let _ = input.parse::<Token![false]>()?;
        return Ok(Expr::LitBool(false));
    }

    // Variable oder Function Call
    if input.peek(Ident) {
        let name: Ident = input.parse()?;
        // Call: name(...)
        if input.peek(token::Paren) {
            let content;
            let _ = parenthesized!(content in input);
            let args = content.parse_terminated(Expr::parse, Token![,])?
                .into_iter().collect();
            return Ok(Expr::Call { func: Box::new(Expr::Variable(name)), args });
        }
        return Ok(Expr::Variable(name));
    }

    Err(input.error("Unerwarteter Ausdruck"))
}

// --- Helper Parser ---

fn parse_generics(input: ParseStream) -> Result<Vec<GenericParam>> {
    let content;
    let _ = syn::bracketed!(content in input);
    let params = content.parse_terminated(GenericParam::parse, Token![,])?
        .into_iter().collect();
    Ok(params)
}

impl Parse for Type {
    fn parse(input: ParseStream) -> Result<Self> {
        let name: Ident = input.parse()?;
        let generics = if input.peek(token::Bracket) {
            let content;
            let _ = syn::bracketed!(content in input);
            content.parse_terminated(Type::parse, Token![,])?
                .into_iter().collect()
        } else {
            Vec::new()
        };
        Ok(Type { name, generics })
    }
}

impl Parse for FnArg {
    fn parse(input: ParseStream) -> Result<Self> {
        let name: Ident = input.parse()?;
        let _ = input.parse::<Token![:]>()?;
        let ty: Type = input.parse()?;
        Ok(FnArg { name, ty })
    }
}

impl Parse for GenericParam {
    fn parse(input: ParseStream) -> Result<Self> {
        let name: Ident = input.parse()?;
        Ok(GenericParam { name })
    }
}

impl Parse for FieldDef {
    fn parse(input: ParseStream) -> Result<Self> {
        let name: Ident = input.parse()?;
        let _ = input.parse::<Token![:]>()?;
        let ty: Type = input.parse()?;
        Ok(FieldDef { name, ty })
    }
}

impl Parse for EnumVariant {
    fn parse(input: ParseStream) -> Result<Self> {
        let name: Ident = input.parse()?;
        let data = if input.peek(token::Brace) {
            let content;
            let _ = braced!(content in input);
            Some(content.parse_terminated(FieldDef::parse, Token![,])?
                .into_iter().collect())
        } else {
            None
        };
        Ok(EnumVariant { name, data })
    }
}

impl Parse for MatchArm {
    fn parse(input: ParseStream) -> Result<Self> {
        let pattern: Expr = input.parse()?; // Vereinfacht: Pattern ist Expr
        let _ = input.parse::<Token![=>]>()?;
        let body: Expr = input.parse()?;
        Ok(MatchArm { pattern, body })
    }
}

impl Parse for AsmBinding {
    fn parse(input: ParseStream) -> Result<Self> {
        // $dst = out(reg) result
        let _ = input.parse::<Token![$]>()?;
        let alias: Ident = input.parse()?;
        let _ = input.parse::<Token![=]>()?;
        
        // Direction: out / in / inout
        let direction_ident: Ident = input.parse()?;
        let direction = direction_ident.to_string();
        
        let content;
        let _ = parenthesized!(content in input);
        let loc_ident: Ident = content.parse()?;
        let location = loc_ident.to_string(); // reg / mem

        let variable: Ident = input.parse()?;
        Ok(AsmBinding { alias, direction, location, variable })
    }
}

