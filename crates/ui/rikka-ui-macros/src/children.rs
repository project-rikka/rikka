use proc_macro2::{TokenStream, TokenTree};
use quote::quote;
use syn::parse::{ParseStream, Parser};
use syn::{Block, Expr, ExprBlock, ExprForLoop, ExprIf, ExprMatch, ExprWhile, Stmt};

pub(crate) fn expand(input: TokenStream) -> syn::Result<TokenStream> {
    let mut tokens = input.into_iter();
    let mut krate = TokenStream::new();
    for token in tokens.by_ref() {
        match &token {
            TokenTree::Punct(punct) if punct.as_char() == ';' => break,
            _ => krate.extend([token]),
        }
    }
    let body: TokenStream = tokens.collect();
    let statements = Parser::parse2(|input: ParseStream<'_>| Block::parse_within(input), body)?;
    let pushes = Pushes { krate: &krate }.statements(&statements, 0);
    if !mentions_children(pushes.clone()) {
        return Ok(quote! {{
            #pushes
            #krate::Children::new()
        }});
    }
    Ok(quote! {{
        let mut __children = #krate::Children::new();
        #pushes
        __children
    }})
}

struct Pushes<'a> {
    krate: &'a TokenStream,
}

impl Pushes<'_> {
    fn statements(&self, statements: &[Stmt], salt: u64) -> TokenStream {
        statements
            .iter()
            .enumerate()
            .map(|(index, statement)| self.statement(statement, salt_of(salt, index as u64)))
            .collect()
    }

    fn statement(&self, statement: &Stmt, salt: u64) -> TokenStream {
        match statement {
            Stmt::Local(_) | Stmt::Item(_) => quote!(#statement),
            Stmt::Expr(expr, _) => self.expression(expr, salt),
            Stmt::Macro(called) => {
                let called = &called.mac;
                quote!(__children.push(#called);)
            }
        }
    }

    fn expression(&self, expr: &Expr, salt: u64) -> TokenStream {
        match expr {
            Expr::If(branching) => self.branching(branching, salt),
            Expr::ForLoop(looping) => self.looping(looping, salt),
            Expr::While(repeating) => self.repeating(repeating, salt),
            Expr::Match(matching) => self.matching(matching, salt),
            Expr::Block(ExprBlock {
                block, label: None, ..
            }) => {
                let inner = self.statements(&block.stmts, salt);
                quote!({ #inner })
            }
            other => quote!(__children.push(#other);),
        }
    }

    fn branch(&self, block: &Block, salt: u64, branch: u64) -> TokenStream {
        let id = salt_of(salt, branch);
        let inner = self.statements(&block.stmts, id);
        quote! {{
            let __mark = __children.mark();
            #inner
            __children.brand(__mark, #id);
        }}
    }

    fn branching(&self, branching: &ExprIf, salt: u64) -> TokenStream {
        let mut arms = Vec::new();
        let mut current = branching;
        let mut index = 0;
        let fallback = loop {
            let condition = &current.cond;
            let body = self.branch(&current.then_branch, salt, index);
            arms.push(quote!(if #condition #body));
            index += 1;
            match current.else_branch.as_ref().map(|(_, other)| &**other) {
                Some(Expr::If(next)) => current = next,
                Some(Expr::Block(ExprBlock { block, .. })) => {
                    break Some(self.branch(block, salt, index));
                }
                Some(other) => {
                    return syn::Error::new_spanned(other, "expected a block after `else`")
                        .to_compile_error();
                }
                None => break None,
            }
        };
        let mut chained = TokenStream::new();
        for (position, arm) in arms.into_iter().enumerate() {
            if position > 0 {
                chained.extend(quote!(else));
            }
            chained.extend(arm);
        }
        if let Some(fallback) = fallback {
            chained.extend(quote!(else #fallback));
        }
        chained
    }

    fn looping(&self, looping: &ExprForLoop, salt: u64) -> TokenStream {
        let ExprForLoop {
            label,
            pat,
            expr,
            body,
            ..
        } = looping;
        let inner = self.statements(&body.stmts, salt);
        quote!(#label for #pat in #expr { #inner })
    }

    fn repeating(&self, repeating: &ExprWhile, salt: u64) -> TokenStream {
        let ExprWhile {
            label, cond, body, ..
        } = repeating;
        let inner = self.statements(&body.stmts, salt);
        quote!(#label while #cond { #inner })
    }

    fn matching(&self, matching: &ExprMatch, salt: u64) -> TokenStream {
        let krate = self.krate;
        let scrutinee = &matching.expr;
        let arms = matching.arms.iter().enumerate().map(|(index, arm)| {
            let pattern = &arm.pat;
            let guard = arm.guard.as_ref().map(|(_, guard)| quote!(if #guard));
            let body = &arm.body;
            let id = salt_of(salt, index as u64);
            quote!(#pattern #guard => #krate::Children::branded(#body, #id),)
        });
        quote!(__children.push(match #scrutinee { #(#arms)* });)
    }
}

fn mentions_children(stream: TokenStream) -> bool {
    stream.into_iter().any(|token| match token {
        TokenTree::Ident(ident) => ident == "__children",
        TokenTree::Group(group) => mentions_children(group.stream()),
        TokenTree::Punct(_) | TokenTree::Literal(_) => false,
    })
}

fn salt_of(salt: u64, index: u64) -> u64 {
    salt.wrapping_mul(0x100_0000_01b3).wrapping_add(index + 1)
}
