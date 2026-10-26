# 🐣 Icube

Linguagem de script com banco de dados, decimal e IA embutidos.

- Banco: PostgreSQL e MySQL
- Decimal: 28 dígitos de precisão
- IA: Ollama / Llama 3
- Servidor web embutido

## Instalação

git clone https://github.com/claudiop33/icube.git
cd icube
cargo build --release

## Exemplo

db.connect_default();
let r = db.query("SELECT * FROM usuarios");
for u in r {
    print(u["nome"]);
}

## Links

- Site: https://claudiop33.github.io/icube/
- Issues: https://github.com/claudiop33/icube/issues

MIT License - 2026 Claudio
