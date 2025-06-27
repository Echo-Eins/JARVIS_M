// STT module exports
pub mod vosk;

// Re-export main functions
pub use vosk::*;

use crate::error::JarvisResult;

pub fn init() -> JarvisResult<()> {
    vosk::init_vosk();
    log::info!("STT system initialized");
    Ok(())
}

pub fn shutdown() -> JarvisResult<()> {
    log::info!("STT system shutdown completed");
    Ok(())
}