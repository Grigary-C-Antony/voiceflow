// mod recorder;
// mod transcriber;
// mod injector;

// use std::sync::{Arc, Mutex};
// use std::time::Duration;
// use std::thread;
// use tauri::{Manager, State, AppHandle};

// pub struct RecordingState {
//     pub is_recording: Arc<Mutex<bool>>,
//     pub recording_thread: Mutex<Option<thread::JoinHandle<()>>>,
// }

// #[tauri::command]
// fn start_recording(state: State<RecordingState>) -> Result<String, String> {
//     let mut is_rec = state.is_recording.lock().unwrap();
//     if *is_rec {
//         return Err("Already recording".to_string());
//     }
//     *is_rec = true;
//     drop(is_rec);

//     let flag = Arc::clone(&state.is_recording);
//     let max_duration = Duration::from_secs(120);

//     let handle = thread::spawn(move || {
//         recorder::start_recording_stream(&flag, max_duration);
//     });

//     *state.recording_thread.lock().unwrap() = Some(handle);

//     Ok("recording_started".to_string())
// }

// #[tauri::command]
// fn stop_recording(
//     state: State<RecordingState>,
//     app: AppHandle,
// ) -> Result<String, String> {
//     {
//         let mut is_rec = state.is_recording.lock().unwrap();
//         if !*is_rec {
//             return Err("Not recording".to_string());
//         }
//         *is_rec = false;
//     }

//     let handle = state.recording_thread.lock().unwrap().take();
//     if let Some(h) = handle {
//         h.join().map_err(|_| "Recording thread panicked".to_string())?;
//     }

//     let temp_path = std::env::temp_dir().join("voiceflow_recording.wav");

//     let raw_text = transcriber::transcribe_audio(
//         temp_path.to_str().unwrap()
//     ).map_err(|e| e.to_string())?;

//     // Hide our window so the previous app regains focus
//     if let Some(window) = app.get_webview_window("main") {
//         window.hide().ok();
//     }

//     // Give the OS time to switch focus back to the previous window
//     thread::sleep(Duration::from_millis(300));

//     injector::type_text(&raw_text)
//         .map_err(|e| e.to_string())?;

//     // Show our window again after typing
//     if let Some(window) = app.get_webview_window("main") {
//         window.show().ok();
//         // Don't steal focus back — just show it
//         window.set_focus().ok();
//     }

//     Ok(raw_text)
// }

// fn main() {
//     tauri::Builder::default()
//         .manage(RecordingState {
//             is_recording: Arc::new(Mutex::new(false)),
//             recording_thread: Mutex::new(None),
//         })
//         .setup(|_app| {
//             Ok(())
//         })
//         .invoke_handler(tauri::generate_handler![start_recording, stop_recording])
//         .run(tauri::generate_context!())
//         .expect("error while running tauri application");
// }

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod recorder;
mod transcriber;
mod injector;
mod models;
mod grammar;

use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::thread;
use tauri::{
    Manager, State, AppHandle, Emitter,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

pub struct RecordingState {
    pub is_recording: Arc<Mutex<bool>>,
    pub recording_thread: Mutex<Option<thread::JoinHandle<()>>>,
}

fn do_start_recording(state: &RecordingState) -> Result<String, String> {
    let mut is_rec = state.is_recording.lock().unwrap();
    if *is_rec {
        return Err("Already recording".to_string());
    }
    *is_rec = true;
    drop(is_rec);

    let flag = Arc::clone(&state.is_recording);
    let max_duration = Duration::from_secs(120);

    let handle = thread::spawn(move || {
        recorder::start_recording_stream(&flag, max_duration);
    });

    *state.recording_thread.lock().unwrap() = Some(handle);
    Ok("recording_started".to_string())
}

fn do_stop_recording(state: &RecordingState, app: &AppHandle) -> Result<String, String> {
    {
        let mut is_rec = state.is_recording.lock().unwrap();
        if !*is_rec {
            return Err("Not recording".to_string());
        }
        *is_rec = false;
    }

    let handle = state.recording_thread.lock().unwrap().take();
    if let Some(h) = handle {
        h.join().map_err(|_| "Recording thread panicked".to_string())?;
    }

    let temp_path = std::env::temp_dir().join("voiceflow_recording.wav");
    let raw_text = transcriber::transcribe_audio(
        app,
        temp_path.to_str().unwrap()
    ).map_err(|e| e.to_string())?;

    do_process_transcription(app, &raw_text);

    Ok(raw_text)
}

fn do_process_transcription(app: &AppHandle, raw_text: &str) {
    let config = models::get_config(app.clone()).unwrap_or_default();
    
    // Apply Grammar Engine if enabled
    let mut text_to_output = raw_text.to_string();
    if config.grammar_enabled {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.emit("status-update", "Grammar check...");
        }
        match grammar::cleanup_text(&text_to_output, &config) {
            Ok(cleaned) => {
                text_to_output = cleaned;
            }
            Err(e) => {
                println!("Grammar engine failed: {}", e);
                // We just fall back to raw_text on error
            }
        }
    }

    if config.output_method == "clipboard" {
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(&text_to_output);
        }
    } else {
        if let Some(window) = app.get_webview_window("main") {
            window.hide().ok();
        }
        thread::sleep(Duration::from_millis(300));
        let _ = injector::type_text(&text_to_output, config.typing_speed);
        if let Some(window) = app.get_webview_window("main") {
            window.show().ok();
        }
    }
}

#[tauri::command]
fn start_recording(state: State<RecordingState>) -> Result<String, String> {
    do_start_recording(&state)
}

#[tauri::command]
fn stop_recording(
    state: State<RecordingState>,
    app: AppHandle,
) -> Result<String, String> {
    do_stop_recording(&state, &app)
}

#[tauri::command]
fn resize_window(app: AppHandle, width: f64, height: f64) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.set_size(tauri::LogicalSize::new(width, height))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn set_always_on_top(app: AppHandle, always_on_top: bool) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.set_always_on_top(always_on_top).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn update_hotkey(app: AppHandle, hotkey: String) -> Result<(), String> {
    let shortcut = hotkey.parse::<Shortcut>().map_err(|e| e.to_string())?;
    let _ = app.global_shortcut().unregister_all();
    
    let app_handle = app.clone();
    app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            handle_hotkey_press(&app_handle);
        }
    }).map_err(|e| e.to_string())?;
    
    Ok(())
}

fn handle_hotkey_press(app_handle: &AppHandle) {
    let state = app_handle.state::<RecordingState>();
    let is_rec = *state.is_recording.lock().unwrap();

    if is_rec {
        let app_clone = app_handle.clone();
        let state_flag = Arc::clone(&state.is_recording);
        let thread_handle = state.recording_thread.lock().unwrap().take();

        thread::spawn(move || {
            *state_flag.lock().unwrap() = false;

            if let Some(h) = thread_handle {
                h.join().ok();
            }

            let temp_path = std::env::temp_dir().join("voiceflow_recording.wav");

            if let Ok(text) = transcriber::transcribe_audio(
                &app_clone,
                temp_path.to_str().unwrap()
            ) {
                do_process_transcription(&app_clone, &text);

                app_clone.get_webview_window("main")
                    .map(|w| w.emit("recording-stopped", text));
            }
        });
    } else {
        do_start_recording(&state).ok();
        app_handle.get_webview_window("main")
            .map(|w| w.emit("recording-started", ()));
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(RecordingState {
            is_recording: Arc::new(Mutex::new(false)),
            recording_thread: Mutex::new(None),
        })
        .setup(|app| {
            let show = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let hide = MenuItem::with_id(app, "hide", "Hide", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &hide, &quit])?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("Voiceflow")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            w.show().ok();
                            w.set_focus().ok();
                        }
                    }
                    "hide" => {
                        if let Some(w) = app.get_webview_window("main") {
                            w.hide().ok();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event {
                        if let Some(w) = tray.app_handle().get_webview_window("main") {
                            if w.is_visible().unwrap_or(false) {
                                w.hide().ok();
                            } else {
                                w.show().ok();
                                w.set_focus().ok();
                            }
                        }
                    }
                })
                .build(app)?;

            let config = models::get_config(app.handle().clone()).unwrap_or_default();
            
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_always_on_top(config.always_on_top);
            }

            let hotkey_str = config.hotkey;
            
            if let Ok(shortcut) = hotkey_str.parse::<Shortcut>() {
                let app_handle = app.handle().clone();
                let _ = app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        handle_hotkey_press(&app_handle);
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_recording, 
            stop_recording,
            resize_window,
            update_hotkey,
            set_always_on_top,
            models::get_config,
            models::set_config,
            models::get_active_model,
            models::set_active_model,
            models::get_available_models,
            models::download_model
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}