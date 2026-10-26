# 🎓 Tutorial de Programação em Icube

Guia prático para aprender Icube do zero em 30 minutos.

## Índice

1. [Primeiros Passos](#primeiros-passos)
2. [Variáveis](#variáveis)
3. [Operadores](#operadores)
4. [Controle de Fluxo](#controle-de-fluxo)
5. [Funções](#funções)
6. [Banco de Dados](#banco-de-dados)
7. [Decimal](#decimal)
8. [IA](#ia)
9. [Projetos Completos](#projetos-completos)

---

## Primeiros Passos

### Hello World

Crie `hello.icb`:

```icube
print("Olá, mundo!");
Execute:

bash
./target/release/icube hello.icb
Saída:

text
Olá, mundo!
REPL interativo
bash
./target/release/icube
text
icube> let x = 42
icube> print(x * 2)
84
icube> exit
Dicas:

; é opcional no REPL

Variáveis persistem entre linhas

Variáveis
icube
let nome = "Ana";               // string
let idade = 30;                 // int
let altura: float = 1.65;       // float
let salario: decimal = 5000.00; // decimal
let ativo: bool = true;         // bool
let apelido: string? = null;    // nullable
Reatribuição:

icube
let contador = 0;
contador = contador + 1;   // ✅
contador = "texto";        // ❌ ERRO
Operadores
Aritméticos
icube
let a = 10 + 5;      // 15
let b = 10 - 5;      // 5
let c = 10 * 5;      // 50
let d = 10 / 5;      // 2
let e = 10 % 3;      // 1
Comparação
icube
print(5 == 5);       // true
print(5 > 3);        // true
print(5 <= 3);       // false
Lógicos
icube
print(true && false);   // false
print(true || false);   // true
print(!true);           // false
Cast
icube
3.9 as int          // 3
42 as float         // 42.0
42 as string        // "42"
Controle de Fluxo
if / else
icube
let idade = 18;

if idade >= 18 {
    print("Maior de idade");
} else {
    print("Menor");
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
for n in [1, 2, 3, 4, 5] {
    print(n * 2);
}
break / continue
icube
for n in [1, 2, 3, 4, 5] {
    if n == 3 { break; }
    if n % 2 == 0 { continue; }
    print(n);
}
// 1
Funções
icube
fn saudar(nome: string) {
    print("Olá,", nome);
}

fn dobro(n: int) -> int {
    return n * 2;
}

fn calcular_total(preco: decimal, qtd: int) -> decimal {
    return preco * qtd;
}
Chamar:

icube
saudar("Ana");
let x = dobro(21);    // 42
Recursão:

icube
fn fatorial(n: int) -> int {
    if n <= 1 { return 1; }
    return n * fatorial(n - 1);
}
Banco de Dados
Configurar
No .env:

text
ICUBE_DB_URL=postgres://usuario:senha@localhost:5432/meubanco
Conectar
icube
db.connect_default();
Executar (INSERT/UPDATE/DELETE)
icube
db.execute(
    "INSERT INTO usuarios (nome, idade) VALUES ($1, $2)",
    ["Ana", 30]
);

db.execute("DELETE FROM usuarios WHERE id = $1", [5]);
Consultar várias linhas
icube
let todos = db.query("SELECT * FROM usuarios");
for u in todos {
    print(u["nome"]);
}
Consultar uma linha
icube
let u = db.query_one(
    "SELECT * FROM usuarios WHERE email = $1",
    ["ana@exemplo.com"]
);

if u == null {
    print("Não encontrado");
} else {
    print("Nome:", u["nome"]);
}
Parâmetros seguros
icube
// ✅ SEGURO
db.query("SELECT * FROM users WHERE nome = $1", ["Ana"]);

// ❌ INSEGURO — SQL injection
db.query("SELECT * FROM users WHERE nome = '" + nome + "'");
Decimal
Precisão de 28 dígitos. Ideal para dinheiro.

icube
let a: decimal = 0.1;
let b: decimal = 0.2;
print(a + b);   // 0.3 (não 0.30000000000000004)
Aritmética
icube
let preco: decimal = 100;
let qtd: decimal = 3;

print(preco + qtd);   // 103
print(preco * qtd);   // 300
print(preco / qtd);   // 33.333...
SQL com NUMERIC
icube
let r = db.query("SELECT preco FROM cotacoes");
for c in r {
    print(c["preco"]);   // 240.99 (não null!)
}
IA
icube
if ai.is_available() {
    print("Modelo:", ai.model());

    let r = ai.ask("O que é PostgreSQL?");
    print(r);

    let system = "Responda apenas: positivo, negativo ou neutro.";
    let sentimento = ai.ask_with_system(system, "Adorei!");
    print(sentimento);
}
Combinar com banco
icube
db.connect_default();

for e in db.query("SELECT id, defeito FROM equipamentos") {
    let urgencia = ai.ask_with_system(
        "Classifique urgência: critico, alto, medio, baixo",
        e["defeito"]
    );
    print("Equip. #", e["id"], "->", urgencia);
}
Projetos Completos
Projeto 1 — Relatório de vendas
icube
db.connect_default();

let vendas = db.query("
    SELECT produto, SUM(valor) AS total
    FROM pedidos
    WHERE data >= $1
    GROUP BY produto
    ORDER BY total DESC
", ["2026-01-01" as date]);

print("=== Vendas desde 2026-01-01 ===");
for v in vendas {
    print(v["produto"], ":", "R$", v["total"]);
}
Projeto 2 — Criar usuário com validação
icube
fn criar_usuario(nome: string, email: string, idade: int) {
    if len(nome) < 2 {
        throw "Nome muito curto";
    }
    if idade < 0 || idade > 150 {
        throw "Idade inválida";
    }

    db.execute(
        "INSERT INTO usuarios (nome, email, idade) VALUES ($1, $2, $3)",
        [nome, email, idade]
    );
    print("Usuário criado:", nome);
}

db.connect_default();
try {
    criar_usuario("Ana Silva", "ana@exemplo.com", 30);
} catch e {
    print("Erro:", e);
}
Projeto 3 — Sincronização com rollback
icube
db.connect_default();
db.begin();

try {
    let inativos = db.query("SELECT id FROM usuarios WHERE ativo = false");

    for u in inativos {
        db.execute("DELETE FROM usuarios WHERE id = $1", [u["id"]]);
    }

    db.commit();
    print("Removidos", len(inativos), "usuários");
} catch e {
    db.rollback();
    throw e;
}
Boas Práticas
Sempre use parâmetros SQL
icube
// ✅ Certo
db.query("SELECT * FROM users WHERE id = $1", [id]);

// ❌ Errado
db.query("SELECT * FROM users WHERE id = " + id);
Verifique null
icube
let u = db.query_one("SELECT * FROM users WHERE id = $1", [id]);
if u == null {
    print("Não encontrado");
    exit(1);
}
Use decimal para dinheiro
icube
// ✅ Certo
let salario: decimal = 5000.00;

// ❌ Errado
let salario: float = 5000.00;
Fim do tutorial — Icube v0.7.7
