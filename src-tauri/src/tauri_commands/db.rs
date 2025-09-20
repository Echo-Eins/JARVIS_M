use crate::DB;

#[tauri::command]
pub fn db_read(key: &str) -> String {
    if let Some(db_arc) = DB.get() {
        if let Ok(db) = db_arc.lock() {
            if let Some(value) = db.get(key) {
                return value;
            }
        }
    }

    String::from("")
}

#[tauri::command]
pub fn db_write(key: &str, val: &str) -> bool {
    if let Some(db_arc) = DB.get() {
        if let Ok(mut db) = db_arc.lock() {
            return db.set(key, val).is_ok();
        }
    }
    false
}
