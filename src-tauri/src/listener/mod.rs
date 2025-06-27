// Wake word detection module exports
pub mod porcupine;
pub mod rustpotter;
pub mod vosk;

// Re-export based on configuration
pub use rustpotter::*; // default

use crate::error::JarvisResult;

pub fn init() -> JarvisResult<()> {
    // Initialize default wake word engine
    rustpotter::init();
}

pub fn shutdown() -> JarvisResult<()> {
    log::info!("Listener shutdown completed");
    Ok(())
}