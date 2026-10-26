# 🔒 Política de Segurança

Como reportar vulnerabilidades e o que está implementado no Icube.

## Reportar vulnerabilidades

**Não abra issues públicas para vulnerabilidades de segurança.**

Se você encontrou uma falha de segurança, envie um email para:

📧 **claudiop33@github.com** (ou abra uma *private security advisory* em https://github.com/claudiop33/icube/security/advisories/new)

Inclua:
- Descrição da vulnerabilidade
- Passos para reproduzir
- Versão afetada
- Impacto estimado
- Sugestão de correção (se tiver)

**Compromisso:**
- Resposta em até 72 horas
- Correção em até 30 dias para vulnerabilidades críticas
- Crédito no CHANGELOG (se você quiser)

---

## Versões suportadas

| Versão | Suporte |
|---|---|
| 0.7.x | ✅ Suportada |
| < 0.7 | ❌ Não suportada |

Sempre use a versão mais recente.

---

## O que está implementado

### Autenticação

| Item | Detalhe |
|---|---|
| Hash de senha | Argon2id (m=19456, t=2, p=1) |
| Session ID | 256 bits via `OsRng` (CSPRNG do SO) |
| Cookie | `HttpOnly`, `SameSite=Lax`, `Secure` em HTTPS |
| TTL | 1 hora (renovado a cada request) |
| Rate limit | 30 req/min por IP em `/api/login` e `/execute` |

### SQL Injection

**Prevenido por design.** Todas as queries usam parâmetros vinculados:

```icube
// ✅ SEGURO — parâmetro vinculado
db.query("SELECT * FROM users WHERE id = $1", [id]);

// ❌ INSEGURO — o programador é responsável
db.query("SELECT * FROM users WHERE id = " + id);
A linguagem oferece o caminho seguro por padrão. O programador precisa ativamente fazer concatenação para ser vulnerável.

Session Hijacking
Session IDs são gerados por OsRng (256 bits), tornando brute-force computacionalmente inviável.

CSRF
Mitigação parcial via SameSite=Lax. Para produção robusta, configure também:

CSP (Content-Security-Policy)

Token CSRF explícito (roadmap 0.9)

HSTS via proxy reverso

XSS
O frontend (www/) usa escHtml() consistentemente antes de inserir dados no DOM. Strings de erro do servidor são escapadas.

Timing Attack
Comparação de token de reset-admin usa constant_time_eq (constant-time).

Exit não capturável
exit() não é interceptável por try/catch — evita que código malicioso mascare o código de saída.

Dev-reset-admin
Exige dois fatores para funcionar:

ICUBE_DEV_RESET=1 no ambiente

Token válido (env ICUBE_DEV_RESET_TOKEN ou arquivo .icube-reset-token)

Comparação em tempo constante.

Vulnerabilidades conhecidas (não corrigidas)
1. Timing attack em usuário inexistente
Impacto: baixo (enumeração de usuários por tempo de resposta)
Mitigação: use rate limiting (implementado)
Correção prevista: 0.9 (hash dummy para user não encontrado)

2. Rate limit por IP compartilhado
Impacto: médio (NAT/CGNAT faz vários usuários compartilharem o bucket)
Mitigação: rate limit generoso (30/min)
Correção prevista: 0.9 (combinar IP + user)

3. Sem token CSRF explícito
Impacto: médio (SameSite=Lax cobre a maioria dos casos)
Mitigação: configure CSP + SameSite=Strict em produção
Correção prevista: 0.9

4. Upload sem validação de magic bytes
Impacto: baixo (só extensão é validada, não conteúdo)
Mitigação: servir de diretório isolado
Correção prevista: 0.8

5. Sem log de auditoria
Impacto: médio (dificulta investigação forense)
Correção prevista: 0.9

6. Sessões em memória
Impacto: baixo (perdidas ao reiniciar; sem persistência entre processos)
Correção prevista: 1.0 (SQLite/Redis)

Boas práticas em produção
Obrigatório
✅ Use HTTPS real (Let's Encrypt)

✅ Não defina ICUBE_INSECURE_COOKIE=1

✅ Remova ICUBE_DEV_RESET do ambiente

✅ Mantenha ICUBE_REQUIRE_AUTH=1

✅ Use senhas fortes para o banco e admin

Recomendado
🔒 Configure nginx ou caddy como proxy reverso

🔒 Adicione HSTS (Strict-Transport-Security)

🔒 Adicione CSP (Content-Security-Policy)

🔒 Monitore logs de autenticação

🔒 Faça backup do banco regularmente

🔒 Atualize para a última versão

Exemplo de config nginx
nginx
server {
    listen 443 ssl http2;
    server_name icube.seudominio.com;

    ssl_certificate /etc/letsencrypt/live/icube.seudominio.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/icube.seudominio.com/privkey.pem;

    add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;
    add_header Content-Security-Policy "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-Frame-Options "SAMEORIGIN" always;

    location / {
        proxy_pass http://localhost:9000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}

server {
    listen 80;
    server_name icube.seudominio.com;
    return 301 https://$server_name$request_uri;
}
Rotação de credenciais (após exposição)
Se você acidentalmente commitou credenciais no git:

1. Revogar imediatamente
Credencial	O que fazer
Token GitHub	https://github.com/settings/tokens → Delete
Senha do banco	ALTER USER user WITH PASSWORD 'nova';
Admin do Icube	Reset via /api/dev/reset-admin
ICUBE_DEV_RESET_TOKEN	openssl rand -hex 32 > .icube-reset-token
Chaves TLS	Gerar novos certificados
2. Remover do histórico
bash
# Opção rápida (sem histórico)
rm -rf .git
git init && git branch -M main
git add . && git commit -m "Reset"
git remote add origin ...
git push --force
3. Confirmar
bash
# Nenhum arquivo sensível deve aparecer
git ls-files | grep -iE "\.env|\.pem|\.key|token|senha"

# Nenhum commit no histórico
git log --all -- "*.env" --oneline
Checklist de segurança (deploy)
Antes de colocar em produção:

□ HTTPS configurado (certificado válido)
□ ICUBE_REQUIRE_AUTH=1
□ ICUBE_DEV_RESET não definido
□ Senha do admin forte (16+ chars)
□ Senha do banco forte
□ .env fora do git
□ icube.toml sem credenciais hardcoded
□ Backup do banco configurado
□ Proxy reverso com HSTS + CSP
□ Rate limit adequado ao tráfego esperado
□ Logs monitorados
Política de Segurança — Icube v0.7.7

Última atualização: Outubro de 2026
