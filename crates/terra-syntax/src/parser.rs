use crate::ast::*;
use crate::token::{Token, TokenKind};
use terra_diagnostics::{DiagKind, Diagnostic, Span};

pub fn parse(file: u32, tokens: &[Token]) -> Result<File, Diagnostic> {
    let mut p = Parser {
        tokens,
        i: 0,
        file,
        last_end: 0,
    };
    p.parse_file()
}

struct Parser<'a> {
    tokens: &'a [Token],
    i: usize,
    file: u32,
    last_end: u32,
}

impl<'a> Parser<'a> {
    fn parse_file(&mut self) -> Result<File, Diagnostic> {
        let package = if self.eat(|k| matches!(k, TokenKind::Package)) {
            let path = self.parse_path()?;
            self.expect_semi()?;
            Some(path)
        } else {
            None
        };
        let mut uses = Vec::new();
        while self.eat(|k| matches!(k, TokenKind::Use)) {
            let path = self.parse_path()?;
            self.expect_semi()?;
            uses.push(path);
        }
        let mut items = Vec::new();
        while !self.at(|k| matches!(k, TokenKind::Eof)) {
            if self.eat(|k| matches!(k, TokenKind::Semi)) {
                continue;
            }
            items.push(self.parse_item(false)?);
        }
        Ok(File {
            package,
            uses,
            items,
        })
    }

    fn parse_item(&mut self, in_class: bool) -> Result<Item, Diagnostic> {
        let start = self.pos();
        let (vis, mods) = self.parse_leading()?;
        if self.eat(|k| matches!(k, TokenKind::Class)) {
            if in_class {
                return Err(self.err_here(DiagKind::NestedType));
            }
            return self.finish_class(vis, mods, start);
        }
        if self.eat(|k| matches!(k, TokenKind::Interface)) {
            if in_class {
                return Err(self.err_here(DiagKind::NestedType));
            }
            return self.finish_interface(vis, mods, start);
        }
        if self.eat(|k| matches!(k, TokenKind::Enum)) {
            if in_class {
                return Err(self.err_here(DiagKind::NestedType));
            }
            return self.finish_enum(vis, mods, start);
        }
        if self.eat(|k| matches!(k, TokenKind::Exception)) {
            if in_class {
                return Err(self.err_here(DiagKind::NestedType));
            }
            let name = self.expect_ident()?;
            if self.eat(|k| matches!(k, TokenKind::LBrace)) {
                if !self.at(|k| matches!(k, TokenKind::RBrace)) {
                    return Err(self.err_here(DiagKind::InterfaceMember));
                }
                self.bump();
            } else {
                self.expect_semi()?;
            }
            return Ok(Item {
                vis,
                mods,
                kind: ItemKind::Exception { name },
                span: self.span_from(start),
            });
        }
        if self.eat(|k| matches!(k, TokenKind::Func)) {
            let func = self.parse_function(false)?;
            return Ok(Item {
                vis,
                mods,
                kind: ItemKind::Function(func),
                span: self.span_from(start),
            });
        }
        if self.at(|k| matches!(k, TokenKind::Const | TokenKind::Var | TokenKind::Mut)) {
            if !in_class && !self.at(|k| matches!(k, TokenKind::Const)) {
                return Err(self.err_here(DiagKind::TopLevelBinding));
            }
            let field = self.parse_field()?;
            return Ok(Item {
                vis,
                mods,
                kind: ItemKind::Field(field),
                span: self.span_from(start),
            });
        }
        Err(self.unexpected("declaration"))
    }

    fn finish_class(&mut self, vis: Vis, mods: Mods, start: u32) -> Result<Item, Diagnostic> {
        let name = self.expect_ident()?;
        let extends = if self.eat(|k| matches!(k, TokenKind::Extends)) {
            Some(self.parse_type()?)
        } else {
            None
        };
        let mut implements = Vec::new();
        if self.eat(|k| matches!(k, TokenKind::Implements)) {
            loop {
                implements.push(self.parse_type()?);
                if !self.eat(|k| matches!(k, TokenKind::Comma)) {
                    break;
                }
            }
        }
        self.expect(|k| matches!(k, TokenKind::LBrace), "'{'")?;
        let mut members = Vec::new();
        while !self.at(|k| matches!(k, TokenKind::RBrace | TokenKind::Eof)) {
            if self.eat(|k| matches!(k, TokenKind::Semi)) {
                continue;
            }
            members.push(self.parse_item(true)?);
        }
        self.expect(|k| matches!(k, TokenKind::RBrace), "'}'")?;
        Ok(Item {
            vis,
            mods,
            kind: ItemKind::Class(Class {
                name,
                extends,
                implements,
                members,
            }),
            span: self.span_from(start),
        })
    }

    fn finish_interface(&mut self, vis: Vis, mods: Mods, start: u32) -> Result<Item, Diagnostic> {
        let name = self.expect_ident()?;
        let mut extends = Vec::new();
        if self.eat(|k| matches!(k, TokenKind::Extends)) {
            loop {
                extends.push(self.parse_type()?);
                if !self.eat(|k| matches!(k, TokenKind::Comma)) {
                    break;
                }
            }
        }
        self.expect(|k| matches!(k, TokenKind::LBrace), "'{'")?;
        let mut methods = Vec::new();
        while !self.at(|k| matches!(k, TokenKind::RBrace | TokenKind::Eof)) {
            if self.eat(|k| matches!(k, TokenKind::Semi)) {
                continue;
            }
            let _ = self.parse_leading()?;
            if !self.eat(|k| matches!(k, TokenKind::Func)) {
                return Err(self.err_here(DiagKind::InterfaceMember));
            }
            methods.push(self.parse_function(true)?);
        }
        self.expect(|k| matches!(k, TokenKind::RBrace), "'}'")?;
        Ok(Item {
            vis,
            mods,
            kind: ItemKind::Interface(Interface {
                name,
                extends,
                methods,
            }),
            span: self.span_from(start),
        })
    }

    fn finish_enum(&mut self, vis: Vis, mods: Mods, start: u32) -> Result<Item, Diagnostic> {
        let name = self.expect_ident()?;
        self.expect(|k| matches!(k, TokenKind::LBrace), "'{'")?;
        let mut variants = Vec::new();
        while !self.at(|k| matches!(k, TokenKind::RBrace | TokenKind::Eof)) {
            if self.eat(|k| matches!(k, TokenKind::Comma | TokenKind::Semi)) {
                continue;
            }
            let span = self.peek().span;
            let vname = self.expect_ident()?;
            variants.push((vname, span));
            if self.at(|k| matches!(k, TokenKind::RBrace)) {
                break;
            }
            if !self.eat(|k| matches!(k, TokenKind::Comma)) {
                return Err(self.unexpected("',' or '}'"));
            }
        }
        self.expect(|k| matches!(k, TokenKind::RBrace), "'}'")?;
        Ok(Item {
            vis,
            mods,
            kind: ItemKind::Enum(EnumDecl { name, variants }),
            span: self.span_from(start),
        })
    }

    fn parse_function(&mut self, in_interface: bool) -> Result<Function, Diagnostic> {
        let start = self.prev_start();
        let name = self.expect_ident()?;
        self.expect(|k| matches!(k, TokenKind::LParen), "'('")?;
        let mut params = Vec::new();
        if !self.at(|k| matches!(k, TokenKind::RParen)) {
            loop {
                let pstart = self.pos();
                let pname = self.expect_ident()?;
                self.expect(|k| matches!(k, TokenKind::Colon), "':'")?;
                let ty = self.parse_type()?;
                params.push(Param {
                    name: pname,
                    ty,
                    span: self.span_from(pstart),
                });
                if !self.eat(|k| matches!(k, TokenKind::Comma)) {
                    break;
                }
                if self.at(|k| matches!(k, TokenKind::RParen)) {
                    break;
                }
            }
        }
        self.expect(|k| matches!(k, TokenKind::RParen), "')'")?;
        let ret = if self.eat(|k| matches!(k, TokenKind::Arrow)) {
            Some(self.parse_type()?)
        } else {
            None
        };
        let body = if in_interface {
            self.expect_semi()?;
            None
        } else if self.at(|k| matches!(k, TokenKind::LBrace)) {
            Some(self.parse_block()?)
        } else {
            return Err(self.err_here(DiagKind::MissingBody { name: name.clone() }));
        };
        Ok(Function {
            name,
            params,
            ret,
            body,
            span: self.span_from(start),
        })
    }

    fn parse_field(&mut self) -> Result<Field, Diagnostic> {
        let start = self.pos();
        let mutable = if self.eat(|k| matches!(k, TokenKind::Const)) {
            Mutability::Const
        } else if self.eat(|k| matches!(k, TokenKind::Var)) {
            Mutability::Var
        } else if self.eat(|k| matches!(k, TokenKind::Mut)) {
            Mutability::Mut
        } else {
            return Err(self.unexpected("const, var, or mut"));
        };
        let name = self.expect_ident()?;
        let ty = if self.eat(|k| matches!(k, TokenKind::Colon)) {
            Some(self.parse_type()?)
        } else {
            None
        };
        let init = if self.eat(|k| matches!(k, TokenKind::Eq)) {
            Some(self.parse_expr()?)
        } else {
            None
        };
        self.expect_semi()?;
        Ok(Field {
            name,
            mutable,
            ty,
            init,
            span: self.span_from(start),
        })
    }

    fn parse_leading(&mut self) -> Result<(Vis, Mods), Diagnostic> {
        let mut vis = Vis::Default;
        let mut mods = Mods::default();
        loop {
            if self.eat(|k| matches!(k, TokenKind::Pub)) {
                if vis != Vis::Default {
                    return Err(self.err_here(DiagKind::BadModifier {
                        detail: "repeated visibility".into(),
                    }));
                }
                vis = Vis::Pub;
            } else if self.eat(|k| matches!(k, TokenKind::Priv)) {
                if vis != Vis::Default {
                    return Err(self.err_here(DiagKind::BadModifier {
                        detail: "repeated visibility".into(),
                    }));
                }
                vis = Vis::Priv;
            } else if self.eat(|k| matches!(k, TokenKind::Static)) {
                mods.is_static = true;
            } else if self.eat(|k| matches!(k, TokenKind::Final)) {
                mods.is_final = true;
            } else if self.eat(|k| matches!(k, TokenKind::Async)) {
                mods.is_async = true;
            } else if self.eat(|k| matches!(k, TokenKind::Override)) {
                mods.is_override = true;
            } else {
                break;
            }
        }
        Ok((vis, mods))
    }

    fn parse_block(&mut self) -> Result<Block, Diagnostic> {
        let start = self.pos();
        self.expect(|k| matches!(k, TokenKind::LBrace), "'{'")?;
        self.parse_block_body(start)
    }

    fn parse_block_body(&mut self, start: u32) -> Result<Block, Diagnostic> {
        let mut stmts = Vec::new();
        while !self.at(|k| matches!(k, TokenKind::RBrace | TokenKind::Eof)) {
            stmts.push(self.parse_stmt()?);
        }
        self.expect(|k| matches!(k, TokenKind::RBrace), "'}'")?;
        Ok(Block {
            stmts,
            span: self.span_from(start),
        })
    }

    fn parse_stmt(&mut self) -> Result<Stmt, Diagnostic> {
        if self.eat(|k| matches!(k, TokenKind::Semi)) {
            return Ok(Stmt::Empty);
        }
        let start = self.pos();
        if self.at(|k| matches!(k, TokenKind::LBrace)) {
            return Ok(Stmt::Block(self.parse_block()?));
        }
        if self.eat(|k| matches!(k, TokenKind::If)) {
            return self.parse_if(start);
        }
        if self.eat(|k| matches!(k, TokenKind::While)) {
            self.expect(|k| matches!(k, TokenKind::LParen), "'('")?;
            let cond = self.parse_expr()?;
            self.expect(|k| matches!(k, TokenKind::RParen), "')'")?;
            let body = self.parse_block()?;
            return Ok(Stmt::While {
                cond,
                body,
                span: self.span_from(start),
            });
        }
        if self.eat(|k| matches!(k, TokenKind::Loop)) {
            let body = self.parse_block()?;
            return Ok(Stmt::Loop {
                body,
                span: self.span_from(start),
            });
        }
        if self.eat(|k| matches!(k, TokenKind::For)) {
            return self.parse_for(start);
        }
        if self.eat(|k| matches!(k, TokenKind::Do)) {
            let body = self.parse_block()?;
            self.expect(|k| matches!(k, TokenKind::While), "'while'")?;
            self.expect(|k| matches!(k, TokenKind::LParen), "'('")?;
            let cond = self.parse_expr()?;
            self.expect(|k| matches!(k, TokenKind::RParen), "')'")?;
            self.expect_semi()?;
            return Ok(Stmt::DoWhile {
                body,
                cond,
                span: self.span_from(start),
            });
        }
        if self.eat(|k| matches!(k, TokenKind::Match)) {
            return self.parse_match(start);
        }
        if self.eat(|k| matches!(k, TokenKind::Try)) {
            return self.parse_try(start);
        }
        if self.eat(|k| matches!(k, TokenKind::Return)) {
            let value = if self.at(|k| matches!(k, TokenKind::Semi)) {
                None
            } else {
                Some(self.parse_expr()?)
            };
            self.expect_semi()?;
            return Ok(Stmt::Return {
                value,
                span: self.span_from(start),
            });
        }
        if self.eat(|k| matches!(k, TokenKind::Break)) {
            self.expect_semi()?;
            return Ok(Stmt::Break {
                span: self.span_from(start),
            });
        }
        if self.eat(|k| matches!(k, TokenKind::Continue)) {
            self.expect_semi()?;
            return Ok(Stmt::Continue {
                span: self.span_from(start),
            });
        }
        if self.eat(|k| matches!(k, TokenKind::Throw)) {
            let value = self.parse_expr()?;
            self.expect_semi()?;
            return Ok(Stmt::Throw {
                value,
                span: self.span_from(start),
            });
        }
        if self.eat(|k| matches!(k, TokenKind::Delete)) {
            let value = self.parse_expr()?;
            self.expect_semi()?;
            return Ok(Stmt::Delete {
                value,
                span: self.span_from(start),
            });
        }
        if self.at(|k| matches!(k, TokenKind::Const | TokenKind::Var | TokenKind::Mut)) {
            let field = self.parse_field()?;
            return Ok(Stmt::Local {
                mutable: field.mutable,
                name: field.name,
                ty: field.ty,
                init: field.init,
                span: field.span,
            });
        }
        let expr = self.parse_expr()?;
        self.expect_semi()?;
        Ok(Stmt::Expr {
            expr,
            span: self.span_from(start),
        })
    }

    fn parse_if(&mut self, start: u32) -> Result<Stmt, Diagnostic> {
        self.expect(|k| matches!(k, TokenKind::LParen), "'('")?;
        let cond = self.parse_expr()?;
        self.expect(|k| matches!(k, TokenKind::RParen), "')'")?;
        let then_branch = self.parse_block()?;
        let else_branch = if self.eat(|k| matches!(k, TokenKind::Else)) {
            if self.eat(|k| matches!(k, TokenKind::If)) {
                Some(Box::new(self.parse_if(self.prev_start())?))
            } else {
                Some(Box::new(Stmt::Block(self.parse_block()?)))
            }
        } else {
            None
        };
        Ok(Stmt::If {
            cond,
            then_branch,
            else_branch,
            span: self.span_from(start),
        })
    }

    fn parse_for(&mut self, start: u32) -> Result<Stmt, Diagnostic> {
        self.expect(|k| matches!(k, TokenKind::LParen), "'('")?;
        if self.at_ident_in() {
            let name = self.expect_ident()?;
            self.bump();
            let iter = self.parse_expr()?;
            self.expect(|k| matches!(k, TokenKind::RParen), "')'")?;
            let body = self.parse_block()?;
            return Ok(Stmt::ForIn {
                name,
                iter,
                body,
                span: self.span_from(start),
            });
        }
        let init = if self.at(|k| matches!(k, TokenKind::Semi)) {
            None
        } else if self.at(|k| matches!(k, TokenKind::Const | TokenKind::Var | TokenKind::Mut)) {
            let local_start = self.pos();
            let mutable = if self.eat(|k| matches!(k, TokenKind::Const)) {
                Mutability::Const
            } else if self.eat(|k| matches!(k, TokenKind::Var)) {
                Mutability::Var
            } else {
                self.bump();
                Mutability::Mut
            };
            let name = self.expect_ident()?;
            let ty = if self.eat(|k| matches!(k, TokenKind::Colon)) {
                Some(self.parse_type()?)
            } else {
                None
            };
            let init_expr = if self.eat(|k| matches!(k, TokenKind::Eq)) {
                Some(self.parse_expr()?)
            } else {
                None
            };
            Some(Box::new(Stmt::Local {
                mutable,
                name,
                ty,
                init: init_expr,
                span: self.span_from(local_start),
            }))
        } else {
            let expr = self.parse_expr()?;
            Some(Box::new(Stmt::Expr {
                span: expr.span,
                expr,
            }))
        };
        self.expect(|k| matches!(k, TokenKind::Semi), "';'")?;
        let cond = if self.at(|k| matches!(k, TokenKind::Semi)) {
            None
        } else {
            Some(self.parse_expr()?)
        };
        self.expect(|k| matches!(k, TokenKind::Semi), "';'")?;
        let step = if self.at(|k| matches!(k, TokenKind::RParen)) {
            None
        } else {
            Some(self.parse_expr()?)
        };
        self.expect(|k| matches!(k, TokenKind::RParen), "')'")?;
        let body = self.parse_block()?;
        Ok(Stmt::ForC {
            init,
            cond,
            step,
            body,
            span: self.span_from(start),
        })
    }

    fn parse_match(&mut self, start: u32) -> Result<Stmt, Diagnostic> {
        self.expect(|k| matches!(k, TokenKind::LParen), "'('")?;
        let scrut = self.parse_expr()?;
        self.expect(|k| matches!(k, TokenKind::RParen), "')'")?;
        self.expect(|k| matches!(k, TokenKind::LBrace), "'{'")?;
        let mut arms = Vec::new();
        while !self.at(|k| matches!(k, TokenKind::RBrace | TokenKind::Eof)) {
            if self.eat(|k| matches!(k, TokenKind::Comma | TokenKind::Semi)) {
                continue;
            }
            let astart = self.pos();
            let pattern = if self.eat(|k| matches!(k, TokenKind::Underscore)) {
                Pattern::Wildcard {
                    span: self.span_from(astart),
                }
            } else {
                Pattern::Expr(self.parse_expr()?)
            };
            self.expect(|k| matches!(k, TokenKind::FatArrow), "'=>'")?;
            let body = self.parse_block()?;
            arms.push(MatchArm {
                pattern,
                body,
                span: self.span_from(astart),
            });
            let _ = self.eat(|k| matches!(k, TokenKind::Comma));
        }
        self.expect(|k| matches!(k, TokenKind::RBrace), "'}'")?;
        Ok(Stmt::Match {
            scrut,
            arms,
            span: self.span_from(start),
        })
    }

    fn parse_try(&mut self, start: u32) -> Result<Stmt, Diagnostic> {
        let body = self.parse_block()?;
        let mut catches = Vec::new();
        while self.eat(|k| matches!(k, TokenKind::Exception)) {
            let cstart = self.prev_start();
            self.expect(|k| matches!(k, TokenKind::LParen), "'('")?;
            let name = self.expect_ident()?;
            self.expect(|k| matches!(k, TokenKind::Colon), "':'")?;
            let ty = self.parse_type()?;
            self.expect(|k| matches!(k, TokenKind::RParen), "')'")?;
            let cbody = self.parse_block()?;
            catches.push(Catch {
                name,
                ty,
                body: cbody,
                span: self.span_from(cstart),
            });
        }
        if catches.is_empty() {
            return Err(self.err_here(DiagKind::MissingHandler));
        }
        Ok(Stmt::Try {
            body,
            catches,
            span: self.span_from(start),
        })
    }

    fn parse_type(&mut self) -> Result<TypeRef, Diagnostic> {
        let start = self.pos();
        let mut path = vec![self.expect_ident()?];
        while self.eat(|k| matches!(k, TokenKind::Dot)) {
            path.push(self.expect_ident()?);
        }
        let mut dims = 0u8;
        while self.at(|k| matches!(k, TokenKind::LBracket))
            && self.lookahead(|k| matches!(k, TokenKind::RBracket))
        {
            self.bump();
            self.bump();
            dims = dims.saturating_add(1);
        }
        Ok(TypeRef {
            path,
            dims,
            span: self.span_from(start),
        })
    }

    fn parse_path(&mut self) -> Result<PathName, Diagnostic> {
        let start = self.pos();
        let mut parts = vec![self.expect_ident()?];
        while self.eat(|k| matches!(k, TokenKind::Dot)) {
            parts.push(self.expect_ident()?);
        }
        Ok(PathName {
            parts,
            span: self.span_from(start),
        })
    }

    fn parse_expr(&mut self) -> Result<Expr, Diagnostic> {
        self.parse_assign()
    }

    fn parse_assign(&mut self) -> Result<Expr, Diagnostic> {
        let start = self.pos();
        let left = self.parse_range()?;
        if self.eat(|k| matches!(k, TokenKind::Eq)) {
            let right = self.parse_assign()?;
            Ok(Expr {
                kind: ExprKind::Assign {
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span: self.span_from(start),
            })
        } else {
            Ok(left)
        }
    }

    fn parse_range(&mut self) -> Result<Expr, Diagnostic> {
        let start = self.pos();
        let left = self.parse_or()?;
        if self.eat(|k| matches!(k, TokenKind::DotDot)) {
            let right = self.parse_or()?;
            Ok(Expr {
                kind: ExprKind::Range {
                    lo: Box::new(left),
                    hi: Box::new(right),
                },
                span: self.span_from(start),
            })
        } else {
            Ok(left)
        }
    }

    fn parse_or(&mut self) -> Result<Expr, Diagnostic> {
        self.parse_bin(|k| matches!(k, TokenKind::OrOr), BinaryOp::Or, |p| p.parse_and())
    }

    fn parse_and(&mut self) -> Result<Expr, Diagnostic> {
        self.parse_bin(|k| matches!(k, TokenKind::AndAnd), BinaryOp::And, |p| p.parse_eq())
    }

    fn parse_eq(&mut self) -> Result<Expr, Diagnostic> {
        let start = self.pos();
        let mut left = self.parse_rel()?;
        loop {
            let op = if self.eat(|k| matches!(k, TokenKind::EqEq)) {
                BinaryOp::Eq
            } else if self.eat(|k| matches!(k, TokenKind::NotEq)) {
                BinaryOp::Ne
            } else {
                break;
            };
            let right = self.parse_rel()?;
            left = Expr {
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span: self.span_from(start),
            };
        }
        Ok(left)
    }

    fn parse_rel(&mut self) -> Result<Expr, Diagnostic> {
        let start = self.pos();
        let mut left = self.parse_add()?;
        loop {
            let op = if self.eat(|k| matches!(k, TokenKind::Lt)) {
                BinaryOp::Lt
            } else if self.eat(|k| matches!(k, TokenKind::Gt)) {
                BinaryOp::Gt
            } else if self.eat(|k| matches!(k, TokenKind::Le)) {
                BinaryOp::Le
            } else if self.eat(|k| matches!(k, TokenKind::Ge)) {
                BinaryOp::Ge
            } else {
                break;
            };
            let right = self.parse_add()?;
            left = Expr {
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span: self.span_from(start),
            };
        }
        Ok(left)
    }

    fn parse_add(&mut self) -> Result<Expr, Diagnostic> {
        let start = self.pos();
        let mut left = self.parse_mul()?;
        loop {
            let op = if self.eat(|k| matches!(k, TokenKind::Plus)) {
                BinaryOp::Add
            } else if self.eat(|k| matches!(k, TokenKind::Minus)) {
                BinaryOp::Sub
            } else {
                break;
            };
            let right = self.parse_mul()?;
            left = Expr {
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span: self.span_from(start),
            };
        }
        Ok(left)
    }

    fn parse_mul(&mut self) -> Result<Expr, Diagnostic> {
        let start = self.pos();
        let mut left = self.parse_unary()?;
        loop {
            let op = if self.eat(|k| matches!(k, TokenKind::Star)) {
                BinaryOp::Mul
            } else if self.eat(|k| matches!(k, TokenKind::Slash)) {
                BinaryOp::Div
            } else if self.eat(|k| matches!(k, TokenKind::Percent)) {
                BinaryOp::Mod
            } else {
                break;
            };
            let right = self.parse_unary()?;
            left = Expr {
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span: self.span_from(start),
            };
        }
        Ok(left)
    }

    fn parse_bin(
        &mut self,
        pred: fn(&TokenKind) -> bool,
        op: BinaryOp,
        mut lower: impl FnMut(&mut Self) -> Result<Expr, Diagnostic>,
    ) -> Result<Expr, Diagnostic> {
        let start = self.pos();
        let mut left = lower(self)?;
        while self.eat(pred) {
            let right = lower(self)?;
            left = Expr {
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span: self.span_from(start),
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, Diagnostic> {
        let start = self.pos();
        if self.eat(|k| matches!(k, TokenKind::Not)) {
            let expr = self.parse_unary()?;
            return Ok(Expr {
                kind: ExprKind::Unary {
                    op: UnaryOp::Not,
                    expr: Box::new(expr),
                },
                span: self.span_from(start),
            });
        }
        if self.eat(|k| matches!(k, TokenKind::Minus)) {
            let expr = self.parse_unary()?;
            return Ok(Expr {
                kind: ExprKind::Unary {
                    op: UnaryOp::Neg,
                    expr: Box::new(expr),
                },
                span: self.span_from(start),
            });
        }
        if self.eat(|k| matches!(k, TokenKind::Await)) {
            let expr = self.parse_unary()?;
            return Ok(Expr {
                kind: ExprKind::Await { expr: Box::new(expr) },
                span: self.span_from(start),
            });
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Expr, Diagnostic> {
        let start = self.pos();
        let mut expr = self.parse_primary()?;
        loop {
            if self.eat(|k| matches!(k, TokenKind::Dot)) {
                let name = self.expect_ident()?;
                expr = Expr {
                    kind: ExprKind::Field {
                        recv: Box::new(expr),
                        name,
                    },
                    span: self.span_from(start),
                };
            } else if self.eat(|k| matches!(k, TokenKind::LBracket)) {
                let index = self.parse_expr()?;
                self.expect(|k| matches!(k, TokenKind::RBracket), "']'")?;
                expr = Expr {
                    kind: ExprKind::Index {
                        recv: Box::new(expr),
                        index: Box::new(index),
                    },
                    span: self.span_from(start),
                };
            } else if self.eat(|k| matches!(k, TokenKind::LParen)) {
                let args = self.parse_args()?;
                expr = Expr {
                    kind: ExprKind::Call {
                        callee: Box::new(expr),
                        args,
                    },
                    span: self.span_from(start),
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, Diagnostic> {
        let start = self.pos();
        let tok_span = self.peek().span;
        if self.eat(|k| matches!(k, TokenKind::Int(_))) {
            let TokenKind::Int(n) = self.previous_kind() else {
                unreachable!()
            };
            return Ok(self.expr(ExprKind::Int(n), start));
        }
        if self.eat(|k| matches!(k, TokenKind::Float(_))) {
            let TokenKind::Float(n) = self.previous_kind() else {
                unreachable!()
            };
            return Ok(self.expr(ExprKind::Float(n), start));
        }
        if self.eat(|k| matches!(k, TokenKind::Double(_))) {
            let TokenKind::Double(n) = self.previous_kind() else {
                unreachable!()
            };
            return Ok(self.expr(ExprKind::Double(n), start));
        }
        if self.eat(|k| matches!(k, TokenKind::StringLit(_))) {
            let TokenKind::StringLit(s) = self.previous_kind() else {
                unreachable!()
            };
            return Ok(self.expr(ExprKind::Str(s), start));
        }
        if self.eat(|k| matches!(k, TokenKind::True)) {
            return Ok(self.expr(ExprKind::Bool(true), start));
        }
        if self.eat(|k| matches!(k, TokenKind::False)) {
            return Ok(self.expr(ExprKind::Bool(false), start));
        }
        if self.eat(|k| matches!(k, TokenKind::Null)) {
            return Ok(self.expr(ExprKind::Null, start));
        }
        if self.eat(|k| matches!(k, TokenKind::SelfKw)) {
            return Ok(self.expr(ExprKind::SelfRef, start));
        }
        if self.eat(|k| matches!(k, TokenKind::Super)) {
            if self.eat(|k| matches!(k, TokenKind::LParen)) {
                let args = self.parse_args()?;
                return Ok(self.expr(ExprKind::SuperCall { args }, start));
            }
            self.expect(|k| matches!(k, TokenKind::Dot), "'.' or '('")?;
            let name = self.expect_ident()?;
            return Ok(self.expr(ExprKind::SuperField { name }, start));
        }
        if self.eat(|k| matches!(k, TokenKind::New)) {
            return self.parse_new(start);
        }
        if self.eat(|k| matches!(k, TokenKind::Ident(_))) {
            let TokenKind::Ident(name) = self.previous_kind() else {
                unreachable!()
            };
            return Ok(self.expr(ExprKind::Name(name), start));
        }
        if self.eat(|k| matches!(k, TokenKind::LParen)) {
            let expr = self.parse_expr()?;
            self.expect(|k| matches!(k, TokenKind::RParen), "')'")?;
            return Ok(Expr {
                kind: expr.kind,
                span: Span::new(self.file, start, self.last_end),
            });
        }
        if self.eat(|k| matches!(k, TokenKind::LBracket)) {
            let mut elems = Vec::new();
            if !self.at(|k| matches!(k, TokenKind::RBracket)) {
                loop {
                    elems.push(self.parse_expr()?);
                    if !self.eat(|k| matches!(k, TokenKind::Comma)) {
                        break;
                    }
                    if self.at(|k| matches!(k, TokenKind::RBracket)) {
                        break;
                    }
                }
            }
            self.expect(|k| matches!(k, TokenKind::RBracket), "']'")?;
            return Ok(self.expr(ExprKind::Array { elems }, start));
        }
        let _ = tok_span;
        Err(self.unexpected("expression"))
    }

    fn parse_new(&mut self, start: u32) -> Result<Expr, Diagnostic> {
        let mut path = vec![self.expect_ident()?];
        while self.eat(|k| matches!(k, TokenKind::Dot)) {
            path.push(self.expect_ident()?);
        }
        let mut dims = 0u8;
        while self.at(|k| matches!(k, TokenKind::LBracket))
            && self.lookahead(|k| matches!(k, TokenKind::RBracket))
        {
            self.bump();
            self.bump();
            dims = dims.saturating_add(1);
        }
        let ty_span = self.span_from(start);
        if self.eat(|k| matches!(k, TokenKind::LBracket)) {
            let len = self.parse_expr()?;
            self.expect(|k| matches!(k, TokenKind::RBracket), "']'")?;
            return Ok(self.expr(
                ExprKind::NewArray {
                    ty: TypeRef {
                        path,
                        dims,
                        span: ty_span,
                    },
                    len: Box::new(len),
                },
                start,
            ));
        }
        if dims != 0 {
            return Err(self.err_here(DiagKind::UnexpectedToken {
                found: "[]".into(),
                expected: "'('".into(),
            }));
        }
        self.expect(|k| matches!(k, TokenKind::LParen), "'(' or '['")?;
        let args = self.parse_args()?;
        Ok(self.expr(
            ExprKind::New {
                ty: TypeRef {
                    path,
                    dims: 0,
                    span: ty_span,
                },
                args,
            },
            start,
        ))
    }

    fn parse_args(&mut self) -> Result<Vec<Expr>, Diagnostic> {
        let mut args = Vec::new();
        if !self.at(|k| matches!(k, TokenKind::RParen)) {
            loop {
                args.push(self.parse_expr()?);
                if !self.eat(|k| matches!(k, TokenKind::Comma)) {
                    break;
                }
                if self.at(|k| matches!(k, TokenKind::RParen)) {
                    break;
                }
            }
        }
        self.expect(|k| matches!(k, TokenKind::RParen), "')'")?;
        Ok(args)
    }

    fn at_ident_in(&self) -> bool {
        matches!(self.peek().kind, TokenKind::Ident(_))
            && self.tokens.get(self.i + 1).is_some_and(|t| matches!(t.kind, TokenKind::In))
    }

    fn expect_semi(&mut self) -> Result<(), Diagnostic> {
        if self.eat(|k| matches!(k, TokenKind::Semi)) {
            Ok(())
        } else {
            Err(self.err_here(DiagKind::ExpectedSemicolon))
        }
    }

    fn expect_ident(&mut self) -> Result<String, Diagnostic> {
        if let TokenKind::Ident(name) = &self.peek().kind {
            let name = name.clone();
            self.bump();
            Ok(name)
        } else {
            Err(self.err_here(DiagKind::ExpectedIdent))
        }
    }

    fn expect(&mut self, pred: fn(&TokenKind) -> bool, expected: &str) -> Result<(), Diagnostic> {
        if self.eat(pred) {
            Ok(())
        } else {
            Err(self.unexpected(expected))
        }
    }

    fn unexpected(&self, expected: &str) -> Diagnostic {
        self.err_here(DiagKind::UnexpectedToken {
            found: self.peek().kind.text(),
            expected: expected.into(),
        })
    }

    fn err_here(&self, kind: DiagKind) -> Diagnostic {
        Diagnostic::error(self.peek().span, kind)
    }

    fn expr(&self, kind: ExprKind, start: u32) -> Expr {
        Expr {
            kind,
            span: self.span_from(start),
        }
    }

    fn at(&self, pred: fn(&TokenKind) -> bool) -> bool {
        pred(&self.peek().kind)
    }

    fn lookahead(&self, pred: fn(&TokenKind) -> bool) -> bool {
        self.tokens
            .get(self.i + 1)
            .is_some_and(|t| pred(&t.kind))
    }

    fn eat(&mut self, pred: fn(&TokenKind) -> bool) -> bool {
        if pred(&self.peek().kind) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn bump(&mut self) {
        if !matches!(self.peek().kind, TokenKind::Eof) {
            self.last_end = self.peek().span.end;
            self.i += 1;
        }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.i.min(self.tokens.len() - 1)]
    }

    fn previous_kind(&self) -> TokenKind {
        self.tokens[self.i - 1].kind.clone()
    }

    fn prev_start(&self) -> u32 {
        if self.i == 0 {
            0
        } else {
            self.tokens[self.i - 1].span.start
        }
    }

    fn pos(&self) -> u32 {
        self.peek().span.start
    }

    fn span_from(&self, start: u32) -> Span {
        Span::new(self.file, start, self.last_end.max(start))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lex;

    fn parse_src(src: &str) -> Result<File, Diagnostic> {
        let tokens = lex(0, src).unwrap();
        parse(0, &tokens)
    }

    #[test]
    fn function_without_return_type() {
        let file = parse_src("func main(name: string, age: int) {\n    println(name);\n}\n").unwrap();
        match &file.items[0].kind {
            ItemKind::Function(f) => {
                assert!(f.ret.is_none());
                assert_eq!(f.params.len(), 2);
            }
            _ => panic!("func"),
        }
    }

    #[test]
    fn optional_return_type() {
        let file = parse_src("func main() -> int {\n    return 0;\n}\n").unwrap();
        match &file.items[0].kind {
            ItemKind::Function(f) => {
                assert!(f.ret.is_some());
            }
            _ => panic!("func"),
        }
    }

    #[test]
    fn missing_semicolon_is_an_error() {
        let err = parse_src("func main(args: string[]) {\n    println(args)\n}\n").unwrap_err();
        assert!(matches!(err.kind, DiagKind::ExpectedSemicolon));
    }

    #[test]
    fn package_and_class_entry() {
        let src = "package com.example.app;\nuse com.terra.lang;\npub class File {\n    func main(args: string[]) {\n        println(\"hi\");\n    }\n}\n";
        let file = parse_src(src).unwrap();
        assert_eq!(file.package.unwrap().join(), "com.example.app");
        assert!(matches!(file.items[0].kind, ItemKind::Class(_)));
    }
}
