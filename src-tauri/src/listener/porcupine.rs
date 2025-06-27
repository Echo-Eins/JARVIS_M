use std::path::Path;

use once_cell::sync::OnceCell;
use porcupine::{Porcupine, PorcupineBuilder};

use crate::db;
use crate::config;
use log::{info, warn, error};

// store porcupine instance
static PORCUPINE: OnceCell<Porcupine> = OnceCell::new();

pub fn init() -> Result<(), ()> {
    let picovoice_api_key: String;

    // retrieve picovoice api key
    picovoice_api_key = db.get().unwrap().api_keys.picovoice.clone();
    if picovoice_api_key.trim().is_empty() {
        warn!("Picovoice API key is not set.");
        return Err(())
    }

    // create porcupine instance with the given API key
    let keywords_dir = crate::config::get_keywords_path()?;
    let keyword_path = keywords_dir.join(config::DEFAULT_KEYWORD);

    match PorcupineBuilder::new_with_keyword_paths(picovoice_api_key, &[keyword_path])
        .sensitivities(&[config::DEFAULT_SENSITIVITY]) // set sensitivity
        .init() {
            Ok(pinstance) => {
                // success
                info!("Porcupine successfully initialized with the given API key.");

                // store
                PORCUPINE.set(pinstance);
            },
            Err(msg) => {
                error!("Porcupine failed to initialize, either API key is not valid or there is no internet connection.");
                error!("Error details: {}", msg);

                return Err(());
            }
    }

    Ok(())
}

pub fn data_callback(frame_buffer: &[i16]) -> Option<i32> {
    if let Ok(keyword_index) = PORCUPINE.get().unwrap().process(&frame_buffer) {
        if keyword_index >= 0 {
            return Some(keyword_index)
        }
    }

    None
}