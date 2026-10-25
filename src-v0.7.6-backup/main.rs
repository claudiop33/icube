mod ai;
mod lexer;
mod parser;
mod eval;
mod type_checker;
mod server;
mod config;

use std::io::{self, Write, BufRead};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    // Auto-carrega .env do diretório atual (silencioso se não existir)
    let _ = dotenvy::dotenv();

    let args: Vec<String> = std::env::args().collect();

    // --version / -V
    if args.len() > 1 && (args[1] == "--version" || args[1] == "-V") {
        println!("icube {}", VERSION);
        return;
    }

    // --help / -h
    if args.len() > 1 && (args[1] == "--help" || args[1] == "-h") {
        print_help();
        return;
    }

    // --serve [porta]
    if args.len() > 1 && (args[1] == "--serve" || args[1] == "serve") {
        let port = args.get(2).and_then(|s| s.parse::<u16>().ok()).unwrap_or(8080);
        if let Err(e) = server::serve_blocking(port) {
            eprintln!("Erro no servidor: {}", e);
            std::process::exit(1);
        }
        return;
    }

    // icube arquivo.icb
    if args.len() > 1 {
        match std::fs::read_to_string(&args[1]) {
            Ok(src) => {
                let code = run_source(&src);
                if code != 0 {
                    std::process::exit(code);
                }
            }
            Err(e) => {
                eprintln!("Erro ao ler '{}': {}", args[1], e);
                std::process::exit(1);
            }
        }
    } else {
        // REPL
        repl();
    }
}

fn print_help() {
    println!("Icube v{} — linguagem de script com banco de dados e IA", VERSION);
    println!();
    println!("USO:");
    println!("    icube [COMANDO] [ARQUIVO]");
    println!();
    println!("COMANDOS:");
    println!("    (sem args)           Inicia o REPL interativo");
    println!("    <arquivo.icb>        Executa um script Icube");
    println!("    --serve [PORTA]      Sobe o servidor web (padrão: 8080)");
    println!("    --version, -V        Mostra a versão");
    println!("    --help, -h           Mostra esta ajuda");
    println!();
    println!("VARIÁVEIS DE AMBIENTE:");
    println!("    ICUBE_DB_URL         URL do banco (postgres://... ou mysql://...)");
    println!("    ICUBE_WWW_DIR        Diretório de arquivos estáticos (padrão: www)");
    println!("    ICUBE_TIMEOUT_SECS   Timeout de execução no servidor (padrão: 30)");
    println!("    ICUBE_REQUIRE_AUTH   1 = exige login no /execute");
    println!("    ICUBE_TLS_AUTO       1 = auto-gerar certificado self-signed");
    println!("    ICUBE_TLS_CERT       Caminho do certificado PEM (HTTPS real)");
    println!("    ICUBE_TLS_KEY        Caminho da chave privada PEM");
    println!("    ICUBE_INSECURE_COOKIE  1 = desabilita Secure no cookie (dev)");
    println!("    ICUBE_DEV_RESET      1 = habilita reset-admin (use com token)");
    println!("    ICUBE_DEV_RESET_TOKEN  Token para reset-admin");
    println!("    OLLAMA_HOST          URL do Ollama (padrão: http://localhost:11434)");
    println!("    ICUBE_AI_MODEL       Modelo de IA (padrão: llama3)");
}

fn compile(src: &str) -> Result<Vec<parser::Stmt>, String> {
    let tokens = lexer::Lexer::new(src).tokenize().map_err(|e| e.to_string())?;
    let mut parser = parser::Parser::new(tokens);
    parser.parse_program().map_err(|e| e.to_string())
}

fn check(program: &[parser::Stmt]) -> Result<(), String> {
    let checker = type_checker::TypeChecker::new();
    checker.check_program(program).map_err(|e| e.to_string())
}

/// Executa um script e retorna o exit code (0 = sucesso, 1 = erro, N = exit(N) do script).
fn run_source(src: &str) -> i32 {
    let program = match compile(src) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Erro: {}", e);
            return 1;
        }
    };
    if let Err(e) = check(&program) {
        eprintln!("Erro de tipo: {}", e);
        return 1;
    }
    let mut evaluator = eval::Evaluator::new();
    match evaluator.run(&program) {
        Ok(()) => 0,
        Err(eval::EvalError(msg)) if msg.starts_with("__ICUBE_EXIT_") => {
            let code: i32 = msg
                .trim_start_matches("__ICUBE_EXIT_")
                .trim_end_matches("__")
                .parse()
                .unwrap_or(0);
            code
        }
        Err(e) => {
            eprintln!("Erro de execução: {}", e);
            1
        }
    }
}

//fn repl() {
//    println!("Icube REPL v{} — digite 'exit' para sair", VERSION);
 //   let stdin = io::stdin();
 //   let mut evaluator = eval::Evaluator::new();
 //   loop {
 //       print!("icube> ");
 //       io::stdout().flush().ok();
 //       let mut line = String::new();
 //       if stdin.lock().read_line(&mut line).is_err() { break; }
 //       let line = line.trim();
 //       if line.eq_ignore_ascii_case("exit") { break; }
 //       if line.is_empty() { continue; }
  //      let program = match compile(line) {
//            Ok(p) => p,
//            Err(e) => { eprintln!("Erro: {}", e); continue; }
//        };
        // Usa o estado atual do evaluator para alimentar o checker
        // (variáveis de linhas anteriores são visíveis ao type checker).
//        let initial_types = evaluator.initial_types();
//        let checker = type_checker::TypeChecker::new();
//        if let Err(e) = checker.check_program_with_env(&program, initial_types) {
//            eprintln!("Erro de tipo: {}", e);
//            continue;
//        }
 //       match evaluator.run(&program) {
 //           Ok(()) => {}
 //           Err(eval::EvalError(msg)) if msg.starts_with("__ICUBE_EXIT_") => {
 //               let code: i32 = msg
 //                   .trim_start_matches("__ICUBE_EXIT_")
 //                   .trim_end_matches("__")
 //                   .parse()
 //                   .unwrap_or(0);
 //               std::process::exit(code);
 //           }
 //           Err(e) => eprintln!("Erro de execução: {}", e),
 //       }
 //   }
//}


fn repl() {
    println!("Icube REPL v{} — digite 'exit' para sair", VERSION);
    let stdin = io::stdin();
    let mut evaluator = eval::Evaluator::new();
    loop {
        print!("icube> ");
        io::stdout().flush().ok();
        let mut raw = String::new();
        if stdin.lock().read_line(&mut raw).is_err() { break; }
        let raw = raw.trim();
        if raw.eq_ignore_ascii_case("exit") { break; }
        if raw.is_empty() { continue; }

        // Auto-insere `;` se o usuário esqueceu.
        // Só pula quando a linha termina em `{` (bloco aberto) ou `}` (fechamento)
        // para não estragar construções multi-linha.
        let line: String = if raw.ends_with(';')
            || raw.ends_with('{')
            || raw.ends_with('}')
        {
            raw.to_string()
        } else {
            format!("{};", raw)
        };

        let program = match compile(&line) {
            Ok(p) => p,
            Err(e) => { eprintln!("Erro: {}", e); continue; }
        };
        // Usa o estado atual do evaluator para alimentar o checker
        // (variáveis de linhas anteriores são visíveis ao type checker).
        let initial_types = evaluator.initial_types();
        let checker = type_checker::TypeChecker::new();
        if let Err(e) = checker.check_program_with_env(&program, initial_types) {
            eprintln!("Erro de tipo: {}", e);
            continue;
        }
        match evaluator.run(&program) {
            Ok(()) => {}
            Err(eval::EvalError(msg)) if msg.starts_with("__ICUBE_EXIT_") => {
                let code: i32 = msg
                    .trim_start_matches("__ICUBE_EXIT_")
                    .trim_end_matches("__")
                    .parse()
                    .unwrap_or(0);
                std::process::exit(code);
            }
            Err(e) => eprintln!("Erro de execução: {}", e),
        }
    }
}

