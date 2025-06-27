// gui/src-tauri/src/tauri_commands/mod.rs - Обновленные команды

use tauri::State;
use serde_json::Value;
use std::collections::HashMap;

use crate::error::{JarvisResult, JarvisError};
use crate::{DB, tts, ai_integration, audio_monitor, recorder};

// Команды базы данных
#[tauri::command]
pub async fn db_read(key: String) -> Result<Value, String> {
    let db = DB.lock().map_err(|e| format!("Database error: {}", e))?;

    match db.get::<String>(&key) {
        Some(value) => {
            // Пытаемся парсить как JSON, если не получается - возвращаем как строку
            match serde_json::from_str::<Value>(&value) {
                Ok(json_value) => Ok(json_value),
                Err(_) => Ok(Value::String(value))
            }
        }
        None => Ok(Value::Null)
    }
}

#[tauri::command]
pub async fn db_write(key: String, val: Value) -> Result<bool, String> {
    let mut db = DB.lock().map_err(|e| format!("Database error: {}", e))?;

    let value_str = match val {
        Value::String(s) => s,
        _ => serde_json::to_string(&val).map_err(|e| format!("Serialization error: {}", e))?
    };

    db.set(&key, &value_str).map_err(|e| format!("Database write error: {}", e))?;
    Ok(true)
}

// Команды аудио устройств с hot-plug поддержкой
#[tauri::command]
pub async fn get_audio_input_devices() -> Result<HashMap<i32, String>, String> {
    let monitor = audio_monitor::get_audio_monitor()
        .ok_or("Audio monitor not initialized")?;

    let devices = monitor.get_devices()
        .map_err(|e| format!("Failed to get devices: {}", e))?;

    let mut input_devices = HashMap::new();
    for device in devices {
        if device.is_input {
            input_devices.insert(device.id, device.name);
        }
    }

    Ok(input_devices)
}

#[tauri::command]
pub async fn get_audio_output_devices() -> Result<HashMap<i32, String>, String> {
    let monitor = audio_monitor::get_audio_monitor()
        .ok_or("Audio monitor not initialized")?;

    let devices = monitor.get_devices()
        .map_err(|e| format!("Failed to get devices: {}", e))?;

    let mut output_devices = HashMap::new();
    for device in devices {
        if device.is_output {
            output_devices.insert(device.id, device.name);
        }
    }

    Ok(output_devices)
}

#[tauri::command]
pub async fn refresh_audio_devices() -> Result<Vec<Value>, String> {
    let monitor = audio_monitor::get_audio_monitor()
        .ok_or("Audio monitor not initialized")?;

    let devices = monitor.get_devices()
        .map_err(|e| format!("Failed to refresh devices: {}", e))?;

    let device_list: Vec<Value> = devices.into_iter().map(|device| {
        serde_json::json!({
            "id": device.id,
            "name": device.name,
            "is_input": device.is_input,
            "is_output": device.is_output,
            "is_default": device.is_default,
            "is_available": device.is_available
        })
    }).collect();

    Ok(device_list)
}

// TTS команды
#[tauri::command]
pub async fn get_available_voices() -> Result<Vec<String>, String> {
    tts::get_available_voices()
        .map_err(|e| format!("Failed to get voices: {}", e))
}

#[tauri::command]
pub async fn test_tts(text: String, voice: Option<String>, speed: Option<f32>, volume: Option<f32>) -> Result<bool, String> {
    // Обновляем настройки TTS если переданы
    if let (Some(v), Some(s), Some(vol)) = (voice, speed, volume) {
        let settings = tts::VoiceSettings {
            voice_id: v,
            speed: s,
            volume: vol,
            ..Default::default()
        };

        tts::update_settings(settings)
            .map_err(|e| format!("Failed to update TTS settings: {}", e))?;
    }

    // Воспроизводим тестовую фразу
    tts::speak(&text)
        .map_err(|e| format!("TTS test failed: {}", e))?;

    Ok(true)
}

#[tauri::command]
pub async fn speak_text(text: String) -> Result<bool, String> {
    tts::speak(&text)
        .map_err(|e| format!("TTS failed: {}", e))?;
    Ok(true)
}

#[tauri::command]
pub async fn stop_speaking() -> Result<bool, String> {
    tts::stop()
        .map_err(|e| format!("Failed to stop TTS: {}", e))?;
    Ok(true)
}

#[tauri::command]
pub async fn is_speaking() -> Result<bool, String> {
    Ok(tts::is_speaking())
}

// AI команды
#[tauri::command]
pub async fn test_ai_connection(
    openai_key: Option<String>,
    openrouter_key: Option<String>,
    model: Option<String>
) -> Result<String, String> {
    // Временно обновляем конфигурацию для теста
    let mut config = ai_integration::AiConfig::default();

    if let Some(key) = openai_key {
        config.openai_api_key = key;
    }
    if let Some(key) = openrouter_key {
        config.openrouter_api_key = key;
    }
    if let Some(m) = model {
        config.preferred_model = m;
    }

    // Создаем временный AI менеджер для теста
    let mut manager = ai_integration::AiManager::new()
        .map_err(|e| format!("Failed to create AI manager: {}", e))?;

    manager.update_config(config)
        .map_err(|e| format!("Failed to update AI config: {}", e))?;

    // Тестовый запрос
    let test_request = ai_integration::AiRequestType::Question(
        "Привет! Это тест подключения. Ответь кратко.".to_string()
    );

    let response = manager.process_request(test_request).await
        .map_err(|e| format!("AI test request failed: {}", e))?;

    Ok(response.text)
}

#[tauri::command]
pub async fn process_ai_command(command: String) -> Result<Value, String> {
    let response = ai_integration::process_voice_command(&command).await
        .map_err(|e| format!("AI command processing failed: {}", e))?;

    Ok(serde_json::json!({
        "text": response.text,
        "response_type": format!("{:?}", response.response_type),
        "tokens_used": response.tokens_used,
        "model_used": response.model_used,
        "confidence": response.confidence
    }))
}

// Команды прослушивания (обновленные)
#[tauri::command]
pub async fn start_listening(app_handle: tauri::AppHandle) -> Result<bool, String> {
    crate::listener::start_listening_enhanced(app_handle).await
        .map_err(|e| format!("Failed to start listening: {}", e))
}

#[tauri::command]
pub async fn stop_listening() -> Result<bool, String> {
    crate::listener::stop_listening_enhanced().await
        .map_err(|e| format!("Failed to stop listening: {}", e))
}

#[tauri::command]
pub async fn is_listening() -> Result<bool, String> {
    Ok(crate::listener::is_listening_enhanced())
}

#[tauri::command]
pub async fn get_listening_status() -> Result<Value, String> {
    let status = crate::listener::get_detailed_status()
        .map_err(|e| format!("Failed to get status: {}", e))?;

    Ok(serde_json::json!({
        "is_listening": status.is_listening,
        "current_engine": format!("{:?}", status.wake_word_engine),
        "is_processing_command": status.is_processing_command,
        "last_activation": status.last_activation,
        "error_count": status.error_count
    }))
}

// Системные команды
#[tauri::command]
pub async fn apply_settings() -> Result<bool, String> {
    // Перезагружаем компоненты с новыми настройками

    // Останавливаем прослушивание
    let _ = crate::listener::stop_listening_enhanced().await;

    // Перезагружаем TTS настройки
    if let Err(e) = tts::init() {
        warn!("Failed to reinitialize TTS: {}", e);
    }

    // Перезагружаем AI настройки
    if let Err(e) = ai_integration::init_ai().await {
        warn!("Failed to reinitialize AI: {}", e);
    }

    // Перезапускаем прослушивание
    // Note: Это должно быть сделано из UI после применения настроек

    info!("Settings applied successfully");
    Ok(true)
}

#[tauri::command]
pub async fn get_system_info() -> Result<Value, String> {
    let info = serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "platform": std::env::consts::OS,
        "architecture": std::env::consts::ARCH,
        "tts_initialized": tts::is_initialized(),
        "ai_initialized": ai_integration::is_initialized(),
        "audio_monitor_running": audio_monitor::is_running(),
        "device_count": audio_monitor::get_all_devices().unwrap_or_default().len()
    });

    Ok(info)
}

// Команды управления файлами
#[tauri::command]
pub async fn search_documents(query: String) -> Result<Vec<String>, String> {
    let found_docs = crate::document_search::search_files(&query)
        .map_err(|e| format!("Document search failed: {}", e))?;

    Ok(found_docs)
}

#[tauri::command]
pub async fn open_document(path: String) -> Result<bool, String> {
    crate::document_search::open_file(&path)
        .map_err(|e| format!("Failed to open document: {}", e))?;

    Ok(true)
}

// Команды логов и диагностики
#[tauri::command]
pub async fn get_recent_logs(lines: Option<usize>) -> Result<Vec<String>, String> {
    let log_lines = lines.unwrap_or(100);

    let log_path = crate::config::get_log_file_path()
        .map_err(|e| format!("Failed to get log path: {}", e))?;

    let log_content = std::fs::read_to_string(log_path)
        .map_err(|e| format!("Failed to read log file: {}", e))?;

    let recent_lines: Vec<String> = log_content
        .lines()
        .rev()
        .take(log_lines)
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    Ok(recent_lines)
}

#[tauri::command]
pub async fn run_diagnostics() -> Result<Value, String> {
    let mut diagnostics = serde_json::Map::new();

    // Проверка аудио устройств
    let audio_devices = audio_monitor::get_all_devices().unwrap_or_default();
    diagnostics.insert("audio_devices_count".to_string(), json!(audio_devices.len()));
    diagnostics.insert("audio_input_devices".to_string(),
                       json!(audio_devices.iter().filter(|d| d.is_input).count()));
    diagnostics.insert("audio_output_devices".to_string(),
                       json!(audio_devices.iter().filter(|d| d.is_output).count()));

    // Проверка TTS
    diagnostics.insert("tts_available".to_string(), json!(tts::is_initialized()));

    // Проверка AI
    diagnostics.insert("ai_available".to_string(), json!(ai_integration::is_initialized()));

    // Проверка wake-word движков
    let wake_word_status = crate::listener::test_wake_word_engines()
        .map_err(|e| format!("Failed to test wake word engines: {}", e))?;
    diagnostics.insert("wake_word_engines".to_string(), json!(wake_word_status));

    // Проверка API ключей (без раскрытия)
    if let Ok(Some(db)) = DB.lock().map(|db| Some(db)) {
        diagnostics.insert("picovoice_key_set".to_string(),
                           json!(!db.get::<String>("api_key_picovoice").unwrap_or_default().is_empty()));
        diagnostics.insert("openai_key_set".to_string(),
                           json!(!db.get::<String>("api_key_openai").unwrap_or_default().is_empty()));
        diagnostics.insert("openrouter_key_set".to_string(),
                           json!(!db.get::<String>("api_key_openrouter").unwrap_or_default().is_empty()));
    }

    // Общий статус
    let issues = check_common_issues().await;
    diagnostics.insert("issues".to_string(), json!(issues));

    Ok(Value::Object(diagnostics))
}

// Вспомогательная функция для проверки частых проблем
async fn check_common_issues() -> Vec<String> {
    let mut issues = Vec::new();

    // Проверка микрофонов
    if let Ok(devices) = audio_monitor::get_all_devices() {
        let input_devices: Vec<_> = devices.iter().filter(|d| d.is_input).collect();
        if input_devices.is_empty() {
            issues.push("Не найдено входных аудио устройств".to_string());
        }

        let available_inputs: Vec<_> = input_devices.iter().filter(|d| d.is_available).collect();
        if available_inputs.is_empty() {
            issues.push("Нет доступных микрофонов".to_string());
        }
    }

    // Проверка разрешений
    #[cfg(target_os = "linux")]
    {
        // Проверка прав доступа к аудио на Linux
        if std::process::Command::new("groups")
            .output()
            .map(|output| !String::from_utf8_lossy(&output.stdout).contains("audio"))
            .unwrap_or(true)
        {
            issues.push("Пользователь не в группе 'audio' на Linux".to_string());
        }
    }

    // Проверка системных зависимостей
    #[cfg(target_os = "linux")]
    {
        let dependencies = ["espeak", "pulseaudio", "pactl"];
        for dep in dependencies {
            if std::process::Command::new("which")
                .arg(dep)
                .output()
                .map(|output| !output.status.success())
                .unwrap_or(true)
            {
                issues.push(format!("Отсутствует системная зависимость: {}", dep));
            }
        }
    }

    issues
}

// Команда экспорта настроек
#[tauri::command]
pub async fn export_settings() -> Result<String, String> {
    let db = DB.lock().map_err(|e| format!("Database error: {}", e))?;

    let settings = serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "export_date": chrono::Utc::now().to_rfc3339(),
        "settings": {
            // Экспортируем все настройки кроме API ключей
            "assistant_voice": db.get::<String>("assistant_voice"),
            "selected_microphone": db.get::<String>("selected_microphone"),
            "selected_speaker": db.get::<String>("selected_speaker"),
            "selected_wake_word_engine": db.get::<String>("selected_wake_word_engine"),
            "ai_model": db.get::<String>("ai_model"),
            "ai_temperature": db.get::<String>("ai_temperature"),
            "ai_max_tokens": db.get::<String>("ai_max_tokens"),
            "tts_engine": db.get::<String>("tts_engine"),
            "tts_voice": db.get::<String>("tts_voice"),
            "tts_speed": db.get::<String>("tts_speed"),
            "tts_volume": db.get::<String>("tts_volume"),
            "enable_conversation_mode": db.get::<String>("enable_conversation_mode"),
            "enable_document_search": db.get::<String>("enable_document_search"),
            "auto_open_documents": db.get::<String>("auto_open_documents"),
            "device_monitoring": db.get::<String>("device_monitoring")
        }
    });

    serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))
}

// Команда импорта настроек
#[tauri::command]
pub async fn import_settings(settings_json: String) -> Result<bool, String> {
    let settings: Value = serde_json::from_str(&settings_json)
        .map_err(|e| format!("Invalid settings format: {}", e))?;

    let settings_obj = settings.get("settings")
        .and_then(|s| s.as_object())
        .ok_or("Invalid settings structure")?;

    let mut db = DB.lock().map_err(|e| format!("Database error: {}", e))?;

    // Импортируем настройки
    for (key, value) in settings_obj {
        if let Some(value_str) = value.as_str() {
            db.set(key, value_str)
                .map_err(|e| format!("Failed to import setting {}: {}", key, e))?;
        }
    }

    info!("Settings imported successfully");
    Ok(true)
}