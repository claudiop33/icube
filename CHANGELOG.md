# 📋 Changelog

Histórico de versões do Icube. Segue [Semantic Versioning](https://semver.org/).

## [0.7.7] — 2026-10-24

### Adicionado

- **Tipo `decimal`** (`rust_decimal`, 28-29 dígitos de precisão)
  - Aritmética: `+`, `-`, `*`, `/`, `%`
  - Comparação: `==`, `!=`, `<`, `<=`, `>`, `>=`
  - Cast implícito: `int → decimal`, `float → decimal`
  - Cast explícito: `decimal → int`, `decimal → float`
- **Suporte a SQL `NUMERIC` / `DECIMAL`**
  - Antes: colunas NUMERIC viravam `null`
  - Agora: viram `decimal` corretamente
- **Propagação de null estilo SQL** (runtime)
  - `null + 1` → `null` (antes: erro de runtime)
  - `null * 5` → `null`
  - `null > 0` → `null` (falsy em `if`)
  - Exceção: `==` e `!=` comparam direto (`null == null` → `true`)

### Corrigido

- Type checker agora propaga nulabilidade em binárias
  - `int? + int` → `int?` (antes: `int` — unsound)
  - `let y: int = x + 1;` (onde `x: int?`) → **erro de tipo** (antes: passava e explodia em runtime)

## [0.7.6] — 2026-10-24

### Segurança

- 🔴 **Session ID com `OsRng`** (256 bits) — antes: SipHash fraco, vulnerável a hijacking
- 🔴 **Cookie `Secure` condicional ao TLS** — antes: sempre sem `Secure`, mesmo com HTTPS
- 🔴 **`/api/dev/reset-admin` exige token** — antes: só `ICUBE_DEV_RESET=1` liberava
  - Token de env `ICUBE_DEV_RESET_TOKEN` ou arquivo `.icube-reset-token`
  - Comparação em tempo constante (`constant_time_eq`)
- Nova env `ICUBE_INSECURE_COOKIE=1` para desenvolvimento local sem TLS

### Corrigido

- **`db.connect_default()` idempotente** — antes recriava pool a cada chamada (15+ recriações por página no frontend, esgotava o Postgres)
- **`exit()` não capturável por `try/catch`** — antes o sentinela `__ICUBE_EXIT_N__` era interceptado pelo `catch` e mascarava o código de saída
- **REPL stateful** — type checker agora usa `initial_types()` do evaluator, variáveis persistem entre linhas
- **REPL com auto-`;`** — `let x = 42` sem ponto-e-vírgula funciona
- **Exit codes corretos** — scripts com erro retornam `1` (antes: `0`)
- **Soundness de nullable em binárias** — checker rejeita `int? → int` sem cast
- **`Cargo.toml`**: chave `axum` duplicada removida
- **`usuarios.html`**: redirect para `/login.html` (inexistente) trocado para `/`
- **`INDEX_HTML` fallback**: link "Entrar" aponta para `/`
- **EML**: foto redimensionada para 240px; otimização via canvas reduz `.eml` em 10-30×
- **Drop de `Runtime` em contexto async** — antes panica com "Cannot drop a runtime"; agora dropa em `std::thread::spawn`
- **`PoisonError` em `Mutex`** — `lock().unwrap()` trocado por `lock().unwrap_or_else(|e| e.into_inner())` em todos os lugares
- **Pool de conexões** — `max_connections(2)` (era 5) + `acquire_timeout(5s)` em cada `DbContext`

### Melhorias

- `[profile.release]` com LTO (`lto="thin"`), `codegen-units=1`, `strip=true` — 20-40% mais rápido
- Banner do servidor mostra `Cookie Secure: SIM/NÃO`
- Nova crate `rand = "0.8"` para geração de session ID

## [0.7.5] — 2026-10-20

### Adicionado

- **Sessões web persistentes** (`session_id` via cookie `icube_session`)
- **Autenticação Argon2id** com 3 papéis: `administrator`, `colaborador`, `visitante`
- **`finally` no `try/catch`** — executa sempre, mesmo com `return`/`break`/`continue`
- **Rate limiting** (30 req/min por IP em `/api/login` e `/execute`)
- Endpoints `POST /api/login`, `POST /api/logout`, `GET /api/me`
- CRUD de usuários via `/api/users/*` (admin only)
- `POST /session/reset`, `GET /session/info`
- Env `ICUBE_REQUIRE_AUTH` (exige login em `/execute`)
- Env `ICUBE_DEV_RESET` (bootstrap de senha do admin)
- Cookie `Secure` opcional

### Corrigido

- `try { } finally { }` sem catch (não compilava)
- Erros de tipo não enxergavam variáveis de sessão web

## [0.7.4] — 2026-10-18

### Adicionado

- `db.begin()`, `db.commit()`, `db.rollback()`
- `enum TxConn` + `Drop for DbContext` (rollback automático em transação aberta)
- Detecção de transação aninhada (erro claro)

## [0.7.3] — 2026-10-15

### Adicionado

- Servidor estático Apache-like (index.html, 404 customizado)
- HTTPS self-signed (`ICUBE_TLS_AUTO=1`) + certificado real (`ICUBE_TLS_CERT` / `ICUBE_TLS_KEY`)
- Env `ICUBE_WWW_DIR` (diretório de estáticos)

## [0.7.2] — 2026-10-10

### Adicionado

- Tipos `date`, `time`, `datetime`
- Built-ins `today()`, `now()`
- Casts para datas (`"2026-01-15" as date`)
- Arrays heterogêneos (`[1, "dois", true]` → `list<any>`)

## [0.7.1] — 2026-10-05

### Adicionado

- Auto-load de `.env` via `dotenvy`
- `db.connect_default()`, `db.connect_named()`, `db.connections()`
- `exit(codigo)`
- `config.rs` com parser do `icube.toml`

## [0.7.0] — 2026-10-01

### Adicionado

- `try` / `catch` / `throw`
- `len()`
- `Flow` (return/break/continue em funções)

## [0.6] — 2026-09-20

### Adicionado

- Array literals (`[1, 2, 3]`)
- `break` / `continue`
- `env("VAR")`
- Timeout de execução no servidor

## [0.5] — 2026-09-10

### Adicionado

- Servidor HTTP (axum)
- Playground HTML
- Integração com Ollama

## [0.4] — 2026-08-25

### Adicionado

- Parâmetros seguros em queries (`$1`, `$2`, ...)
- `for` loop (`for x in lista`)
- Suporte a MySQL

## [0.3] — 2026-08-10

### Adicionado

- PostgreSQL via sqlx
- Tipos `Row`, `List`, `Connection`
- `db.connect`, `db.execute`, `db.query`, `db.query_one`

## [0.2] — 2026-07-25

### Adicionado

- Type checker estático

## [0.1] — 2026-07-10

### Adicionado

- Pipeline completo (Lexer → Parser → TypeChecker → Evaluator)
- Tipos primitivos: `int`, `float`, `bool`, `string`
- Nulidade (`T?`)
- `if/else`, `while`, `fn`, `return`

---

## Formato

- **Adicionado** — novas features
- **Corrigido** — bug fixes
- **Segurança** — vulnerabilidades corrigidas
- **Melhorias** — otimizações, DX, dependências
- **Removido** — features removidas (breaking)

## Versionamento

O Icube segue [SemVer](https://semver.org/):
- **Major** (1.0.0) — mudanças incompatíveis
- **Minor** (0.X.0) — features compatíveis
- **Patch** (0.0.X) — correções compatíveis

Até 1.0.0, a API pode mudar em versões minor. Sempre leia o changelog antes de atualizar.
