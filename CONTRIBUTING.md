# 🤝 Como contribuir com o Icube

Obrigado pelo interesse em contribuir! Este guia explica como reportar bugs, sugerir features e enviar pull requests.

## Código de conduta

Seja respeitoso. Não toleramos assédio, discriminação ou comportamento agressivo. Isso vale para issues, PRs, discussões e qualquer interação no projeto.

## Como reportar bugs

**Antes de abrir uma issue**, verifique se já não existe uma aberta:

👉 https://github.com/claudiop33/icube/issues

Se não existir, abra uma nova com:

### Template de bug report

```markdown
**Descrição**
Uma frase clara do que está errado.

**Passos para reproduzir**
1. Execute `icube script.icb`
2. Com o código `...`
3. Veja o erro `...`

**Comportamento esperado**
O que você achava que ia acontecer.

**Comportamento atual**
O que de fato aconteceu.

**Ambiente**
- Icube: 0.7.7
- OS: Ubuntu 22.04
- Rust: 1.75.0
- PostgreSQL: 15.2

**Código de exemplo**
```icube
db.connect_default();
// ...
Logs

text
cole o output completo
text

⚠️ **Nunca cole credenciais** (senhas, tokens, URL com senha) nas issues. Mascare com `***`.

## Como sugerir features

Abra uma issue com o prefixo `[FEATURE]` no título:

```markdown
**Problema**
Que dor você está tentando resolver?

**Solução proposta**
Como você imagina que poderia funcionar?

**Alternativas consideradas**
Outras formas de resolver o mesmo problema.

**Contexto adicional**
Exemplos, links, screenshots.
Antes de sugerir, verifique o roadmap no CHANGELOG — talvez já esteja planejado.

Como enviar pull requests
1. Faça um fork
bash
# No GitHub, clique em "Fork" no repo claudiop33/icube
git clone https://github.com/SEU_USUARIO/icube.git
cd icube
git remote add upstream https://github.com/claudiop33/icube.git
2. Crie uma branch
bash
git checkout -b feature/nome-curto
# ou:
git checkout -b fix/nome-do-bug
Convenção de nomes:

feature/ — nova funcionalidade

fix/ — correção de bug

docs/ — documentação

refactor/ — refatoração

test/ — testes

3. Faça a mudança
Siga os padrões de código abaixo.

4. Teste
bash
cargo build --release
cargo test
Se adicionou uma feature, adicione um teste também.

5. Commit
bash
git add .
git commit -m "feat: adiciona suporte a JSONB"
Formato da mensagem (Conventional Commits):

Prefixo	Uso
feat:	Nova feature
fix:	Correção de bug
docs:	Documentação
refactor:	Refatoração (sem mudança de comportamento)
test:	Adicionar/corrigir testes
chore:	Tarefas de manutenção
Exemplos:

feat: adiciona tipo json

fix: corrige cast de decimal para int

docs: atualiza tutorial de decimal

6. Push e PR
bash
git push origin feature/nome-curto
No GitHub, clique em "Compare & pull request". Descreva:

markdown
**O que muda**
Resumo das mudanças.

**Por quê**
Motivação/problema resolvido.

**Como testar**
Passos para quem revisar reproduzir.

**Checklist**
- [ ] Compila (`cargo build --release`)
- [ ] Testes passam (`cargo test`)
- [ ] Adicionei testes para a mudança
- [ ] Atualizei o CHANGELOG.md
- [ ] Atualizei a documentação (se aplicável)
Padrões de código
Rust
Use cargo fmt antes de commitar

Use cargo clippy e corrija warnings

Mensagens de erro em português (consistente com o resto do projeto)

Prefira unwrap_or_else(|e| e.into_inner()) para Mutex (não .unwrap())

Prefira Result<T, EvalError> em vez de panic!

Icube (script)
Use ; no final (exceto REPL)

Nomes descritivos (preco_total, não pt)

Parâmetros SQL sempre com $1, $2 (nunca concatenação)

Mensagens de erro com contexto ("erro ao salvar: " + e)

Documentação
Markdown com cabeçalhos claros

Exemplos executáveis (que funcionam de verdade)

Português (o projeto é PT-BR nativo)

Estrutura do projeto
text
icube/
├── src/           → código Rust
├── www/           → frontend (HTML/CSS/JS)
├── site/          → landing page
├── examples/      → scripts .icb de exemplo
├── docs/          → documentação
├── Cargo.toml     → dependências
└── README.md
Onde fazer cada tipo de mudança
Mudança	Arquivo
Nova sintaxe/token	src/lexer.rs + src/parser.rs
Nova regra de tipo	src/type_checker.rs
Novo comportamento de runtime	src/eval.rs
Nova integração (db/ia)	src/eval.rs (call handlers)
Novo endpoint HTTP	src/server.rs
Nova feature de IA	src/ai.rs
Config	src/config.rs
CLI/REPL	src/main.rs
UI do frontend	www/*.html, www/*.js
Landing page	site/index.html
Documentação	docs/*.md
Áreas onde ajuda é bem-vinda
🟢 Fácil — bom para primeira contribuição
Corrigir typos na documentação

Adicionar exemplos em examples/

Melhorar mensagens de erro

Adicionar testes para features existentes

Traduzir doc para inglês

🟡 Médio
Implementar json (tipo JSONB)

Implementar switch/match

Adicionar push/pop em listas

Melhorar performance do parser

Adicionar spans (linha:coluna) nos erros de tipo

🔴 Difícil
Interpolação de string ("${x}")

LSP (Language Server Protocol)

Debugger

Bytecode compilation (substituir tree-walking evaluator)

Se quiser pegar uma dessas, comente na issue ou abra uma nova para discutir.

Processo de revisão
Eu (mantenedor) reviso o PR em até 7 dias

Se precisar de mudanças, deixo comentários

Você faz ajustes e faz push (a PR atualiza automaticamente)

Quando aprovado, faço o merge

Não fique ofendido com pedidos de mudança. Revisão é normal e ajuda o projeto a manter qualidade.

CI/CD
Todo PR roda:

bash
cargo build --release
cargo test
Se falhar, veja o log em Actions e corrija.

Dúvidas
💬 Abra uma issue com a tag question

📧 Ou mande email para claudiop33@github.com

Reconhecimento
Todo contribuidor é citado no CHANGELOG da versão onde sua contribuição entrou.

Obrigado por contribuir! 🦀

Última atualização: Outubro de 2026
