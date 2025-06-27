// src-tauri/src/log.rs
use crate::error::JarvisResult;

pub fn init() -> JarvisResult<()> {
    // Инициализация логирования
    simple_log::new(log::LevelFilter::Info)
        .map_err(|e| crate::error::JarvisError::Generic(format!("Log init failed: {}", e)))?;
    Ok(())
}