// gui/src-tauri/src/tauri_commands/listener.rs - Рефакторинг с улучшенной обработкой ошибок

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::SystemTime;
use std::path::Path;

use once_cell::sync::OnceCell;
use porcupine::{Porcupine, PorcupineBuilder};
use rustpotter::{Rustpotter, RustpotterConfig, WavFmt, DetectorConfig, FiltersConfig, ScoreMode, GainNormalizationConfig, BandPassConfig};
use tauri::Manager;
use rand::seq::SliceRandom;
use log::{info, warn, error};

use crate::assistant_commands;
use crate::events;
use crate::config;
use crate::vosk;
use crate::recorder::{self, FRAME_LENGTH};
use crate::COMMANDS;
use crate::DB;

use crate::errors::{JarvisResult, JarvisError, ListenerError, RecorderError};

// Состояние listener'а
static LISTENING: AtomicBool = AtomicBool::new(false);
static STOP_LISTENING: AtomicBool = AtomicBool::new(false);
static TAURI_APP_HANDLE: OnceCell<tauri::AppHandle> = OnceCell::new();
static PORCUPINE: OnceCell<Porcupine> = OnceCell::new();
static RUSTPOTTER: OnceCell<Mutex<Rustpotter>> = OnceCell::new();

/// Tauri команда для проверки статуса прослушивания
#[tauri::command]
pub fn is_listening() -> bool {
    LISTENING.load(Ordering::SeqCst)
}

/// Tauri команда для остановки прослушивания с обработкой ошибок
#[tauri::command]
pub fn stop_listening() -> Result<bool, String> {
    if is_listening() {
        STOP_LISTENING.store(true, Ordering::SeqCst);

        // Ожидаем завершения прослушивания с таймаутом
        let timeout = std::time::Duration::from_secs(5);
        let start = std::time::Instant::now();

        while is_listening() && start.elapsed() < timeout {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        if is_listening() {
            warn!("Forced stop listening due to timeout");
            force_stop_recording()?;
        }
    }

    Ok(true)
}

/// Принудительная остановка записи
fn force_stop_recording() -> Result<(), String> {
    stop_recording().map_err(|e| {
        error!("Failed to force stop recording: {}", e);
        format!("Failed to stop recording: {}", e)
    })?;
    Ok(())
}

/// Определение движка распознавания wake-word из настроек
fn get_wake_word_engine() -> Result<config::WakeWordEngine, String> {
    let db = DB.lock().map_err(|e| format!("Database lock error: {}", e))?;

    if let Some(engine_str) = db.get::<String>("selected_wake_word_engine") {
        match engine_str.trim().to_lowercase().as_str() {
            "rustpotter" => Ok(config::WakeWordEngine::Rustpotter),
            "vosk" => Ok(config::WakeWordEngine::Vosk),
            "picovoice" => Ok(config::WakeWordEngine::Porcupine),
            _ => {
                warn!("Unknown wake word engine '{}', using default", engine_str);
                Ok(config::DEFAULT_WAKE_WORD_ENGINE)
            }
        }
    } else {
        info!("No wake word engine configured, using default");
        Ok(config::DEFAULT_WAKE_WORD_ENGINE)
    }
}

/// Tauri команда для начала прослушивания
#[tauri::command(async)]
pub fn start_listening(app_handle: tauri::AppHandle) -> Result<bool, String> {
    // Проверяем, не запущено ли уже прослушивание
    if is_listening() {
        return Err("Already listening".to_string());
    }

    // Сохраняем app handle
    if TAURI_APP_HANDLE.get().is_none() {
        TAURI_APP_HANDLE.set(app_handle).map_err(|_| {
            "Failed to set app handle".to_string()
        })?;
    }

    // Определяем движок и запускаем соответствующий инициализатор
    let engine = get_wake_word_engine()?;

    match engine {
        config::WakeWordEngine::Rustpotter => {
            info!("Starting RUSTPOTTER wake-word engine...");
            rustpotter_init()
        },
        config::WakeWordEngine::Vosk => {
            info!("Starting VOSK wake-word engine...");
            vosk_init()
        },
        config::WakeWordEngine::Porcupine => {
            info!("Starting PICOVOICE PORCUPINE wake-word engine...");
            picovoice_init()
        }
    }
}

/// Инициализация Rustpotter с улучшенной обработкой ошибок
fn rustpotter_init() -> Result<bool, String> {
    // Конфигурация Rustpotter
    let rustpotter_config = RustpotterConfig {
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
    };

    // Создаем экземпляр Rustpotter
    let mut rustpotter = Rustpotter::new(&rustpotter_config)
        .map_err(|e| format!("Failed to create Rustpotter instance: {}", e))?;

    // Загружаем wake-word файлы
    let rustpotter_wake_word_files = [
        "rustpotter/jarvis-default.rpw",
        "rustpotter/jarvis-community-1.rpw",
        "rustpotter/jarvis-community-2.rpw",
        "rustpotter/jarvis-community-3.rpw",
        "rustpotter/jarvis-community-4.rpw",
    ];

    let mut loaded_files = 0;
    for rpw_file in rustpotter_wake_word_files {
        match rustpotter.add_wakeword_from_file(rpw_file) {
            Ok(_) => {
                info!("Loaded wake-word file: {}", rpw_file);
                loaded_files += 1;
            }
            Err(e) => {
                warn!("Failed to load wake-word file {}: {}", rpw_file, e);
            }
        }
    }

    if loaded_files == 0 {
        return Err("No Rustpotter wake-word files could be loaded".to_string());
    }

    info!("Loaded {} wake-word files for Rustpotter", loaded_files);

    // Сохраняем экземпляр Rustpotter
    RUSTPOTTER.set(Mutex::new(rustpotter)).map_err(|_| {
        "Failed to store Rustpotter instance".to_string()
    })?;

    // Запускаем запись
    start_recording()
}

/// Инициализация Vosk
fn vosk_init() -> Result<bool, String> {
    // Проверяем наличие модели Vosk
    let model_path = std::path::Path::new(config::VOSK_MODEL_PATH);
    if !model_path.exists() {
        return Err(format!("Vosk model not found at: {}", model_path.display()));
    }

    info!("Using Vosk model from: {}", model_path.display());
    start_recording()
}

/// Инициализация Picovoice с улучшенной обработкой ошибок
fn picovoice_init() -> Result<bool, String> {
    // Получаем API ключ из базы данных
    let api_key = {
        let db = DB.lock().map_err(|e| format!("Database lock error: {}", e))?;

        db.get::<String>("api_key__picovoice")
            .ok_or_else(|| "Picovoice API key not set in settings".to_string())?
    };

    if api_key.trim().is_empty() {
        return Err("Picovoice API key is empty. Please set it in settings.".to_string());
    }

    // Проверяем наличие файла ключевого слова
    let keyword_path = Path::new(config::KEYWORDS_PATH).join(config::DEFAULT_KEYWORD);
    if !keyword_path.exists() {
        return Err(format!("Picovoice keyword file not found: {}", keyword_path.display()));
    }

    // Создаем экземпляр Porcupine
    let porcupine = PorcupineBuilder::new_with_keyword_paths(
        api_key.trim(),
        &[keyword_path]
    )
        .sensitivities(&[config::DEFAULT_SENSITIVITY])
        .init()
        .map_err(|e| {
            error!("Porcupine initialization failed: {}", e);
            match e {
                porcupine::PorcupineError::InvalidApiKey => {
                    "Invalid Picovoice API key. Please check your settings.".to_string()
                }
                porcupine::PorcupineError::NetworkError => {
                    "Network error. Please check your internet connection.".to_string()
                }
                _ => format!("Porcupine error: {}", e)
            }
        })?;

    info!("Porcupine initialized successfully");

    // Сохраняем экземпляр Porcupine
    PORCUPINE.set(porcupine).map_err(|_| {
        "Failed to store Porcupine instance".to_string()
    })?;

    // Запускаем запись
    start_recording()
}

/// Начало записи с обработкой ошибок
fn start_recording() -> Result<bool, String> {
    let frame_length = recorder::FRAME_LENGTH.load(Ordering::SeqCst) as usize;
    let mut frame_buffer = vec![0; frame_length];

    // Инициализируем и запускаем запись
    recorder::init().map_err(|e| format!("Recorder init failed: {}", e))?;
    recorder::start_recording().map_err(|e| format!("Failed to start recording: {}", e))?;

    LISTENING.store(true, Ordering::SeqCst);
    info!("Started listening...");

    // Воспроизводим приветственный звук
    if let Some(app_handle) = TAURI_APP_HANDLE.get() {
        events::play("run", app_handle);
    }

    // Основной цикл записи
    let result = match recorder::RECORDER_TYPE.load(Ordering::SeqCst) {
        recorder::RecorderType::PvRecorder => {
            recording_loop(&mut frame_buffer)
        },
        recorder::RecorderType::PortAudio => {
            Err("PortAudio not implemented".to_string())
        },
        recorder::RecorderType::Cpal => {
            Err("CPAL not implemented".to_string())
        }
    };

    // Всегда останавливаем запись при выходе из цикла
    stop_recording()?;

    result
}

/// Основной цикл записи
fn recording_loop(frame_buffer: &mut [i16]) -> Result<bool, String> {
    while !STOP_LISTENING.load(Ordering::SeqCst) {
        // Читаем данные с микрофона
        recorder::read_microphone(frame_buffer)
            .map_err(|e| format!("Failed to read microphone: {}", e))?;

        // Обрабатываем данные через движок распознавания
        if let Err(e) = data_callback(frame_buffer) {
            warn!("Data callback error: {}", e);
            // Не прерываем цикл из-за ошибки обработки
        }
    }

    Ok(true)
}

/// Остановка записи
fn stop_recording() -> Result<(), String> {
    recorder::stop_recording().map_err(|e| format!("Failed to stop recording: {}", e))?;

    LISTENING.store(false, Ordering::SeqCst);
    STOP_LISTENING.store(false, Ordering::SeqCst);

    info!("Stopped listening");
    Ok(())
}

/// Обработка данных от микрофона
pub fn data_callback(frame_buffer: &[i16]) -> Result<(), String> {
    let engine = get_wake_word_engine()?;

    match engine {
        config::WakeWordEngine::Rustpotter => {
            rustpotter_callback(frame_buffer)
        },
        config::WakeWordEngine::Vosk => {
            vosk_callback(frame_buffer)
        },
        config::WakeWordEngine::Porcupine => {
            porcupine_callback(frame_buffer)
        }
    }
}

/// Обработка данных Rustpotter
fn rustpotter_callback(frame_buffer: &[i16]) -> Result<(), String> {
    let rustpotter_mutex = RUSTPOTTER.get()
        .ok_or("Rustpotter not initialized")?;

    let mut rustpotter = rustpotter_mutex.lock()
        .map_err(|e| format!("Failed to lock Rustpotter: {}", e))?;

    if let Some(detection) = rustpotter.process_i16(frame_buffer) {
        if detection.score > config::RUSPOTTER_MIN_SCORE {
            info!("Rustpotter detection: score={:.2}", detection.score);
            keyword_callback(0)?;
        }
    }

    Ok(())
}

/// Обработка данных Vosk
fn vosk_callback(frame_buffer: &[i16]) -> Result<(), String> {
    let recognized = vosk::recognize(frame_buffer, true)
        .map_err(|e| format!("Vosk recognition error: {}", e))?
        .unwrap_or_default();

    if !recognized.trim().is_empty() {
        let words: Vec<&str> = recognized.split_whitespace().collect();
        let phrase_ratio = words.iter()
            .filter(|word| word.to_lowercase().contains(config::VOSK_FETCH_PHRASE))
            .count() as f64 / words.len() as f64 * 100.0;

        if phrase_ratio >= config::VOSK_MIN_RATIO {
            info!("Vosk wake-word detected: '{}' (ratio: {:.1}%)", recognized, phrase_ratio);
            keyword_callback(0)?;
        }
    }

    Ok(())
}

/// Обработка данных Porcupine
fn porcupine_callback(frame_buffer: &[i16]) -> Result<(), String> {
    let porcupine = PORCUPINE.get()
        .ok_or("Porcupine not initialized")?;

    match porcupine.process(frame_buffer) {
        Ok(keyword_index) => {
            if keyword_index >= 0 {
                info!("Porcupine wake-word detected, keyword index: {}", keyword_index);
                keyword_callback(keyword_index)?;
            }
        }
        Err(e) => {
            return Err(format!("Porcupine processing error: {}", e));
        }
    }

    Ok(())
}

/// Обработка обнаружения ключевого слова
fn keyword_callback(keyword_index: i32) -> Result<(), String> {
    info!("Wake-word detected, starting voice command recognition...");

    // Уведомляем UI о начале распознавания
    if let Some(app_handle) = TAURI_APP_HANDLE.get() {
        // Воспроизводим приветственный звук
        let greet_phrase = config::ASSISTANT_GREET_PHRASES
            .choose(&mut rand::thread_rng())
            .unwrap_or(&"greet1");

        events::play(greet_phrase, app_handle);

        // Отправляем событие в UI
        app_handle.emit_all(events::EventTypes::AssistantGreet.get(), ())
            .map_err(|e| format!("Failed to emit greet event: {}", e))?;
    }

    // Запускаем распознавание голосовых команд
    voice_command_recognition()?;

    Ok(())
}

/// Распознавание голосовых команд после активации
fn voice_command_recognition() -> Result<(), String> {
    let start_time = SystemTime::now();
    let frame_length = recorder::FRAME_LENGTH.load(Ordering::SeqCst) as usize;
    let mut frame_buffer = vec![0; frame_length];

    info!("Listening for voice commands...");

    while !STOP_LISTENING.load(Ordering::SeqCst) {
        // Читаем данные с микрофона
        recorder::read_microphone(&mut frame_buffer)
            .map_err(|e| format!("Failed to read microphone during command recognition: {}", e))?;

        // Распознаем речь через Vosk
        if let Some(mut recognized_text) = vosk::recognize(&frame_buffer, false)? {
            if !recognized_text.is_empty() {
                info!("Recognized speech: '{}'", recognized_text);

                // Фильтруем распознанную речь
                recognized_text = filter_recognized_text(recognized_text);

                if !recognized_text.trim().is_empty() {
                    info!("Filtered speech: '{}'", recognized_text);

                    // Ищем подходящую команду
                    if let Some(command_result) = find_and_execute_command(&recognized_text)? {
                        if command_result {
                            // Команда выполнена, продолжаем слушать команды
                            continue;
                        } else {
                            // Команда завершена, возвращаемся к распознаванию wake-word
                            break;
                        }
                    }
                }
            }
        }

        // Проверяем таймаут
        if let Ok(elapsed) = start_time.elapsed() {
            if elapsed > config::CMS_WAIT_DELAY {
                info!("Voice command timeout, returning to wake-word detection");
                break;
            }
        }
    }

    // Уведомляем UI о завершении распознавания команд
    if let Some(app_handle) = TAURI_APP_HANDLE.get() {
        let _ = app_handle.emit_all(events::EventTypes::AssistantWaiting.get(), ());
    }

    Ok(())
}

/// Фильтрация распознанного текста
fn filter_recognized_text(mut text: String) -> String {
    text = text.to_lowercase();

    // Удаляем служебные фразы
    for phrase in &config::ASSISTANT_PHRASES_TBR {
        text = text.replace(phrase, "");
    }

    text.trim().to_string()
}

/// Поиск и выполнение команды
fn find_and_execute_command(text: &str) -> Result<Option<bool>, String> {
    let commands = COMMANDS.get()
        .ok_or("Commands not initialized")?;

    if let Some((cmd_path, cmd_config)) = assistant_commands::fetch_command(text, commands) {
        info!("Found matching command: {:?}", cmd_path);

        let app_handle = TAURI_APP_HANDLE.get()
            .ok_or("App handle not available")?;

        match assistant_commands::execute_command(cmd_path, cmd_config, app_handle) {
            Ok(chain_continue) => {
                info!("Command executed successfully");
                Ok(Some(chain_continue))
            }
            Err(e) => {
                error!("Command execution failed: {}", e);
                Err(format!("Failed to execute command: {}", e))
            }
        }
    } else {
        info!("No matching command found for: '{}'", text);
        Ok(None)
    }
}