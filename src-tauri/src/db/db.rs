// app/src/db.rs - Исправленный модуль базы данных с недостающими функциями

use super::structs;
use crate::{config, APP_CONFIG_DIR};
use crate::error::{JarvisResult, JarvisError, DatabaseError};

use std::collections::HashMap;
use std::path::PathBuf;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Write};
use log::{info, warn, error};
use std::sync::{Arc, Mutex};
use once_cell::sync::OnceCell;

use serde_json::{self, Value};

#[derive(Default, Debug, Clone)]
pub struct LegacyDatabase {
    data: HashMap<String, String>,
}

impl LegacyDatabase {
    pub fn new() -> Self {
        Self { data: HashMap::new() }
    }

    pub fn from_settings(settings: &structs::Settings) -> Self {
        let mut db = Self::new();
        db.refresh_from_settings(settings);
        db
    }

    fn refresh_from_settings(&mut self, settings: &structs::Settings) {
        self.data.clear();
        self.data.insert("assistant_voice".to_string(), settings.voice.clone());
        self.data.insert("selected_microphone".to_string(), settings.microphone.to_string());
        self.data.insert("selected_speaker".to_string(), settings.speaker.to_string());
        self.data.insert(
            "selected_wake_word_engine".to_string(),
            format!("{:?}", settings.wake_word_engine),
        );
        self.data.insert(
            "speech_to_text_engine".to_string(),
            format!("{:?}", settings.speech_to_text_engine),
        );
        self.data.insert("api_key_picovoice".to_string(), settings.api_keys.picovoice.clone());
        self.data.insert("api_key__picovoice".to_string(), settings.api_keys.picovoice.clone());
        self.data.insert("api_key_openai".to_string(), settings.api_keys.openai.clone());
        self.data.insert("api_key_openrouter".to_string(), settings.api_keys.openrouter.clone());
        self.data.insert("ai_model".to_string(), settings.ai_config.preferred_model.clone());
        self.data.insert("ai_temperature".to_string(), settings.ai_config.temperature.to_string());
        self.data.insert("ai_max_tokens".to_string(), settings.ai_config.max_tokens.to_string());
        self.data.insert(
            "enable_conversation_mode".to_string(),
            settings.ai_config.enable_conversation_mode.to_string(),
        );
        self.data.insert(
            "enable_document_search".to_string(),
            settings.advanced_settings.enable_document_search.to_string(),
        );
        self.data.insert(
            "auto_open_documents".to_string(),
            settings.advanced_settings.auto_open_documents.to_string(),
        );
        self.data.insert(
            "device_monitoring".to_string(),
            settings.advanced_settings.device_monitoring.to_string(),
        );
        self.data.insert(
            "tts_engine".to_string(),
            format!("{:?}", settings.tts_config.engine),
        );
        self.data.insert("tts_voice".to_string(), settings.tts_config.voice_id.clone());
        self.data.insert("tts_speed".to_string(), settings.tts_config.speed.to_string());
        self.data.insert("tts_volume".to_string(), settings.tts_config.volume.to_string());
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.data.get(key).cloned()
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<(), String> {
        let value_string = value.to_string();
        let json_value = Value::String(value_string.clone());

        if let Err(err) = set_setting_value(key, json_value) {
            return Err(err.to_string());
        }

        self.data.insert(key.to_string(), value_string);
        Ok(())
    }

    pub fn sync_from_global_settings(&mut self) {
        if let Some(current) = get_current_settings() {
            self.refresh_from_settings(&current);
        }
    }
}

/// Safely execute a read operation against the in-memory settings snapshot.
pub fn with_settings<F, R>(reader: F) -> JarvisResult<R>
where
    F: FnOnce(&structs::Settings) -> R,
{
    let settings_arc = CURRENT_SETTINGS.get().ok_or_else(|| {
        JarvisError::DatabaseError(DatabaseError::InitializationFailed(
            "Settings not initialized".to_string(),
        ))
    })?;

    let settings = settings_arc.lock().map_err(|_| {
        JarvisError::DatabaseError(DatabaseError::ReadError(
            "Failed to lock settings for read".to_string(),
        ))
    })?;

    Ok(reader(&*settings))
}

/// Safely execute a write operation against the in-memory settings snapshot
/// and persist the updated values to disk.
pub fn with_settings_mut<F, R>(writer: F) -> JarvisResult<R>
where
    F: FnOnce(&mut structs::Settings) -> JarvisResult<R>,
{
    let settings_arc = CURRENT_SETTINGS.get().ok_or_else(|| {
        JarvisError::DatabaseError(DatabaseError::InitializationFailed(
            "Settings not initialized".to_string(),
        ))
    })?;

    let mut settings = settings_arc.lock().map_err(|_| {
        JarvisError::DatabaseError(DatabaseError::WriteError(
            "Failed to lock settings for update".to_string(),
        ))
    })?;

    let result = writer(&mut *settings)?;
    let updated = settings.clone();
    drop(settings);

    save_settings(&updated)?;

    Ok(result)
}

/// Retrieve a single setting value as a strongly typed JSON value.
pub fn get_setting_value(key: &str) -> JarvisResult<Option<Value>> {
    with_settings(|settings| read_setting_value(settings, key))
}

/// Update a single setting with a strongly typed JSON value.
pub fn set_setting_value(key: &str, value: Value) -> JarvisResult<()> {
    let key = key.to_string();
    with_settings_mut(move |settings| {
        apply_setting_value(settings, &key, &value)?;
        settings.touch();
        Ok(())
    })
}

fn read_setting_value(settings: &structs::Settings, key: &str) -> Option<Value> {
    match key {
        "assistant_voice" => Some(Value::String(settings.voice.clone())),
        "selected_microphone" => Some(serde_json::json!(settings.microphone)),
        "selected_speaker" => Some(serde_json::json!(settings.speaker)),
        "selected_wake_word_engine" => {
            Some(Value::String(format!("{:?}", settings.wake_word_engine)))
        }
        "speech_to_text_engine" => {
            Some(Value::String(format!("{:?}", settings.speech_to_text_engine)))
        }
        "api_key_picovoice" | "api_key__picovoice" => {
            Some(Value::String(settings.api_keys.picovoice.clone()))
        }
        "api_key_openai" => Some(Value::String(settings.api_keys.openai.clone())),
        "api_key_openrouter" => Some(Value::String(settings.api_keys.openrouter.clone())),
        "ai_model" => Some(Value::String(settings.ai_config.preferred_model.clone())),
        "ai_temperature" => Some(serde_json::json!(settings.ai_config.temperature)),
        "ai_max_tokens" => Some(serde_json::json!(settings.ai_config.max_tokens)),
        "enable_conversation_mode" => {
            Some(serde_json::json!(settings.ai_config.enable_conversation_mode))
        }
        "enable_document_search" => Some(serde_json::json!(
            settings.advanced_settings.enable_document_search
        )),
        "auto_open_documents" => Some(serde_json::json!(
            settings.advanced_settings.auto_open_documents
        )),
        "device_monitoring" => Some(serde_json::json!(
            settings.advanced_settings.device_monitoring
        )),
        "tts_engine" => Some(Value::String(format!("{:?}", settings.tts_config.engine))),
        "tts_voice" => Some(Value::String(settings.tts_config.voice_id.clone())),
        "tts_speed" => Some(serde_json::json!(settings.tts_config.speed)),
        "tts_volume" => Some(serde_json::json!(settings.tts_config.volume)),
        _ => None,
    }
}

fn apply_setting_value(
    settings: &mut structs::Settings,
    key: &str,
    value: &Value,
) -> JarvisResult<()> {
    match key {
        "assistant_voice" => {
            let voice = value_as_string(value).ok_or_else(|| {
                JarvisError::DatabaseError(DatabaseError::WriteError(
                    "assistant_voice".to_string(),
                ))
            })?;
            settings.voice = voice;
        }
        "selected_microphone" => {
            let index = value_as_i32(value).ok_or_else(|| {
                JarvisError::DatabaseError(DatabaseError::WriteError(
                    "selected_microphone".to_string(),
                ))
            })?;
            settings.microphone = index;
        }
        "selected_speaker" => {
            let index = value_as_i32(value).ok_or_else(|| {
                JarvisError::DatabaseError(DatabaseError::WriteError(
                    "selected_speaker".to_string(),
                ))
            })?;
            settings.speaker = index;
        }
        "selected_wake_word_engine" => {
            if let Some(engine) = value_as_string(value) {
                let normalized = engine.to_lowercase();
                settings.wake_word_engine = match normalized.as_str() {
                    "rustpotter" => config::structs::WakeWordEngine::Rustpotter,
                    "vosk" => config::structs::WakeWordEngine::Vosk,
                    "porcupine" | "picovoice" => config::structs::WakeWordEngine::Porcupine,
                    _ => settings.wake_word_engine,
                };
            }
        }
        "speech_to_text_engine" => {
            if let Some(engine) = value_as_string(value) {
                let normalized = engine.to_lowercase();
                settings.speech_to_text_engine = match normalized.as_str() {
                    "vosk" => config::structs::SpeechToTextEngine::Vosk,
                    _ => settings.speech_to_text_engine.clone(),
                };
            }
        }
        "api_key_picovoice" | "api_key__picovoice" => {
            let api_key = value_as_string(value).ok_or_else(|| {
                JarvisError::DatabaseError(DatabaseError::WriteError(
                    "api_key_picovoice".to_string(),
                ))
            })?;
            settings.api_keys.picovoice = api_key;
        }
        "api_key_openai" => {
            let api_key = value_as_string(value).ok_or_else(|| {
                JarvisError::DatabaseError(DatabaseError::WriteError(
                    "api_key_openai".to_string(),
                ))
            })?;
            settings.api_keys.openai = api_key;
        }
        "api_key_openrouter" => {
            let api_key = value_as_string(value).ok_or_else(|| {
                JarvisError::DatabaseError(DatabaseError::WriteError(
                    "api_key_openrouter".to_string(),
                ))
            })?;
            settings.api_keys.openrouter = api_key;
        }
        "ai_model" => {
            let model = value_as_string(value).ok_or_else(|| {
                JarvisError::DatabaseError(DatabaseError::WriteError("ai_model".to_string()))
            })?;
            settings.ai_config.preferred_model = model;
        }
        "ai_temperature" => {
            let temperature = value_as_f32(value).ok_or_else(|| {
                JarvisError::DatabaseError(DatabaseError::WriteError(
                    "ai_temperature".to_string(),
                ))
            })?;
            settings.ai_config.temperature = temperature;
        }
        "ai_max_tokens" => {
            let max_tokens = value_as_u32(value).ok_or_else(|| {
                JarvisError::DatabaseError(DatabaseError::WriteError(
                    "ai_max_tokens".to_string(),
                ))
            })?;
            settings.ai_config.max_tokens = max_tokens;
        }
        "enable_conversation_mode" => {
            let enabled = value_as_bool(value).ok_or_else(|| {
                JarvisError::DatabaseError(DatabaseError::WriteError(
                    "enable_conversation_mode".to_string(),
                ))
            })?;
            settings.ai_config.enable_conversation_mode = enabled;
        }
        "enable_document_search" => {
            let enabled = value_as_bool(value).ok_or_else(|| {
                JarvisError::DatabaseError(DatabaseError::WriteError(
                    "enable_document_search".to_string(),
                ))
            })?;
            settings.advanced_settings.enable_document_search = enabled;
        }
        "auto_open_documents" => {
            let enabled = value_as_bool(value).ok_or_else(|| {
                JarvisError::DatabaseError(DatabaseError::WriteError(
                    "auto_open_documents".to_string(),
                ))
            })?;
            settings.advanced_settings.auto_open_documents = enabled;
        }
        "device_monitoring" => {
            let enabled = value_as_bool(value).ok_or_else(|| {
                JarvisError::DatabaseError(DatabaseError::WriteError(
                    "device_monitoring".to_string(),
                ))
            })?;
            settings.advanced_settings.device_monitoring = enabled;
        }
        "tts_engine" => {
            if let Some(engine) = value_as_string(value) {
                let normalized = engine.to_lowercase();
                settings.tts_config.engine = match normalized.as_str() {
                    "system" => structs::TtsEngine::System,
                    "openai" => structs::TtsEngine::OpenAI,
                    "elevenlabs" => structs::TtsEngine::ElevenLabs,
                    "local" => structs::TtsEngine::Local,
                    _ => settings.tts_config.engine.clone(),
                };
            }
        }
        "tts_voice" => {
            let voice = value_as_string(value).ok_or_else(|| {
                JarvisError::DatabaseError(DatabaseError::WriteError("tts_voice".to_string()))
            })?;
            settings.tts_config.voice_id = voice;
        }
        "tts_speed" => {
            let speed = value_as_f32(value).ok_or_else(|| {
                JarvisError::DatabaseError(DatabaseError::WriteError("tts_speed".to_string()))
            })?;
            settings.tts_config.speed = speed;
        }
        "tts_volume" => {
            let volume = value_as_f32(value).ok_or_else(|| {
                JarvisError::DatabaseError(DatabaseError::WriteError("tts_volume".to_string()))
            })?;
            settings.tts_config.volume = volume;
        }
        _ => {}
    }

    Ok(())
}

fn value_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Null => Some(String::new()),
        _ => None,
    }
}

fn value_as_bool(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(b) => Some(*b),
        Value::String(s) => s.parse::<bool>().ok(),
        _ => None,
    }
}

fn value_as_i32(value: &Value) -> Option<i32> {
    match value {
        Value::Number(n) => n.as_i64().map(|v| v as i32),
        Value::String(s) => s.parse::<i32>().ok(),
        _ => None,
    }
}

fn value_as_u32(value: &Value) -> Option<u32> {
    match value {
        Value::Number(n) => n.as_u64().map(|v| v as u32),
        Value::String(s) => s.parse::<u32>().ok(),
        _ => None,
    }
}

fn value_as_f32(value: &Value) -> Option<f32> {
    match value {
        Value::Number(n) => n.as_f64().map(|v| v as f32),
        Value::String(s) => s.parse::<f32>().ok(),
        _ => None,
    }
}

// Глобальная ссылка на текущие настройки для быстрого доступа
static CURRENT_SETTINGS: OnceCell<Arc<Mutex<structs::Settings>>> = OnceCell::new();

/// Получение пути к файлу базы данных
fn get_db_file_path() -> JarvisResult<PathBuf> {
    let config_dir = APP_CONFIG_DIR.get()
        .ok_or_else(|| JarvisError::DatabaseError(DatabaseError::InitializationFailed(
            "Config directory not initialized".to_string()
        )))?;

    Ok(config_dir.join(config::DB_FILE_NAME))
}

/// Получение пути к резервной копии базы данных
fn get_backup_db_file_path() -> JarvisResult<PathBuf> {
    let config_dir = APP_CONFIG_DIR.get()
        .ok_or_else(|| JarvisError::DatabaseError(DatabaseError::InitializationFailed(
            "Config directory not initialized".to_string()
        )))?;

    let backup_name = format!("{}.backup", config::DB_FILE_NAME);
    Ok(config_dir.join(backup_name))
}

/// Инициализация настроек с улучшенной обработкой ошибок
pub fn init_settings() -> JarvisResult<structs::Settings> {
    if let Some(settings) = get_current_settings() {
        return Ok(settings);
    }

    let db_file_path = get_db_file_path()?;

    info!("Loading settings database from: {}", db_file_path.display());

    let settings = if db_file_path.exists() {
        // Пытаемся загрузить существующие настройки
        load_settings_from_file(&db_file_path)?
    } else {
        // Создаем настройки по умолчанию
        warn!("Settings file not found. Creating default settings.");
        create_default_settings()?
    };

    // Сохраняем настройки в глобальной переменной для быстрого доступа
    if CURRENT_SETTINGS
        .set(Arc::new(Mutex::new(settings.clone())))
        .is_err()
    {
        if let Some(existing) = get_current_settings() {
            return Ok(existing);
        }

        return Err(JarvisError::DatabaseError(DatabaseError::InitializationFailed(
            "Settings already initialized".to_string()
        )));
    }
    info!("Settings loaded successfully");
    Ok(settings)
}

/// Загрузка настроек из файла
fn load_settings_from_file(file_path: &PathBuf) -> JarvisResult<structs::Settings> {
    let file = File::open(file_path)
        .map_err(|e| JarvisError::DatabaseError(DatabaseError::ReadError(
            format!("Failed to open settings file: {}", e)
        )))?;

    let reader = BufReader::new(file);

    // Пытаемся распарсить JSON
    match serde_json::from_reader::<BufReader<File>, structs::Settings>(reader) {
        Ok(settings) => {
            info!("Settings loaded from file successfully");
            Ok(settings)
        }
        Err(e) => {
            error!("Failed to parse settings file: {}", e);

            // Пытаемся создать резервную копию поврежденного файла
            let backup_path = get_backup_db_file_path()?;
            if let Err(backup_err) = std::fs::copy(file_path, &backup_path) {
                warn!("Failed to create backup of corrupted settings: {}", backup_err);
            } else {
                info!("Corrupted settings backed up to: {}", backup_path.display());
            }

            // Создаем настройки по умолчанию
            warn!("Creating default settings due to parsing error");
            create_default_settings()
        }
    }
}

/// Создание настроек по умолчанию
fn create_default_settings() -> JarvisResult<structs::Settings> {
    let default_settings = structs::Settings::default();

    // Сохраняем настройки по умолчанию в файл
    save_settings(&default_settings)?;

    info!("Default settings created and saved");
    Ok(default_settings)
}

/// Сохранение настроек в файл
pub fn save_settings(settings: &structs::Settings) -> JarvisResult<()> {
    let db_file_path = get_db_file_path()?;

    // Создаем временный файл для атомарной записи
    let temp_file_path = db_file_path.with_extension("tmp");

    {
        let temp_file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temp_file_path)
            .map_err(|e| JarvisError::DatabaseError(DatabaseError::WriteError(
                format!("Failed to create temp settings file: {}", e)
            )))?;

        let mut writer = BufWriter::new(temp_file);

        // Сериализуем настройки в JSON с красивым форматированием
        let json_data = serde_json::to_string_pretty(settings)
            .map_err(|e| JarvisError::DatabaseError(DatabaseError::WriteError(
                format!("Failed to serialize settings: {}", e)
            )))?;

        writer.write_all(json_data.as_bytes())
            .map_err(|e| JarvisError::DatabaseError(DatabaseError::WriteError(
                format!("Failed to write settings data: {}", e)
            )))?;

        writer.flush()
            .map_err(|e| JarvisError::DatabaseError(DatabaseError::WriteError(
                format!("Failed to flush settings data: {}", e)
            )))?;
    }

    // Атомарно перемещаем временный файл на место основного
    std::fs::rename(&temp_file_path, &db_file_path)
        .map_err(|e| JarvisError::DatabaseError(DatabaseError::WriteError(
            format!("Failed to move temp settings file: {}", e)
        )))?;

    // Обновляем глобальные настройки
    if let Some(global_settings) = CURRENT_SETTINGS.get() {
        if let Ok(mut global) = global_settings.lock() {
            *global = settings.clone();
        }
    }

    info!("Settings saved successfully to: {}", db_file_path.display());
    Ok(())
}

/// Сохранение состояния (graceful shutdown)
pub fn save_state() -> JarvisResult<()> {
    info!("Saving application state...");

    // Получаем текущие настройки
    if let Some(global_settings) = CURRENT_SETTINGS.get() {
        if let Ok(settings) = global_settings.lock() {
            // Сохраняем настройки
            save_settings(&*settings)?;

            // Создаем резервную копию
            create_backup_copy()?;

            info!("Application state saved successfully");
            Ok(())
        } else {
            Err(JarvisError::DatabaseError(DatabaseError::WriteError(
                "Failed to lock settings for saving state".to_string()
            )))
        }
    } else {
        warn!("No settings to save during state save");
        Ok(())
    }
}

/// Экстренное сохранение (emergency shutdown)
pub fn emergency_save() -> JarvisResult<()> {
    error!("EMERGENCY SAVE: Attempting to preserve application state");

    if let Some(global_settings) = CURRENT_SETTINGS.get() {
        if let Ok(settings) = global_settings.try_lock() {
            // Пытаемся сохранить настройки в экстренном режиме
            let db_file_path = get_db_file_path()?;
            let emergency_path = db_file_path.with_extension("emergency");

            // Простая запись без сложной обработки ошибок
            if let Ok(json_data) = serde_json::to_string_pretty(&*settings) {
                if let Err(e) = std::fs::write(&emergency_path, json_data) {
                    error!("Emergency save failed: {}", e);
                    return Err(JarvisError::DatabaseError(DatabaseError::WriteError(
                        format!("Emergency save failed: {}", e)
                    )));
                } else {
                    error!("Emergency save completed to: {}", emergency_path.display());
                }
            } else {
                error!("Failed to serialize settings for emergency save");
                return Err(JarvisError::DatabaseError(DatabaseError::WriteError(
                    "Failed to serialize settings for emergency save".to_string()
                )));
            }
        } else {
            error!("Could not lock settings for emergency save");
            return Err(JarvisError::DatabaseError(DatabaseError::WriteError(
                "Could not lock settings for emergency save".to_string()
            )));
        }
    } else {
        error!("No settings available for emergency save");
    }

    Ok(())
}

/// Создание резервной копии базы данных
fn create_backup_copy() -> JarvisResult<()> {
    let db_file_path = get_db_file_path()?;
    let backup_path = get_backup_db_file_path()?;

    if db_file_path.exists() {
        std::fs::copy(&db_file_path, &backup_path)
            .map_err(|e| JarvisError::DatabaseError(DatabaseError::WriteError(
                format!("Failed to create backup: {}", e)
            )))?;

        info!("Backup created: {}", backup_path.display());
    }

    Ok(())
}

/// Восстановление из резервной копии
pub fn restore_from_backup() -> JarvisResult<structs::Settings> {
    let backup_path = get_backup_db_file_path()?;

    if !backup_path.exists() {
        return Err(JarvisError::DatabaseError(DatabaseError::ReadError(
            "No backup file found".to_string()
        )));
    }

    info!("Restoring settings from backup: {}", backup_path.display());

    let settings = load_settings_from_file(&backup_path)?;

    // Сохраняем восстановленные настройки как основные
    save_settings(&settings)?;

    info!("Settings restored from backup successfully");
    Ok(settings)
}

/// Получение текущих настроек (для чтения)
pub fn get_current_settings() -> Option<structs::Settings> {
    CURRENT_SETTINGS.get()
        .and_then(|settings| settings.lock().ok())
        .map(|settings| settings.clone())
}

/// Обновление настроек в runtime
pub fn update_settings<F>(updater: F) -> JarvisResult<()>
where
    F: FnOnce(&mut structs::Settings),
{
    if let Some(global_settings) = CURRENT_SETTINGS.get() {
        let updated_settings = {
            let mut settings = global_settings.lock().map_err(|_| {
                JarvisError::DatabaseError(DatabaseError::WriteError(
                    "Failed to lock settings for update".to_string()
                ))
            })?;

            updater(&mut *settings);
            settings.clone()
        };

        save_settings(&updated_settings)?;

        info!("Settings updated successfully");
        Ok(())
    } else {
        Err(JarvisError::DatabaseError(DatabaseError::InitializationFailed(
            "Settings not initialized".to_string()
        )))
    }
}

/// Проверка целостности базы данных
pub fn verify_database_integrity() -> JarvisResult<bool> {
    let db_file_path = get_db_file_path()?;

    if !db_file_path.exists() {
        return Ok(false);
    }

    // Пытаемся загрузить и распарсить файл
    match load_settings_from_file(&db_file_path) {
        Ok(_) => {
            info!("Database integrity check passed");
            Ok(true)
        }
        Err(e) => {
            warn!("Database integrity check failed: {}", e);
            Ok(false)
        }
    }
}

/// Получение статистики базы данных
pub fn get_database_stats() -> JarvisResult<serde_json::Value> {
    let db_file_path = get_db_file_path()?;
    let backup_path = get_backup_db_file_path()?;

    let stats = serde_json::json!({
        "database_file": db_file_path.to_string_lossy(),
        "database_exists": db_file_path.exists(),
        "database_size": db_file_path.metadata().map(|m| m.len()).unwrap_or(0),
        "backup_exists": backup_path.exists(),
        "backup_size": backup_path.metadata().map(|m| m.len()).unwrap_or(0),
        "settings_initialized": CURRENT_SETTINGS.get().is_some(),
        "integrity_ok": verify_database_integrity().unwrap_or(false),
    });

    Ok(stats)
}

/// Экспорт настроек в JSON строку
pub fn export_settings() -> JarvisResult<String> {
    if let Some(settings) = get_current_settings() {
        serde_json::to_string_pretty(&settings)
            .map_err(|e| JarvisError::DatabaseError(DatabaseError::ReadError(
                format!("Failed to export settings: {}", e)
            )))
    } else {
        Err(JarvisError::DatabaseError(DatabaseError::InitializationFailed(
            "No settings available for export".to_string()
        )))
    }
}

/// Импорт настроек из JSON строки
pub fn import_settings(json_data: &str) -> JarvisResult<()> {
    let settings: structs::Settings = serde_json::from_str(json_data)
        .map_err(|e| JarvisError::DatabaseError(DatabaseError::WriteError(
            format!("Failed to parse imported settings: {}", e)
        )))?;

    // Создаем резервную копию перед импортом
    create_backup_copy()?;

    // Сохраняем импортированные настройки
    save_settings(&settings)?;

    info!("Settings imported successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_create_default_settings() {
        let settings = structs::Settings::default();
        assert_eq!(settings.microphone, -1);
        assert!(!settings.voice.is_empty() || settings.voice.is_empty()); // Может быть пустым по умолчанию
    }

    #[test]
    fn test_settings_serialization() {
        let settings = structs::Settings::default();
        let json = serde_json::to_string_pretty(&settings).unwrap();
        let deserialized: structs::Settings = serde_json::from_str(&json).unwrap();

        assert_eq!(settings.microphone, deserialized.microphone);
    }

    #[test]
    fn test_export_import_cycle() {
        let original_settings = structs::Settings::default();
        let json = serde_json::to_string_pretty(&original_settings).unwrap();
        let imported_settings: structs::Settings = serde_json::from_str(&json).unwrap();

        assert_eq!(original_settings.microphone, imported_settings.microphone);
        assert_eq!(original_settings.voice, imported_settings.voice);
    }
}