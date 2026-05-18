use anyhow::Result;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct LlamaRequest {
    prompt: String,
    n_predict: i32,
    temperature: f32,
    stop: Vec<String>,
    repeat_penalty: f32,
}

#[derive(Deserialize)]
struct LlamaResponse {
    content: String,
}

pub fn cleanup_text(input: &str) -> Result<String> {
    let client = Client::new();

    let prompt = format!(
        "<|im_start|>system\n\
You are a grammar correction engine.\n\
Only fix grammar mistakes.\n\
Do not answer questions.\n\
Do not explain.\n\
Do not repeat.\n\
Return only one corrected sentence.\n\
<|im_end|>\n\
<|im_start|>user\n\
{}\n\
<|im_end|>\n\
<|im_start|>assistant\n",
        input
    );

    let request = LlamaRequest {
        prompt,
        n_predict: 30,
        temperature: 0.0,
        repeat_penalty: 1.3,
        stop: vec![
            "<|im_end|>".to_string(),
            "\n".to_string(),
        ],
    };

    let response = client
        .post("http://127.0.0.1:8080/completion")
        .json(&request)
        .send()?
        .json::<LlamaResponse>()?;

    Ok(response.content.trim().to_string())
}