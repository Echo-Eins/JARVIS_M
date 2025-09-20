// src-tauri/src/main.rs - Исправленная точка входа приложения

use std::env;
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

// Импортируем все из нашей библиотеки
use jarvis::{
    JarvisResult, JarvisError,
    config, error, db, audio, stt, listener,
    commands, app, structs,
    COMMANDS, COMMANDS_LIST, DB
};

#[cfg(feature = "document-search")]
use jarvis::document_search;

#[cfg(all(not(target_os = "macos"), feature = "system-tray"))]
use jarvis::tray;

// Логирование
#[macro_use]
extern crate log;

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
    info!("🚀 Starting JARVIS Voice Assistant v{}", jarvis::VERSION);

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

    // === ЭТАП 5: Основной цикл ===
    info!("🎯 Phase 5: Starting main loop");

    // Отмечаем инициализацию как завершенную
    INITIALIZATION_COMPLETE.store(true, Ordering::Relaxed);

    run_main_loop()?;

    info!("✅ All phases completed successfully");
    Ok(())
}

/// Инициализация конфигурации
fn initialize_configuration() -> JarvisResult<()> {
    info!("Initializing configuration...");
    config::init_dirs()?;
    config::validate_configuration()?;
    info!("✅ Configuration initialized");
    Ok(())
}

/// Инициализация логирования
fn initialize_logging() -> JarvisResult<()> {
    info!("Initializing logging...");

    if let Ok(log_path) = config::get_log_file_path() {
        // Здесь можно настроить более продвинутое логирование
        info!("Log file: {}", log_path.display());
    }

    info!("✅ Logging initialized");
    Ok(())
}

/// Инициализация базы данных
fn initialize_database() -> JarvisResult<()> {
    info!("Initializing database...");

    let settings = db::init_settings()?;

    let legacy_db = db::LegacyDatabase::from_settings(&settings);
    if DB.set(Arc::new(Mutex::new(legacy_db))).is_err() {
        if let Some(existing) = DB.get() {
            if let Ok(mut guard) = existing.lock() {
                guard.sync_from_global_settings();
            }
        }
    }

    info!("✅ Database initialized");
    Ok(())
}

/// Инициализация аудио системы
fn initialize_audio_system() -> JarvisResult<()> {
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

    let commands_data = commands::parse_commands().map_err(|e| {
        JarvisError::CommandError(error::CommandError::ParseError(
            format!("Failed to parse commands: {}", e)
        ))
    })?;

    let _ = COMMANDS.set(commands_data.clone());
    let _ = COMMANDS_LIST.set(commands_data.clone());

    // Инициализируем поиск документов (если включен)
    #[cfg(feature = "document-search")]
    {
        info!("Starting document search...");
        document_search::init().map_err(|e| {
            warn!("Document search initialization failed: {}", e);
            e
        })?;
    }

    info!("✅ Commands and document search initialized");
    Ok(())
}

/// Инициализация системной интеграции
fn initialize_system_integration() -> JarvisResult<()> {
    // Инициализируем системный трей (если поддерживается)
    #[cfg(all(not(target_os = "macos"), feature = "system-tray"))]
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

/// Настройка обработчиков сигналов
fn setup_signal_handlers() {
    #[cfg(unix)]
    {
        use signal_hook::{consts::SIGINT, consts::SIGTERM, iterator::Signals};
        use std::thread;

        let mut signals = Signals::new(&[SIGINT, SIGTERM]).unwrap();
        thread::spawn(move || {
            for sig in signals.forever() {
                match sig {
                    SIGINT | SIGTERM => {
                        info!("Received shutdown signal: {}", sig);
                        request_shutdown();
                        break;
                    }
                    _ => {}
                }
            }
        });
    }

    #[cfg(windows)]
    {
        use winapi::um::consoleapi::SetConsoleCtrlHandler;
        use winapi::um::wincon::{CTRL_C_EVENT, CTRL_CLOSE_EVENT};
        use winapi::shared::minwindef::{BOOL, DWORD, TRUE};

        unsafe extern "system" fn ctrl_handler(ctrl_type: DWORD) -> BOOL {
            match ctrl_type {
                CTRL_C_EVENT | CTRL_CLOSE_EVENT => {
                    info!("Received Windows shutdown signal: {}", ctrl_type);
                    request_shutdown();
                    TRUE
                }
                _ => TRUE,
            }
        }

        unsafe {
            SetConsoleCtrlHandler(Some(ctrl_handler), TRUE);
        }
    }
}

/// Запрос на завершение приложения
pub fn request_shutdown() {
    info!("Shutdown requested");
    SHUTDOWN_REQUESTED.store(true, Ordering::Relaxed);
}

/// Проверка запроса на завершение
pub fn should_shutdown() -> bool {
    SHUTDOWN_REQUESTED.load(Ordering::Relaxed)
}

/// Выполнение graceful shutdown
fn perform_graceful_shutdown() -> JarvisResult<()> {
    info!("Performing graceful shutdown...");

    // Останавливаем компоненты в обратном порядке инициализации

    // Системная интеграция
    #[cfg(all(not(target_os = "macos"), feature = "system-tray"))]
    {
        if let Err(e) = tray::shutdown() {
            warn!("Tray shutdown error: {}", e);
        }
    }

    // Аудио система
    if let Err(e) = listener::shutdown() {
        warn!("Listener shutdown error: {}", e);
    }

    if let Err(e) = stt::shutdown() {
        warn!("STT shutdown error: {}", e);
    }

    if let Err(e) = audio::shutdown() {
        warn!("Audio shutdown error: {}", e);
    }

    info!("✅ Graceful shutdown completed");
    Ok(())
}