use std::time::Duration;

pub struct AiContext {
    client: reqwest::blocking::Client,
    base_url: String,
    model: String,
}

impl AiContext {
    pub fn new() -> Self {
        let base_url = std::env::var("OLLAMA_HOST")
            .unwrap_or_else(|_| "http://localhost:11434".to_string())
            .trim_end_matches('/')
            .to_string();
        let model = std::env::var("ICUBE_AI_MODEL")
            .unwrap_or_else(|_| "llama3".to_string());

        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(180))
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());

        Self { client, base_url, model }
    }

    pub fn model_name(&self) -> String {
        self.model.clone()
    }

    pub fn is_available(&self) -> bool {
        self.client
            .get(format!("{}/api/tags", self.base_url))
            .timeout(Duration::from_secs(2))
            .send()
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    pub fn ask(&self, prompt: &str) -> Result<String, String> {
        let body = serde_json::json!({
            "model": self.model,
            "prompt": prompt,
            "stream": false,
        });
        self.post_generate(body)
    }

    pub fn ask_with_system(&self, system: &str, prompt: &str) -> Result<String, String> {
        let body = serde_json::json!({
            "model": self.model,
            "system": system,
            "prompt": prompt,
            "stream": false,
        });
        self.post_generate(body)
    }

    fn post_generate(&self, body: serde_json::Value) -> Result<String, String> {
        let resp = self.client
            .post(format!("{}/api/generate", self.base_url))
            .json(&body)
            .send()
            .map_err(|e| format!(
                "falha ao chamar Ollama em {}: {} (o serviço está rodando? `ollama serve`)",
                self.base_url, e
            ))?;

        if !resp.status().is_success() {
            return Err(format!(
                "Ollama retornou status {} — verifique se o modelo '{}' está baixado (`ollama pull {}`)",
                resp.status(), self.model, self.model
            ));
        }

        let json: serde_json::Value = resp.json()
            .map_err(|e| format!("resposta inválida do Ollama: {}", e))?;

        let response = json.get("response")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        Ok(response.trim().to_string())
    }
}