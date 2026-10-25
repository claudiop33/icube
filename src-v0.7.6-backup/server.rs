use axum::{
    body::Body,
    extract::{ConnectInfo, Extension, Json, Multipart, Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode, Uri},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Router,
};

use axum_server::tls_rustls::RustlsConfig;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path as FsPath, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tower_http::services::ServeDir;

use crate::eval::Evaluator;

const SESSION_COOKIE: &str = "icube_session";
const SESSION_TTL: Duration = Duration::from_secs(3600);
const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);
const RATE_LIMIT_MAX: usize = 30;

const VALID_ROLES: &[&str] = &["administrator", "colaborador", "visitante"];

#[derive(Clone, Debug, Serialize)]
struct AuthUser {
    id: i64,
    username: String,
    role: String,
}

struct Session {
    evaluator: Arc<Mutex<Evaluator>>,
    last_used: Instant,
    user: Option<AuthUser>,
}

#[derive(Clone)]
struct AppState {
    sessions: Arc<Mutex<HashMap<String, Session>>>,
    rate_limits: Arc<Mutex<HashMap<IpAddr, Vec<Instant>>>>,
    www_dir: PathBuf,
    timeout_secs: u64,
    db_pool: Option<PgPool>,
    auth_required: bool,
    secure_cookies: bool,
}

impl AppState {
    fn new(
        www_dir: PathBuf,
        timeout_secs: u64,
        db_pool: Option<PgPool>,
        auth_required: bool,
        secure_cookies: bool,
    ) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            rate_limits: Arc::new(Mutex::new(HashMap::new())),
            www_dir,
            timeout_secs,
            db_pool,
            auth_required,
            secure_cookies,
        }
    }

    fn check_rate_limit(&self, ip: IpAddr) -> bool {
        let mut limits = self.rate_limits.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        let entries = limits.entry(ip).or_insert_with(Vec::new);
        entries.retain(|t| now.duration_since(*t) < RATE_LIMIT_WINDOW);
        if entries.len() >= RATE_LIMIT_MAX {
            false
        } else {
            entries.push(now);
            true
        }
    }

    fn get_or_create_session(&self, session_id: Option<&str>) -> (String, bool) {
        let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(id) = session_id {
            if let Some(s) = sessions.get_mut(id) {
                s.last_used = Instant::now();
                return (id.to_string(), false);
            }
        }
        let new_id = new_session_id();
        sessions.insert(new_id.clone(), Session {
            evaluator: Arc::new(Mutex::new(Evaluator::new())),
            last_used: Instant::now(),
            user: None,
        });
        (new_id, true)
    }

    fn session_evaluator(&self, session_id: &str) -> Option<Arc<Mutex<Evaluator>>> {
        let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        sessions.get(session_id).map(|s| s.evaluator.clone())
    }

    fn session_user(&self, session_id: &str) -> Option<AuthUser> {
        let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        sessions.get(session_id).and_then(|s| s.user.clone())
    }

    fn set_session_user(&self, session_id: &str, user: AuthUser) {
        let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(s) = sessions.get_mut(session_id) {
            s.user = Some(user);
            s.last_used = Instant::now();
        }
    }

    fn clear_session_user(&self, session_id: &str) {
        let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(s) = sessions.get_mut(session_id) {
            s.user = None;
            if let Ok(mut e) = s.evaluator.lock() {
                e.reset();
            }
        }
    }

    fn cleanup_sessions(&self) {
        let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        sessions.retain(|_, s| now.duration_since(s.last_used) < SESSION_TTL);
        drop(sessions);

        let mut limits = self.rate_limits.lock().unwrap_or_else(|e| e.into_inner());
        limits.retain(|_, v| {
            v.retain(|t| now.duration_since(*t) < RATE_LIMIT_WINDOW);
            !v.is_empty()
        });
    }
}

fn new_session_id() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn parse_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    for part in raw.split(';') {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix(&format!("{}=", name)) {
            return Some(rest.to_string());
        }
    }
    None
}

fn session_cookie(value: &str, max_age_secs: i64, secure: bool) -> String {
    format!(
        "{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}{}",
        SESSION_COOKIE,
        value,
        max_age_secs,
        if secure { "; Secure" } else { "" }
    )
}

fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| format!("erro ao hashear senha: {}", e))
}

fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

fn read_dev_reset_token() -> Option<String> {
    if let Ok(t) = std::env::var("ICUBE_DEV_RESET_TOKEN") {
        let t = t.trim().to_string();
        if !t.is_empty() {
            return Some(t);
        }
    }
    for path in &[".icube-reset-token", "/etc/icube/reset-token"] {
        if let Ok(t) = std::fs::read_to_string(path) {
            let t = t.trim().to_string();
            if !t.is_empty() {
                return Some(t);
            }
        }
    }
    None
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

pub fn serve_blocking(port: u16) -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .map_err(|e| format!("falha ao criar runtime: {}", e))?;
    rt.block_on(serve(port))
}

async fn serve(port: u16) -> Result<(), String> {
    let www_dir = std::env::var("ICUBE_WWW_DIR").unwrap_or_else(|_| "www".to_string());
    let _ = std::fs::create_dir_all(&www_dir);
    let www_path = PathBuf::from(&www_dir);

    let timeout_secs: u64 = std::env::var("ICUBE_TIMEOUT_SECS")
        .ok().and_then(|s| s.parse().ok()).unwrap_or(30);

    let auth_required = std::env::var("ICUBE_REQUIRE_AUTH")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    // TLS precisa ser detectado ANTES do state (para o cookie Secure)
    let tls_cert = std::env::var("ICUBE_TLS_CERT").ok();
    let tls_key = std::env::var("ICUBE_TLS_KEY").ok();
    let tls_auto = std::env::var("ICUBE_TLS_AUTO")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let use_tls = (tls_cert.is_some() && tls_key.is_some()) || tls_auto;

    let insecure_cookie = std::env::var("ICUBE_INSECURE_COOKIE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let secure_cookies = use_tls && !insecure_cookie;

    let db_pool = if let Ok(url) = std::env::var("ICUBE_DB_URL") {
        if url.is_empty() {
            None
        } else {
            match sqlx::postgres::PgPoolOptions::new()
                .max_connections(5)
                .connect(&url).await {
                Ok(pool) => Some(pool),
                Err(e) => {
                    eprintln!("⚠ Aviso: falha ao conectar pool de autenticação: {}", e);
                    None
                }
            }
        }
    } else {
        None
    };

    let state = AppState::new(www_path.clone(), timeout_secs, db_pool, auth_required, secure_cookies);
    let has_db_pool = state.db_pool.is_some();

    let cleanup_state = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(300)).await;
            cleanup_state.cleanup_sessions();
        }
    });

    let serve_dir = ServeDir::new(&www_dir)
        .append_index_html_on_directories(true)
        .not_found_service(get(not_found_handler));

    let app = Router::new()
        .route("/health", get(health))
        .route("/execute", post(execute))
        .route("/api/equipamentos", post(execute))
        .route("/session/reset", post(session_reset))
        .route("/session/info", get(session_info))
        .route("/api/login", post(api_login))
        .route("/api/logout", post(api_logout))
        .route("/api/me", get(api_me))
        .route("/api/upload", post(upload_photo))
        .route("/api/users", get(api_users_list))
        .route("/api/users/create", post(api_users_create))
        .route("/api/users/:id/password", post(api_users_password))
        .route("/api/users/:id/role", post(api_users_role))
        .route("/api/users/:id/delete", post(api_users_delete))
        .route("/api/dev/reset-admin", post(dev_reset_admin))
        .fallback_service(serve_dir)
        .layer(Extension(www_path))
        .with_state(state);

    let scheme = if use_tls { "https" } else { "http" };
    println!("╔══════════════════════════════════════════════╗");
    println!("║  Icube Server v0.7.6                         ║");
    println!("║  {}://localhost:{:<4}                       ║", scheme, port);
    println!("╚══════════════════════════════════════════════╝");
    println!();
    println!("  Diretório estático: {}", www_dir);
    println!("  TLS:                {}", if use_tls { "SIM" } else { "NÃO" });
    println!("  Auth:               {}", if auth_required { "OBRIGATÓRIA" } else { "OPCIONAL" });
    println!("  Cookie Secure:      {}", if secure_cookies { "SIM" } else { "NÃO" });
    println!("  Pool de auth:       {}", if has_db_pool { "OK" } else { "NÃO CONFIGURADO" });
    println!();

    let addr: SocketAddr = format!("0.0.0.0:{}", port)
        .parse()
        .map_err(|e| format!("endereço inválido: {}", e))?;

    let handle = axum_server::Handle::new();
    let handle_clone = handle.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            println!("\nDesligando servidor...");
            handle_clone.graceful_shutdown(Some(Duration::from_secs(30)));
        }
    });

    let svc = app.into_make_service_with_connect_info::<SocketAddr>();

    if use_tls {
        let config = if let (Some(cert), Some(key)) = (tls_cert, tls_key) {
            RustlsConfig::from_pem_file(cert, key).await
                .map_err(|e| format!("erro ao carregar certificado: {}", e))?
        } else {
            let (cert_pem, key_pem) = generate_self_signed()?;
            let _ = std::fs::create_dir_all("certs");
            let _ = std::fs::write("certs/self-signed-cert.pem", &cert_pem);
            let _ = std::fs::write("certs/self-signed-key.pem", &key_pem);
            RustlsConfig::from_pem(cert_pem.into_bytes(), key_pem.into_bytes()).await
                .map_err(|e| format!("erro ao configurar TLS: {}", e))?
        };
        axum_server::bind_rustls(addr, config)
            .handle(handle).serve(svc).await
            .map_err(|e| format!("erro no servidor: {}", e))?;
    } else {
        axum_server::bind(addr)
            .handle(handle).serve(svc).await
            .map_err(|e| format!("erro no servidor: {}", e))?;
    }

    Ok(())
}

fn generate_self_signed() -> Result<(String, String), String> {
    use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair, SanType};
    use time::OffsetDateTime;

    let mut params = CertificateParams::new(vec![
        "localhost".to_string(), "127.0.0.1".to_string(), "::1".to_string(),
    ]).map_err(|e| format!("params: {}", e))?;

    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, "localhost");
    dn.push(DnType::OrganizationName, "Icube Dev");
    params.distinguished_name = dn;

    let now = OffsetDateTime::now_utc();
    params.not_before = now;
    params.not_after = now + time::Duration::days(365);

    params.subject_alt_names = vec![
        SanType::DnsName("localhost".try_into().map_err(|e| format!("dns: {}", e))?),
        SanType::IpAddress("127.0.0.1".parse().map_err(|e| format!("ip4: {}", e))?),
        SanType::IpAddress("::1".parse().map_err(|e| format!("ip6: {}", e))?),
    ];

    let kp = KeyPair::generate().map_err(|e| format!("chave: {}", e))?;
    let cert = params.self_signed(&kp).map_err(|e| format!("cert: {}", e))?;
    Ok((cert.pem(), kp.serialize_pem()))
}

async fn health() -> &'static str { "ok" }

async fn not_found_handler(uri: Uri, Extension(www_dir): Extension<PathBuf>) -> Response {
    let path_str = uri.path();
    if path_str == "/" || path_str.is_empty() {
        return Html(INDEX_HTML).into_response();
    }
    let rel = path_str.trim_start_matches('/');
    if !rel.contains('.') && !rel.is_empty() {
        let html = www_dir.join(format!("{}.html", rel));
        if html.is_file() { return serve_existing_file(&html).await; }
        let index = www_dir.join(rel).join("index.html");
        if index.is_file() { return serve_existing_file(&index).await; }
    }
    Html(NOT_FOUND_HTML).into_response()
}

async fn serve_existing_file(path: &FsPath) -> Response {
    match tokio::fs::read(path).await {
        Ok(bytes) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", mime.as_ref())
                .header("Cache-Control", "no-cache")
                .body(Body::from(bytes))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

// ============================================================================
// AUTENTICAÇÃO
// ============================================================================

#[derive(Deserialize)]
struct LoginReq { username: String, password: String }

async fn api_login(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(req): Json<LoginReq>,
) -> Response {
    if !state.check_rate_limit(addr.ip()) {
        return (StatusCode::TOO_MANY_REQUESTS, Json(serde_json::json!({
            "ok": false, "error": "Muitas tentativas. Aguarde um minuto."
        }))).into_response();
    }

    let Some(pool) = state.db_pool.as_ref() else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
            "ok": false, "error": "Servidor sem banco de dados configurado (ICUBE_DB_URL)"
        }))).into_response();
    };

    let username = req.username.trim();
    if username.is_empty() || req.password.is_empty() {
        return Json(serde_json::json!({"ok": false, "error": "Usuário e senha obrigatórios"})).into_response();
    }

    let row = match sqlx::query("SELECT id, username, password_hash, role FROM usuarios WHERE username = $1")
        .bind(username).fetch_optional(pool).await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[login] erro SQL: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "ok": false, "error": "Erro interno ao consultar usuário"
            }))).into_response();
        }
    };

    let Some(row) = row else {
        return Json(serde_json::json!({"ok": false, "error": "Usuário ou senha inválidos"})).into_response();
    };

    let id: i32 = row.get("id");
    let username: String = row.get("username");
    let password_hash: String = row.get("password_hash");
    let role: String = row.get("role");

    if !verify_password(&req.password, &password_hash) {
        return Json(serde_json::json!({"ok": false, "error": "Usuário ou senha inválidos"})).into_response();
    }

    let user = AuthUser { id: id as i64, username, role };

    let cookie_session = parse_cookie(&headers, SESSION_COOKIE);
    let (session_id, _is_new) = state.get_or_create_session(cookie_session.as_deref());
    state.set_session_user(&session_id, user.clone());

    let body = serde_json::json!({"ok": true, "user": user, "session_id": session_id});
    let mut resp = Json(body).into_response();

    let cookie = session_cookie(&session_id, SESSION_TTL.as_secs() as i64, state.secure_cookies);
    if let Ok(v) = HeaderValue::from_str(&cookie) {
        resp.headers_mut().append(header::SET_COOKIE, v);
    }
    resp
}

async fn api_logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(sid) = parse_cookie(&headers, SESSION_COOKIE) {
        state.clear_session_user(&sid);
    }
    let mut resp = Json(serde_json::json!({"ok": true})).into_response();
    let cookie = session_cookie("", 0, state.secure_cookies);
    if let Ok(v) = HeaderValue::from_str(&cookie) {
        resp.headers_mut().append(header::SET_COOKIE, v);
    }
    resp
}

async fn api_me(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let user = parse_cookie(&headers, SESSION_COOKIE)
        .and_then(|sid| state.session_user(&sid));
    match user {
        Some(u) => Json(serde_json::json!({"ok": true, "authenticated": true, "user": u})).into_response(),
        None => Json(serde_json::json!({"ok": true, "authenticated": false})).into_response(),
    }
}

// ============================================================================
// GESTÃO DE USUÁRIOS
// ============================================================================

fn require_auth(state: &AppState, headers: &HeaderMap) -> Option<AuthUser> {
    parse_cookie(headers, SESSION_COOKIE).and_then(|sid| state.session_user(&sid))
}

fn require_admin(state: &AppState, headers: &HeaderMap) -> Result<AuthUser, Response> {
    match require_auth(state, headers) {
        None => Err((StatusCode::UNAUTHORIZED, Json(serde_json::json!({
            "ok": false, "error": "Não autenticado"
        }))).into_response()),
        Some(u) if u.role != "administrator" => Err((StatusCode::FORBIDDEN, Json(serde_json::json!({
            "ok": false, "error": "Acesso restrito a administradores"
        }))).into_response()),
        Some(u) => Ok(u),
    }
}

async fn api_users_list(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(e) = require_admin(&state, &headers) { return e; }
    let Some(pool) = state.db_pool.as_ref() else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
            "ok": false, "error": "Sem banco de dados"
        }))).into_response();
    };

    match sqlx::query("SELECT id, username, role, created_at FROM usuarios ORDER BY id")
        .fetch_all(pool).await
    {
        Ok(rows) => {
            let users: Vec<serde_json::Value> = rows.iter().map(|r| {
                let id: i32 = r.get("id");
                let username: String = r.get("username");
                let role: String = r.get("role");
                let created: chrono::DateTime<chrono::Utc> = r.get("created_at");
                serde_json::json!({
                    "id": id,
                    "username": username,
                    "role": role,
                    "created_at": created.to_rfc3339(),
                })
            }).collect();
            Json(serde_json::json!({"ok": true, "users": users})).into_response()
        }
        Err(e) => {
            eprintln!("[users/list] erro SQL: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "ok": false, "error": "Erro ao listar usuários"
            }))).into_response()
        }
    }
}

#[derive(Deserialize)]
struct CreateUserReq { username: String, password: String, role: String }

async fn api_users_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateUserReq>,
) -> Response {
    if let Err(e) = require_admin(&state, &headers) { return e; }
    let Some(pool) = state.db_pool.as_ref() else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
            "ok": false, "error": "Sem banco de dados"
        }))).into_response();
    };

    let username = req.username.trim();
    if username.len() < 3 || username.len() > 50 {
        return Json(serde_json::json!({"ok": false, "error": "Username deve ter 3-50 caracteres"})).into_response();
    }
    if req.password.len() < 6 {
        return Json(serde_json::json!({"ok": false, "error": "Senha deve ter pelo menos 6 caracteres"})).into_response();
    }
    if !VALID_ROLES.contains(&req.role.as_str()) {
        return Json(serde_json::json!({"ok": false, "error": format!("Role inválida. Use: {}", VALID_ROLES.join(", "))})).into_response();
    }

    let hash = match hash_password(&req.password) {
        Ok(h) => h,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "ok": false, "error": e
        }))).into_response(),
    };

    match sqlx::query("INSERT INTO usuarios (username, password_hash, role) VALUES ($1, $2, $3) RETURNING id, username, role, created_at")
        .bind(username).bind(&hash).bind(&req.role)
        .fetch_one(pool).await
    {
        Ok(row) => {
            let id: i32 = row.get("id");
            let username: String = row.get("username");
            let role: String = row.get("role");
            let created: chrono::DateTime<chrono::Utc> = row.get("created_at");
            Json(serde_json::json!({"ok": true, "user": {
                "id": id, "username": username, "role": role,
                "created_at": created.to_rfc3339()
            }})).into_response()
        }
        Err(e) => {
            let msg = if e.to_string().contains("unique") || e.to_string().contains("duplicate") {
                "Já existe um usuário com esse nome"
            } else {
                "Erro ao criar usuário"
            };
            eprintln!("[users/create] erro SQL: {}", e);
            Json(serde_json::json!({"ok": false, "error": msg})).into_response()
        }
    }
}

#[derive(Deserialize)]
struct PasswordReq { password: String }

async fn api_users_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(target_id): Path<i64>,
    Json(req): Json<PasswordReq>,
) -> Response {
    let Some(me) = require_auth(&state, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({
            "ok": false, "error": "Não autenticado"
        }))).into_response();
    };
    if me.id != target_id && me.role != "administrator" {
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({
            "ok": false, "error": "Só é possível alterar a própria senha"
        }))).into_response();
    }
    if req.password.len() < 6 {
        return Json(serde_json::json!({"ok": false, "error": "Senha deve ter pelo menos 6 caracteres"})).into_response();
    }
    let Some(pool) = state.db_pool.as_ref() else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
            "ok": false, "error": "Sem banco de dados"
        }))).into_response();
    };

    let hash = match hash_password(&req.password) {
        Ok(h) => h,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "ok": false, "error": e
        }))).into_response(),
    };

    let n = match sqlx::query("UPDATE usuarios SET password_hash = $1 WHERE id = $2")
        .bind(&hash).bind(target_id as i32)
        .execute(pool).await
    {
        Ok(r) => r.rows_affected(),
        Err(e) => {
            eprintln!("[users/password] erro SQL: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "ok": false, "error": "Erro ao atualizar senha"
            }))).into_response();
        }
    };
    if n == 0 {
        return Json(serde_json::json!({"ok": false, "error": "Usuário não encontrado"})).into_response();
    }
    Json(serde_json::json!({"ok": true})).into_response()
}

#[derive(Deserialize)]
struct RoleReq { role: String }

async fn api_users_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(target_id): Path<i64>,
    Json(req): Json<RoleReq>,
) -> Response {
    let me = match require_admin(&state, &headers) {
        Ok(u) => u,
        Err(e) => return e,
    };
    if me.id == target_id {
        return Json(serde_json::json!({"ok": false, "error": "Não é possível alterar o próprio papel"})).into_response();
    }
    if !VALID_ROLES.contains(&req.role.as_str()) {
        return Json(serde_json::json!({"ok": false, "error": format!("Role inválida. Use: {}", VALID_ROLES.join(", "))})).into_response();
    }
    let Some(pool) = state.db_pool.as_ref() else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
            "ok": false, "error": "Sem banco de dados"
        }))).into_response();
    };

    let n = match sqlx::query("UPDATE usuarios SET role = $1 WHERE id = $2")
        .bind(&req.role).bind(target_id as i32)
        .execute(pool).await
    {
        Ok(r) => r.rows_affected(),
        Err(e) => {
            eprintln!("[users/role] erro SQL: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "ok": false, "error": "Erro ao alterar papel"
            }))).into_response();
        }
    };
    if n == 0 {
        return Json(serde_json::json!({"ok": false, "error": "Usuário não encontrado"})).into_response();
    }
    Json(serde_json::json!({"ok": true})).into_response()
}

async fn api_users_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(target_id): Path<i64>,
) -> Response {
    let me = match require_admin(&state, &headers) {
        Ok(u) => u,
        Err(e) => return e,
    };
    if me.id == target_id {
        return Json(serde_json::json!({"ok": false, "error": "Não é possível remover a própria conta"})).into_response();
    }
    let Some(pool) = state.db_pool.as_ref() else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
            "ok": false, "error": "Sem banco de dados"
        }))).into_response();
    };

    let n = match sqlx::query("DELETE FROM usuarios WHERE id = $1")
        .bind(target_id as i32).execute(pool).await
    {
        Ok(r) => r.rows_affected(),
        Err(e) => {
            eprintln!("[users/delete] erro SQL: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "ok": false, "error": "Erro ao remover usuário"
            }))).into_response();
        }
    };
    if n == 0 {
        return Json(serde_json::json!({"ok": false, "error": "Usuário não encontrado"})).into_response();
    }
    Json(serde_json::json!({"ok": true})).into_response()
}

// ============================================================================
// DEV BOOTSTRAP
// ============================================================================

#[derive(Deserialize)]
struct DevResetAdminReq {
    password: String,
    token: String,
}

async fn dev_reset_admin(
    State(state): State<AppState>,
    Json(req): Json<DevResetAdminReq>,
) -> Response {
    if std::env::var("ICUBE_DEV_RESET").as_deref() != Ok("1") {
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({
            "ok": false,
            "error": "Desabilitado. Defina ICUBE_DEV_RESET=1 no ambiente."
        }))).into_response();
    }

    let expected = match read_dev_reset_token() {
        Some(t) => t,
        None => {
            return (StatusCode::FORBIDDEN, Json(serde_json::json!({
                "ok": false,
                "error": "ICUBE_DEV_RESET=1 mas nenhum token configurado. Crie .icube-reset-token ou defina ICUBE_DEV_RESET_TOKEN."
            }))).into_response();
        }
    };

    if !constant_time_eq(req.token.as_bytes(), expected.as_bytes()) {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({
            "ok": false,
            "error": "Token inválido"
        }))).into_response();
    }

    if req.password.len() < 6 {
        return Json(serde_json::json!({
            "ok": false, "error": "Senha deve ter pelo menos 6 caracteres"
        })).into_response();
    }
    let Some(pool) = state.db_pool.as_ref() else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
            "ok": false, "error": "Sem banco de dados configurado"
        }))).into_response();
    };

    let hash = match hash_password(&req.password) {
        Ok(h) => h,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "ok": false, "error": e
        }))).into_response(),
    };

    match sqlx::query("UPDATE usuarios SET password_hash = $1 WHERE username = 'admin'")
        .bind(&hash).execute(pool).await
    {
        Ok(r) => {
            let n = r.rows_affected();
            if n == 0 {
                return Json(serde_json::json!({
                    "ok": false, "error": "Usuário 'admin' não encontrado no banco"
                })).into_response();
            }
            eprintln!("[SECURITY] dev/reset-admin usado com sucesso — senha do admin alterada");
            Json(serde_json::json!({"ok": true, "updated": n})).into_response()
        }
        Err(e) => {
            eprintln!("[dev/reset-admin] erro SQL: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "ok": false, "error": "Erro SQL"
            }))).into_response()
        }
    }
}

// ============================================================================
// EXECUTE / SESSÃO
// ============================================================================

#[derive(Deserialize)]
struct ExecRequest { code: String }

#[derive(Serialize)]
struct ExecResponse {
    ok: bool,
    output: String,
    error: Option<String>,
    session_id: String,
    is_new_session: bool,
}

#[derive(Serialize)]
struct SessionInfoOut {
    session_id: String,
    uptime_secs: u64,
    vars_count: usize,
}

async fn execute(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(req): Json<ExecRequest>,
) -> Response {
    if !state.check_rate_limit(addr.ip()) {
        let body = serde_json::json!({
            "ok": false, "output": "",
            "error": format!("rate limit excedido ({} req/{}s).", RATE_LIMIT_MAX, RATE_LIMIT_WINDOW.as_secs())
        });
        return (StatusCode::TOO_MANY_REQUESTS, Json(body)).into_response();
    }

    let cookie_session = parse_cookie(&headers, SESSION_COOKIE);
    let (session_id, is_new) = state.get_or_create_session(cookie_session.as_deref());

    if state.auth_required && state.session_user(&session_id).is_none() {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({
            "ok": false, "output": "", "error": "Faça login para executar scripts",
            "session_id": session_id, "is_new_session": is_new
        }))).into_response();
    }

    let Some(evaluator) = state.session_evaluator(&session_id) else {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "ok": false, "output": "", "error": "sessão inválida"
        }))).into_response();
    };

    let code = req.code;
    let timeout_secs = state.timeout_secs;
    let sid = session_id.clone();

    let result = tokio::task::spawn_blocking(move || {
        let mut eval = evaluator.lock().unwrap_or_else(|e| e.into_inner());
        run_script(&mut eval, &code, timeout_secs)
    }).await;

    let (ok, output, error) = match result {
        Ok(Ok(out)) => (true, out, None),
        Ok(Err(e)) => (false, String::new(), Some(e)),
        Err(join) => (false, String::new(), Some(format!("erro interno: {}", join))),
    };

    let body = ExecResponse { ok, output, error, session_id: sid, is_new_session: is_new };
    let mut resp = Json(body).into_response();
    if is_new {
        let cookie = session_cookie(&session_id, SESSION_TTL.as_secs() as i64, state.secure_cookies);
        if let Ok(v) = HeaderValue::from_str(&cookie) {
            resp.headers_mut().append(header::SET_COOKIE, v);
        }
    }
    resp
}

async fn session_reset(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(sid) = parse_cookie(&headers, SESSION_COOKIE) else {
        return Json(serde_json::json!({"ok": false, "error": "sem sessão"})).into_response();
    };
    let sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(s) = sessions.get(&sid) {
        if let Ok(mut eval) = s.evaluator.lock() {
            eval.reset();
        }
        Json(serde_json::json!({"ok": true, "message": "sessão resetada"})).into_response()
    } else {
        Json(serde_json::json!({"ok": false, "error": "sessão não encontrada"})).into_response()
    }
}

async fn session_info(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(sid) = parse_cookie(&headers, SESSION_COOKIE) else {
        return Json(serde_json::json!({"ok": false, "error": "sem sessão"})).into_response();
    };
    let sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(s) = sessions.get(&sid) {
        let uptime = Instant::now().duration_since(s.last_used).as_secs();
        let vars_count = s.evaluator.lock().unwrap_or_else(|e| e.into_inner()).vars_count();
        Json(SessionInfoOut {
            session_id: sid,
            uptime_secs: uptime,
            vars_count,
        }).into_response()
    } else {
        Json(serde_json::json!({"ok": false, "error": "sessão não encontrada"})).into_response()
    }
}

fn run_script(eval: &mut Evaluator, src: &str, timeout_secs: u64) -> Result<String, String> {
    let tokens = crate::lexer::Lexer::new(src)
        .tokenize().map_err(|e| format!("erro léxico: {}", e))?;

    let mut parser = crate::parser::Parser::new(tokens);
    let program = parser.parse_program().map_err(|e| format!("erro sintático: {}", e))?;

    let initial_types = eval.initial_types();
    let checker = crate::type_checker::TypeChecker::new();
    checker.check_program_with_env(&program, initial_types)
        .map_err(|e| format!("erro de tipo: {}", e))?;

    eval.set_timeout(Duration::from_secs(timeout_secs));
    eval.begin_capture();
    eval.run(&program).map_err(|e| format!("erro de execução: {}", e))?;

    Ok(eval.take_output())
}

// ============================================================================
// UPLOAD DE FOTO
// ============================================================================

async fn upload_photo(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Response {
    if require_auth(&state, &headers).is_none() {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({
            "ok": false, "error": "Não autenticado"
        }))).into_response();
    }

    let uploads_dir = state.www_dir.join("uploads");
    if let Err(e) = tokio::fs::create_dir_all(&uploads_dir).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "ok": false, "error": format!("Erro ao criar pasta: {}", e)
        }))).into_response();
    }

    let max_size: usize = 5 * 1024 * 1024;
    let allowed_ext = ["jpg", "jpeg", "png", "gif", "webp"];
    let mut saved_url: Option<String> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        if name != "file" { continue; }

        let filename = field.file_name().unwrap_or("photo.jpg").to_string();
        let ext = std::path::Path::new(&filename)
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_else(|| "jpg".to_string());

        if !allowed_ext.contains(&ext.as_str()) {
            return Json(serde_json::json!({
                "ok": false,
                "error": format!("Formato .{} não suportado (use jpg, png, gif ou webp)", ext)
            })).into_response();
        }

        let bytes = match field.bytes().await {
            Ok(b) => b,
            Err(e) => return Json(serde_json::json!({
                "ok": false, "error": format!("Erro ao ler arquivo: {}", e)
            })).into_response(),
        };

        if bytes.len() > max_size {
            return Json(serde_json::json!({
                "ok": false, "error": "Arquivo muito grande (máx 5 MB)"
            })).into_response();
        }

        let unique = format!("{}_{}.{}",
            chrono::Utc::now().timestamp_millis(),
            new_session_id().chars().take(8).collect::<String>(),
            ext
        );
        let path = uploads_dir.join(&unique);

        if let Err(e) = tokio::fs::write(&path, &bytes).await {
            return Json(serde_json::json!({
                "ok": false, "error": format!("Erro ao salvar: {}", e)
            })).into_response();
        }

        saved_url = Some(format!("/uploads/{}", unique));
    }

    match saved_url {
        Some(url) => Json(serde_json::json!({"ok": true, "url": url})).into_response(),
        None => Json(serde_json::json!({
            "ok": false, "error": "Nenhum arquivo recebido (campo 'file')"
        })).into_response(),
    }
}

const INDEX_HTML: &str = r#"<!DOCTYPE html>
<html lang="pt-BR"><head><meta charset="UTF-8"><title>Icube</title>
<link rel="stylesheet" href="/style.css"></head>
<body>
<h1 style="text-align:center;margin-top:40px;">🐣 Icube</h1>
<div style="text-align:center;max-width:400px;margin:20px auto;">
<p><a href="/">🔐 Entrar</a></p>
<p><a href="/equipamentos">📦 Equipamentos</a></p>
<p><a href="/cotacoes">💰 Cotações</a></p>
<p><a href="/usuarios.html">👥 Usuários</a></p>
</div>
</body></html>"#;

const NOT_FOUND_HTML: &str = r#"<!DOCTYPE html><html><head><meta charset="UTF-8"><title>404</title></head>
<body style="font-family:system-ui;text-align:center;padding:80px;">
<h1 style="font-size:72px;color:#1976d2;">404</h1><p>Não encontrado</p>
<p><a href="/">← Voltar</a></p></body></html>"#;