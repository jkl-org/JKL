use crate::expr::Expr;
use crate::scanner::Token;
use crate::stmt::Stmt;
use std::collections::HashMap;
#[derive(Copy, Clone, PartialEq)]
enum FunctionType {
    None,
    Function,
}
#[derive(Copy, Clone, PartialEq)]
enum LoopType {
    None,
    Loop,
}
pub struct Resolver {
    scopes: Vec<HashMap<String, bool>>,
    current_function: FunctionType,
    current_loop: LoopType,
    locals: HashMap<usize, usize>,
}
impl Resolver {
    pub fn new() -> Self {
        Self {
            scopes: vec![],
            current_function: FunctionType::None,
            current_loop: LoopType::None,
            locals: HashMap::new(),
        }
    }
    fn resolve_internal(&mut self, stmt: &Stmt) -> Result<(), String> {
        match stmt {
            Stmt::Block { statements: _ } => self.resolve_block(stmt)?,
            Stmt::Var {
                name: _,
                initializer: _,
            } => self.resolve_var(stmt)?,
            Stmt::Function {
                name: _,
                params: _,
                body: _,
            } => self.resolve_function(stmt, FunctionType::Function)?,
            Stmt::CmdFunction { name: _, cmd: _ } => self.resolve_var(stmt)?,
            Stmt::Expression { expression } => self.resolve_expr(expression)?,
            Stmt::IfStmt {
                predicate: _,
                then: _,
                elif_branches: _,
                els: _,
            } => self.resolve_if_stmt(stmt)?,
            Stmt::Print { expression } => self.resolve_expr(expression)?,
            Stmt::Errors { expression } => self.resolve_expr(expression)?,
            Stmt::Import { expression } => self.resolve_expr(expression)?,
            Stmt::Exits {} => (),
            Stmt::ReturnStmt { keyword: _, value } => {
                if self.current_function == FunctionType::None {
                    panic!("\n Return statement is not allowed outside of a function");
                } else if let Some(value) = value {
                    self.resolve_expr(value)?;
                }
            }
            Stmt::WhileStmt { condition, body } => {
                self.resolve_expr(condition)?;
                let previous_loop = self.current_loop;
                self.current_loop = LoopType::Loop;
                self.resolve_internal(body.as_ref())?;
                self.current_loop = previous_loop;
            }
            Stmt::WaitStmt { time, body, before } => {
                self.resolve_expr(time)?;
                self.resolve_internal(body)?;
                if let Some(before_block) = before {
                    self.resolve_expr(&before_block.time)?;
                    self.resolve_internal(&before_block.body)?;
                }
            }
            Stmt::BreakStmt { keyword: _ } => {
                if self.current_loop == LoopType::None {
                    panic!("\n Break statement is not allowed outside of a loop");
                }
            }
            Stmt::BenchStmt { body } => {
                self.resolve_internal(body.as_ref())?;
            }
        }
        Ok(())
    }
    fn resolve_many(&mut self, stmts: &Vec<&Stmt>) -> Result<(), String> {
        for stmt in stmts {
            self.resolve_internal(stmt)?;
        }
        Ok(())
    }
    pub fn resolve(mut self, stmts: &Vec<&Stmt>) -> Result<HashMap<usize, usize>, String> {
        self.resolve_many(stmts)?;
        Ok(self.locals)
    }
    fn resolve_block(&mut self, stmt: &Stmt) -> Result<(), String> {
        match stmt {
            Stmt::Block { statements } => {
                self.begin_scope();
                self.resolve_many(&statements.iter().map(|b| b.as_ref()).collect())?;
                self.end_scope();
            }
            _ => panic!("\n Wrong type"),
        }
        Ok(())
    }
    fn resolve_var(&mut self, stmt: &Stmt) -> Result<(), String> {
        if let Stmt::Var { name, initializer } = stmt {
            self.declare(name)?;
            self.resolve_expr(initializer)?;
            self.define(name);
        } else if let Stmt::CmdFunction { name, cmd: _ } = stmt {
            self.declare(name)?;
            self.define(name);
        } else {
            panic!("\n Wrong type in resolve var");
        }
        Ok(())
    }
    fn resolve_function(&mut self, stmt: &Stmt, fn_type: FunctionType) -> Result<(), String> {
        if let Stmt::Function { name, params, body } = stmt {
            self.declare(name)?;
            self.define(name);
            self.resolve_function_helper(
                params,
                &body.iter().map(|b| b.as_ref()).collect(),
                fn_type,
            )
        } else {
            panic!("\n Wrong type in resolve function");
        }
    }
    fn resolve_if_stmt(&mut self, stmt: &Stmt) -> Result<(), String> {
        if let Stmt::IfStmt {
            predicate,
            then,
            elif_branches,
            els,
        } = stmt
        {
            self.resolve_expr(predicate)?;
            self.resolve_internal(then.as_ref())?;

            for (elif_predicate, elif_stmt) in elif_branches {
                self.resolve_expr(elif_predicate)?;
                self.resolve_internal(elif_stmt.as_ref())?;
            }

            if let Some(els) = els {
                self.resolve_internal(els.as_ref())?;
            }
            Ok(())
        } else {
            panic!("\n Wrong type in resolve_if_stmt");
        }
    }
    fn resolve_function_helper(
        &mut self,
        params: &Vec<Token>,
        body: &Vec<&Stmt>,
        resolving_function: FunctionType,
    ) -> Result<(), String> {
        let enclosing_function = self.current_function;
        self.current_function = resolving_function;
        self.begin_scope();
        for param in params {
            self.declare(param)?;
            self.define(param);
        }
        self.resolve_many(body)?;
        self.end_scope();
        self.current_function = enclosing_function;
        Ok(())
    }
    fn begin_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }
    fn end_scope(&mut self) {
        self.scopes.pop().expect("Stack underflow");
    }
    fn declare(&mut self, name: &Token) -> Result<(), String> {
        let size = self.scopes.len();
        if self.scopes.is_empty() {
            return Ok(());
        } else if self.scopes[size - 1].contains_key(&name.lexeme.clone()) {
            panic!("\n A variable with this name is already in scope");
        }
        self.scopes[size - 1].insert(name.lexeme.clone(), false);
        Ok(())
    }
    fn define(&mut self, name: &Token) {
        if self.scopes.is_empty() {
            return;
        }
        let size = self.scopes.len();
        self.scopes[size - 1].insert(name.lexeme.clone(), true);
    }
    fn resolve_expr(&mut self, expr: &Expr) -> Result<(), String> {
        match expr {
            Expr::Variable { id: _, name: _ } => self.resolve_expr_var(expr, expr.get_id()),
            Expr::Assign {
                id: _,
                name: _,
                value: _,
            } => self.resolve_expr_assign(expr, expr.get_id()),
            Expr::Array { id: _, elements } => {
                for element in elements {
                    self.resolve_expr(element)?;
                }
                Ok(())
            }
            Expr::Binary {
                id: _,
                left,
                operator: _,
                right,
            } => {
                self.resolve_expr(left)?;
                self.resolve_expr(right)
            }
            Expr::Call {
                id: _,
                callee,
                paren: _,
                arguments,
            } => {
                self.resolve_expr(callee.as_ref())?;
                for arg in arguments {
                    self.resolve_expr(arg)?;
                }
                Ok(())
            }
            Expr::Get {
                id: _,
                object,
                name: _,
            } => self.resolve_expr(object),
            Expr::Grouping { id: _, expression } => self.resolve_expr(expression),
            Expr::Literal { id: _, value: _ } => Ok(()),
            Expr::Logical {
                id: _,
                left,
                operator: _,
                right,
            } => {
                self.resolve_expr(left)?;
                self.resolve_expr(right)
            }
            Expr::Set {
                id: _,
                object,
                name: _,
                value,
            } => {
                self.resolve_expr(value)?;
                self.resolve_expr(object)
            }
            Expr::Unary {
                id: _,
                operator: _,
                right,
            } => self.resolve_expr(right),
            Expr::AnonFunction {
                id: _,
                paren: _,
                arguments,
                body,
            } => self.resolve_function_helper(
                arguments,
                &body.iter().map(|b| b.as_ref()).collect(),
                FunctionType::Function,
            ),
        }
    }
    fn resolve_expr_var(&mut self, expr: &Expr, resolve_id: usize) -> Result<(), String> {
        match expr {
            Expr::Variable { id: _, name } => {
                if !self.scopes.is_empty() {
                    if let Some(false) = self.scopes[self.scopes.len() - 1].get(&name.lexeme) {
                        panic!("\n  Can't read local variable in its own initializer");
                    }
                }
                self.resolve_local(name, resolve_id)
            }
            Expr::Call {
                id: _,
                callee,
                paren: _,
                arguments: _,
            } => match callee.as_ref() {
                Expr::Variable { id: _, name } => self.resolve_local(&name, resolve_id),
                _ => panic!("\n Wrong type in resolve_expr_var"),
            },
            _ => panic!("\n Wrong type in resolve_expr_var"),
        }
    }
    fn resolve_local(&mut self, name: &Token, resolve_id: usize) -> Result<(), String> {
        let size = self.scopes.len();
        if size == 0 {
            return Ok(());
        }
        for i in (0..=(size - 1)).rev() {
            let scope = &self.scopes[i];
            if scope.contains_key(&name.lexeme) {
                self.locals.insert(resolve_id, size - 1 - i);
                return Ok(());
            }
        }
        Ok(())
    }
    fn resolve_expr_assign(&mut self, expr: &Expr, resolve_id: usize) -> Result<(), String> {
        if let Expr::Assign { id: _, name, value } = expr {
            self.resolve_expr(value.as_ref())?;
            self.resolve_local(name, resolve_id)?;
        } else {
            panic!("\n Wrong type in resolve assign");
        }
        Ok(())
    }
}
