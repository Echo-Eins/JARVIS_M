use std::path::Path;
use std::sync::Mutex;

use once_cell::sync::OnceCell;
use rustpotter::{Rustpotter, RustpotterConfig, WavFmt, DetectorConfig, FiltersConfig, ScoreMode, GainNormalizationConfig, BandPassConfig};

use crate::db;
use crate::config;
use log::{warn, info, error};
// store rustpotter instance
static RUSTPOTTER: OnceCell<Mutex<Rustpotter>> = OnceCell::new();

pub fn init() -> Result<(), ()> {
    let rustpotter_config = config::RUSTPOTTER_DEFAULT_CONFIG;

    // create rustpotter instance
    match Rustpotter::new(&rustpotter_config) {
        Ok(mut rinstance) => {
            // success
            // wake word files list
            // @TODO. Make it configurable via GUI for custom user voice.
            let rustpotter_dir = crate::config::get_rustpotter_path()?;

            let rustpotter_wake_word_files = [
                rustpotter_dir.join("jarvis-default.rpw"),
                rustpotter_dir.join("jarvis-community-1.rpw"),
                rustpotter_dir.join("jarvis-community-2.rpw"),
                rustpotter_dir.join("jarvis-community-3.rpw"),
                rustpotter_dir.join("jarvis-community-4.rpw"),
            ];

            // load wake word files
            for rpw_path in rustpotter_wake_word_files {
                rinstance.add_wakeword_from_file(&rpw_path).unwrap();
            }

            // load wake word files
            for rpw in rustpotter_wake_word_files {
                rinstance.add_wakeword_from_file(rpw).unwrap();
            }

            // store
            RUSTPOTTER.set(Mutex::new(rinstance));
        },
        Err(msg) => {
            error!("Rustpotter failed to initialize.\nError details: {}", msg);

            return Err(());
        }
    }

    Ok(())
}

pub fn data_callback(frame_buffer: &[i16]) -> Option<i32> {
    let mut lock = RUSTPOTTER.get().unwrap().lock();
    let rustpotter = lock.as_mut().unwrap();
    let detection = rustpotter.process_i16(&frame_buffer);

    if let Some(detection) = detection {
        if detection.score > config::RUSPOTTER_MIN_SCORE {
            info!("Rustpotter detection info:\n{:?}", detection);

            return Some(0)
        } else {
            info!("Rustpotter detection info:\n{:?}", detection)
        }
    }

    None
}