// use anyhow::Result;
// use std::process::Command;

// pub fn transcribe_audio(path: &str) -> Result<String> {
//     let temp_output = std::env::temp_dir().join("voiceflow_transcript");

//     let status = Command::new("../bin/whisper-cli.exe")
//         .args([
//             "-m",
//             "../models/ggml-tiny.en.bin",
//             "-f",
//             path,
//             "-otxt",
//             "-of",
//             temp_output.to_str().unwrap(),
//             "-l",
//             "en",
//             "--no-timestamps",
//             "--max-context", "64",   // more context = better tail completion
//         ])
//         .status()?;

//     if !status.success() {
//         anyhow::bail!("whisper transcription failed");
//     }

//     let transcript_file = temp_output.with_extension("txt");

//     let text = std::fs::read_to_string(transcript_file)?;

//     Ok(text.trim().replace('\n', " ").replace("  ", " "))
// }

use anyhow::Result;
use std::path::PathBuf;
use std::process::Command;

use tauri::AppHandle;

pub fn transcribe_audio(app: &AppHandle, path: &str) -> Result<String> {
    let temp_output = std::env::temp_dir().join("voiceflow_transcript");

    let dev_whisper = PathBuf::from("../bin/whisper-cli.exe");
    let exe_dir = std::env::current_exe()?
        .parent()
        .unwrap()
        .to_path_buf();
    let bundled_whisper = exe_dir.join("bin/whisper-cli.exe");

    let whisper = if dev_whisper.exists() {
        dev_whisper
    } else {
        bundled_whisper
    };

    let config = crate::models::get_config(app.clone()).unwrap_or_default();
    let model = crate::models::get_active_model_path(app);

    #[cfg(target_os = "windows")]
    use std::os::windows::process::CommandExt;

    let mut cmd = Command::new(&whisper);
    let mut args = vec![
        "-m".to_string(),
        model.to_str().unwrap().to_string(),
        "-f".to_string(),
        path.to_string(),
        "-otxt".to_string(),
        "-of".to_string(),
        temp_output.to_str().unwrap().to_string(),
        "-l".to_string(),
        config.language.clone(),
        "--no-timestamps".to_string(),
        "--max-context".to_string(),
        "64".to_string(),
        "-t".to_string(),
        "4".to_string(),
        "--no-gpu".to_string(),
    ];

    if config.translate {
        args.push("-tr".to_string());
    }

    cmd.args(args);

    #[cfg(target_os = "windows")]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let status = cmd.status()?;

    if !status.success() {
        anyhow::bail!("whisper transcription failed");
    }

    let transcript_file = temp_output.with_extension("txt");
    let text = std::fs::read_to_string(transcript_file)?;

    Ok(text.trim().replace('\n', " ").replace("  ", " "))
}