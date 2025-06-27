// src-tauri/src/audio/mod.rs

pub mod monitor;
pub mod recorder;

// Re-export main functions
pub use audio_monitor::*;
pub use recorder::*;

use crate::error::JarvisResult;
use log::info;

pub fn init() -> JarvisResult<()> {
    monitor::init()?;
    recorder::init()?;
    info!("Audio system initialized");
    Ok(())
}

pub fn shutdown() -> JarvisResult<()> {
    monitor::shutdown()?;
    recorder::shutdown()?;
    info!("Audio system shutdown completed");
    Ok(())
}