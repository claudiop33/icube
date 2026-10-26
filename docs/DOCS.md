# 📖 Referência da Linguagem Icube

Documentação completa da linguagem Icube v0.7.7.

## Índice

1. [Visão Geral](#visão-geral)
2. [Sintaxe Básica](#sintaxe-básica)
3. [Tipos](#tipos)
4. [Operadores](#operadores)
5. [Controle de Fluxo](#controle-de-fluxo)
6. [Funções](#funções)
7. [Built-ins](#built-ins)
8. [Erros](#erros)
9. [Banco de Dados](#banco-de-dados)
10. [Transações](#transações)
11. [Decimal](#decimal)
12. [IA](#ia)

---

## Visão Geral

Icube é uma linguagem de script estaticamente tipada com banco de dados, decimal e IA embutidos.

**Pipeline:** Lexer → Parser → TypeChecker → Evaluator

## Sintaxe Básica

### Comentários

```icube
// Comentário de linha
let x = 42;  // também no fim
Variáveis
icube
let nome = "Ana";               // string inferido
let idade = 30;                 // int inferido
let preco: decimal = 1234.56;   // decimal explícito
let ativo: bool = true;
let apelido: string? = null;    // nullable
Palavras-chave
let, fn, if, else, while, for, in, break, continue, return, try, catch, finally, throw, as, true, false, null

Tipos
int, float, decimal, bool, string, date, time, datetime

Tipos
Primitivos
Tipo	Exemplo	SQL correspondente
int	42	INTEGER, BIGINT
float	3.14	REAL, DOUBLE
decimal	1234.56	NUMERIC, DECIMAL
bool	true	BOOLEAN
string	"oi"	TEXT, VARCHAR
Data e tempo
Tipo	Exemplo
date	"2026-01-15" as date
time	"14:30:00" as time
datetime	now()
Nulidade
Qualquer tipo pode ser T?:

icube
let nome: string = "Ana";
let apelido: string? = null;
Cast
icube
3.9 as int              // 3
42 as float             // 42.0
1234.56 as decimal      // decimal
100 as decimal          // decimal (implícito)
d as int                // decimal → int (exige as)
Operadores
Aritméticos
icube
10 + 5      // 15
10 - 5      // 5
10 * 5      // 50
10 / 5      // 2
10 % 3      // 1
Comparação
icube
5 == 5      // true
5 != 3      // true
5 > 3       // true
5 <= 3      // false
Lógicos
icube
true && false   // false (AND)
true || false   // true  (OR)
!true           // false (NOT)
Concatenação
icube
"Ana" + " Silva"    // "Ana Silva"
Controle de Fluxo
if/else
icube
if idade >= 18 {
    print("Maior");
} else if idade >= 12 {
    print("Adolescente");
} else {
    print("Criança");
}
while
icube
let i = 0;
while i < 5 {
    print(i);
    i = i + 1;
}
for
icube
for n in [1, 2, 3] {
    print(n);
}

for u in db.query("SELECT nome FROM usuarios") {
    print(u["nome"]);
}
break / continue
icube
for n in [1, 2, 3, 4, 5] {
    if n == 3 { break; }
    if n % 2 == 0 { continue; }
    print(n);
}
Funções
icube
fn saudar(nome: string) {
    print("Olá,", nome);
}

fn dobro(n: int) -> int {
    return n * 2;
}

fn calcular(preco: decimal, qtd: int) -> decimal {
    return preco * qtd;
}
Funções não enxergam variáveis externas. Passe por parâmetro.

Built-ins
Função	O que faz
print(...)	Imprime valores
len(x)	Tamanho de string ou lista
env(nome)	Variável de ambiente (ou null)
exit(codigo)	Encerra o processo (não capturável)
today()	Data atual
now()	Data/hora atual (UTC)
Erros
icube
try {
    let r = 10 / 0;
} catch e {
    print("Erro:", e);
} finally {
    print("sempre executa");
}

throw "mensagem de erro";
Banco de Dados
Conectar
icube
db.connect_default();           // via ICUBE_DB_URL
db.connect("postgres://...");   // direto
db.connect_mysql("mysql://...");
Executar
icube
db.execute("INSERT INTO t (a, b) VALUES ($1, $2)", [1, 2]);
db.execute("DELETE FROM t WHERE id = $1", [5]);
Consultar
icube
let todos = db.query("SELECT * FROM usuarios");
let um = db.query_one("SELECT * FROM u WHERE id = $1", [5]);

if um == null {
    print("não encontrado");
} else {
    print(um["nome"]);
}
Placeholders
PostgreSQL: $1, $2, ...

MySQL: ?

Transações
icube
db.connect_default();
db.begin();

try {
    db.execute("INSERT INTO pedidos (cliente) VALUES ($1)", ["Ana"]);
    db.commit();
} catch e {
    db.rollback();
    throw e;
}
Decimal
Precisão de 28-29 dígitos. Ideal para dinheiro.

icube
let a: decimal = 0.1;
let b: decimal = 0.2;
print(a + b);           // 0.3 ✅

let preco: decimal = 1234.56;
let total = preco * 2;  // 2469.12
Implicitamente converte int e float:

icube
let p: decimal = 1234.56;   // float → decimal
let q: decimal = 100;       // int → decimal
IA
icube
ai.is_available()                       // bool
ai.ask(prompt)                          // string
ai.ask_with_system(system, prompt)      // string
ai.model()                              // string

if ai.is_available() {
    let r = ai.ask("O que é PostgreSQL?");
    print(r);
}
Configuração:

OLLAMA_HOST (padrão: http://localhost:11434)

ICUBE_AI_MODEL (padrão: llama3)
