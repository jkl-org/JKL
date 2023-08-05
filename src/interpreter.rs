use crate::environment::Environment;
use crate::expr::{CallableImpl, JekoFunctionImpl, LiteralValue, NativeFunctionImpl};
use crate::scanner::Token;
use crate::stmt::Stmt;
use std::collections::HashMap;
use std::io;
use std::process::exit;
use std::process::Command;
use std::rc::Rc;
use colored::Colorize;

pub struct Interpreter {
    pub specials: HashMap<String, LiteralValue>,
    pub environment: Environment,
}

impl Interpreter {
    pub fn new() -> Self {
        Self {
            specials: HashMap::new(),
            environment: Environment::new(HashMap::new()),
        }
    }

    pub fn resolve(&mut self, locals: HashMap<usize, usize>) {
        self.environment.resolve(locals);
    }

    pub fn with_env(env: Environment) -> Self {
        Self {
            specials: HashMap::new(),
            environment: env,
        }
    }

    #[allow(dead_code)]
    pub fn for_anon(parent: Environment) -> Self {
        let env = parent.enclose();
        Self {
            specials: HashMap::new(),
            environment: env,
        }
    }

    pub fn interpret(&mut self, stmts: Vec<&Stmt>) -> Result<(), String> {
        for stmt in stmts {
            match stmt {
                Stmt::Expression { expression } => {
                    expression.evaluate(self.environment.clone())?;
                }
                Stmt::Print { expression } => {
                    let value = expression.evaluate(self.environment.clone())?;
                    println!("{}", value.to_string().green().to_string());
                }
                Stmt::Input { expression } => {
                    let value = expression.evaluate(self.environment.clone())?;
                    println!("{}", value.to_string().yellow().to_string());
                    let mut input = String::new();
                    io::stdin().read_line(&mut input).unwrap();
                }
                Stmt::Errors { expression } => {
                    let value = expression.evaluate(self.environment.clone())?;
                    println!("Error: {}", value.to_string().red().to_string());
                    exit(1)
                }
                Stmt::Var { name, initializer } => {
                    let value = initializer.evaluate(self.environment.clone())?;
                    self.environment.define(name.lexeme.clone(), value);
                }
                Stmt::Block { statements } => {
                    let new_environment = self.environment.enclose();
                    let old_environment = self.environment.clone();
                    self.environment = new_environment;
                    let block_result =
                        self.interpret((*statements).iter().map(|b| b.as_ref()).collect());
                    self.environment = old_environment;
                    block_result?;
                }
                Stmt::Class {
                    name,
                    methods,
                    superclass,
                } => {
                    let mut methods_map = HashMap::new();
                    let superclass_value;
                    if let Some(superclass) = superclass {
                        let superclass = superclass.evaluate(self.environment.clone())?;
                        if let LiteralValue::JekoClass { .. } = superclass {
                            superclass_value = Some(Box::new(superclass));
                        } else {
                            return Err(format!(
                                "Superclass must be a class, not {}",
                                superclass.to_type()
                            ).red().to_string());
                        }
                    } else {
                        superclass_value = None;
                    }
                    self.environment
                        .define(name.lexeme.clone(), LiteralValue::Nil);

                    self.environment = self.environment.enclose();
                    if let Some(sc) = superclass_value.clone() {
                        self.environment.define("super".to_string(), *sc);
                    }
                    for method in methods {
                        if let Stmt::Function {
                            name,
                            params: _,
                            body: _,
                        } = method.as_ref()
                        {
                            let function = self.make_function(method);
                            methods_map.insert(name.lexeme.clone(), function);
                        } else {
                            panic!(
                                "Something that was not a function was in the methods of a class"
                            );
                        }
                    }
                    let klass = LiteralValue::JekoClass {
                        name: name.lexeme.clone(),
                        methods: methods_map,
                        superclass: superclass_value,
                    };
                    if !self.environment.assign_global(&name.lexeme, klass) {
                        return Err(format!("Class definition failed for {}", name.lexeme).red().to_string());
                    }
                    self.environment = *self.environment.enclosing.clone().unwrap();
                }
                Stmt::IfStmt {
                    predicate,
                    then,
                    els,
                } => {
                    let truth_value = predicate.evaluate(self.environment.clone())?;
                    if truth_value.is_truthy() == LiteralValue::True {
                        let statements = vec![then.as_ref()];
                        self.interpret(statements)?;
                    } else if let Some(els_stmt) = els {
                        let statements = vec![els_stmt.as_ref()];
                        self.interpret(statements)?;
                    }
                }
                Stmt::IfShortStmt {
                    predicate,
                    then,
                    els,
                } => {
                    let truth_value = predicate.evaluate(self.environment.clone())?;
                    if truth_value.is_truthy() == LiteralValue::True {
                        let statements = vec![then.as_ref()];
                        self.interpret(statements)?;
                    } else if let Some(els_stmt) = els {
                        let statements = vec![els_stmt.as_ref()];
                        self.interpret(statements)?;
                    }
                }
                Stmt::WhileStmt { condition, body } => {
                    let mut flag = condition.evaluate(self.environment.clone())?;
                    while flag.is_truthy() == LiteralValue::True {
                        let statements = vec![body.as_ref()];
                        self.interpret(statements)?;
                        flag = condition.evaluate(self.environment.clone())?;
                    }
                }
                Stmt::Function {
                    name,
                    params: _,
                    body: _,
                } => {
                    let callable = self.make_function(stmt);
                    let fun = LiteralValue::Callable(CallableImpl::JekoFunction(callable));
                    self.environment.define(name.lexeme.clone(), fun);
                }
                Stmt::CmdFunction { name, cmd } => {
                    let cmd = cmd.clone();
                    let local_fn = move |_args: &Vec<LiteralValue>| {
                        let cmd = cmd.clone();
                        let parts = cmd.split(" ").collect::<Vec<&str>>();
                        let mut command = Command::new(parts[0].replace("\"", ""));
                        for part in parts[1..].iter() {
                            command.arg(part.replace("\"", ""));
                        }
                        let output = command.output().expect("Failed to run command");

                        return LiteralValue::StringValue(
                            std::str::from_utf8(output.stdout.as_slice())
                                .unwrap()
                                .to_string(),
                        );
                    };

                    let fun_val =
                        LiteralValue::Callable(CallableImpl::NativeFunction(NativeFunctionImpl {
                            name: name.lexeme.clone(),
                            arity: 0,
                            fun: Rc::new(local_fn),
                        }));
                    self.environment.define(name.lexeme.clone(), fun_val);
                }
                Stmt::ReturnStmt { keyword: _, value } => {
                    let eval_val;
                    if let Some(value) = value {
                        eval_val = value.evaluate(self.environment.clone())?;
                    } else {
                        eval_val = LiteralValue::Nil;
                    }
                    self.specials.insert("return".to_string(), eval_val);
                }
            };
        }
        Ok(())
    }
    fn make_function(&self, fn_stmt: &Stmt) -> JekoFunctionImpl {
        if let Stmt::Function { name, params, body } = fn_stmt {
            let arity = params.len();
            let params: Vec<Token> = params.iter().map(|t| (*t).clone()).collect();
            let body: Vec<Box<Stmt>> = body.iter().map(|b| (*b).clone()).collect();
            let name_clone = name.lexeme.clone();
            let parent_env = self.environment.clone();

            let callable_impl = JekoFunctionImpl {
                name: name_clone,
                arity,
                parent_env,
                params,
                body,
            };

            callable_impl
        } else {
            panic!("Tried to make a function from a non-function statement");
        }
    }
}