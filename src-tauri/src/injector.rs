use anyhow::Result;
use enigo::{Enigo, Keyboard, Settings};

pub fn type_text(text: &str, delay_ms: u64) -> Result<()> {
    std::thread::sleep(std::time::Duration::from_millis(100));

    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| anyhow::anyhow!("Enigo init failed: {}", e))?;

    // Split into chunks — enigo drops tail on long strings on Windows
    let chunk_size = 50;
    let chars: Vec<char> = text.chars().collect();

    for chunk in chars.chunks(chunk_size) {
        let s: String = chunk.iter().collect();
        enigo.text(&s)
            .map_err(|e| anyhow::anyhow!("Typing failed: {}", e))?;

        if delay_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        }
    }

    Ok(())
}