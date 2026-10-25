use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;
use sqlx::{Column, Row};
use sqlx::{PgPool, MySqlPool};
use sqlx::postgres::PgRow;
use sqlx::mysql::MySqlRow;
use chrono::{NaiveDate, NaiveTime, DateTime, Utc};

use crate::ai::AiContext;
use crate::config::Config;
use crate::parser::{BinOp, Expr, FnDecl, Stmt, Type, UnOp};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Date(NaiveDate),
    Time(NaiveTime),
    DateTime(DateTime<Utc>),
    Null,
    Row(HashMap<String, Value>),
    List(Vec<Value>),
}

#[derive(Debug)]
pub struct EvalError(pub String);

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

enum Flow { Normal, Return(Value), Break, Continue }

enum DbPool {
    Postgres(PgPool),
    Mysql(MySqlPool),
}

enum TxConn {
    Pg(sqlx::pool::PoolConnection<sqlx::Postgres>),
    MySql(sqlx::pool::PoolConnection<sqlx::MySql>),
}

pub struct DbContext {
    runtime: Option<Runtime>,
    pool: Option<DbPool>,
    tx: Option<TxConn>,
}

impl DbContext {
    pub fn new() -> Self {
        Self { runtime: None, pool: None, tx: None }
    }

    /// Retorna true se já existe um pool ativo.
    /// Usado por `db.connect_default()` para evitar recriar pool a cada chamada.
    pub fn is_connected(&self) -> bool {
        self.pool.is_some()
    }

    fn ensure_runtime(&mut self) -> Result<&Runtime, EvalError> {
        if self.runtime.is_none() {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| EvalError(format!("falha ao criar runtime tokio: {}", e)))?;
            self.runtime = Some(rt);
        }
        Ok(self.runtime.as_ref().unwrap())
    }

    pub fn connect_pg(&mut self, url: &str) -> Result<(), EvalError> {
        if self.tx.is_some() {
            return Err(EvalError("não é possível reconectar durante uma transação; chame db.commit() ou db.rollback() primeiro".into()));
        }
        let rt = self.ensure_runtime()?;
        let pool = rt.block_on(async {
            sqlx::postgres::PgPoolOptions::new()
                .max_connections(2)
                .acquire_timeout(Duration::from_secs(5))
                .connect(url)
                .await
        }).map_err(|e| EvalError(format!("falha ao conectar ao PostgreSQL: {}", e)))?;
        self.pool = Some(DbPool::Postgres(pool));
        Ok(())
    }

    pub fn connect_mysql(&mut self, url: &str) -> Result<(), EvalError> {
        if self.tx.is_some() {
            return Err(EvalError("não é possível reconectar durante uma transação; chame db.commit() ou db.rollback() primeiro".into()));
        }
        let rt = self.ensure_runtime()?;
        let pool = rt.block_on(async {
            sqlx::mysql::MySqlPoolOptions::new()
                .max_connections(2)
                .acquire_timeout(Duration::from_secs(5))
                .connect(url)
                .await
        }).map_err(|e| EvalError(format!("falha ao conectar ao MySQL: {}", e)))?;
        self.pool = Some(DbPool::Mysql(pool));
        Ok(())
    }

    fn require_pool(&self) -> Result<&DbPool, EvalError> {
        self.pool.as_ref().ok_or_else(|| {
            EvalError("db não conectado; chame db.connect_default() ou db.connect(url) primeiro".into())
        })
    }

    fn require_runtime(&self) -> Result<&Runtime, EvalError> {
        self.runtime.as_ref().ok_or_else(|| EvalError("db não inicializado".into()))
    }

    pub fn begin(&mut self) -> Result<(), EvalError> {
        if self.tx.is_some() {
            return Err(EvalError("transação já em andamento; chame db.commit() ou db.rollback() antes de iniciar outra".into()));
        }
        self.ensure_runtime()?;
        let rt = self.runtime.as_ref().unwrap();
        let pool = self.pool.as_ref().ok_or_else(|| {
            EvalError("db não conectado; chame db.connect_default() ou db.connect(url) primeiro".into())
        })?;

        let new_tx = match pool {
            DbPool::Postgres(p) => {
                let conn = rt.block_on(async {
                    let mut conn = p.acquire().await
                        .map_err(|e| EvalError(format!("falha ao adquirir conexão: {}", e)))?;
                    sqlx::query("BEGIN").execute(&mut *conn).await
                        .map_err(|e| EvalError(format!("erro ao iniciar transação: {}", e)))?;
                    Ok::<_, EvalError>(conn)
                })?;
                TxConn::Pg(conn)
            }
            DbPool::Mysql(p) => {
                let conn = rt.block_on(async {
                    let mut conn = p.acquire().await
                        .map_err(|e| EvalError(format!("falha ao adquirir conexão: {}", e)))?;
                    sqlx::query("BEGIN").execute(&mut *conn).await
                        .map_err(|e| EvalError(format!("erro ao iniciar transação: {}", e)))?;
                    Ok::<_, EvalError>(conn)
                })?;
                TxConn::MySql(conn)
            }
        };
        self.tx = Some(new_tx);
        Ok(())
    }

    pub fn commit(&mut self) -> Result<(), EvalError> {
        let Some(tx) = self.tx.take() else {
            return Err(EvalError("db.commit() sem transação ativa; chame db.begin() primeiro".into()));
        };
        let rt = self.require_runtime()?;
        rt.block_on(async move {
            match tx {
                TxConn::Pg(mut conn) => {
                    sqlx::query("COMMIT").execute(&mut *conn).await
                        .map_err(|e| EvalError(format!("erro ao confirmar transação: {}", e)))?;
                    Ok(())
                }
                TxConn::MySql(mut conn) => {
                    sqlx::query("COMMIT").execute(&mut *conn).await
                        .map_err(|e| EvalError(format!("erro ao confirmar transação: {}", e)))?;
                    Ok(())
                }
            }
        })
    }

    pub fn rollback(&mut self) -> Result<(), EvalError> {
        let Some(tx) = self.tx.take() else {
            return Err(EvalError("db.rollback() sem transação ativa; chame db.begin() primeiro".into()));
        };
        let rt = self.require_runtime()?;
        rt.block_on(async move {
            match tx {
                TxConn::Pg(mut conn) => {
                    sqlx::query("ROLLBACK").execute(&mut *conn).await
                        .map_err(|e| EvalError(format!("erro ao desfazer transação: {}", e)))?;
                    Ok(())
                }
                TxConn::MySql(mut conn) => {
                    sqlx::query("ROLLBACK").execute(&mut *conn).await
                        .map_err(|e| EvalError(format!("erro ao desfazer transação: {}", e)))?;
                    Ok(())
                }
            }
        })
    }

    pub fn execute(&mut self, sql: &str, params: &[Value]) -> Result<i64, EvalError> {
        if self.runtime.is_none() {
            self.ensure_runtime()?;
        }

        if self.tx.is_some() {
            let rt = self.runtime.as_ref().unwrap();
            let tx = self.tx.as_mut().unwrap();
            return match tx {
                TxConn::Pg(conn) => {
                    let mut q = sqlx::query::<sqlx::Postgres>(sql);
                    for v in params { q = bind_pg(q, v)?; }
                    let r = rt.block_on(async { q.execute(&mut **conn).await })
                        .map_err(|e| EvalError(format!("erro SQL: {}", e)))?;
                    Ok(r.rows_affected() as i64)
                }
                TxConn::MySql(conn) => {
                    let mut q = sqlx::query::<sqlx::MySql>(sql);
                    for v in params { q = bind_mysql(q, v)?; }
                    let r = rt.block_on(async { q.execute(&mut **conn).await })
                        .map_err(|e| EvalError(format!("erro SQL: {}", e)))?;
                    Ok(r.rows_affected() as i64)
                }
            };
        }

        let rt = self.runtime.as_ref().unwrap();
        let pool = self.pool.as_ref().ok_or_else(|| {
            EvalError("db não conectado; chame db.connect_default() ou db.connect(url) primeiro".into())
        })?;
        match pool {
            DbPool::Postgres(p) => {
                let mut q = sqlx::query::<sqlx::Postgres>(sql);
                for v in params { q = bind_pg(q, v)?; }
                let r = rt.block_on(async { q.execute(p).await })
                    .map_err(|e| EvalError(format!("erro SQL: {}", e)))?;
                Ok(r.rows_affected() as i64)
            }
            DbPool::Mysql(p) => {
                let mut q = sqlx::query::<sqlx::MySql>(sql);
                for v in params { q = bind_mysql(q, v)?; }
                let r = rt.block_on(async { q.execute(p).await })
                    .map_err(|e| EvalError(format!("erro SQL: {}", e)))?;
                Ok(r.rows_affected() as i64)
            }
        }
    }

    pub fn query(&mut self, sql: &str, params: &[Value]) -> Result<Vec<Value>, EvalError> {
        if self.runtime.is_none() {
            self.ensure_runtime()?;
        }

        if self.tx.is_some() {
            let rt = self.runtime.as_ref().unwrap();
            let tx = self.tx.as_mut().unwrap();
            return match tx {
                TxConn::Pg(conn) => {
                    let mut q = sqlx::query::<sqlx::Postgres>(sql);
                    for v in params { q = bind_pg(q, v)?; }
                    let rows = rt.block_on(async { q.fetch_all(&mut **conn).await })
                        .map_err(|e| EvalError(format!("erro SQL: {}", e)))?;
                    Ok(rows.iter().map(pg_row_to_value).collect())
                }
                TxConn::MySql(conn) => {
                    let mut q = sqlx::query::<sqlx::MySql>(sql);
                    for v in params { q = bind_mysql(q, v)?; }
                    let rows = rt.block_on(async { q.fetch_all(&mut **conn).await })
                        .map_err(|e| EvalError(format!("erro SQL: {}", e)))?;
                    Ok(rows.iter().map(mysql_row_to_value).collect())
                }
            };
        }

        let rt = self.runtime.as_ref().unwrap();
        let pool = self.pool.as_ref().ok_or_else(|| {
            EvalError("db não conectado; chame db.connect_default() ou db.connect(url) primeiro".into())
        })?;
        match pool {
            DbPool::Postgres(p) => {
                let mut q = sqlx::query::<sqlx::Postgres>(sql);
                for v in params { q = bind_pg(q, v)?; }
                let rows = rt.block_on(async { q.fetch_all(p).await })
                    .map_err(|e| EvalError(format!("erro SQL: {}", e)))?;
                Ok(rows.iter().map(pg_row_to_value).collect())
            }
            DbPool::Mysql(p) => {
                let mut q = sqlx::query::<sqlx::MySql>(sql);
                for v in params { q = bind_mysql(q, v)?; }
                let rows = rt.block_on(async { q.fetch_all(p).await })
                    .map_err(|e| EvalError(format!("erro SQL: {}", e)))?;
                Ok(rows.iter().map(mysql_row_to_value).collect())
            }
        }
    }

    pub fn query_one(&mut self, sql: &str, params: &[Value]) -> Result<Option<Value>, EvalError> {
        if self.runtime.is_none() {
            self.ensure_runtime()?;
        }

        if self.tx.is_some() {
            let rt = self.runtime.as_ref().unwrap();
            let tx = self.tx.as_mut().unwrap();
            return match tx {
                TxConn::Pg(conn) => {
                    let mut q = sqlx::query::<sqlx::Postgres>(sql);
                    for v in params { q = bind_pg(q, v)?; }
                    let row = rt.block_on(async { q.fetch_optional(&mut **conn).await })
                        .map_err(|e| EvalError(format!("erro SQL: {}", e)))?;
                    Ok(row.as_ref().map(pg_row_to_value))
                }
                TxConn::MySql(conn) => {
                    let mut q = sqlx::query::<sqlx::MySql>(sql);
                    for v in params { q = bind_mysql(q, v)?; }
                    let row = rt.block_on(async { q.fetch_optional(&mut **conn).await })
                        .map_err(|e| EvalError(format!("erro SQL: {}", e)))?;
                    Ok(row.as_ref().map(mysql_row_to_value))
                }
            };
        }

        let rt = self.runtime.as_ref().unwrap();
        let pool = self.pool.as_ref().ok_or_else(|| {
            EvalError("db não conectado; chame db.connect_default() ou db.connect(url) primeiro".into())
        })?;
        match pool {
            DbPool::Postgres(p) => {
                let mut q = sqlx::query::<sqlx::Postgres>(sql);
                for v in params { q = bind_pg(q, v)?; }
                let row = rt.block_on(async { q.fetch_optional(p).await })
                    .map_err(|e| EvalError(format!("erro SQL: {}", e)))?;
                Ok(row.as_ref().map(pg_row_to_value))
            }
            DbPool::Mysql(p) => {
                let mut q = sqlx::query::<sqlx::MySql>(sql);
                for v in params { q = bind_mysql(q, v)?; }
                let row = rt.block_on(async { q.fetch_optional(p).await })
                    .map_err(|e| EvalError(format!("erro SQL: {}", e)))?;
                Ok(row.as_ref().map(mysql_row_to_value))
            }
        }
    }
}

/// Garante que uma transação aberta seja dropada dentro do runtime e que o
/// próprio Runtime seja dropado fora do contexto async (evita panic
/// "Cannot drop a runtime in a context where blocking is not allowed").
impl Drop for DbContext {
    fn drop(&mut self) {
        // 1. Rollback de qualquer transação pendente, usando o runtime atual.
        let tx = self.tx.take();
        if let Some(tx) = tx {
            if let Some(rt) = self.runtime.as_ref() {
                let _ = rt.block_on(async move {
                    match tx {
                        TxConn::Pg(mut conn) => {
                            let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                        }
                        TxConn::MySql(mut conn) => {
                            let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                        }
                    }
                });
            }
        }

        // 2. Drop do Runtime em thread separada (não-async).
        //    Tokio panica se o Runtime for dropado dentro de contexto async,
        //    que é o que acontece quando o axum descarta a sessão.
        if let Some(rt) = self.runtime.take() {
            std::thread::spawn(move || drop(rt));
        }
    }
}

fn bind_pg<'q>(
    q: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
    v: &Value,
) -> Result<sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>, EvalError> {
    match v {
        Value::Int(i) => Ok(q.bind(*i)),
        Value::Float(f) => Ok(q.bind(*f)),
        Value::Bool(b) => Ok(q.bind(*b)),
        Value::String(s) => Ok(q.bind(s.clone())),
        Value::Date(d) => Ok(q.bind(*d)),
        Value::Time(t) => Ok(q.bind(*t)),
        Value::DateTime(dt) => Ok(q.bind(*dt)),
        Value::Null => Ok(q.bind(Option::<String>::None)),
        other => Err(EvalError(format!("parâmetro não suportado: {:?}", other))),
    }
}

fn bind_mysql<'q>(
    q: sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments>,
    v: &Value,
) -> Result<sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments>, EvalError> {
    match v {
        Value::Int(i) => Ok(q.bind(*i)),
        Value::Float(f) => Ok(q.bind(*f)),
        Value::Bool(b) => Ok(q.bind(*b)),
        Value::String(s) => Ok(q.bind(s.clone())),
        Value::Date(d) => Ok(q.bind(*d)),
        Value::Time(t) => Ok(q.bind(*t)),
        Value::DateTime(dt) => Ok(q.bind(dt.naive_utc())),
        Value::Null => Ok(q.bind(Option::<String>::None)),
        other => Err(EvalError(format!("parâmetro não suportado: {:?}", other))),
    }
}

fn pg_row_to_value(row: &PgRow) -> Value {
    let mut map = HashMap::new();
    for col in row.columns() {
        map.insert(col.name().to_string(), pg_column_to_value(row, col.name()));
    }
    Value::Row(map)
}

fn pg_column_to_value(row: &PgRow, col: &str) -> Value {
    if let Ok(v) = row.try_get::<Option<NaiveDate>, _>(col) {
        return v.map(Value::Date).unwrap_or(Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<NaiveTime>, _>(col) {
        return v.map(Value::Time).unwrap_or(Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<DateTime<Utc>>, _>(col) {
        return v.map(Value::DateTime).unwrap_or(Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<i16>, _>(col) { return v.map(|x| Value::Int(x as i64)).unwrap_or(Value::Null); }
    if let Ok(v) = row.try_get::<Option<i32>, _>(col) { return v.map(|x| Value::Int(x as i64)).unwrap_or(Value::Null); }
    if let Ok(v) = row.try_get::<Option<i64>, _>(col) { return v.map(Value::Int).unwrap_or(Value::Null); }
    if let Ok(v) = row.try_get::<Option<f32>, _>(col) { return v.map(|x| Value::Float(x as f64)).unwrap_or(Value::Null); }
    if let Ok(v) = row.try_get::<Option<f64>, _>(col) { return v.map(Value::Float).unwrap_or(Value::Null); }
    if let Ok(v) = row.try_get::<Option<bool>, _>(col) { return v.map(Value::Bool).unwrap_or(Value::Null); }
    if let Ok(v) = row.try_get::<Option<String>, _>(col) { return v.map(Value::String).unwrap_or(Value::Null); }
    Value::Null
}

fn mysql_row_to_value(row: &MySqlRow) -> Value {
    let mut map = HashMap::new();
    for col in row.columns() {
        map.insert(col.name().to_string(), mysql_column_to_value(row, col.name()));
    }
    Value::Row(map)
}

fn mysql_column_to_value(row: &MySqlRow, col: &str) -> Value {
    if let Ok(v) = row.try_get::<Option<NaiveDate>, _>(col) {
        return v.map(Value::Date).unwrap_or(Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<NaiveTime>, _>(col) {
        return v.map(Value::Time).unwrap_or(Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<chrono::NaiveDateTime>, _>(col) {
        return v.map(|nd| Value::DateTime(DateTime::from_naive_utc_and_offset(nd, Utc))).unwrap_or(Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<i8>, _>(col)  { return v.map(|x| Value::Int(x as i64)).unwrap_or(Value::Null); }
    if let Ok(v) = row.try_get::<Option<u8>, _>(col)  { return v.map(|x| Value::Int(x as i64)).unwrap_or(Value::Null); }
    if let Ok(v) = row.try_get::<Option<i16>, _>(col) { return v.map(|x| Value::Int(x as i64)).unwrap_or(Value::Null); }
    if let Ok(v) = row.try_get::<Option<u16>, _>(col) { return v.map(|x| Value::Int(x as i64)).unwrap_or(Value::Null); }
    if let Ok(v) = row.try_get::<Option<i32>, _>(col) { return v.map(|x| Value::Int(x as i64)).unwrap_or(Value::Null); }
    if let Ok(v) = row.try_get::<Option<u32>, _>(col) { return v.map(|x| Value::Int(x as i64)).unwrap_or(Value::Null); }
    if let Ok(v) = row.try_get::<Option<i64>, _>(col) { return v.map(Value::Int).unwrap_or(Value::Null); }
    if let Ok(v) = row.try_get::<Option<u64>, _>(col) { return v.map(|x| Value::Int(x as i64)).unwrap_or(Value::Null); }
    if let Ok(v) = row.try_get::<Option<f32>, _>(col) { return v.map(|x| Value::Float(x as f64)).unwrap_or(Value::Null); }
    if let Ok(v) = row.try_get::<Option<f64>, _>(col) { return v.map(Value::Float).unwrap_or(Value::Null); }
    if let Ok(v) = row.try_get::<Option<bool>, _>(col) { return v.map(Value::Bool).unwrap_or(Value::Null); }
    if let Ok(v) = row.try_get::<Option<String>, _>(col) { return v.map(Value::String).unwrap_or(Value::Null); }
    Value::Null
}

pub struct Evaluator {
    vars: HashMap<String, Value>,
    functions: HashMap<String, FnDecl>,
    db: DbContext,
    ai: AiContext,
    output: Option<String>,
    deadline: Option<Instant>,
}

impl Evaluator {
    pub fn new() -> Self {
        Self {
            vars: HashMap::new(),
            functions: HashMap::new(),
            db: DbContext::new(),
            ai: AiContext::new(),
            output: None,
            deadline: None,
        }
    }

    pub fn with_output_capture() -> Self {
        let mut e = Self::new();
        e.output = Some(String::new());
        e
    }

    pub fn with_timeout_and_output(timeout: Duration) -> Self {
        let mut e = Self::with_output_capture();
        e.deadline = Some(Instant::now() + timeout);
        e
    }

    pub fn take_output(&mut self) -> String {
        self.output.take().unwrap_or_default()
    }

    // ========================================================================
    // Métodos usados pelo servidor web (sessões)
    // ========================================================================

    /// Limpa variáveis, funções e conexão — usado pelo `POST /session/reset`
    pub fn reset(&mut self) {
        self.vars.clear();
        self.functions.clear();
        self.deadline = None;
        self.db = DbContext::new();
    }

    /// Define um novo prazo de execução (chamado antes de cada `run`)
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.deadline = Some(Instant::now() + timeout);
    }

    /// Número de variáveis no escopo atual — usado por `GET /session/info`
    pub fn vars_count(&self) -> usize {
        self.vars.len()
    }

    /// Inicia captura de output para o buffer interno
    pub fn begin_capture(&mut self) {
        self.output = Some(String::new());
    }

    /// Retorna os tipos das variáveis atuais.
    /// Usado para alimentar o type checker a cada novo request na mesma sessão.
    pub fn initial_types(&self) -> Vec<(String, Type)> {
        self.vars.iter()
            .map(|(k, v)| (k.clone(), value_to_type(v)))
            .collect()
    }

    pub fn run(&mut self, program: &[Stmt]) -> Result<(), EvalError> {
        for s in program {
            if let Stmt::Fn(decl) = s {
                self.functions.insert(decl.name.clone(), decl.clone());
            }
        }
        match self.exec_block(program)? {
            Flow::Normal => Ok(()),
            Flow::Return(_) => Err(EvalError("return fora de função".into())),
            Flow::Break => Err(EvalError("'break' fora de loop".into())),
            Flow::Continue => Err(EvalError("'continue' fora de loop".into())),
        }
    }

    fn check_deadline(&self) -> Result<(), EvalError> {
        if let Some(d) = self.deadline {
            if Instant::now() > d {
                return Err(EvalError("timeout de execução excedido".into()));
            }
        }
        Ok(())
    }

    fn write_output(&mut self, s: &str) {
        if let Some(buf) = &mut self.output {
            buf.push_str(s);
            buf.push('\n');
        } else {
            println!("{}", s);
        }
    }

    fn exec_block(&mut self, stmts: &[Stmt]) -> Result<Flow, EvalError> {
        for s in stmts {
            match self.exec_stmt(s)? {
                Flow::Normal => {}
                other => return Ok(other),
            }
        }
        Ok(Flow::Normal)
    }

    fn exec_stmt(&mut self, stmt: &Stmt) -> Result<Flow, EvalError> {
        self.check_deadline()?;
        match stmt {
            Stmt::Let { name, ty, value } => {
                let v = self.eval_expr(value)?;
                if let Some(expected) = ty {
                    if !type_matches(expected, &v) {
                        return Err(EvalError(format!(
                            "tipo incompatível em '{}': esperado {:?}, recebido {}",
                            name, expected, format_value(&v)
                        )));
                    }
                }
                self.vars.insert(name.clone(), v);
                Ok(Flow::Normal)
            }
            Stmt::Assign { name, value } => {
                if !self.vars.contains_key(name) {
                    return Err(EvalError(format!("variável '{}' não definida", name)));
                }
                let v = self.eval_expr(value)?;
                self.vars.insert(name.clone(), v);
                Ok(Flow::Normal)
            }
            Stmt::Expr(e) => { self.eval_expr(e)?; Ok(Flow::Normal) }
            Stmt::If { cond, then, else_ } => {
                let c = self.eval_expr(cond)?;
                if as_bool(&c) { self.exec_block(then) }
                else if let Some(eb) = else_ { self.exec_block(eb) }
                else { Ok(Flow::Normal) }
            }
            Stmt::While { cond, body } => {
                loop {
                    self.check_deadline()?;
                    if !as_bool(&self.eval_expr(cond)?) { break; }
                    match self.exec_block(body)? {
                        Flow::Normal | Flow::Continue => continue,
                        Flow::Break => break,
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                    }
                }
                Ok(Flow::Normal)
            }
            Stmt::For { var, iter, body } => {
                let it = self.eval_expr(iter)?;
                let Value::List(items) = it else {
                    return Err(EvalError(format!("'for' requer list, recebeu {}", format_value(&it))));
                };
                for item in items {
                    self.check_deadline()?;
                    self.vars.insert(var.clone(), item);
                    match self.exec_block(body)? {
                        Flow::Normal | Flow::Continue => continue,
                        Flow::Break => break,
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                    }
                }
                Ok(Flow::Normal)
            }
            Stmt::Break => Ok(Flow::Break),
            Stmt::Continue => Ok(Flow::Continue),
            Stmt::Try { body, err_var, catch, finally } => {
                // err_var == "_" significa "sem catch" (sintaxe `try { } finally { }`)
                let has_catch = err_var != "_";

                let mut result = match self.exec_block(body) {
                    Ok(flow) => Ok(flow),
                    Err(EvalError(msg)) => {
                        // Exit sentinel NÃO é capturado pelo catch.
                        // Sem isso, `try { exit(1) } catch e {}` sobrescreve o
                        // código de saída e o processo continua rodando.
                        if msg.starts_with("__ICUBE_EXIT_") {
                            Err(EvalError(msg))
                        } else if has_catch {
                            let saved = self.vars.get(err_var).cloned();
                            self.vars.insert(err_var.clone(), Value::String(msg));
                            let r = self.exec_block(catch);
                            match saved {
                                Some(v) => { self.vars.insert(err_var.clone(), v); }
                                None => { self.vars.remove(err_var); }
                            }
                            r
                        } else {
                            // sem catch: guarda o erro para re-lançar após o finally
                            Err(EvalError(msg))
                        }
                    }
                };

                // finally SEMPRE executa (mesmo com return/break/continue).
                if let Some(fin) = finally {
                    match self.exec_block(fin) {
                        Ok(Flow::Normal) => { /* mantém result */ }
                        Ok(other) => result = Ok(other),
                        Err(e) => result = Err(e),
                    }
                }

                result
            }
            Stmt::Throw(e) => {
                let v = self.eval_expr(e)?;
                let msg = match v {
                    Value::String(s) => s,
                    other => format_value(&other),
                };
                Err(EvalError(msg))
            }
            Stmt::Block(stmts) => self.exec_block(stmts),
            Stmt::Fn(_) => Ok(Flow::Normal),
            Stmt::Return(opt) => {
                let v = match opt {
                    Some(e) => self.eval_expr(e)?,
                    None => Value::Null,
                };
                Ok(Flow::Return(v))
            }
        }
    }

    fn eval_expr(&mut self, expr: &Expr) -> Result<Value, EvalError> {
        self.check_deadline()?;
        match expr {
            Expr::IntLit(i) => Ok(Value::Int(*i)),
            Expr::FloatLit(f) => Ok(Value::Float(*f)),
            Expr::BoolLit(b) => Ok(Value::Bool(*b)),
            Expr::StringLit(s) => Ok(Value::String(s.clone())),
            Expr::NullLit => Ok(Value::Null),
            Expr::Ident(n) => self.vars.get(n).cloned()
                .ok_or_else(|| EvalError(format!("variável '{}' não definida", n))),
            Expr::ArrayLit(items) => {
                let mut values = Vec::with_capacity(items.len());
                for item in items { values.push(self.eval_expr(item)?); }
                Ok(Value::List(values))
            }
            Expr::Unary { op, operand } => {
                let v = self.eval_expr(operand)?;
                match op {
                    UnOp::Neg => match v {
                        Value::Int(i) => Ok(Value::Int(-i)),
                        Value::Float(f) => Ok(Value::Float(-f)),
                        _ => Err(EvalError("não é possível negar um não-numérico".into())),
                    },
                    UnOp::Not => Ok(Value::Bool(!as_bool(&v))),
                }
            }
            Expr::Binary { left, op, right } => {
                let l = self.eval_expr(left)?;
                match op {
                    BinOp::And => {
                        if !as_bool(&l) { return Ok(Value::Bool(false)); }
                        return Ok(Value::Bool(as_bool(&self.eval_expr(right)?)));
                    }
                    BinOp::Or => {
                        if as_bool(&l) { return Ok(Value::Bool(true)); }
                        return Ok(Value::Bool(as_bool(&self.eval_expr(right)?)));
                    }
                    _ => {}
                }
                let r = self.eval_expr(right)?;
                self.binary_op(*op, l, r)
            }
            Expr::Call { callee, args } => {
                let Expr::Ident(name) = callee.as_ref() else {
                    return Err(EvalError("alvo de chamada deve ser um identificador".into()));
                };

                if name == "print" {
                    let mut buf = String::new();
                    let mut first = true;
                    for a in args {
                        let v = self.eval_expr(a)?;
                        if !first { buf.push(' '); }
                        buf.push_str(&format_value(&v));
                        first = false;
                    }
                    self.write_output(&buf);
                    return Ok(Value::Null);
                }

                if name == "exit" {
                    let code = if args.is_empty() {
                        0
                    } else {
                        match self.eval_expr(&args[0])? {
                            Value::Int(i) => i as i32,
                            other => return Err(EvalError(format!("exit espera int, recebeu {}", format_value(&other)))),
                        }
                    };
                    return Err(EvalError(format!("__ICUBE_EXIT_{}__", code)));
                }

                if name == "len" {
                    if args.len() != 1 {
                        return Err(EvalError("len espera 1 argumento".into()));
                    }
                    let v = self.eval_expr(&args[0])?;
                    return match v {
                        Value::String(s) => Ok(Value::Int(s.chars().count() as i64)),
                        Value::List(items) => Ok(Value::Int(items.len() as i64)),
                        other => Err(EvalError(format!("len: tipo não suportado: {}", format_value(&other)))),
                    };
                }

                if name == "env" {
                    if args.len() != 1 {
                        return Err(EvalError("env espera 1 argumento".into()));
                    }
                    let key = self.eval_expr(&args[0])?;
                    let Value::String(key) = key else {
                        return Err(EvalError("env espera string".into()));
                    };
                    return Ok(match std::env::var(&key) {
                        Ok(v) => Value::String(v),
                        Err(_) => Value::Null,
                    });
                }

                if name == "now" {
                    if !args.is_empty() { return Err(EvalError("now não espera argumentos".into())); }
                    return Ok(Value::DateTime(Utc::now()));
                }

                if name == "today" {
                    if !args.is_empty() { return Err(EvalError("today não espera argumentos".into())); }
                    return Ok(Value::Date(Utc::now().date_naive()));
                }

                if let Some(rest) = name.strip_prefix("db.") { return self.call_db(rest, args); }
                if let Some(rest) = name.strip_prefix("ai.") { return self.call_ai(rest, args); }

                let Some(decl) = self.functions.get(name).cloned() else {
                    return Err(EvalError(format!("função '{}' não definida", name)));
                };
                if args.len() != decl.params.len() {
                    return Err(EvalError(format!(
                        "função '{}' espera {} argumento(s), recebeu {}",
                        name, decl.params.len(), args.len()
                    )));
                }
                let mut arg_vals = Vec::new();
                for (i, a) in args.iter().enumerate() {
                    let v = self.eval_expr(a)?;
                    let p = &decl.params[i];
                    if !type_matches(&p.ty, &v) {
                        return Err(EvalError(format!(
                            "argumento {} de '{}' deve ser {:?}, recebido {}",
                            i + 1, name, p.ty, format_value(&v)
                        )));
                    }
                    arg_vals.push(v);
                }
                let saved = std::mem::take(&mut self.vars);
                for (p, v) in decl.params.iter().zip(arg_vals) {
                    self.vars.insert(p.name.clone(), v);
                }
                let mut result = Value::Null;
                match self.exec_block(&decl.body)? {
                    Flow::Return(v) => result = v,
                    Flow::Normal => {}
                    Flow::Break => { self.vars = saved; return Err(EvalError("'break' fora de loop em função".into())); }
                    Flow::Continue => { self.vars = saved; return Err(EvalError("'continue' fora de loop em função".into())); }
                }
                self.vars = saved;
                if let Some(ret_ty) = &decl.ret {
                    if !type_matches(ret_ty, &result) {
                        return Err(EvalError(format!(
                            "função '{}' deve retornar {:?}, mas retornou {}",
                            name, ret_ty, format_value(&result)
                        )));
                    }
                }
                Ok(result)
            }
            Expr::Cast { expr, ty } => {
                let v = self.eval_expr(expr)?;
                cast_value(v, ty)
            }
            Expr::Index { obj, index } => {
                let o = self.eval_expr(obj)?;
                let i = self.eval_expr(index)?;
                match (o, i) {
                    (Value::Row(map), Value::String(key)) => {
                        map.get(&key).cloned()
                            .ok_or_else(|| EvalError(format!("coluna '{}' não encontrada", key)))
                    }
                    (Value::List(items), Value::Int(idx)) => {
                        if idx < 0 || idx as usize >= items.len() {
                            return Err(EvalError(format!(
                                "índice {} fora dos limites (len = {})", idx, items.len()
                            )));
                        }
                        Ok(items[idx as usize].clone())
                    }
                    (Value::Null, _) => Err(EvalError("não é possível indexar null".into())),
                    _ => Err(EvalError("indexação inválida".into())),
                }
            }
        }
    }

    fn eval_db_params(&mut self, args: &[Expr]) -> Result<Vec<Value>, EvalError> {
        if args.len() < 2 { return Ok(Vec::new()); }
        let p = self.eval_expr(&args[1])?;
        match p {
            Value::List(items) => Ok(items),
            Value::Null => Ok(Vec::new()),
            other => Err(EvalError(format!(
                "2º argumento de db.* deve ser list, recebeu {}", format_value(&other)
            ))),
        }
    }

    fn call_db(&mut self, fname: &str, args: &[Expr]) -> Result<Value, EvalError> {
        match fname {
            "connect" => {
                if args.len() != 1 {
                    return Err(EvalError("db.connect espera 1 argumento".into()));
                }
                let url = self.eval_expr(&args[0])?;
                let Value::String(url) = url else {
                    return Err(EvalError("db.connect espera string".into()));
                };
                if url.starts_with("postgres://") || url.starts_with("postgresql://") {
                    self.db.connect_pg(&url)?;
                } else if url.starts_with("mysql://") {
                    self.db.connect_mysql(&url)?;
                } else {
                    return Err(EvalError(format!(
                        "URL não reconhecida (deve começar com postgres:// ou mysql://). Recebido: '{}'",
                        &url[..url.len().min(40)]
                    )));
                }
                Ok(Value::Null)
            }
            "connect_default" => {
                if !args.is_empty() {
                    return Err(EvalError("db.connect_default não espera argumentos".into()));
                }
                // Idempotente: se já conectado, não recria o pool
                // (evita 15+ recriações por página no servidor web).
                if self.db.is_connected() {
                    return Ok(Value::Null);
                }
                let url = std::env::var("ICUBE_DB_URL").map_err(|_| EvalError(
                    "ICUBE_DB_URL não configurada. Crie um .env com: ICUBE_DB_URL=postgres://..."
                        .to_string()
                ))?;
                if url.is_empty() {
                    return Err(EvalError("ICUBE_DB_URL está vazia. Edite o .env e preencha.".into()));
                }
                if url.starts_with("postgres://") || url.starts_with("postgresql://") {
                    self.db.connect_pg(&url)?;
                } else if url.starts_with("mysql://") {
                    self.db.connect_mysql(&url)?;
                } else {
                    return Err(EvalError(format!("ICUBE_DB_URL inválida. Recebido: '{}'", &url[..url.len().min(40)])));
                }
                Ok(Value::Null)
            }
            "connect_named" => {
                if args.len() != 1 {
                    return Err(EvalError("db.connect_named espera 1 argumento".into()));
                }
                let name = self.eval_expr(&args[0])?;
                let Value::String(name) = name else {
                    return Err(EvalError("db.connect_named espera string".into()));
                };
                let cfg = Config::load().map_err(EvalError)?
                    .ok_or_else(|| EvalError("icube.toml não encontrado".into()))?;
                let url = cfg.resolve(&name).map_err(EvalError)?;
                if url.starts_with("postgres://") || url.starts_with("postgresql://") {
                    self.db.connect_pg(&url)?;
                } else if url.starts_with("mysql://") {
                    self.db.connect_mysql(&url)?;
                } else {
                    return Err(EvalError(format!("URL da conexão '{}' inválida", name)));
                }
                Ok(Value::Null)
            }
            "connections" => {
                if !args.is_empty() {
                    return Err(EvalError("db.connections não espera argumentos".into()));
                }
                let cfg = Config::load().map_err(EvalError)?;
                let names = cfg.map(|c| c.names()).unwrap_or_default();
                Ok(Value::List(names.into_iter().map(Value::String).collect()))
            }
            "connect_pg" => {
                if args.len() != 1 {
                    return Err(EvalError("db.connect_pg espera 1 argumento".into()));
                }
                let url = self.eval_expr(&args[0])?;
                let Value::String(url) = url else {
                    return Err(EvalError("db.connect_pg espera string".into()));
                };
                self.db.connect_pg(&url)?;
                Ok(Value::Null)
            }
            "connect_mysql" => {
                if args.len() != 1 {
                    return Err(EvalError("db.connect_mysql espera 1 argumento".into()));
                }
                let url = self.eval_expr(&args[0])?;
                let Value::String(url) = url else {
                    return Err(EvalError("db.connect_mysql espera string".into()));
                };
                self.db.connect_mysql(&url)?;
                Ok(Value::Null)
            }
            "begin" => {
                if !args.is_empty() {
                    return Err(EvalError("db.begin não espera argumentos".into()));
                }
                self.db.begin()?;
                Ok(Value::Null)
            }
            "commit" => {
                if !args.is_empty() {
                    return Err(EvalError("db.commit não espera argumentos".into()));
                }
                self.db.commit()?;
                Ok(Value::Null)
            }
            "rollback" => {
                if !args.is_empty() {
                    return Err(EvalError("db.rollback não espera argumentos".into()));
                }
                self.db.rollback()?;
                Ok(Value::Null)
            }
            "execute" | "query" | "query_one" => {
                if args.is_empty() || args.len() > 2 {
                    return Err(EvalError(format!(
                        "db.{} espera 1 ou 2 argumentos (sql, [params])", fname
                    )));
                }
                let sql = self.eval_expr(&args[0])?;
                let Value::String(sql) = sql else {
                    return Err(EvalError(format!("db.{}: 1º argumento deve ser string", fname)));
                };
                let params = self.eval_db_params(args)?;
                match fname {
                    "execute" => Ok(Value::Int(self.db.execute(&sql, &params)?)),
                    "query" => Ok(Value::List(self.db.query(&sql, &params)?)),
                    "query_one" => Ok(self.db.query_one(&sql, &params)?.unwrap_or(Value::Null)),
                    _ => unreachable!(),
                }
            }
            other => Err(EvalError(format!(
                "db.{} não é suportado (connect/connect_default/connect_named/connect_pg/connect_mysql/connections/begin/commit/rollback/execute/query/query_one)", other
            ))),
        }
    }

    fn call_ai(&mut self, fname: &str, args: &[Expr]) -> Result<Value, EvalError> {
        match fname {
            "is_available" => {
                if !args.is_empty() {
                    return Err(EvalError("ai.is_available não espera argumentos".into()));
                }
                Ok(Value::Bool(self.ai.is_available()))
            }
            "ask" => {
                if args.len() != 1 {
                    return Err(EvalError("ai.ask espera 1 argumento (prompt)".into()));
                }
                let prompt = self.eval_expr(&args[0])?;
                let Value::String(prompt) = prompt else {
                    return Err(EvalError("ai.ask espera string".into()));
                };
                let resposta = self.ai.ask(&prompt).map_err(EvalError)?;
                Ok(Value::String(resposta))
            }
            "ask_with_system" => {
                if args.len() != 2 {
                    return Err(EvalError(
                        "ai.ask_with_system espera 2 argumentos (system, prompt)".into()
                    ));
                }
                let sys = self.eval_expr(&args[0])?;
                let prompt = self.eval_expr(&args[1])?;
                let Value::String(sys) = sys else {
                    return Err(EvalError("ai.ask_with_system: system deve ser string".into()));
                };
                let Value::String(prompt) = prompt else {
                    return Err(EvalError("ai.ask_with_system: prompt deve ser string".into()));
                };
                let resposta = self.ai.ask_with_system(&sys, &prompt).map_err(EvalError)?;
                Ok(Value::String(resposta))
            }
            "model" => {
                if !args.is_empty() {
                    return Err(EvalError("ai.model não espera argumentos".into()));
                }
                Ok(Value::String(self.ai.model_name()))
            }
            other => Err(EvalError(format!(
                "ai.{} não é suportado (ask/ask_with_system/is_available/model)", other
            ))),
        }
    }

    fn binary_op(&self, op: BinOp, l: Value, r: Value) -> Result<Value, EvalError> {
        use Value::*;
        match op {
            BinOp::Add => match (l, r) {
                (Int(a), Int(b)) => Ok(Int(a + b)),
                (Float(a), Float(b)) => Ok(Float(a + b)),
                (Float(a), Int(b)) => Ok(Float(a + b as f64)),
                (Int(a), Float(b)) => Ok(Float(a as f64 + b)),
                (String(a), String(b)) => Ok(String(a + &b)),
                _ => Err(EvalError("operandos inválidos para '+'".into())),
            },
            BinOp::Sub => match (l, r) {
                (Int(a), Int(b)) => Ok(Int(a - b)),
                (Float(a), Float(b)) => Ok(Float(a - b)),
                (Float(a), Int(b)) => Ok(Float(a - b as f64)),
                (Int(a), Float(b)) => Ok(Float(a as f64 - b)),
                _ => Err(EvalError("operandos inválidos para '-'".into())),
            },
            BinOp::Mul => match (l, r) {
                (Int(a), Int(b)) => Ok(Int(a * b)),
                (Float(a), Float(b)) => Ok(Float(a * b)),
                (Float(a), Int(b)) => Ok(Float(a * b as f64)),
                (Int(a), Float(b)) => Ok(Float(a as f64 * b)),
                _ => Err(EvalError("operandos inválidos para '*'".into())),
            },
            BinOp::Div => match (l, r) {
                (Int(_), Int(0)) => Err(EvalError("divisão por zero".into())),
                (Int(a), Int(b)) => Ok(Int(a / b)),
                (Float(a), Float(b)) => Ok(Float(a / b)),
                (Float(a), Int(b)) => Ok(Float(a / b as f64)),
                (Int(a), Float(b)) => Ok(Float(a as f64 / b)),
                _ => Err(EvalError("operandos inválidos para '/'".into())),
            },
            BinOp::Mod => match (l, r) {
                (Int(_), Int(0)) => Err(EvalError("divisão por zero".into())),
                (Int(a), Int(b)) => Ok(Int(a % b)),
                _ => Err(EvalError("operandos inválidos para '%'".into())),
            },
            BinOp::Eq => Ok(Bool(l == r)),
            BinOp::Neq => Ok(Bool(l != r)),
            BinOp::Lt => cmp_op(l, r, |o| o == std::cmp::Ordering::Less),
            BinOp::Lte => cmp_op(l, r, |o| o != std::cmp::Ordering::Greater),
            BinOp::Gt => cmp_op(l, r, |o| o == std::cmp::Ordering::Greater),
            BinOp::Gte => cmp_op(l, r, |o| o != std::cmp::Ordering::Less),
            BinOp::And | BinOp::Or => unreachable!(),
        }
    }
}

fn cmp_op<F: Fn(std::cmp::Ordering) -> bool>(l: Value, r: Value, f: F) -> Result<Value, EvalError> {
    use std::cmp::Ordering;
    use Value::*;
    let ord = match (l, r) {
        (Int(a), Int(b)) => a.cmp(&b),
        (Float(a), Float(b)) => a.partial_cmp(&b).unwrap_or(Ordering::Equal),
        (Float(a), Int(b)) => a.partial_cmp(&(b as f64)).unwrap_or(Ordering::Equal),
        (Int(a), Float(b)) => (a as f64).partial_cmp(&b).unwrap_or(Ordering::Equal),
        (String(a), String(b)) => a.cmp(&b),
        (Date(a), Date(b)) => a.cmp(&b),
        (Time(a), Time(b)) => a.cmp(&b),
        (DateTime(a), DateTime(b)) => a.cmp(&b),
        _ => return Err(EvalError("não é possível comparar esses tipos".into())),
    };
    Ok(Bool(f(ord)))
}

fn as_bool(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Null => false,
        Value::Int(i) => *i != 0,
        Value::Float(f) => *f != 0.0,
        Value::String(_) => true,
        Value::Date(_) => true,
        Value::Time(_) => true,
        Value::DateTime(_) => true,
        Value::Row(_) => true,
        Value::List(l) => !l.is_empty(),
    }
}

fn type_matches(ty: &Type, v: &Value) -> bool {
    match ty {
        Type::Int => matches!(v, Value::Int(_)),
        Type::Float => matches!(v, Value::Float(_)),
        Type::Bool => matches!(v, Value::Bool(_)),
        Type::String => matches!(v, Value::String(_)),
        Type::Date => matches!(v, Value::Date(_)),
        Type::Time => matches!(v, Value::Time(_)),
        Type::DateTime => matches!(v, Value::DateTime(_)),
        Type::Connection => false,
        Type::Row => matches!(v, Value::Row(_)),
        Type::List(_) => matches!(v, Value::List(_)),
        Type::Any => true,
        Type::Nullable(inner) => matches!(v, Value::Null) || type_matches(inner, v),
    }
}

/// Converte um `Value` de runtime para seu `Type` estático correspondente.
/// Usado por `Evaluator::initial_types()` para alimentar o type checker
/// a cada novo request na mesma sessão web.
fn value_to_type(v: &Value) -> Type {
    match v {
        Value::Int(_) => Type::Int,
        Value::Float(_) => Type::Float,
        Value::Bool(_) => Type::Bool,
        Value::String(_) => Type::String,
        Value::Date(_) => Type::Date,
        Value::Time(_) => Type::Time,
        Value::DateTime(_) => Type::DateTime,
        Value::Null => Type::Nullable(Box::new(Type::Any)),
        Value::Row(_) => Type::Row,
        Value::List(_) => Type::List(Box::new(Type::Any)),
    }
}

fn cast_value(v: Value, ty: &Type) -> Result<Value, EvalError> {
    match ty {
        Type::Int => match v {
            Value::Int(i) => Ok(Value::Int(i)),
            Value::Float(f) => Ok(Value::Int(f as i64)),
            Value::Bool(b) => Ok(Value::Int(if b { 1 } else { 0 })),
            Value::String(s) => s.parse::<i64>().map(Value::Int)
                .map_err(|_| EvalError("não é possível converter string para int".into())),
            _ => Err(EvalError("conversão para int falhou".into())),
        },
        Type::Float => match v {
            Value::Int(i) => Ok(Value::Float(i as f64)),
            Value::Float(f) => Ok(Value::Float(f)),
            Value::Bool(b) => Ok(Value::Float(if b { 1.0 } else { 0.0 })),
            Value::String(s) => s.parse::<f64>().map(Value::Float)
                .map_err(|_| EvalError("não é possível converter string para float".into())),
            _ => Err(EvalError("conversão para float falhou".into())),
        },
        Type::String => Ok(Value::String(format_value(&v))),
        Type::Bool => Ok(Value::Bool(as_bool(&v))),
        Type::Date => match v {
            Value::Date(d) => Ok(Value::Date(d)),
            Value::DateTime(dt) => Ok(Value::Date(dt.date_naive())),
            Value::String(s) => NaiveDate::parse_from_str(&s, "%Y-%m-%d")
                .map(Value::Date)
                .map_err(|_| EvalError(format!("data inválida: '{}' (use YYYY-MM-DD)", s))),
            other => Err(EvalError(format!("não é possível converter {} para date", format_value(&other)))),
        },
        Type::Time => match v {
            Value::Time(t) => Ok(Value::Time(t)),
            Value::DateTime(dt) => Ok(Value::Time(dt.time())),
            Value::String(s) => {
                if let Ok(t) = NaiveTime::parse_from_str(&s, "%H:%M:%S") {
                    return Ok(Value::Time(t));
                }
                if let Ok(t) = NaiveTime::parse_from_str(&s, "%H:%M") {
                    return Ok(Value::Time(t));
                }
                Err(EvalError(format!("hora inválida: '{}' (use HH:MM:SS ou HH:MM)", s)))
            }
            other => Err(EvalError(format!("não é possível converter {} para time", format_value(&other)))),
        },
        Type::DateTime => match v {
            Value::DateTime(dt) => Ok(Value::DateTime(dt)),
            Value::Date(d) => Ok(Value::DateTime(
                DateTime::from_naive_utc_and_offset(d.and_hms_opt(0, 0, 0).unwrap(), Utc)
            )),
            Value::String(s) => {
                if let Ok(dt) = DateTime::parse_from_rfc3339(&s) {
                    return Ok(Value::DateTime(dt.with_timezone(&Utc)));
                }
                for fmt in &["%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M:%S", "%Y-%m-%d"] {
                    if let Ok(nd) = chrono::NaiveDateTime::parse_from_str(&s, fmt) {
                        return Ok(Value::DateTime(DateTime::from_naive_utc_and_offset(nd, Utc)));
                    }
                    if let Ok(nd) = NaiveDate::parse_from_str(&s, fmt) {
                        return Ok(Value::DateTime(
                            DateTime::from_naive_utc_and_offset(nd.and_hms_opt(0, 0, 0).unwrap(), Utc)
                        ));
                    }
                }
                Err(EvalError(format!("datetime inválido: '{}' (use YYYY-MM-DD ou YYYY-MM-DD HH:MM:SS)", s)))
            }
            other => Err(EvalError(format!("não é possível converter {} para datetime", format_value(&other)))),
        },
        Type::Nullable(inner) => {
            if matches!(v, Value::Null) { Ok(Value::Null) } else { cast_value(v, inner) }
        }
        Type::Any => Ok(v),
        Type::Connection | Type::Row | Type::List(_) => Err(EvalError(format!(
            "conversão para {:?} não é suportada", ty
        ))),
    }
}

pub fn format_value(v: &Value) -> String {
    match v {
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::String(s) => s.clone(),
        Value::Date(d) => d.format("%Y-%m-%d").to_string(),
        Value::Time(t) => t.format("%H:%M:%S").to_string(),
        Value::DateTime(dt) => dt.format("%Y-%m-%d %H:%M:%S+00:00").to_string(),
        Value::Null => "null".to_string(),
        Value::Row(map) => {
            let mut s = String::from("{");
            let mut first = true;
            for (k, v) in map {
                if !first { s.push_str(", "); }
                s.push_str(&format!("\"{}\": {}", k, format_json_value(v)));
                first = false;
            }
            s.push('}');
            s
        }
        Value::List(items) => {
            let mut s = String::from("[");
            let mut first = true;
            for v in items {
                if !first { s.push_str(", "); }
                s.push_str(&format_json_value(v));
                first = false;
            }
            s.push(']');
            s
        }
    }
}

fn format_json_value(v: &Value) -> String {
    match v {
        Value::String(s) => {
            let mut out = String::from("\"");
            for c in s.chars() {
                match c {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    '\t' => out.push_str("\\t"),
                    '\r' => out.push_str("\\r"),
                    c => out.push(c),
                }
            }
            out.push('"');
            out
        }
        Value::Date(_) | Value::Time(_) | Value::DateTime(_) => {
            format!("\"{}\"", format_value(v))
        }
        Value::Row(_) | Value::List(_) => format_value(v),
        _ => format_value(v),
    }
}