# 🔌 Serial (Arduino / USB) — Icube v0.9.0

Comunicação com Arduino, ESP32, CH340, FTDI e dispositivos USB-serial.

## Funções

| Função | Retorno | Descrição |
|--------|---------|-----------|
| `serial.list()` | `list<Row>` | Lista portas disponíveis |
| `serial.open(porta, baud)` | `null` | Abre conexão |
| `serial.send(mensagem)` | `null` | Envia texto |
| `serial.read()` | `string` | Lê linha (bloqueia) |
| `serial.read_available()` | `string` | Lê disponível (não bloqueia) |
| `serial.close()` | `null` | Fecha conexão |

## Instalação

### Linux

```bash
sudo apt install -y libudev-dev
sudo usermod -a -G dialout $USER
# Depois: logout + login
Windows
Nenhuma dependência extra. Driver CH340 para Arduinos clones.

Uso básico
icube
serial.open("/dev/ttyACM0", 9600);   // Linux
// serial.open("COM3", 9600);        // Windows

serial.send("LIGAR_LED");
let resposta = serial.read();
print("Arduino:", resposta);

serial.close();
text

**Cola no nano** (Shift+Insert ou botão direito → Paste).

---

### Parte B — Exemplos e HTTP (cola depois)
Exemplos práticos
Gravar leituras no banco
icube
db.connect_default();
serial.open("/dev/ttyACM0", 9600);

while true {
    let linha = serial.read_available();
    if linha != "" {
        db.execute(
            "INSERT INTO leituras (dados, data_hora) VALUES ($1, $2)",
            [linha, now()]
        );
        print("Gravado:", linha);
    }
}
Classificar com IA
icube
serial.open("/dev/ttyACM0", 9600);
let linha = serial.read();
if linha != "" {
    let analise = ai.ask("Classifique como normal ou anormal: " + linha);
    print("Leitura:", linha, "->", analise);
}
serial.close();
Endpoints HTTP
Método	Rota	Auth
GET	/api/serial/ports	❌
POST	/api/serial/open	✅
POST	/api/serial/send	✅
GET	/api/serial/read	✅
POST	/api/serial/close	✅
Páginas web
www/serial.html — Monitor serial em tempo real

www/led.html — Controle de LED com sequências

text

**Cola no nano** depois da parte A.

---

### Parte C — Segurança + Troubleshooting (cola por último)
Segurança
Permissões (Linux)
bash
# Adicionar usuário ao dialout
sudo usermod -a -G dialout $USER
# Depois: logout + login
⚠️ Servidor rodando como root: root não está no dialout. Rode como usuário normal.

Recomendações
Não exponha o servidor com serial na internet sem HTTPS

Feche a conexão (serial.close()) ao terminar

/api/serial/* exige autenticação (exceto /ports)

Troubleshooting
serial.list() retorna vazio
Arduino não conectado → ls /dev/ttyACM*

Sem permissão → groups | grep dialout

Windows → instalar driver CH340

Servidor como root → rodar como usuário normal

"falha ao abrir"
Porta em uso → fechar Arduino IDE / minicom

Sem permissão → sudo chmod 666 /dev/ttyACM0 (temporário)

Dados estranhos
Baud rate errado. Confirme que Icube usa o mesmo baud do Arduino.

Documentação Serial — Icube v0.9.0

text

**Cola no nano** depois da parte B.

---

## Passo 4 — Salvar no nano

Depois de colar tudo:

1. **`Ctrl+O`** (salvar)
2. **`Enter`** (confirma o nome do arquivo)
3. **`Ctrl+X`** (sair do nano)

O terminal volta ao normal.

---

## Passo 5 — Verificar

```bash
cd /home/clauudio/icube

echo "=== Tamanho ==="
wc -l docs/SERIAL.md

echo ""
echo "=== Primeiras 5 linhas ==="
head -5 docs/SERIAL.md

echo ""
echo "=== Últimas 3 linhas ==="
tail -3 docs/SERIAL.md
Esperado:

text
=== Tamanho ===
~90 docs/SERIAL.md

=== Primeiras 5 linhas ===
# 🔌 Serial (Arduino / USB) — Icube v0.9.0

Comunicação com Arduino, ESP32, CH340, FTDI e dispositivos USB-serial.

## Funções

=== Últimas 3 linhas ===
---

**Documentação Serial — Icube v0.9.0**
Se aparecer isso → SERIAL.md está pronto. ✅

# 🔌 Serial (Arduino / USB) — Icube v0.9.0

Comunicação com Arduino, ESP32, CH340, FTDI e dispositivos USB-serial.

## Funções

| Função | Retorno | Descrição |
|--------|---------|-----------|
| `serial.list()` | `list<Row>` | Lista portas disponíveis |
| `serial.open(porta, baud)` | `null` | Abre conexão |
| `serial.send(mensagem)` | `null` | Envia texto |
| `serial.read()` | `string` | Lê linha (bloqueia) |
| `serial.read_available()` | `string` | Lê disponível (não bloqueia) |
| `serial.close()` | `null` | Fecha conexão |

## Instalação

### Linux

```bash
sudo apt install -y libudev-dev
sudo usermod -a -G dialout $USER
# Depois: logout + login
Windows
Nenhuma dependência extra. Driver CH340 para Arduinos clones.

Uso básico
icube
serial.open("/dev/ttyACM0", 9600);   // Linux
// serial.open("COM3", 9600);        // Windows

serial.send("LIGAR_LED");
let resposta = serial.read();
print("Arduino:", resposta);

serial.close();
text

**Cola no nano** (Shift+Insert ou botão direito → Paste).

---

### Parte B — Exemplos e HTTP (cola depois)
Exemplos práticos
Gravar leituras no banco
icube
db.connect_default();
serial.open("/dev/ttyACM0", 9600);

while true {
    let linha = serial.read_available();
    if linha != "" {
        db.execute(
            "INSERT INTO leituras (dados, data_hora) VALUES ($1, $2)",
            [linha, now()]
        );
        print("Gravado:", linha);
    }
}
Classificar com IA
icube
serial.open("/dev/ttyACM0", 9600);
let linha = serial.read();
if linha != "" {
    let analise = ai.ask("Classifique como normal ou anormal: " + linha);
    print("Leitura:", linha, "->", analise);
}
serial.close();
Endpoints HTTP
Método	Rota	Auth
GET	/api/serial/ports	❌
POST	/api/serial/open	✅
POST	/api/serial/send	✅
GET	/api/serial/read	✅
POST	/api/serial/close	✅
Páginas web
www/serial.html — Monitor serial em tempo real

www/led.html — Controle de LED com sequências

text

**Cola no nano** depois da parte A.

---

### Parte C — Segurança + Troubleshooting (cola por último)
Segurança
Permissões (Linux)
bash
# Adicionar usuário ao dialout
sudo usermod -a -G dialout $USER
# Depois: logout + login
⚠️ Servidor rodando como root: root não está no dialout. Rode como usuário normal.

Recomendações
Não exponha o servidor com serial na internet sem HTTPS

Feche a conexão (serial.close()) ao terminar

/api/serial/* exige autenticação (exceto /ports)

Troubleshooting
serial.list() retorna vazio
Arduino não conectado → ls /dev/ttyACM*

Sem permissão → groups | grep dialout

Windows → instalar driver CH340

Servidor como root → rodar como usuário normal

"falha ao abrir"
Porta em uso → fechar Arduino IDE / minicom

Sem permissão → sudo chmod 666 /dev/ttyACM0 (temporário)

Dados estranhos
Baud rate errado. Confirme que Icube usa o mesmo baud do Arduino.

Documentação Serial — Icube v0.9.0

text

**Cola no nano** depois da parte B.

---

## Passo 4 — Salvar no nano

Depois de colar tudo:

1. **`Ctrl+O`** (salvar)
2. **`Enter`** (confirma o nome do arquivo)
3. **`Ctrl+X`** (sair do nano)

O terminal volta ao normal.

---

## Passo 5 — Verificar

```bash
cd /home/clauudio/icube

echo "=== Tamanho ==="
wc -l docs/SERIAL.md

echo ""
echo "=== Primeiras 5 linhas ==="
head -5 docs/SERIAL.md

echo ""
echo "=== Últimas 3 linhas ==="
tail -3 docs/SERIAL.md
Esperado:

text
=== Tamanho ===
~90 docs/SERIAL.md

=== Primeiras 5 linhas ===
# 🔌 Serial (Arduino / USB) — Icube v0.9.0

Comunicação com Arduino, ESP32, CH340, FTDI e dispositivos USB-serial.

## Funções

=== Últimas 3 linhas ===
---

**Documentação Serial — Icube v0.9.0**
Se aparecer isso → SERIAL.md está pronto. ✅

