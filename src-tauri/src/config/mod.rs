// src-tauri/src/config/mod.rs

pub mod structs;
pub mod config;

// Re-export main config functions from parent module
pub use crate::config::*;
pub use crate::structs::*;

// Re-export structures
pub use structs::*;
pub use config::*;