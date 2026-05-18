use tauri::{AppHandle, Manager, Emitter};
use std::path::PathBuf;
use std::fs;
use std::io::Read;
use reqwest::blocking::Client;
use std::thread;

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct AppConfig {
    pub active_model: String,
    pub output_method: String, // "type" | "clipboard"
    pub typing_speed: u64, // delay in ms
    pub translate: bool,
    pub language: String,
    pub hotkey: String,
    pub always_on_top: bool,
    pub grammar_enabled: bool,
    pub openrouter_api_key: String,
    pub openrouter_model: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            active_model: "tiny.en".to_string(),
            output_method: "type".to_string(),
            typing_speed: 20,
            translate: false,
            language: "en".to_string(),
            hotkey: "Control+Shift+Space".to_string(),
            always_on_top: true,
            grammar_enabled: false,
            openrouter_api_key: "".to_string(),
            openrouter_model: "openai/gpt-4o-mini".to_string(),
        }
    }
}

pub fn get_models_dir(app: &AppHandle) -> PathBuf {
    let dir = app.path().app_local_data_dir().unwrap().join("models");
    if !dir.exists() {
        fs::create_dir_all(&dir).unwrap();
    }
    dir
}

pub fn get_active_model_path(app: &AppHandle) -> PathBuf {
    let config = get_config(app.clone()).unwrap_or_default();
    let active_model = config.active_model;
    
    // First, check if the downloaded model exists in app_local_data
    let downloaded_path = get_models_dir(app).join(format!("ggml-{}.bin", active_model));
    if downloaded_path.exists() {
        return downloaded_path;
    }
    
    // Fallback: Check if bundled locally (for tiny.en which is in tauri bundle)
    let exe_dir = std::env::current_exe().unwrap().parent().unwrap().to_path_buf();
    let bundled_path = exe_dir.join("models").join(format!("ggml-{}.bin", active_model));
    if bundled_path.exists() {
        return bundled_path;
    }
    
    // Fallback for dev mode
    let dev_model = PathBuf::from(format!("../models/ggml-{}.bin", active_model));
    if dev_model.exists() {
        return dev_model;
    }
    
    // Default fallback to whatever path so it can be handled by whisper
    downloaded_path
}

#[tauri::command]
pub fn get_config(app: AppHandle) -> Result<AppConfig, String> {
    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    let config_path = config_dir.join("config.json");
    if let Ok(content) = fs::read_to_string(&config_path) {
        if let Ok(config) = serde_json::from_str::<AppConfig>(&content) {
            return Ok(config);
        }
    }
    Ok(AppConfig::default())
}

#[tauri::command]
pub fn set_config(app: AppHandle, config: AppConfig) -> Result<(), String> {
    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
    }
    let config_path = config_dir.join("config.json");
    
    let json = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    fs::write(config_path, json).map_err(|e| e.to_string())?;
    Ok(())
}

// Keep these for backward compatibility with frontend if they are still used individually
#[tauri::command]
pub fn get_active_model(app: AppHandle) -> Result<String, String> {
    Ok(get_config(app)?.active_model)
}

#[tauri::command]
pub fn set_active_model(app: AppHandle, model_name: String) -> Result<(), String> {
    let mut config = get_config(app.clone())?;
    config.active_model = model_name;
    set_config(app, config)
}

#[tauri::command]
pub fn get_available_models(app: AppHandle) -> Result<Vec<String>, String> {
    let mut models = vec![];
    let dir = get_models_dir(&app);
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_file() {
                    let file_name = entry.file_name().to_string_lossy().to_string();
                    if file_name.starts_with("ggml-") && file_name.ends_with(".bin") {
                        let name = file_name.replace("ggml-", "").replace(".bin", "");
                        models.push(name);
                    }
                }
            }
        }
    }
    // Always include tiny.en since it's bundled
    if !models.contains(&"tiny.en".to_string()) {
        models.push("tiny.en".to_string());
    }
    Ok(models)
}

#[derive(Clone, serde::Serialize)]
struct ProgressPayload {
    model: String,
    progress: f64,
}

#[derive(Clone, serde::Serialize)]
struct ErrorPayload {
    model: String,
    error: String,
}

#[tauri::command]
pub fn download_model(app: AppHandle, model_name: String) -> Result<(), String> {
    let dir = get_models_dir(&app);
    let target_path = dir.join(format!("ggml-{}.bin", model_name));
    let url = format!("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{}.bin", model_name);
    
    thread::spawn(move || {
        let client = Client::new();
        match client.get(&url).send() {
            Ok(mut response) => {
                if !response.status().is_success() {
                    let _ = app.emit("download-error", ErrorPayload { model: model_name.clone(), error: format!("HTTP {}", response.status()) });
                    return;
                }
                let total_size = response.content_length().unwrap_or(0);
                let mut file = match fs::File::create(&target_path) {
                    Ok(f) => f,
                    Err(e) => {
                        let _ = app.emit("download-error", ErrorPayload { model: model_name.clone(), error: e.to_string() });
                        return;
                    }
                };
                
                let mut downloaded: u64 = 0;
                let mut buffer = [0; 32768]; // 32KB buffer
                let mut last_progress = 0.0;
                
                loop {
                    match response.read(&mut buffer) {
                        Ok(0) => break, // EOF
                        Ok(n) => {
                            if let Err(e) = std::io::Write::write_all(&mut file, &buffer[0..n]) {
                                let _ = app.emit("download-error", ErrorPayload { model: model_name.clone(), error: e.to_string() });
                                return;
                            }
                            downloaded += n as u64;
                            if total_size > 0 {
                                let progress = (downloaded as f64 / total_size as f64) * 100.0;
                                // Emit every 1% or on completion
                                if progress - last_progress >= 1.0 || progress == 100.0 {
                                    last_progress = progress;
                                    let _ = app.emit("download-progress", ProgressPayload {
                                        model: model_name.clone(),
                                        progress,
                                    });
                                }
                            }
                        }
                        Err(e) => {
                            let _ = app.emit("download-error", ErrorPayload { model: model_name.clone(), error: e.to_string() });
                            return;
                        }
                    }
                }
                let _ = app.emit("download-complete", model_name);
            }
            Err(e) => {
                let _ = app.emit("download-error", ErrorPayload { model: model_name.clone(), error: e.to_string() });
            }
        }
    });
    Ok(())
}
