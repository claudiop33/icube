# 🌐 API HTTP do Servidor Icube

Referência dos endpoints HTTP/HTTPS expostos pelo servidor Icube.

## Iniciar o servidor

```bash
icube --serve              # porta 8080
icube --serve 9000         # porta customizada
ICUBE_TLS_AUTO=1 icube --serve 9000   # HTTPS self-signed
Autenticação
A autenticação usa cookie de sessão (icube_session) com Argon2id.

Item	Detalhe
Cookie	icube_session (HttpOnly, SameSite=Lax, Secure em HTTPS)
Session ID	64 hex chars (32 bytes) gerado por OsRng
TTL	1 hora (renovado a cada request)
Rate limit	30 req/min por IP em /api/login e /execute
Endpoints
Health
GET /health
Health check simples.

Resposta: 200 OK com corpo ok

Autenticação
POST /api/login
Autentica usuário.

Request:

json
{
  "username": "admin",
  "password": "senha123"
}
Resposta (sucesso):

json
{
  "ok": true,
  "user": {
    "id": 1,
    "username": "admin",
    "role": "administrator"
  },
  "session_id": "abc..."
}
Resposta (erro):

json
{ "ok": false, "error": "Usuário ou senha inválidos" }
Status:

200 — sucesso ou credenciais inválidas

429 — rate limit excedido

503 — banco não configurado

POST /api/logout
Encerra sessão.

Resposta: { "ok": true }

GET /api/me
Retorna usuário autenticado.

Resposta (autenticado):

json
{
  "ok": true,
  "authenticated": true,
  "user": { "id": 1, "username": "admin", "role": "administrator" }
}
Resposta (não autenticado):

json
{ "ok": true, "authenticated": false }
Execução de scripts
POST /execute
Executa código Icube na sessão atual.

Request:

json
{ "code": "print(\"Olá\");" }
Resposta (sucesso):

json
{
  "ok": true,
  "output": "Olá\n",
  "error": null,
  "session_id": "abc...",
  "is_new_session": false
}
Resposta (erro):

json
{
  "ok": false,
  "output": "",
  "error": "erro de tipo: ...",
  "session_id": "abc...",
  "is_new_session": false
}
Status:

200 — execução OK ou erro no script

401 — sem autenticação (se ICUBE_REQUIRE_AUTH=1)

429 — rate limit excedido

Alias: POST /api/equipamentos (mesmo handler)

Sessão
POST /session/reset
Limpa variáveis, funções e conexão DB da sessão.

Resposta: { "ok": true, "message": "sessão resetada" }

GET /session/info
Info da sessão atual.

Resposta:

json
{
  "session_id": "abc...",
  "uptime_secs": 120,
  "vars_count": 5
}
Usuários (admin)
Todos os endpoints abaixo exigem role=administrator.

GET /api/users
Lista usuários.

Resposta:

json
{
  "ok": true,
  "users": [
    {
      "id": 1,
      "username": "admin",
      "role": "administrator",
      "created_at": "2026-01-15T10:00:00Z"
    }
  ]
}
POST /api/users/create
Cria usuário.

Request:

json
{
  "username": "novo",
  "password": "senha123",
  "role": "colaborador"
}
Roles válidas: administrator, colaborador, visitante

Validações:

username — 3 a 50 caracteres

password — mínimo 6 caracteres

POST /api/users/:id/password
Altera senha (própria ou admin pode alterar de qualquer um).

Request:

json
{ "password": "novaSenha123" }
POST /api/users/:id/role
Altera papel (admin only).

Request:

json
{ "role": "colaborador" }
Restrição: não pode alterar o próprio papel.

POST /api/users/:id/delete
Remove usuário (admin only).

Restrição: não pode remover a própria conta.

Upload de arquivo
POST /api/upload
Upload de foto (multipart/form-data).

Campo: file

Formatos: jpg, jpeg, png, gif, webp

Tamanho máximo: 5 MB

Resposta:

json
{ "ok": true, "url": "/uploads/1234_abcd.jpg" }
Requer autenticação.

Dev bootstrap (produção: desabilitar)
POST /api/dev/reset-admin
Reset de senha do admin. Requer dois fatores:

ICUBE_DEV_RESET=1 no ambiente

Token válido (env ICUBE_DEV_RESET_TOKEN ou arquivo .icube-reset-token)

Request:

json
{
  "password": "novaSenha123",
  "token": "meutoken123"
}
Resposta (sucesso):

json
{ "ok": true, "updated": 1 }
Status:

200 — senha alterada

401 — token inválido

403 — ICUBE_DEV_RESET não definido ou sem token configurado

⚠️ Nunca deixe ICUBE_DEV_RESET=1 em produção.

Variáveis de ambiente
Variável	Padrão	Efeito
ICUBE_DB_URL	—	URL do banco (obrigatório para auth)
ICUBE_WWW_DIR	www	Diretório de estáticos
ICUBE_TIMEOUT_SECS	30	Timeout de execução
ICUBE_REQUIRE_AUTH	0	1 = exige login em /execute
ICUBE_TLS_AUTO	0	1 = cert self-signed
ICUBE_TLS_CERT	—	Caminho do cert PEM
ICUBE_TLS_KEY	—	Caminho da chave PEM
ICUBE_INSECURE_COOKIE	0	1 = sem Secure (dev)
ICUBE_DEV_RESET	0	1 = habilita dev reset
ICUBE_DEV_RESET_TOKEN	—	Token do dev reset
OLLAMA_HOST	http://localhost:11434	URL do Ollama
ICUBE_AI_MODEL	llama3	Modelo de IA
Exemplos com curl
Login
bash
curl -c cookies.txt -X POST http://localhost:9000/api/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"senha123"}'
Executar script
bash
curl -b cookies.txt -X POST http://localhost:9000/execute \
  -H "Content-Type: application/json" \
  -d '{"code":"print(\"Olá\");"}'
Listar usuários (admin)
bash
curl -b cookies.txt http://localhost:9000/api/users
Reset do admin
bash
curl -X POST http://localhost:9000/api/dev/reset-admin \
  -H "Content-Type: application/json" \
  -d '{"password":"nova123","token":"meutoken123"}'
Códigos de status
Status	Significado
200	OK
401	Não autenticado
403	Sem permissão (role insuficiente)
404	Não encontrado
429	Rate limit excedido
500	Erro interno
503	Serviço não disponível (banco sem config)
Referência da API — Icube v0.7.7
