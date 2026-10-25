use std::collections::HashMap;
use std::path::PathBuf;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, Default)]
pub struct Config {
    #[serde(default)]
    pub connections: HashMap<String, ConnectionEntry>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ConnectionEntry {
    pub url: Option<String>,
    pub url_env: Option<String>,
}

impl Config {
    /// Procura por `./icube.toml` e depois por `~/.icube/config.toml`.
    /// Retorna `Ok(None)` se nenhum existir (não é erro).
    pub fn load() -> Result<Option<Self>, String> {
        for p in candidate_paths() {
            if p.exists() {
                let content = std::fs::read_to_string(&p)
                    .map_err(|e| format!("erro ao ler {}: {}", p.display(), e))?;
                let cfg: Config = toml::from_str(&content)
                    .map_err(|e| format!("erro no TOML {}: {}", p.display(), e))?;
                return Ok(Some(cfg));
            }
        }
        Ok(None)
    }

    pub fn resolve(&self, name: &str) -> Result<String, String> {
        let entry = self.connections.get(name).ok_or_else(|| {
            let disponiveis: Vec<&String> = self.connections.keys().collect();
            if disponiveis.is_empty() {
                format!("conexão '{}' não encontrada (icube.toml não tem [connections])", name)
            } else {
                let lista: Vec<String> = disponiveis.iter().map(|s| s.to_string()).collect();
                format!("conexão '{}' não encontrada. Disponíveis: {}", name, lista.join(", "))
            }
        })?;

        if let Some(url) = &entry.url {
            return Ok(url.clone());
        }
        if let Some(var) = &entry.url_env {
            return match std::env::var(var) {
                Ok(v) if !v.is_empty() => Ok(v),
                Ok(_) => Err(format!("variável '{}' está vazia", var)),
                Err(_) => Err(format!("variável '{}' não definida (configure no .env)", var)),
            };
        }
        Err(format!("conexão '{}' não tem 'url' nem 'url_env'", name))
    }

    pub fn names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.connections.keys().cloned().collect();
        v.sort();
        v
    }
}

fn candidate_paths() -> Vec<PathBuf> {
    let mut paths = vec![PathBuf::from("icube.toml")];
    if let Ok(home) = std::env::var("HOME") {
        paths.push(PathBuf::from(home).join(".icube/config.toml"));
    }
    paths
}