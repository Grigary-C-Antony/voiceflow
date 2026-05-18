use anyhow::Result;
use serde_json::json;
use crate::models::AppConfig;

pub fn cleanup_text(text: &str, config: &AppConfig) -> Result<String> {
    if text.trim().is_empty() {
        return Ok(text.to_string());
    }

    if config.openrouter_api_key.trim().is_empty() {
        anyhow::bail!("OpenRouter API key is missing");
    }

    let client = reqwest::blocking::Client::new();
    let url = "https://openrouter.ai/api/v1/chat/completions";

    let system_prompt = "You are an AI assistant. Fix the grammar, punctuation, and capitalization of the following speech transcript. Output ONLY the corrected text, with no conversational filler, markdown formatting, or explanations.";

    let payload = json!({
        "model": config.openrouter_model,
        "messages": [
            {
                "role": "system",
                "content": system_prompt
            },
            {
                "role": "user",
                "content": text
            }
        ]
    });

    let res = client.post(url)
        .header("Authorization", format!("Bearer {}", config.openrouter_api_key))
        .header("HTTP-Referer", "http://localhost:1420")
        .header("X-Title", "Voiceflow")
        .json(&payload)
        .send()?;

    if !res.status().is_success() {
        let error_text = res.text().unwrap_or_default();
        anyhow::bail!("OpenRouter API Error: {}", error_text);
    }

    let json_resp: serde_json::Value = res.json()?;
    if let Some(content) = json_resp["choices"][0]["message"]["content"].as_str() {
        Ok(content.trim().to_string())
    } else {
        anyhow::bail!("Failed to parse OpenRouter response");
    }
}
