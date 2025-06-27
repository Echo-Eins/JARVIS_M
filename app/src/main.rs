// app/src/main.rs - Исправленная точка входа JARVIS

use std::env;
use std::path::PathBuf;
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};

use once_cell::sync::{Lazy, OnceCell};
use platform_dirs::AppDirs;

// Модули ошибок (первым делом)
mod error;
use error::{JarvisResult, JarvisError};

// Основные модули
mod config;
mod log;
mod db;

// Аудио и ввод/вывод
mod recorder;
mod audio;

// Распознавание и синтез речи
mod stt;
mod listener;

// Система команд
mod commands;
mod document_search;

pub use db::structs;
// Системная интеграция
#[cfg(not(target_os = "macos"))]
mod tray;

// Основное приложение
mod app;

use commands::AssistantCommand;

// Логирование
#[macro_use]
extern crate simple_log;

// Глобальные переменные
static APP_DIR: Lazy<PathBuf> = Lazy::new(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
static SOUND_DIR: Lazy<PathBuf> = Lazy::new(|| APP_DIR.clone().join("sound"));
static APP_DIRS: OnceCell<AppDirs> = OnceCell::new();
static APP_CONFIG_DIR: OnceCell<PathBuf> = OnceCell::new();
static APP_LOG_DIR: OnceCell<PathBuf> = OnceCell::new();
static DB: OnceCell<db::structs::Settings> = OnceCell::new();
static COMMANDS_LIST: OnceCell<Vec<AssistantCommand>> = OnceCell::new();

// Флаги состояния
static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);
static INITIALIZATION_COMPLETE: AtomicBool = AtomicBool::new(false);

fn main() {
    // Устанавливаем обработчики сигналов для graceful shutdown
    setup_signal_handlers();

    // Запускаем приложение
    if let Err(e) = run_application() {
        error!("Application failed: {}", e);

        // Выполняем graceful shutdown
        if let Err(shutdown_error) = perform_graceful_shutdown() {
            error!("Shutdown failed: {}", shutdown_error);
        }

        process::exit(1);
    }

    info!("JARVIS completed successfully");
}

/// Основная функция приложения
fn run_application() -> JarvisResult<()> {
    info!("🚀 Starting JARVIS Voice Assistant v{}", config::APP_VERSION.unwrap_or("unknown"));

    // === ЭТАП 1: Базовая инициализация ===
    info!("📁 Phase 1: Basic initialization");
    initialize_configuration()?;
    initialize_logging()?;
    initialize_database()?;

    // === ЭТАП 2: Аудио система ===
    info!("🎵 Phase 2: Audio system initialization");
    initialize_audio_system()?;

    // === ЭТАП 3: Команды и документы ===
    info!("📋 Phase 3: Commands and document search");
    initialize_commands_and_documents()?;

    // === ЭТАП 4: Системная интеграция ===
    info!("🖥️ Phase 4: System integration");
    initialize_system_integration()?;

    // === ЭТАП 5: Запуск основного цикла ===
    info!("🎯 Phase 5: Starting main application loop");
    INITIALIZATION_COMPLETE.store(true, Ordering::SeqCst);

    run_main_loop()?;

    Ok(())
}

/// Инициализация конфигурации
fn initialize_configuration() -> JarvisResult<()> {
    info!("Initializing configuration...");

    config::init_dirs()?;
    // config::validate_config()?; // Добавим позже если нужно

    info!("✅ Configuration initialized");
    Ok(())
}

/// Инициализация логирования
fn initialize_logging() -> JarvisResult<()> {
    info!("Initializing logging system...");

    log::init_logging().map_err(|e| {
        JarvisError::ConfigError(error::ConfigError::InvalidConfiguration(
            format!("Failed to initialize logging: {}", e)
        ))
    })?;

    // Логируем информацию о приложении
    info!("JARVIS Voice Assistant v{}", config::APP_VERSION.unwrap_or("unknown"));
    info!("Platform: {} {}", std::env::consts::OS, std::env::consts::ARCH);
    info!("Config directory: {}", APP_CONFIG_DIR.get().unwrap().display());
    info!("Log directory: {}", APP_LOG_DIR.get().unwrap().display());

    info!("✅ Logging system initialized");
    Ok(())
}

/// Инициализация базы данных
fn initialize_database() -> JarvisResult<()> {
    info!("Initializing database...");

    let settings = db::init_settings()?;

    DB.set(settings).map_err(|_| {
        JarvisError::DatabaseError(error::DatabaseError::InitializationFailed(
            "Database already initialized".to_string()
        ))
    })?;

    info!("✅ Database initialized");
    Ok(())
}

/// Инициализация аудио системы
fn initialize_audio_system() -> JarvisResult<()> {
    info!("Initializing audio system...");

    // Инициализируем аудио рекордер
    info!("Starting audio recorder...");
    recorder::init().map_err(|e| {
        error!("❌ Critical: Audio recorder initialization failed");
        e
    })?;

    // Инициализируем аудио плеер
    info!("Starting audio player...");
    audio::init().map_err(|e| {
        error!("❌ Critical: Audio player initialization failed");
        e
    })?;

    // Инициализируем STT
    info!("Starting Speech-to-Text...");
    stt::init().map_err(|e| {
        error!("❌ Critical: STT initialization failed");
        e
    })?;

    // Инициализируем wake-word engine
    info!("Starting wake-word detection...");
    listener::init().map_err(|e| {
        error!("❌ Critical: Wake-word engine initialization failed");
        e
    })?;

    info!("✅ Audio system initialized");
    Ok(())
}

/// Инициализация команд и поиска документов
fn initialize_commands_and_documents() -> JarvisResult<()> {
    info!("Initializing command system...");

    let commands = commands::parse_commands().map_err(|e| {
        JarvisError::CommandError(error::CommandError::ParseError(
            format!("Failed to parse commands: {}", e)
        ))
    })?;

    info!("Commands loaded: {} total", commands.len());

    // Логируем доступные команды
    let command_names = commands::list(&commands);
    info!("Available commands: {:?}", command_names);

    COMMANDS_LIST.set(commands).map_err(|_| {
        JarvisError::CommandError(error::CommandError::ParseError(
            "Commands list already initialized".to_string()
        ))
    })?;

    // Инициализируем поиск документов
    info!("Starting document search...");
    document_search::init().map_err(|e| {
        warn!("Document search initialization failed: {}", e);
        e
    })?;

    info!("✅ Commands and document search initialized");
    Ok(())
}

/// Инициализация системной интеграции
fn initialize_system_integration() -> JarvisResult<()> {
    // Инициализируем системный трей (если поддерживается)
    #[cfg(not(target_os = "macos"))]
    {
        info!("Starting system tray...");
        tray::init().map_err(|e| {
            warn!("System tray initialization failed: {}", e);
            // Не критическая ошибка для tray
            e
        })?;
    }

    #[cfg(target_os = "macos")]
    {
        info!("System tray is not supported on macOS");
    }

    info!("✅ System integration initialized");
    Ok(())
}

/// Основной цикл приложения
fn run_main_loop() -> JarvisResult<()> {
    info!("🎯 JARVIS is ready and listening!");

    // Воспроизводим звук запуска
    if let Ok(sounds_directory) = audio::get_sound_directory() {
        if let Err(e) = audio::play_sound(&sounds_directory.join("run.wav")) {
            warn!("Failed to play startup sound: {}", e);
        }
    }

    // Запускаем основной цикл приложения
    app::start().map_err(|_| {
        JarvisError::Generic("Main loop failed".to_string())
    })?;

    Ok(())
}

/// Установка обработчиков сигналов
fn setup_signal_handlers() {
    #[cfg(unix)]
    {
        use signal_hook::{consts::SIGINT, iterator::Signals};
        use std::thread;

        if let Ok(mut signals) = Signals::new(&[SIGINT]) {
            thread::spawn(move || {
                for sig in signals.forever() {
                    match sig {
                        SIGINT => {
                            info!("📡 Received SIGINT, initiating graceful shutdown...");
                            SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
                            break;
                        }
                        _ => unreachable!(),
                    }
                }
            });
        }
    }

    #[cfg(windows)]
    {
        use winapi::um::wincon::{SetConsoleCtrlHandler, CTRL_C_EVENT};
        use winapi::shared::minwindef::{BOOL, DWORD, TRUE, FALSE};

        unsafe extern "system" fn ctrl_handler(ctrl_type: DWORD) -> BOOL {
            match ctrl_type {
                CTRL_C_EVENT => {
                    info!("📡 Received Ctrl+C, initiating graceful shutdown...");
                    SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
                    TRUE
                }
                _ => FALSE,
            }
        }

        unsafe {
            SetConsoleCtrlHandler(Some(ctrl_handler), TRUE);
        }
    }
}

/// Проверка запроса на завершение
pub fn should_shutdown() -> bool {
    SHUTDOWN_REQUESTED.load(Ordering::SeqCst)
}

/// Проверка завершения инициализации
pub fn is_initialization_complete() -> bool {
    INITIALIZATION_COMPLETE.load(Ordering::SeqCst)
}

/// Запрос graceful shutdown
pub fn request_shutdown() {
    SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
    info!("🛑 Shutdown requested");
}

/// Graceful shutdown всех компонентов
fn perform_graceful_shutdown() -> JarvisResult<()> {
    info!("🛑 Initiating graceful shutdown...");

    // Фаза 1: Останавливаем прослушивание
    info!("Phase 1: Stopping voice recognition...");
    if recorder::is_recording() {
        if let Err(e) = recorder::stop_recording() {
            warn!("Failed to stop recording gracefully: {}", e);
        }
    }

    // Фаза 2: Закрываем аудио системы
    info!("Phase 2: Shutting down audio systems...");
    if let Err(e) = recorder::shutdown() {
        warn!("Failed to shutdown recorder gracefully: {}", e);
    }

    if let Err(e) = audio::shutdown() {
        warn!("Failed to shutdown audio gracefully: {}", e);
    }

    // Фаза 3: Сохраняем состояние базы данных
    info!("Phase 3: Saving database state...");
    if let Err(e) = db::save_state() {
        warn!("Failed to save database state: {}", e);
    }

    // Фаза 4: Закрываем системную интеграцию
    info!("Phase 4: Shutting down system integration...");
    #[cfg(not(target_os = "macos"))]
    {
        if let Err(e) = tray::shutdown() {
            warn!("Failed to shutdown tray gracefully: {}", e);
        }
    }

    info!("✅ Graceful shutdown completed");
    Ok(())
}

/// Экстренное завершение при критической ошибке
pub fn emergency_shutdown(error: &JarvisError) -> ! {
    error!("💥 CRITICAL ERROR: {}", error);
    error!("🚨 Performing emergency shutdown...");

    // Попытка экстренного сохранения данных
    let _ = db::emergency_save();

    // Принудительная остановка всех процессов
    let _ = recorder::shutdown();
    let _ = audio::shutdown();

    error!("🚨 Emergency shutdown completed");
    process::exit(1);
}

/// Получение статистики приложения
pub fn get_app_stats() -> serde_json::Value {
    serde_json::json!({
        "version": config::APP_VERSION.unwrap_or("unknown"),
        "platform": format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        "initialization_complete": is_initialization_complete(),
        "components": {
            "recorder_initialized": recorder::is_initialized(),
            "recorder_recording": recorder::is_recording(),
            "audio_initialized": audio::is_initialized(),
        },
        "commands": {
            "total_commands": COMMANDS_LIST.get().map(|c| c.len()).unwrap_or(0),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shutdown_flag() {
        SHUTDOWN_REQUESTED.store(false, Ordering::SeqCst);
        assert!(!should_shutdown());

        request_shutdown();
        assert!(should_shutdown());
    }

    #[test]
    fn test_initialization_flag() {
        INITIALIZATION_COMPLETE.store(false, Ordering::SeqCst);
        assert!(!is_initialization_complete());

        INITIALIZATION_COMPLETE.store(true, Ordering::SeqCst);
        assert!(is_initialization_complete());
    }

    #[test]
    fn test_app_stats() {
        let stats = get_app_stats();
        assert!(stats.get("version").is_some());
        assert!(stats.get("platform").is_some());
        assert!(stats.get("components").is_some());
    }
}