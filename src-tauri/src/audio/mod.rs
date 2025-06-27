// src-tauri/src/audio/mod.rs

pub mod audio_monitor;
pub mod recorder;
pub mod pvrecorder;
pub mod portaudio;
pub mod kira;
pub mod cpal;
pub mod rodio;

// Re-export main functions
pub use audio_monitor::*;
pub use recorder::*;

use crate::error::JarvisResult;
use log::info;

pub fn init() -> JarvisResult<()> {
    audio_monitor::init()?;
    recorder::init()?;
    info!("Audio system initialized");
    Ok(())
}

pub fn shutdown() -> JarvisResult<()> {
    audio_monitor::shutdown()?;
    recorder::shutdown()?;
    info!("Audio system shutdown completed");
    Ok(())
}