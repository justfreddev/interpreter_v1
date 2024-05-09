//! The parser module is responsible for parsing the tokens generated by the lexer and constructing
//! an abstract syntax tree (AST) for the interpreter. The parser analyses the structure and
//! grammar of the source code to determine its meaning and validity. The parser follows the rules
//! defined by the programming language's grammar to ensure that the code is syntactically correct.
//! If any syntax errors are encountered during parsing, the parser reports them as
//! `ParserError`, which is then returned as an Err(), which safely halts the program. Once the
//! parsing process is complete, the parser returns the AST, which is then used by the interpreter
//! to execute the program.
//!
//! This module defines the Parser struct, which maintains the state of the parsing process.
//! It contains methods for parsing different language features such as declarations,
//! expressions, statements, loops, conditionals, and more. The parse() method is the entry
//! point of the parser, which starts the parsing process and returns the resulting AST.
//! The Parser struct also keeps track of the current token being parsed and provides
//! utility methods for token matching, consuming tokens, and error handling.
//! 
//! 
//! ## Example
//! 
//! ```rust
//! use crate::parser::Parser;
//! use crate::lexer::Lexer;
//!
//! fn main() {
//!     let source_code = r#"
//!         var x = 10;
//!         if (x > 5) {
//!             print "Hello, world!";
//!         }
//!     "#;
//!
//!     let mut lexer = Lexer::new(source_code);
//!     let tokens = lexer.run().unwrap();
//!
//!     let mut parser = Parser::new(tokens);
//!     let ast = parser.parse().unwrap();
//!
//!     // Use the AST to execute the program
//!     // ...
//! }
//! ```
//! 
//! ## The Process
//! 
//! Parsing the tokens works as follows:
//! 
//! 1. The parser evaluates the tokens one by one, and starts off by 

use crate::{
    error::ParserError,
    expr::Expr,
    stmt::Stmt,
    token::{Token, TokenType},
    value::LiteralType,
};

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        return Self { tokens, current: 0 };
    }

    pub fn parse(&mut self) -> Result<Vec<Stmt>, ParserError> {
        let mut statements = Vec::new();

        while !self.is_at_end() {
            match self.declaration() {
                Ok(stmt) => statements.push(stmt),
                Err(e) => return Err(e),
            }
        }

        return Ok(statements);
    }

    fn declaration(&mut self) -> Result<Stmt, ParserError> {
        if self.match_token(vec![&TokenType::Def]) {
            return match self.function("function") {
                Ok(v) => Ok(v),
                Err(e) => {
                    self.synchronize();
                    Err(e)
                }
            }
        } else if self.match_token(vec![&TokenType::Var]) {
            return match self.var_declaration() {
                Ok(v) => Ok(v),
                Err(e) => {
                    self.synchronize();
                    Err(e)
                }
            }
        } else {
            return self.statement();
        }
    }

    fn function(&mut self, kind: &str) -> Result<Stmt, ParserError> {
        let name = match self.consume(
            TokenType::Identifier,
            format!(
                "Expected{}Name",
                kind.chars()
                    .next()
                    .unwrap()
                    .to_uppercase()
                    .collect::<String>()
                    + &kind[1..]
            )
            .as_str(),
        ) {
            Ok(v) => v,
            Err(e) => return Err(e),
        };

        self.consume(
            TokenType::LParen,
            format!(
                "ExpectedLParenAfter{}Name",
                kind.chars()
                    .next()
                    .unwrap()
                    .to_uppercase()
                    .collect::<String>()
                    + &kind[1..]
            )
            .as_str(),
        )?;

        let mut params: Vec<Token> = Vec::new();
        if !self.check(TokenType::RParen) {
            loop {
                if params.len() >= 255 {
                    let token = self.peek();
                    return Err(ParserError::TooManyParameters {
                        name: name.lexeme,
                        line: token.line,
                    });
                }

                let parameter = self.consume(TokenType::Identifier, "ExpectedParameterName")?;
                params.push(parameter);

                if !self.match_token(vec![&TokenType::Comma]) {
                    break;
                };
            }
        }

        self.consume(TokenType::RParen, "ExpectedRParenAfterParameters")?;

        self.consume(TokenType::LBrace, "ExpectedRParenAfterParameters")?;

        let body = self.block()?;

        return Ok(Stmt::Function { name, params, body });
    }

    fn var_declaration(&mut self) -> Result<Stmt, ParserError> {
        let name = self.consume(TokenType::Identifier, "ExpectedVariableName")?;

        let initializer = if self.match_token(vec![&TokenType::Equal]) {
            let expr = self.expression()?;
            Some(expr)
        } else {
            None
        };

        self.consume(TokenType::Semicolon, "ExpectedSemicolonAfterVariableDeclaration")?;

        return Ok(Stmt::Var { name, initializer });
    }

    fn statement(&mut self) -> Result<Stmt, ParserError> {
        if self.match_token(vec![&TokenType::For]) {
            return self.for_statement();
        };
        if self.match_token(vec![&TokenType::If]) {
            return self.if_statement();
        };
        if self.match_token(vec![&TokenType::Print]) {
            return self.print_statement();
        };
        if self.match_token(vec![&TokenType::Return]) {
            return self.return_statement();
        };
        if self.match_token(vec![&TokenType::While]) {
            return self.while_statement();
        };
        if self.match_token(vec![&TokenType::LBrace]) {
            return Ok(Stmt::Block {
                statements: self.block()?,
            });
        };

        return self.expression_statement();
    }

    fn for_statement(&mut self) -> Result<Stmt, ParserError> {
        self.consume(TokenType::LParen, "ExpectedLParenAfterFor")?;

        let initializer;
        if self.match_token(vec![&TokenType::Semicolon]) {
            initializer = None;
        } else if self.match_token(vec![&TokenType::Var]) {
            let var_declaration = self.var_declaration()?;
            initializer = Some(Box::new(var_declaration));
        } else {
            let expr_stmt = self.expression_statement()?;
            initializer = Some(Box::new(expr_stmt));
        }

        let condition = if !self.check(TokenType::Semicolon) {
            self.expression()?
        } else {
            Expr::Literal { value: LiteralType::True }
        };

        self.consume(TokenType::Semicolon, "ExpectedSemiColonAfterForCondition")?;

        let mut increment = None;
        if !self.check(TokenType::RParen) {
            increment = Some(self.expression()?);
        }

        self.consume(TokenType::RParen, "ExpectedRParenAfterForClauses")?;

        let body = self.statement()?;

        return Ok(Stmt::For {
            initializer,
            condition,
            increment,
            body: Box::new(body),
        });
    }

    fn if_statement(&mut self) -> Result<Stmt, ParserError> {
        self.consume(TokenType::LParen, "ExpectedLParenAfterIf")?;
        let condition = self.expression()?;
        self.consume(TokenType::RParen, "ExpectedLParenAfterCondition")?;

        let then_branch = self.statement()?;

        let mut else_branch = None;
        if self.match_token(vec![&TokenType::Else]) {
            let result = self.statement()?;
            else_branch = Some(Box::new(result));
        };

        return Ok(Stmt::If {
            condition,
            then_branch: Box::new(then_branch),
            else_branch,
        });
    }

    fn print_statement(&mut self) -> Result<Stmt, ParserError> {
        let value = self.expression()?;
        self.consume(TokenType::Semicolon, "ExpectedSemicolonAfterPrintValue")?;

        return Ok(Stmt::Print { expression: value });
    }

    fn return_statement(&mut self) -> Result<Stmt, ParserError> {
        let keyword = self.previous().clone();
        let mut value = None;
        if !self.check(TokenType::Semicolon) {
            value = Some(self.expression()?);
        }
        self.consume(TokenType::Semicolon, "ExpectedSemicolonAfterReturnValue")?;

        return Ok(Stmt::Return { keyword, value });
    }

    fn while_statement(&mut self) -> Result<Stmt, ParserError> {
        self.consume(TokenType::LParen, "ExpectedLParenAfterWhile")?;
        let condition = self.expression()?;
        self.consume(TokenType::RParen, "ExpectedLParenAfterCondition")?;

        let body = self.statement()?;

        return Ok(Stmt::While { condition, body: Box::new(body) });
    }

    fn block(&mut self) -> Result<Vec<Stmt>, ParserError> {
        let mut statements = Vec::new();

        while !self.check(TokenType::RBrace) && !self.is_at_end() {
            let stmt = self.declaration()?;
            statements.push(stmt);
        }
        self.consume(TokenType::RBrace, "ExpectedRBraceAfterBlock")?;

        return Ok(statements);
    }

    fn expression(&mut self) -> Result<Expr, ParserError> {
        return self.assignment();
    }

    fn assignment(&mut self) -> Result<Expr, ParserError> {
        let expr = self.or()?;

        if self.match_token(vec![&TokenType::Incr, &TokenType::Decr]) {
            match expr {
                Expr::Var { name } => match self.previous().token_type {
                    TokenType::Incr => {
                        return Ok(Expr::Alteration {
                            name,
                            alteration_type: TokenType::Incr,
                        })
                    }
                    TokenType::Decr => {
                        return Ok(Expr::Alteration {
                            name,
                            alteration_type: TokenType::Decr,
                        })
                    }
                    _ => {
                        let token = self.previous();
                        return Err(ParserError::ExpectedAlterationExpression {
                            line: token.start,
                        });
                    }
                },
                _ => {
                    let token = self.previous();
                    return Err(ParserError::InvalidAlterationTarget {
                        target: token.lexeme.clone(),
                        line: token.line,
                    });
                }
            }
        } else if self.match_token(vec![&TokenType::Equal]) {
            let value = self.assignment()?;

            match expr {
                Expr::Var { name } => {
                    return Ok(Expr::Assign {
                        name,
                        value: Box::new(value),
                    })
                }
                _ => {
                    let token = self.previous();
                    return Err(ParserError::InvalidAssignmentTarget {
                        target: token.lexeme.clone(),
                        line: token.line,
                    });
                }
            }
        }

        return Ok(expr);
    }

    fn or(&mut self) -> Result<Expr, ParserError> {
        let mut expr = self.and()?;

        while self.match_token(vec![&TokenType::Or]) {
            let operator = self.previous().clone();
            let right = self.and()?;
            expr = Expr::Logical {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            };
        }

        return Ok(expr);
    }

    fn and(&mut self) -> Result<Expr, ParserError> {
        let mut expr = self.equality()?;

        while self.match_token(vec![&TokenType::And]) {
            let operator = self.previous().clone();
            let right = self.equality()?;
            expr = Expr::Logical {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            }
        }

        return Ok(expr);
    }

    fn equality(&mut self) -> Result<Expr, ParserError> {
        let mut expr: Expr = self.comparison()?;

        while self.match_token(vec![&TokenType::Bang, &TokenType::EqualEqual]) {
            let operator = self.previous().clone();
            let right = self.comparison()?;
            expr = Expr::Binary {
                left: Box::new(expr.clone()),
                operator,
                right: Box::new(right),
            };
        }

        return Ok(expr);
    }

    fn comparison(&mut self) -> Result<Expr, ParserError> {
        let mut expr: Expr = self.term()?;

        while self.match_token(vec![
            &TokenType::Greater,
            &TokenType::GreaterEqual,
            &TokenType::Less,
            &TokenType::LessEqual,
            &TokenType::BangEqual,
            &TokenType::EqualEqual,
        ]) {
            let operator = self.previous().clone();
            let right = self.term()?;
            expr = Expr::Binary {
                left: Box::new(expr.clone()),
                operator,
                right: Box::new(right),
            };
        }

        return Ok(expr);
    }

    fn term(&mut self) -> Result<Expr, ParserError> {
        let mut expr = self.factor()?;

        while self.match_token(vec![&TokenType::Minus, &TokenType::Plus]) {
            let operator = self.previous().clone();
            let right = self.factor()?;
            expr = Expr::Binary {
                left: Box::new(expr.clone()),
                operator,
                right: Box::new(right),
            };
        }

        return Ok(expr);
    }

    fn factor(&mut self) -> Result<Expr, ParserError> {
        let mut expr = self.unary()?;

        while self.match_token(vec![&TokenType::FSlash, &TokenType::Asterisk]) {
            let operator = self.previous().clone();
            let right = self.unary()?;
            expr = Expr::Binary {
                left: Box::new(expr.clone()),
                operator,
                right: Box::new(right),
            };
        }

        return Ok(expr);
    }

    fn unary(&mut self) -> Result<Expr, ParserError> {
        if self.match_token(vec![&TokenType::Bang, &TokenType::Minus]) {
            let operator = self.previous().clone();
            let right = self.unary()?;
            return Ok(Expr::Unary {
                operator,
                right: Box::new(right),
            });
        }

        return self.call();
    }

    fn call(&mut self) -> Result<Expr, ParserError> {
        let mut expr = self.primary()?;

        loop {
            if self.match_token(vec![&TokenType::LParen]) {
                expr = self.finish_call(expr)?;
            } else if self.match_token(vec![&TokenType::Dot]) {
                let call = self.call()?;
                let name = match expr {
                    Expr::Var { ref name } => name,
                    _ => {
                        let token = self.peek();
                        return Err(ParserError::CanOnlyCallIdentifiers {
                            value: token.lexeme.clone(),
                            line: token.line,
                        })
                    },
                };

                return Ok(Expr::ListMethodCall { object: name.clone(), call: Box::new(call) })
            } else {
                break;
            }
        }

        return Ok(expr);
    }

    fn finish_call(&mut self, callee: Expr) -> Result<Expr, ParserError> {
        let mut arguments: Vec<Expr> = Vec::new();

        if !self.check(TokenType::RParen) {
            loop {
                if arguments.len() >= 255 {
                    return Err(ParserError::TooManyArguments { callee });
                }
                let expr = self.expression()?;
                arguments.push(expr);
                if !self.match_token(vec![&TokenType::Comma]) {
                    break;
                };
            }
        }

        self.consume(TokenType::RParen, "ExpectedRParenAfterArguments")?;

        return Ok(Expr::Call {
            callee: Box::new(callee),
            arguments,
        });
    }

    fn primary(&mut self) -> Result<Expr, ParserError> {
        if self.match_token(vec![&TokenType::True]) {
            return Ok(Expr::Literal {
                value: LiteralType::True,
            });
        };
        if self.match_token(vec![&TokenType::False]) {
            return Ok(Expr::Literal {
                value: LiteralType::False,
            });
        };
        if self.match_token(vec![&TokenType::Null]) {
            return Ok(Expr::Literal {
                value: LiteralType::Null,
            });
        };

        if self.match_token(vec![&TokenType::Num, &TokenType::String]) {
            match self.previous().token_type {
                TokenType::String => {
                    return Ok(Expr::Literal {
                        value: LiteralType::Str(self.previous().literal.clone()),
                    })
                }
                TokenType::Num => {
                    let n = match self.previous().literal.clone().trim().parse() {
                        Ok(v) => v,
                        Err(_) => {
                            let token = self.previous();
                            return Err(ParserError::UnableToParseLiteralToFloat {
                                value: token.lexeme.clone(),
                                line: token.line,
                            });
                        }
                    };
                    return Ok(Expr::Literal {
                        value: LiteralType::Num(n),
                    });
                }
                _ => {
                    let token = self.previous();
                    return Err(ParserError::ExpectedStringOrNumber {
                        value: token.lexeme.clone(),
                        line: token.line,
                    });
                }
            }
        }

        if self.match_token(vec![&TokenType::Identifier]) {
            let name = self.previous().clone();
            let expr = if self.match_token(vec![&TokenType::LBrack]) {
                let mut start: Option<Box<Expr>> = None;
                let mut end: Option<Box<Expr>> = None;
                let mut is_splice = false;
                if self.peek().token_type != TokenType::Colon {
                    start = Some(Box::new(self.expression()?));
                }
                start = if start.is_some() {
                    Some(start.unwrap())
                } else {
                    None
                };
                if self.match_token(vec![&TokenType::Colon]) {
                    is_splice = true;
                    if self.peek().token_type != TokenType::RBrack {
                        end = Some(Box::new(self.expression()?));
                    }
                    end = if end.is_some() {
                        Some(end.unwrap())
                    } else {
                        None
                    };
                }
                self.consume(TokenType::RBrack, "ExpectedRBrackAfterIndex")?;
                Expr::Splice { list: name, is_splice, start, end }
            } else {
                Expr::Var { name: name.clone() }
            };
            return Ok(expr);
        }

        if self.match_token(vec![&TokenType::LParen]) {
            let expr = self.expression()?;
            self.consume(TokenType::RParen, "ExpectedRParenAfterExpression")?;
            return Ok(Expr::Grouping {
                expression: Box::new(expr),
            });
        }

        if self.match_token(vec![&TokenType::LBrack]) {
            let mut items: Vec<Expr> = Vec::new();
            if self.match_token(vec![&TokenType::RBrack]) {
                return Ok(Expr::List { items });
            }
            loop {
                if self.match_token(vec![&TokenType::RBrack]) {
                    break;
                }
                items.push(self.expression()?);
                if !self.match_token(vec![&TokenType::Comma]) {
                    break;
                }
            }

            self.consume(TokenType::RBrack, "ExpectedRBrackAfterValues")?;

            return Ok(Expr::List { items });
        }

        let prev = self.previous();
        let token = self.peek();

        return Err(ParserError::ExpectedExpression {
            prev: prev.lexeme.clone(),
            line: token.line,
        });
    }

    fn expression_statement(&mut self) -> Result<Stmt, ParserError> {
        let expr = self.expression()?;

        self.consume(TokenType::Semicolon, "ExpectedExpression")?;

        return Ok(Stmt::Expression { expression: expr });
    }

    fn match_token(&mut self, types: Vec<&TokenType>) -> bool {
        for token_type in types {
            if self.check(*token_type) {
                self.advance();
                return true;
            }
        }

        return false;
    }

    fn check(&mut self, token_type: TokenType) -> bool {
        if self.is_at_end() {
            return false;
        };

        return self.peek().token_type == token_type;
    }

    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.current += 1
        };

        return self.previous();
    }

    fn previous(&self) -> &Token {
        return &self.tokens[self.current - 1];
    }

    fn peek(&self) -> &Token {
        return &self.tokens[self.current];
    }

    fn is_at_end(&mut self) -> bool {
        return self.peek().token_type == TokenType::Eof;
    }

    fn synchronize(&mut self) {
        self.advance();

        while !self.is_at_end() {
            if self.previous().token_type == TokenType::Semicolon {
                return;
            };

            match self.peek().token_type {
                TokenType::Class
                | TokenType::Def
                | TokenType::Var
                | TokenType::For
                | TokenType::If
                | TokenType::While
                | TokenType::Print
                | TokenType::Return => return,
                _ => {
                    self.advance();
                }
            }
        }
    }

    fn consume(&mut self, token_type: TokenType, error: &str) -> Result<Token, ParserError> {
        if self.check(token_type) {
            return Ok(self.advance().clone());
        };

        return match error {
            "ExpectedVariableName" => {
                let token = self.previous().clone();
                Err(ParserError::ExpectedVariableName {
                    lexeme: token.lexeme,
                    line: token.line,
                })
            },
            "ExpectedSemicolonAfterVariableDeclaration" => {
                let token = self.previous().clone();
                Err(ParserError::ExpectedSemicolonAfterVariableDeclaration {
                    lexeme: token.lexeme,
                    line: token.line,
                })
            },
            "ExpectedLParenAfterFor" => {
                let token = self.peek();
                Err(ParserError::ExpectedLParenAfterFor {
                    line: token.line,
                })
            },
            "ExpectedSemiColonAfterForCondition" => {
                let token = self.peek();
                Err(ParserError::ExpectedSemiColonAfterForCondition {
                    line: token.line,
                })
            },
            "ExpectedRParenAfterForClauses" => {
                let token = self.peek();
                Err(ParserError::ExpectedRParenAfterForClauses {
                    line: token.line,
                })
            },
            "ExpectedLParenAfterIf" => {
                let token = self.peek();
                Err(ParserError::ExpectedLParenAfterIf {
                    line: token.line,
                })
            },
            "ExpectedLParenAfterCondition" => {
                let token = self.peek();
                Err(ParserError::ExpectedLParenAfterCondition {
                    line: token.line,
                })
            },
            "ExpectedSemicolonAfterPrintValue" => {
                let token = self.previous();
                Err(ParserError::ExpectedSemicolonAfterPrintValue {
                    value: token.lexeme.clone(),
                    line: token.line,
                })
            },
            "ExpectedSemicolonAfterReturnValue" => {
                let token = self.previous();
                Err(ParserError::ExpectedSemicolonAfterReturnValue {
                    value: token.lexeme.clone(),
                    line: token.line,
                })
            },
            "ExpectedLParenAfterWhile" => {
                let token = self.peek();
                Err(ParserError::ExpectedLParenAfterWhile {
                    line: token.line,
                })
            },
            "ExpectedRBraceAfterBlock" => {
                let token = self.peek();
                Err(ParserError::ExpectedRBraceAfterBlock {
                    line: token.line,
                })
            },
            "ExpectedRParenAfterArguments" => {
                let token = self.peek();
                Err(ParserError::ExpectedRParenAfterArguments {
                    line: token.line,
                })
            },
            "ExpectedRParenAfterExpression" => {
                let token = self.peek();
                Err(ParserError::ExpectedRParenAfterExpression {
                    line: token.line,
                })
            },
            "ExpectedExpression" => {
                let prev = self.previous();
                let token = self.peek();
                Err(ParserError::ExpectedExpression {
                    prev: prev.lexeme.clone(),
                    line: token.line,
                })
            },
            "ExpectedFunctionName" => {
                let token = self.peek();
                Err(ParserError::ExpectedFunctionName {
                    line: token.line,
                })
            },
            "ExpectedLParenAfterFunctionName" => {
                let token = self.peek();
                Err(ParserError::ExpectedLParenAfterFunctionName {
                    line: token.line,
                })
            },
            "ExpectedParameterName" => {
                let token = self.peek();
                Err(ParserError::ExpectedParameterName {
                    line: token.line,
                })
            },
            "ExpectedRBrackAfterValues" => {
                let token = self.peek();
                Err(ParserError::ExpectedRBrackAfterValues {
                    line: token.line,
                })
            },
            _ => Err(ParserError::Unknown),
        }
    }
}
