// use anyhow::Result;
// use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
// use std::sync::{Arc, Mutex};
// use std::time::Duration;

// pub fn record_audio() -> Result<()> {
//     let host = cpal::default_host();

//     let device = host
//         .default_input_device()
//         .expect("No input device found");

//     let config = device.default_input_config()?;

//     println!("Input config: {:?}", config);

//     let sample_rate = config.sample_rate().0;
//     let channels = config.channels();

//     let spec = hound::WavSpec {
//         channels,
//         sample_rate,
//         bits_per_sample: 16,
//         sample_format: hound::SampleFormat::Int,
//     };

//     let temp_path = std::env::temp_dir().join("voiceflow_recording.wav");

//     let writer = Arc::new(Mutex::new(
//         hound::WavWriter::create(&temp_path, spec)?
//     ));

//     let writer_clone = writer.clone();

//     let err_fn = |err| eprintln!("stream error: {}", err);

//     let stream = match config.sample_format() {
//         cpal::SampleFormat::F32 => {
//             let cfg: cpal::StreamConfig = config.clone().into();

//             device.build_input_stream(
//                 &cfg,
//                 move |data: &[f32], _| {
//                     let mut writer = writer_clone.lock().unwrap();

//                     for &sample in data {
//                         let sample_i16 = (sample * i16::MAX as f32) as i16;
//                         writer.write_sample(sample_i16).ok();
//                     }
//                 },
//                 err_fn,
//                 None,
//             )?
//         }

//         cpal::SampleFormat::I16 => {
//             let cfg: cpal::StreamConfig = config.clone().into();
//             let writer_clone = writer.clone();

//             device.build_input_stream(
//                 &cfg,
//                 move |data: &[i16], _| {
//                     let mut writer = writer_clone.lock().unwrap();

//                     for &sample in data {
//                         writer.write_sample(sample).ok();
//                     }
//                 },
//                 err_fn,
//                 None,
//             )?
//         }

//         cpal::SampleFormat::U16 => {
//             let cfg: cpal::StreamConfig = config.clone().into();
//             let writer_clone = writer.clone();

//             device.build_input_stream(
//                 &cfg,
//                 move |data: &[u16], _| {
//                     let mut writer = writer_clone.lock().unwrap();

//                     for &sample in data {
//                         let centered = (sample as i32 - 32768) as i16;
//                         writer.write_sample(centered).ok();
//                     }
//                 },
//                 err_fn,
//                 None,
//             )?
//         }

//         _ => panic!("Unsupported sample format"),
//     };

//     stream.play()?;

//     std::thread::sleep(Duration::from_secs(5));

//     drop(stream);

//     println!("Saved {:?}", temp_path);

//     Ok(())
// }

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
// use std::fs::File;
use std::path::PathBuf;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hound::{WavWriter, WavSpec, SampleFormat};

pub fn start_recording_stream(
    is_recording: &Arc<Mutex<bool>>,
    max_duration: Duration,
) {
    let host = cpal::default_host();
    let device = host.default_input_device().expect("No input device");
    let config = device.default_input_config().expect("No default config");

    let temp_path: PathBuf = std::env::temp_dir().join("voiceflow_recording.wav");

    let spec = WavSpec {
        channels: config.channels(),
        sample_rate: config.sample_rate().0,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let writer = Arc::new(Mutex::new(
        Some(WavWriter::create(&temp_path, spec).expect("Failed to create WAV"))
    ));
    let writer_clone = Arc::clone(&writer);

    let stream = device.build_input_stream(
        &config.into(),
        move |data: &[f32], _| {
            if let Ok(mut guard) = writer_clone.lock() {
                if let Some(ref mut w) = *guard {
                    for &sample in data {
                        let s = (sample * i16::MAX as f32) as i16;
                        w.write_sample(s).ok();
                    }
                }
            }
        },
        |err| eprintln!("Stream error: {}", err),
        None,
    ).expect("Failed to build stream");

    stream.play().expect("Failed to start stream");

    let start = Instant::now();

    // Poll the flag — stop when user clicks stop or 2min elapses
    loop {
        std::thread::sleep(Duration::from_millis(50));

        let still_recording = *is_recording.lock().unwrap();
        let timed_out = start.elapsed() >= max_duration;

        if !still_recording || timed_out {
            if timed_out {
                // Force flag off so stop_recording knows
                *is_recording.lock().unwrap() = false;
                eprintln!("Recording stopped: 2 minute timeout reached");
            }
            break;
        }
    }

    std::thread::sleep(Duration::from_millis(500));
    drop(stream); // stops capturing

    // Finalize the WAV file
    // if let Ok(mut guard) = writer.lock() {
    //     if let Some(w) = guard.take() {
    //         w.finalize().expect("Failed to finalize WAV");
    //     }
    // }
    {
        let mut guard = writer.lock().unwrap();
        if let Some(w) = guard.take() {
            w.finalize().expect("Failed to finalize WAV");
        }
    } //
}