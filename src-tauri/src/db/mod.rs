// src-tauri/src/db/mod.rs

pub mod structs;
pub mod db;
// Re-export structures
pub use structs::*;
pub use db::*;
// Re-export main database functions from parent module
pub use crate::db::*;