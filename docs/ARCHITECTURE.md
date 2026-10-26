# 🏗️ Arquitetura do Icube

Como a linguagem funciona por dentro.

## Pipeline de execução
Código .icb
│
▼
┌─────────┐
│ Lexer │ → Vec<Token>
└─────────┘
│
▼
┌─────────┐
│ Parser │ → Vec<Stmt> (AST)
└─────────┘
│
▼
┌─────────────┐
│ TypeChecker │ → Valida tipos
└─────────────┘
│
▼
┌───────────┐
│ Evaluator │ → Executa
└───────────┘
│
├──→ PostgreSQL / MySQL
├──→ Ollama (IA)
└──→ stdout / buffer

text

## Módulos

| Arquivo | Responsabilidade | Linhas |
|---|---|---|
| `src/lexer.rs` | Texto → tokens | ~280 |
| `src/parser.rs` | Tokens → AST | ~500 |
| `src/type_checker.rs` | Verificação estática | ~700 |
| `src/eval.rs` | Execução + SQL + IA | ~1500 |
| `src/ai.rs` | Cliente Ollama | ~80 |
| `src/config.rs` | Parser do icube.toml | ~80 |
| `src/server.rs` | Servidor axum + auth | ~1000 |
| `src/main.rs` | CLI + REPL | ~180 |

## Lexer

Transforma o texto-fonte em uma lista de tokens.

**Entrada:** `let x = 42;`

**Saída:**
[Let, Ident("x"), Assign, Int(42), Semicolon, Eof]

text

**Tipos de token:**
- Literais: `Int`, `Float`, `String`, `Bool`
- Keywords: `Let`, `Fn`, `If`, `Else`, etc.
- Tipos: `IntType`, `DecimalType`, `StringType`, etc.
- Operadores: `Plus`, `Minus`, `Star`, `Slash`, etc.
- Delimitadores: `LParen`, `RParen`, `LBrace`, etc.

**Tratamento de identificadores com ponto:**
`db.query` é **um único token** `Ident("db.query")`. Isso permite que `db.*` e `ai.*` sejam identificados por prefixo no parser.

## Parser

Constrói a AST (Abstract Syntax Tree) a partir dos tokens.

**Estrutura principal:**

```rust
pub enum Stmt {
    Let { name: String, ty: Option<Type>, value: Expr },
    Assign { name: String, value: Expr },
    If { cond: Expr, then: Vec<Stmt>, else_: Option<Vec<Stmt>> },
    While { cond: Expr, body: Vec<Stmt> },
    For { var: String, iter: Expr, body: Vec<Stmt> },
    Fn(FnDecl),
    Return(Option<Expr>),
    Try { body: Vec<Stmt>, err_var: String, catch: Vec<Stmt>, finally: Option<Vec<Stmt>> },
    Throw(Expr),
    Expr(Expr),
    Block(Vec<Stmt>),
    Break,
    Continue,
}

pub enum Expr {
    IntLit(i64),
    FloatLit(f64),
    BoolLit(bool),
    StringLit(String),
    NullLit,
    Ident(String),
    ArrayLit(Vec<Expr>),
    Binary { left: Box<Expr>, op: BinOp, right: Box<Expr> },
    Unary { op: UnOp, operand: Box<Expr> },
    Call { callee: Box<Expr>, args: Vec<Expr> },
    Cast { expr: Box<Expr>, ty: Type },
    Index { obj: Box<Expr>, index: Box<Expr> },
}
Precedência: implementada com recursive descent parser (mais simples que Pratt, suficiente para a gramática do Icube).

Type Checker
Verifica tipos antes da execução. Roda em duas passadas:

Registrar funções — coleta todas as fn do escopo raiz

Checar statements — valida cada um, propagando tipos em escopos

Estruturas:

rust
pub struct TypeChecker {
    functions: HashMap<String, FuncSig>,  // assinaturas de funções
    scopes: Vec<HashMap<String, Type>>,    // escopos aninhados
    current_return: Option<Option<Type>>,  // tipo de retorno esperado
    current_fn: Option<String>,            // função atual (para erros)
    loop_depth: usize,                     // para validar break/continue
}
Nulidade:
int? é Type::Nullable(Box::new(Type::Int)).

Nullable(T) aceita T e null

Aritmética propaga: int? + int → int? (0.7.6+)

int? + int não pode ser atribuído a int — precisa de cast ou check explícito

Soundness: o checker garante que se compila, executa (sem erros de tipo em runtime). Correção do 0.7.6.

Evaluator
Executa a AST. É o módulo mais complexo.

Tipos de valor em runtime:

rust
pub enum Value {
    Int(i64),
    Float(f64),
    Decimal(Decimal),
    Bool(bool),
    String(String),
    Date(NaiveDate),
    Time(NaiveTime),
    DateTime<Utc>,
    Null,
    Row(HashMap<String, Value>),
    List(Vec<Value>),
}
Controle de fluxo (Flow):

Execução de statements retorna Flow:

Normal — continua

Return(value) — retorna

Break — quebra loop

Continue — continua loop

Nulidade (SQL-style, 0.7.7+):

Operações aritméticas com null retornam null:

icube
null + 1     // null
null * 5     // null
null == null // true
Exceção: == e != comparam direto (retornam bool).

DbContext
Gerencia a conexão com o banco.

rust
pub struct DbContext {
    runtime: Option<Runtime>,   // tokio runtime sob demanda
    pool: Option<DbPool>,       // pool ativo
    tx: Option<TxConn>,         // transação aberta
}

enum DbPool {
    Postgres(PgPool),
    MySql(MySqlPool),
}
Idempotência (0.7.6+):
db.connect_default() não recria pool se já conectado. Antes recriava a cada chamada (bug de pool exhaustion).

Runtime drop (0.7.6+):
Drop for DbContext dropa o runtime em thread separada (std::thread::spawn), não em contexto async. Tokio panica se Runtime for dropado em async context.

Transações:

rust
pub fn begin(&mut self) -> Result<(), EvalError>
pub fn commit(&mut self) -> Result<(), EvalError>
pub fn rollback(&mut self) -> Result<(), EvalError>
Rollback automático em:

Drop do DbContext

Processo morre com tx aberta

exit() com tx aberta

AiContext
Cliente HTTP do Ollama.

rust
pub struct AiContext {
    client: reqwest::blocking::Client,   // timeout 180s
    base_url: String,                    // OLLAMA_HOST
    model: String,                       // ICUBE_AI_MODEL
}
Métodos:

is_available() — GET rápido em /api/tags (timeout 2s)

ask(prompt) — POST em /api/generate

ask_with_system(system, prompt) — com system prompt

model_name() — retorna o modelo atual

Tratamento de erro: mensagens acionáveis ("o serviço está rodando? ollama serve").

Servidor (server.rs)
Servidor HTTP/HTTPS com axum.

Estado compartilhado:

rust
pub struct AppState {
    sessions: Arc<Mutex<HashMap<String, Session>>>,   // sessões
    rate_limits: Arc<Mutex<HashMap<IpAddr, Vec<Instant>>>>, // rate limit por IP
    www_dir: PathBuf,                                 // estáticos
    timeout_secs: u64,                                // timeout de execução
    db_pool: Option<PgPool>,                          // pool de auth
    auth_required: bool,                              // ICUBE_REQUIRE_AUTH
    secure_cookies: bool,                             // Secure condicional
}
Sessão:

rust
struct Session {
    evaluator: Arc<Mutex<Evaluator>>,  // estado persistente
    last_used: Instant,                // TTL
    user: Option<AuthUser>,            // auth
}
Cada sessão tem seu próprio Evaluator. Isso significa que:

Variáveis persistem entre requests

Conexão DB persiste (com is_connected() idempotente)

Funções persistem

Session ID (0.7.6+):

rust
fn new_session_id() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}
64 hex chars, CSPRNG de verdade. Antes era SipHash fraco (vulnerável a hijacking).

Poison recovery (0.7.6+):

Todos os Mutex::lock() usam .unwrap_or_else(|e| e.into_inner()) em vez de .unwrap(). Se uma thread panicar, as outras continuam funcionando.

Cliente Web (www/)
O frontend (equipamentos, cotações, playground) usa a linguagem Icube como protocolo RPC:

JS monta SQL

JS escapa em código Icube (db.query(...))

POST em /execute

Icube executa, retorna output como string

JS parseia (JSON.parse)

Vantagem: toda query passa pelo mesmo caminho seguro (db.query parametrizado).

Desvantagem: overhead de gerar código como string (resolvido no 0.8 com interpolação de string).

Configuração
.env
Carregado automaticamente por dotenvy::dotenv().

icube.toml
Conexões nomeadas:

toml
[connections.producao]
url = "postgres://user:pass@host/db"

[connections.homolog]
url_env = "ICUBE_HOMOLOG_URL"
Uso:

icube
db.connect_named("producao");
Decisões de design
Decisão	Por quê
. faz parte do identificador	Permite db.query sem precisar de field access
Escopo por função (não por bloco)	Mais simples para não-devs (público-alvo)
Sem closures	Mantém o modelo mental simples
Null propagation (SQL-style)	Alinhado com a origem "linguagem de dados"
decimal nativo	Precisão monetária é crítica no domínio
LLM local via Ollama	Privacidade de dados (não envia para cloud)
Servidor web embutido	Zero setup para o usuário final
Arquitetura do Icube — v0.7.7
