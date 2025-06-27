// app/src/config.rs - Рефакторинг с улучшенной обработкой ошибок

pub mod structs;
use structs::{WakeWordEngine, SpeechToTextEngine, RecorderType, AudioType};
use tauri::api::path::resource_dir;
use std::path::PathBuf;
use std::fs;
use std::env;
use std::path::PathBuf;
use once_cell::sync::Lazy;

use platform_dirs::AppDirs;
use rustpotter::{RustpotterConfig, WavFmt, DetectorConfig, FiltersConfig, ScoreMode, GainNormalizationConfig, BandPassConfig};

use crate::error::{JarvisResult, JarvisError, ConfigError};
use crate::{APP_DIRS, APP_CONFIG_DIR, APP_LOG_DIR};

#[allow(dead_code)]

/// Инициализация директорий с улучшенной обработкой ошибок
pub fn init_dirs() -> JarvisResult<()> {
    // Проверяем, не инициализированы ли уже директории
    if APP_DIRS.get().is_some() {
        return Ok(());
    }

    // Создаем AppDirs с обработкой ошибки
    let app_dirs = AppDirs::new(Some(BUNDLE_IDENTIFIER), false)
        .ok_or_else(|| {
            JarvisError::ConfigError(ConfigError::InvalidConfiguration(
                "Failed to determine application directories".to_string()
            ))
        })?;

    // Устанавливаем глобальную переменную
    APP_DIRS.set(app_dirs).map_err(|_| {
        JarvisError::ConfigError(ConfigError::InvalidConfiguration(
            "APP_DIRS already initialized".to_string()
        ))
    })?;

    // Получаем пути к директориям
    let config_dir = PathBuf::from(&APP_DIRS.get().unwrap().config_dir);
    let log_dir = PathBuf::from(&APP_DIRS.get().unwrap().config_dir);

    // Создаем config директорию с правильной обработкой ошибок
    let final_config_dir = ensure_directory_exists(config_dir, "config")?;

    // Создаем log директорию с правильной обработкой ошибок  
    let final_log_dir = ensure_directory_exists(log_dir, "log")?;

    // Сохраняем пути в глобальных переменных
    APP_CONFIG_DIR.set(final_config_dir).map_err(|_| {
        JarvisError::ConfigError(ConfigError::InvalidConfiguration(
            "APP_CONFIG_DIR already initialized".to_string()
        ))
    })?;

    APP_LOG_DIR.set(final_log_dir).map_err(|_| {
        JarvisError::ConfigError(ConfigError::InvalidConfiguration(
            "APP_LOG_DIR already initialized".to_string()
        ))
    })?;

    info!("Directories initialized successfully");
    info!("Config directory: {}", APP_CONFIG_DIR.get().unwrap().display());
    info!("Log directory: {}", APP_LOG_DIR.get().unwrap().display());

    Ok(())
}

/// Обеспечивает существование директории, создавая её при необходимости
fn ensure_directory_exists(mut dir: PathBuf, dir_type: &str) -> JarvisResult<PathBuf> {
    if !dir.exists() {
        // Пытаемся создать директорию
        if let Err(e) = fs::create_dir_all(&dir) {
            warn!("Failed to create {} directory at {}: {}", dir_type, dir.display(), e);

            // Fallback к текущей директории
            dir = env::current_dir().map_err(|e| {
                JarvisError::ConfigError(ConfigError::DirectoryCreationFailed(
                    format!("Cannot determine current directory: {}", e)
                ))
            })?;

            // Пытаемся создать в текущей директории
            fs::create_dir_all(&dir).map_err(|e| {
                JarvisError::ConfigError(ConfigError::DirectoryCreationFailed(
                    format!("Cannot create {} directory in current path {}: {}",
                            dir_type, dir.display(), e)
                ))
            })?;

            warn!("Using fallback {} directory: {}", dir_type, dir.display());
        }
    }

    // Проверяем права доступа
    if !dir.is_dir() {
        return Err(JarvisError::ConfigError(ConfigError::DirectoryCreationFailed(
            format!("Path {} exists but is not a directory", dir.display())
        )));
    }

    // Проверяем возможность записи (создаем временный файл)
    let test_file = dir.join(".write_test");
    if let Err(e) = fs::write(&test_file, "test") {
        return Err(JarvisError::ConfigError(ConfigError::DirectoryCreationFailed(
            format!("No write permission for {} directory {}: {}",
                    dir_type, dir.display(), e)
        )));
    }

    // Удаляем тестовый файл
    let _ = fs::remove_file(&test_file);

    Ok(dir)
}

/// Проверка корректности конфигурации
pub fn validate_configuration() -> JarvisResult<()> {
    // Проверяем, что все необходимые директории инициализированы
    if APP_CONFIG_DIR.get().is_none() {
        return Err(JarvisError::ConfigError(ConfigError::MissingRequiredSetting(
            "CONFIG_DIR not initialized".to_string()
        )));
    }

    if APP_LOG_DIR.get().is_none() {
        return Err(JarvisError::ConfigError(ConfigError::MissingRequiredSetting(
            "LOG_DIR not initialized".to_string()
        )));
    }

    // Проверяем существование критически важных файлов/директорий
    let config_dir = APP_CONFIG_DIR.get().unwrap();
    if !config_dir.exists() {
        return Err(JarvisError::ConfigError(ConfigError::DirectoryCreationFailed(
            format!("Config directory does not exist: {}", config_dir.display())
        )));
    }

    // Проверяем наличие моделей для разных движков
    validate_engine_requirements()?;

    info!("Configuration validation passed");
    Ok(())
}

/// Проверка требований для движков распознавания
fn validate_engine_requirements() -> JarvisResult<()> {
    // Проверяем наличие файлов для Vosk
    let vosk_model_path = PathBuf::from(VOSK_MODEL_PATH);
    if !vosk_model_path.exists() {
        warn!("Vosk model not found at: {}", vosk_model_path.display());
        warn!("STT functionality may not work properly");
    }

    // Проверяем наличие файлов для Rustpotter
    let rustpotter_files = [
        "rustpotter/jarvis-default.rpw",
        "rustpotter/jarvis-community-1.rpw",
        "rustpotter/jarvis-community-2.rpw",
        "rustpotter/jarvis-community-3.rpw",
        "rustpotter/jarvis-community-4.rpw",
    ];

    let mut missing_rustpotter = 0;
    for file in &rustpotter_files {
        if !PathBuf::from(file).exists() {
            missing_rustpotter += 1;
        }
    }

    if missing_rustpotter == rustpotter_files.len() {
        warn!("No Rustpotter wake-word files found");
        warn!("Rustpotter wake-word detection may not work");
    }

    // Проверяем наличие файлов для Picovoice
    let picovoice_keyword_path = PathBuf::from(KEYWORDS_PATH).join(DEFAULT_KEYWORD);
    if !picovoice_keyword_path.exists() {
        warn!("Picovoice keyword file not found at: {}", picovoice_keyword_path.display());
        warn!("Picovoice wake-word detection may not work");
    }

    Ok(())
}

/// Получение пути к конфигурационному файлу с проверкой
pub fn get_config_file_path() -> JarvisResult<PathBuf> {
    let config_dir = APP_CONFIG_DIR.get()
        .ok_or_else(|| {
            JarvisError::ConfigError(ConfigError::MissingRequiredSetting(
                "CONFIG_DIR not initialized".to_string()
            ))
        })?;

    Ok(config_dir.join(DB_FILE_NAME))
}

/// Получение пути к лог файлу с проверкой
pub fn get_log_file_path() -> JarvisResult<PathBuf> {
    let log_dir = APP_LOG_DIR.get()
        .ok_or_else(|| {
            JarvisError::ConfigError(ConfigError::MissingRequiredSetting(
                "LOG_DIR not initialized".to_string()
            ))
        })?;

    Ok(log_dir.join(LOG_FILE_NAME))
}

/*
    Defaults - без изменений
 */
pub const DEFAULT_AUDIO_TYPE: AudioType = AudioType::Kira;
pub const DEFAULT_RECORDER_TYPE: RecorderType = RecorderType::PvRecorder;
pub const DEFAULT_WAKE_WORD_ENGINE: WakeWordEngine = WakeWordEngine::Rustpotter;
pub const DEFAULT_SPEECH_TO_TEXT_ENGINE: SpeechToTextEngine = SpeechToTextEngine::Vosk;

pub const DEFAULT_VOICE: &str = "jarvis-og";

pub const BUNDLE_IDENTIFIER: &str = "com.priler.jarvis";
pub const DB_FILE_NAME: &str = "app.db";
pub const LOG_FILE_NAME: &str = "log.txt";
pub const APP_VERSION: Option<&str> = option_env!("CARGO_PKG_VERSION");
pub const AUTHOR_NAME: Option<&str> = option_env!("CARGO_PKG_AUTHORS");
pub const REPOSITORY_LINK: Option<&str> = option_env!("CARGO_PKG_REPOSITORY");
pub const TG_OFFICIAL_LINK: Option<&str> = Some("https://t.me/howdyho_official");
pub const FEEDBACK_LINK: Option<&str> = Some("https://t.me/jarvis_feedback_bot");

/*
    Tray - без изменений пока
 */
pub const TRAY_ICON: &str = "32x32.png";
pub const TRAY_TOOLTIP: &str = "Jarvis Voice Assistant";

// RUSTPOTTER - улучшенная конфигурация
pub const RUSPOTTER_MIN_SCORE: f32 = 0.62;
pub const RUSTPOTTER_DEFAULT_CONFIG: Lazy<RustpotterConfig> = Lazy::new(|| {
    RustpotterConfig {
        fmt: WavFmt::default(),
        detector: DetectorConfig {
            avg_threshold: 0.,
            threshold: 0.5,
            min_scores: 15,
            score_mode: ScoreMode::Average,
            comparator_band_size: 5,
            comparator_ref: 0.22
        },
        filters: FiltersConfig {
            gain_normalizer: GainNormalizationConfig {
                enabled: true,
                gain_ref: None,
                min_gain: 0.7,
                max_gain: 1.0,
            },
            band_pass: BandPassConfig {
                enabled: true,
                low_cutoff: 80.,
                high_cutoff: 400.,
            }
        }
    }
});

// PICOVOICE


pub const DEFAULT_KEYWORD: &str = "jarvis_windows.ppn";
pub const DEFAULT_SENSITIVITY: f32 = 1.0;

// НОВЫЕ функции (ДОБАВИТЬ):
use tauri::api::path::resource_dir;
use tauri::Env;

/// Получение пути к директории команд
pub fn get_commands_path() -> JarvisResult<PathBuf> {
    let resource_dir = resource_dir(&tauri::generate_context!().config())
        .ok_or_else(|| JarvisError::ConfigError(ConfigError::FileNotFound(
            "Resource directory not found".to_string()
        )))?;
    Ok(resource_dir.join("commands"))
}

/// Получение пути к директории ключевых слов Picovoice
pub fn get_keywords_path() -> JarvisResult<PathBuf> {
    let resource_dir = resource_dir(&tauri::generate_context!().config())
        .ok_or_else(|| JarvisError::ConfigError(ConfigError::FileNotFound(
            "Resource directory not found".to_string()
        )))?;
    Ok(resource_dir.join("picovoice").join("keywords"))
}

/// Получение пути к модели Vosk
pub fn get_vosk_model_path() -> JarvisResult<PathBuf> {
    let resource_dir = resource_dir(&tauri::generate_context!().config())
        .ok_or_else(|| JarvisError::ConfigError(ConfigError::FileNotFound(
            "Resource directory not found".to_string()
        )))?;
    Ok(resource_dir.join("vosk").join("model_small"))
}

/// Получение пути к директории звуков
pub fn get_sound_directory() -> JarvisResult<PathBuf> {
    let resource_dir = resource_dir(&tauri::generate_context!().config())
        .ok_or_else(|| JarvisError::ConfigError(ConfigError::FileNotFound(
            "Resource directory not found".to_string()
        )))?;
    Ok(resource_dir.join("sound"))
}

/// Получение пути к директории Rustpotter
pub fn get_rustpotter_path() -> JarvisResult<PathBuf> {
    let resource_dir = resource_dir(&tauri::generate_context!().config())
        .ok_or_else(|| JarvisError::ConfigError(ConfigError::FileNotFound(
            "Resource directory not found".to_string()
        )))?;
    Ok(resource_dir.join("rustpotter"))
}

// VOSK
pub const VOSK_FETCH_PHRASE: &str = "джарвис";

pub const VOSK_MIN_RATIO: f64 = 70.0;

// ETC
pub const CMD_RATIO_THRESHOLD: f64 = 65f64;
pub const CMS_WAIT_DELAY: std::time::Duration = std::time::Duration::from_secs(15);

pub const ASSISTANT_GREET_PHRASES: [&str; 3] = ["greet1", "greet2", "greet3"];
pub const ASSISTANT_PHRASES_TBR: [&str; 17] = [
    "джарвис", "сэр", "слушаю сэр", "всегда к услугам", "произнеси",
    "ответь", "покажи", "скажи", "давай", "да сэр", "к вашим услугам сэр",
    "всегда к вашим услугам сэр", "запрос выполнен сэр", "выполнен сэр",
    "есть", "загружаю сэр", "очень тонкое замечание сэр",
];