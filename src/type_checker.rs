use std::collections::HashMap;
use crate::parser::{BinOp, Expr, FnDecl, Stmt, Type, UnOp};

#[derive(Debug)]
pub struct TypeError { pub msg: String }

impl std::fmt::Display for TypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

#[derive(Clone)]
struct FuncSig { params: Vec<Type>, ret: Option<Type> }

pub struct TypeChecker {
    functions: HashMap<String, FuncSig>,
    scopes: Vec<HashMap<String, Type>>,
    current_return: Option<Option<Type>>,
    current_fn: Option<String>,
    loop_depth: usize,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            scopes: Vec::new(),
            current_return: None,
            current_fn: None,
            loop_depth: 0,
        }
    }

    pub fn check_program(mut self, program: &[Stmt]) -> Result<(), TypeError> {
        for s in program {
            if let Stmt::Fn(decl) = s {
                self.functions.insert(decl.name.clone(), FuncSig {
                    params: decl.params.iter().map(|p| p.ty.clone()).collect(),
                    ret: decl.ret.clone(),
                });
            }
        }
        self.push_scope();
        for s in program { self.check_stmt(s)?; }
        self.pop_scope();
        Ok(())
    }

    pub fn check_program_with_env(
        mut self,
        program: &[Stmt],
        initial_vars: Vec<(String, Type)>,
    ) -> Result<(), TypeError> {
        for s in program {
            if let Stmt::Fn(decl) = s {
                self.functions.insert(decl.name.clone(), FuncSig {
                    params: decl.params.iter().map(|p| p.ty.clone()).collect(),
                    ret: decl.ret.clone(),
                });
            }
        }

        self.push_scope();
        for (name, ty) in initial_vars {
            self.declare(&name, ty);
        }

        for s in program { self.check_stmt(s)?; }

        self.pop_scope();
        Ok(())
    }

    fn push_scope(&mut self) { self.scopes.push(HashMap::new()); }
    fn pop_scope(&mut self) { self.scopes.pop(); }
    fn declare(&mut self, name: &str, ty: Type) {
        if let Some(scope) = self.scopes.last_mut() { scope.insert(name.to_string(), ty); }
    }
    fn lookup(&self, name: &str) -> Option<&Type> {
        for scope in self.scopes.iter().rev() {
            if let Some(t) = scope.get(name) { return Some(t); }
        }
        None
    }
    fn err<T>(&self, msg: impl Into<String>) -> Result<T, TypeError> {
        let mut msg = msg.into();
        if let Some(fname) = &self.current_fn { msg = format!("em função '{}': {}", fname, msg); }
        Err(TypeError { msg })
    }

    fn check_stmt(&mut self, stmt: &Stmt) -> Result<(), TypeError> {
        match stmt {
            Stmt::Let { name, ty, value } => {
                let inferred = self.infer(value)?;
                let stored = match (ty, &inferred) {
                    (Some(declared), None) => {
                        if !matches!(declared, Type::Nullable(_)) {
                            return self.err(format!("variável '{}' declarada como {:?} não pode receber null", name, declared));
                        }
                        declared.clone()
                    }
                    (Some(declared), Some(actual)) => {
                        if !compatible(declared, actual) {
                            return self.err(format!("tipo incompatível em '{}': esperado {:?}, recebido {:?}", name, declared, actual));
                        }
                        declared.clone()
                    }
                    (None, None) => {
                        return self.err(format!("não é possível inferir o tipo de '{}' a partir de null; adicione anotação", name));
                    }
                    (None, Some(actual)) => actual.clone(),
                };
                self.declare(name, stored);
                Ok(())
            }
            Stmt::Assign { name, value } => {
                let declared = self.lookup(name).cloned().ok_or_else(|| {
                    let mut msg = format!("variável '{}' não definida", name);
                    if let Some(f) = &self.current_fn { msg = format!("em função '{}': {}", f, msg); }
                    TypeError { msg }
                })?;
                let inferred = self.infer(value)?;
                match &inferred {
                    None => {
                        if !matches!(declared, Type::Nullable(_)) {
                            return self.err(format!("não é possível atribuir null a '{}' de tipo {:?}", name, declared));
                        }
                    }
                    Some(actual) => {
                        if !compatible(&declared, actual) {
                            return self.err(format!("tipo incompatível em atribuição a '{}': esperado {:?}, recebido {:?}", name, declared, actual));
                        }
                    }
                }
                Ok(())
            }
            Stmt::Expr(e) => { self.infer(e)?; Ok(()) }
            Stmt::If { cond, then, else_ } => {
                self.require_bool(cond, "if")?;
                self.push_scope();
                for s in then { self.check_stmt(s)?; }
                self.pop_scope();
                if let Some(eb) = else_ {
                    self.push_scope();
                    for s in eb { self.check_stmt(s)?; }
                    self.pop_scope();
                }
                Ok(())
            }
            Stmt::While { cond, body } => {
                self.require_bool(cond, "while")?;
                self.push_scope();
                self.loop_depth += 1;
                for s in body { self.check_stmt(s)?; }
                self.loop_depth -= 1;
                self.pop_scope();
                Ok(())
            }
            Stmt::For { var, iter, body } => {
                let it = self.infer(iter)?;
                let elem_ty = match it {
                    None => return self.err("não é possível iterar sobre null"),
                    Some(t) => match unwrap(&t).clone() {
                        Type::List(inner) => *inner,
                        Type::Any => Type::Any,
                        other => return self.err(format!("'for' requer list<T>, recebeu {:?}", other)),
                    },
                };
                self.push_scope();
                self.declare(var, elem_ty);
                self.loop_depth += 1;
                for s in body { self.check_stmt(s)?; }
                self.loop_depth -= 1;
                self.pop_scope();
                Ok(())
            }
            Stmt::Break => {
                if self.loop_depth == 0 { return self.err("'break' fora de loop"); }
                Ok(())
            }
            Stmt::Continue => {
                if self.loop_depth == 0 { return self.err("'continue' fora de loop"); }
                Ok(())
            }
            Stmt::Try { body, err_var, catch, finally } => {
                self.push_scope();
                for s in body { self.check_stmt(s)?; }
                self.pop_scope();

                if err_var != "_" {
                    self.push_scope();
                    self.declare(err_var, Type::String);
                    for s in catch { self.check_stmt(s)?; }
                    self.pop_scope();
                }

                if let Some(fin) = finally {
                    self.push_scope();
                    for s in fin { self.check_stmt(s)?; }
                    self.pop_scope();
                }
                Ok(())
            }
            Stmt::Throw(e) => {
                let t = self.infer(e)?;
                match t {
                    None => self.err("throw requer uma mensagem não-nula"),
                    Some(t) => {
                        if !matches!(unwrap(&t), Type::String) {
                            self.err(format!("throw espera string, recebeu {:?}", t))
                        } else { Ok(()) }
                    }
                }
            }
            Stmt::Block(stmts) => {
                self.push_scope();
                for s in stmts { self.check_stmt(s)?; }
                self.pop_scope();
                Ok(())
            }
            Stmt::Fn(decl) => self.check_fn(decl),
            Stmt::Return(opt) => self.check_return(opt.as_ref()),
        }
    }

    fn require_bool(&mut self, expr: &Expr, ctx: &str) -> Result<(), TypeError> {
        let t = self.infer(expr)?;
        match t {
            None => self.err(format!("condição do '{}' não pode ser null", ctx)),
            Some(t) => {
                if !matches!(unwrap(&t), Type::Bool) {
                    self.err(format!("condição do '{}' deve ser bool, recebeu {:?}", ctx, t))
                } else { Ok(()) }
            }
        }
    }

    fn check_fn(&mut self, decl: &FnDecl) -> Result<(), TypeError> {
        let saved_return = self.current_return.clone();
        let saved_fn = self.current_fn.clone();
        let saved_scopes = std::mem::take(&mut self.scopes);
        let saved_loop = self.loop_depth;
        self.loop_depth = 0;
        self.current_return = Some(decl.ret.clone());
        self.current_fn = Some(decl.name.clone());
        self.push_scope();
        for p in &decl.params { self.declare(&p.name, p.ty.clone()); }
        for s in &decl.body { self.check_stmt(s)?; }
        self.pop_scope();
        self.scopes = saved_scopes;
        self.current_return = saved_return;
        self.current_fn = saved_fn;
        self.loop_depth = saved_loop;
        Ok(())
    }

    fn check_return(&mut self, opt: Option<&Expr>) -> Result<(), TypeError> {
        let expected = match &self.current_return {
            None => return self.err("'return' fora de função"),
            Some(t) => t.clone(),
        };
        match (opt, expected) {
            (None, None) => Ok(()),
            (None, Some(t)) => {
                if !matches!(t, Type::Nullable(_)) {
                    self.err(format!("'return' sem valor, mas a função deve retornar {:?}", t))
                } else { Ok(()) }
            }
            (Some(e), None) => { self.infer(e)?; Ok(()) }
            (Some(e), Some(t)) => {
                let inferred = self.infer(e)?;
                match inferred {
                    None => {
                        if !matches!(t, Type::Nullable(_)) {
                            self.err(format!("função deve retornar {:?}, mas 'return' retorna null", t))
                        } else { Ok(()) }
                    }
                    Some(actual) => {
                        if !compatible(&t, &actual) {
                            self.err(format!("função deve retornar {:?}, mas 'return' retorna {:?}", t, actual))
                        } else { Ok(()) }
                    }
                }
            }
        }
    }

    fn infer(&mut self, expr: &Expr) -> Result<Option<Type>, TypeError> {
        match expr {
            Expr::IntLit(_) => Ok(Some(Type::Int)),
            Expr::FloatLit(_) => Ok(Some(Type::Float)),
            Expr::BoolLit(_) => Ok(Some(Type::Bool)),
            Expr::StringLit(_) => Ok(Some(Type::String)),
            Expr::NullLit => Ok(None),
            Expr::Ident(name) => {
                let t = self.lookup(name).cloned().ok_or_else(|| {
                    let mut msg = format!("variável '{}' não definida", name);
                    if let Some(f) = &self.current_fn { msg = format!("em função '{}': {}", f, msg); }
                    TypeError { msg }
                })?;
                Ok(Some(t))
            }
            Expr::ArrayLit(items) => {
                if items.is_empty() {
                    return Ok(Some(Type::List(Box::new(Type::Any))));
                }
                let mut elem_ty: Option<Type> = None;
                for item in items {
                    let t = self.infer(item)?;
                    let Some(t) = t else {
                        return Ok(Some(Type::List(Box::new(Type::Any))));
                    };
                    elem_ty = Some(match elem_ty {
                        None => t,
                        Some(prev) => {
                            if compatible(&prev, &t) { prev }
                            else if compatible(&t, &prev) { t }
                            else { Type::Any }
                        }
                    });
                }
                Ok(Some(Type::List(Box::new(elem_ty.unwrap()))))
            }
            Expr::Unary { op, operand } => {
                let vt = self.infer(operand)?;
                let Some(vt) = vt else {
                    return self.err("não é possível aplicar operador unário a null");
                };
                let inner = unwrap(&vt).clone();
                match op {
                    UnOp::Neg => {
                        if !matches!(inner, Type::Int | Type::Float | Type::Decimal) {
                            self.err(format!("operador '-' requer número, recebeu {:?}", vt))
                        } else { Ok(Some(inner)) }
                    }
                    UnOp::Not => {
                        if !matches!(inner, Type::Bool) {
                            self.err(format!("operador '!' requer bool, recebeu {:?}", vt))
                        } else { Ok(Some(Type::Bool)) }
                    }
                }
            }
            Expr::Binary { left, op, right } => self.check_binary(left, *op, right),
            Expr::Call { callee, args } => self.check_call(callee, args),
            Expr::Cast { expr, ty } => {
                let t = self.infer(expr)?;
                if matches!(ty, Type::Date | Type::Time | Type::DateTime) {
                    match &t {
                        Some(Type::String) | None | Some(Type::Any) => {}
                        Some(other) => {
                            return self.err(format!("cast para {:?} requer string, recebeu {:?}", ty, other));
                        }
                    }
                }
                Ok(Some(ty.clone()))
            }
            Expr::Index { obj, index } => {
                let ot = self.infer(obj)?;
                let it = self.infer(index)?;
                let Some(ot) = ot else {
                    return self.err("não é possível indexar null");
                };
                let base = unwrap(&ot).clone();
                match base {
                    Type::Row => {
                        if !matches!(it, Some(Type::String)) {
                            return self.err("índice de Row deve ser string");
                        }
                        Ok(Some(Type::Any))
                    }
                    Type::List(inner) => {
                        if !matches!(it, Some(Type::Int)) {
                            return self.err("índice de lista deve ser int");
                        }
                        Ok(Some(*inner))
                    }
                    Type::Any => Ok(Some(Type::Any)),
                    other => self.err(format!("não é possível indexar tipo {:?}", other)),
                }
            }
        }
    }

    fn check_binary(&mut self, left: &Expr, op: BinOp, right: &Expr) -> Result<Option<Type>, TypeError> {
        let lt = self.infer(left)?;
        let rt = self.infer(right)?;

        if matches!(op, BinOp::Eq | BinOp::Neq) && (lt.is_none() || rt.is_none()) {
            return Ok(Some(Type::Bool));
        }

        let lt = match lt { Some(t) => t, None => return self.err("operando esquerdo é null") };
        let rt = match rt { Some(t) => t, None => return self.err("operando direito é null") };

        if matches!(lt, Type::Any) || matches!(rt, Type::Any) {
            return Ok(Some(Type::Any));
        }

        // Detecta nulabilidade antes de unwrapar (Patch 4b)
        let any_nullable = matches!(lt, Type::Nullable(_)) || matches!(rt, Type::Nullable(_));

        let l_inner = unwrap(&lt).clone();
        let r_inner = unwrap(&rt).clone();

        let base = match op {
            BinOp::And | BinOp::Or => {
                if !matches!(l_inner, Type::Bool) || !matches!(r_inner, Type::Bool) {
                    return self.err(format!("operador {:?} requer bool (recebeu {:?} e {:?})", op, lt, rt));
                }
                Type::Bool
            }
            BinOp::Add => match (&l_inner, &r_inner) {
                (Type::Int, Type::Int) => Type::Int,
                (Type::Float, Type::Float) => Type::Float,
                (Type::Int, Type::Float) | (Type::Float, Type::Int) => Type::Float,
                (Type::Decimal, Type::Decimal) => Type::Decimal,
                (Type::Decimal, Type::Int) | (Type::Int, Type::Decimal) => Type::Decimal,
                (Type::Decimal, Type::Float) | (Type::Float, Type::Decimal) => Type::Decimal,
                (Type::String, Type::String) => Type::String,
                _ => return self.err(format!("'+' inválido entre {:?} e {:?}", lt, rt)),
            },
            BinOp::Sub | BinOp::Mul => match (&l_inner, &r_inner) {
                (Type::Int, Type::Int) => Type::Int,
                (Type::Float, Type::Float) => Type::Float,
                (Type::Int, Type::Float) | (Type::Float, Type::Int) => Type::Float,
                (Type::Decimal, Type::Decimal) => Type::Decimal,
                (Type::Decimal, Type::Int) | (Type::Int, Type::Decimal) => Type::Decimal,
                (Type::Decimal, Type::Float) | (Type::Float, Type::Decimal) => Type::Decimal,
                _ => return self.err(format!("operador aritmético inválido entre {:?} e {:?}", lt, rt)),
            },
            BinOp::Div | BinOp::Mod => match (&l_inner, &r_inner) {
                (Type::Int, Type::Int) => Type::Int,
                (Type::Float, Type::Float) => Type::Float,
                (Type::Int, Type::Float) | (Type::Float, Type::Int) => Type::Float,
                (Type::Decimal, Type::Decimal) => Type::Decimal,
                (Type::Decimal, Type::Int) | (Type::Int, Type::Decimal) => Type::Decimal,
                (Type::Decimal, Type::Float) | (Type::Float, Type::Decimal) => Type::Decimal,
                _ => return self.err(format!("operador aritmético inválido entre {:?} e {:?}", lt, rt)),
            },
            BinOp::Eq | BinOp::Neq => Type::Bool,
            BinOp::Lt | BinOp::Lte | BinOp::Gt | BinOp::Gte => {
                let ok = matches!(
                    (&l_inner, &r_inner),
                    (&Type::Int, &Type::Int)
                        | (&Type::Float, &Type::Float)
                        | (&Type::Int, &Type::Float)
                        | (&Type::Float, &Type::Int)
                        | (&Type::Decimal, &Type::Decimal)
                        | (&Type::Decimal, &Type::Int)
                        | (&Type::Int, &Type::Decimal)
                        | (&Type::Decimal, &Type::Float)
                        | (&Type::Float, &Type::Decimal)
                        | (&Type::String, &Type::String)
                        | (&Type::Date, &Type::Date)
                        | (&Type::Time, &Type::Time)
                        | (&Type::DateTime, &Type::DateTime)
                );
                if !ok {
                    return self.err(format!("comparação inválida entre {:?} e {:?}", lt, rt));
                }
                Type::Bool
            }
        };

        match op {
            BinOp::Eq | BinOp::Neq | BinOp::And | BinOp::Or => Ok(Some(base)),
            _ if any_nullable => Ok(Some(Type::Nullable(Box::new(base)))),
            _ => Ok(Some(base)),
        }
    }

    fn check_call(&mut self, callee: &Expr, args: &[Expr]) -> Result<Option<Type>, TypeError> {
        let Expr::Ident(name) = callee else {
            return self.err("alvo de chamada deve ser um identificador");
        };

        if name == "print" {
            for a in args { self.infer(a)?; }
            return Ok(None);
        }

        if name == "exit" {
            if args.len() > 1 { return self.err("exit espera 0 ou 1 argumento (código de saída)"); }
            if args.len() == 1 {
                let t = self.infer(&args[0])?;
                if !matches!(t, Some(Type::Int)) {
                    return self.err(format!("exit espera int, recebeu {:?}", t));
                }
            }
            return Ok(None);
        }

        if name == "len" {
            if args.len() != 1 { return self.err("len espera 1 argumento"); }
            let t = self.infer(&args[0])?;
            match t {
                Some(Type::String) | Some(Type::List(_)) | Some(Type::Any) => return Ok(Some(Type::Int)),
                other => return self.err(format!("len espera string ou list, recebeu {:?}", other)),
            }
        }

        if name == "env" {
            if args.len() != 1 { return self.err("env espera 1 argumento (nome da variável)"); }
            let t = self.infer(&args[0])?;
            if !matches!(t, Some(Type::String)) {
                return self.err(format!("env espera string, recebeu {:?}", t));
            }
            return Ok(Some(Type::Nullable(Box::new(Type::String))));
        }

        if name == "now" {
            if !args.is_empty() { return self.err("now não espera argumentos"); }
            return Ok(Some(Type::DateTime));
        }

        if name == "today" {
            if !args.is_empty() { return self.err("today não espera argumentos"); }
            return Ok(Some(Type::Date));
        }

        if let Some(rest) = name.strip_prefix("db.") { return self.check_db_call(rest, args); }
        if let Some(rest) = name.strip_prefix("ai.") { return self.check_ai_call(rest, args); }

        let sig = self.functions.get(name).cloned().ok_or_else(|| {
            let mut msg = format!("função '{}' não definida", name);
            if let Some(f) = &self.current_fn { msg = format!("em função '{}': {}", f, msg); }
            TypeError { msg }
        })?;

        if args.len() != sig.params.len() {
            return self.err(format!("função '{}' espera {} argumento(s), recebeu {}", name, sig.params.len(), args.len()));
        }

        for (i, (a, expected)) in args.iter().zip(sig.params.iter()).enumerate() {
            let inferred = self.infer(a)?;
            match inferred {
                None => {
                    if !matches!(expected, Type::Nullable(_)) {
                        return self.err(format!("argumento {} de '{}' não pode ser null (esperado {:?})", i + 1, name, expected));
                    }
                }
                Some(actual) => {
                    if !compatible(expected, &actual) {
                        return self.err(format!("argumento {} de '{}': esperado {:?}, recebido {:?}", i + 1, name, expected, actual));
                    }
                }
            }
        }

        Ok(sig.ret)
    }

    fn check_db_args(&mut self, fname: &str, args: &[Expr]) -> Result<(), TypeError> {
        if args.is_empty() || args.len() > 2 {
            return self.err(format!("db.{} espera 1 ou 2 argumentos (sql, [params])", fname));
        }
        let t0 = self.infer(&args[0])?;
        if !matches!(t0, Some(Type::String)) {
            return self.err(format!("db.{}: 1º argumento deve ser string (sql), recebeu {:?}", fname, t0));
        }
        if args.len() == 2 {
            let t1 = self.infer(&args[1])?;
            match t1 {
                Some(Type::List(_)) | Some(Type::Any) | None => {}
                other => return self.err(format!("db.{}: 2º argumento deve ser list, recebeu {:?}", fname, other)),
            }
        }
        Ok(())
    }

    fn check_db_call(&mut self, fname: &str, args: &[Expr]) -> Result<Option<Type>, TypeError> {
        match fname {
            "connect" | "connect_pg" | "connect_mysql" => {
                if args.len() != 1 { return self.err(format!("db.{} espera 1 argumento", fname)); }
                let t = self.infer(&args[0])?;
                if !matches!(t, Some(Type::String)) {
                    return self.err(format!("db.{} espera string, recebeu {:?}", fname, t));
                }
                Ok(Some(Type::Connection))
            }
            "connect_default" => {
                if !args.is_empty() { return self.err("db.connect_default não espera argumentos"); }
                Ok(Some(Type::Connection))
            }
            "connect_named" => {
                if args.len() != 1 { return self.err("db.connect_named espera 1 argumento (nome da conexão)"); }
                let t = self.infer(&args[0])?;
                if !matches!(t, Some(Type::String)) {
                    return self.err(format!("db.connect_named espera string, recebeu {:?}", t));
                }
                Ok(Some(Type::Connection))
            }
            "connections" => {
                if !args.is_empty() { return self.err("db.connections não espera argumentos"); }
                Ok(Some(Type::List(Box::new(Type::String))))
            }
            "begin" | "commit" | "rollback" => {
                if !args.is_empty() { return self.err(format!("db.{} não espera argumentos", fname)); }
                Ok(None)
            }
            "execute" => { self.check_db_args(fname, args)?; Ok(Some(Type::Int)) }
            "query" => { self.check_db_args(fname, args)?; Ok(Some(Type::List(Box::new(Type::Row)))) }
            "query_one" => { self.check_db_args(fname, args)?; Ok(Some(Type::Nullable(Box::new(Type::Row)))) }
            other => self.err(format!("db.{} não é suportado", other)),
        }
    }

    fn check_ai_call(&mut self, fname: &str, args: &[Expr]) -> Result<Option<Type>, TypeError> {
        match fname {
            "is_available" => {
                if !args.is_empty() { return self.err("ai.is_available não espera argumentos"); }
                Ok(Some(Type::Bool))
            }
            "ask" => {
                if args.len() != 1 { return self.err("ai.ask espera 1 argumento (prompt)"); }
                let t = self.infer(&args[0])?;
                if !matches!(t, Some(Type::String)) {
                    return self.err(format!("ai.ask espera string, recebeu {:?}", t));
                }
                Ok(Some(Type::String))
            }
            "ask_with_system" => {
                if args.len() != 2 { return self.err("ai.ask_with_system espera 2 argumentos (system, prompt)"); }
                for (i, a) in args.iter().enumerate() {
                    let t = self.infer(a)?;
                    if !matches!(t, Some(Type::String)) {
                        return self.err(format!("ai.ask_with_system: argumento {} deve ser string, recebeu {:?}", i + 1, t));
                    }
                }
                Ok(Some(Type::String))
            }
            "model" => {
                if !args.is_empty() { return self.err("ai.model não espera argumentos"); }
                Ok(Some(Type::String))
            }
            other => self.err(format!("ai.{} não é suportado", other)),
        }
    }
}

fn unwrap(t: &Type) -> &Type {
    match t { Type::Nullable(inner) => unwrap(inner), other => other }
}

fn compatible(expected: &Type, actual: &Type) -> bool {
    match (expected, actual) {
        (Type::Any, _) | (_, Type::Any) => true,
        (Type::Int, Type::Int) => true,
        (Type::Float, Type::Float) => true,
        (Type::Decimal, Type::Decimal) => true,
        (Type::Decimal, Type::Float) => true,
        (Type::Decimal, Type::Int) => true,
        (Type::Bool, Type::Bool) => true,
        (Type::String, Type::String) => true,
        (Type::Date, Type::Date) => true,
        (Type::Time, Type::Time) => true,
        (Type::DateTime, Type::DateTime) => true,
        (Type::Connection, Type::Connection) => true,
        (Type::Row, Type::Row) => true,
        (Type::List(e), Type::List(a)) => compatible(e, a),
        (Type::Nullable(e), Type::Nullable(a)) => compatible(e, a),
        (Type::Nullable(e), a) => compatible(e, a),
        _ => false,
    }
}