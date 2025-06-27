// app/src/app.rs - Исправленный основной цикл приложения

use std::time::SystemTime;
use crate::{config, audio, listener, stt, commands, COMMANDS_LIST, should_shutdown, error, db};
use crate::error::{JarvisResult, JarvisError};
use rand::seq::SliceRandom;
use log::{info, warn, error};
use audio::recorder::*;

/// Основная функция запуска приложения
pub fn start() -> JarvisResult<()> {
    info!("Starting main application loop...");
    main_loop()
}

/// Главный цикл обработки голосовых команд
fn main_loop() -> JarvisResult<()> {
    let sounds_directory = audio::get_sound_directory()?;
    let frame_length: usize = 512; // стандартный размер для всех wake-word движков
    let mut frame_buffer: Vec<i16> = vec![0; frame_length];

    // Воспроизводим звук запуска
    if let Err(e) = audio::play_sound(&sounds_directory.join("run.wav")) {
        warn!("Failed to play startup sound: {}", e);
    }

    // Начинаем запись
    recorder::start_recording().map_err(|e| {
        error!("Cannot start recording: {}", e);
        e
    })?;

    info!("Main loop started, listening for wake-word...");

    // Основной цикл распознавания wake-word
    'wake_word_loop: loop {
        // Проверяем флаг завершения
        if should_shutdown() {
            info!("Shutdown requested, exiting main loop");
            break 'wake_word_loop;
        }

        // Читаем данные с микрофона
        if let Err(e) = recorder::read_microphone(&mut frame_buffer) {
            error!("Failed to read from microphone: {}", e);
            continue;
        }

        // Распознаем wake-word
        match listener::data_callback(&frame_buffer) {
            Some(keyword_index) => {
                info!("Wake-word detected! Index: {}", keyword_index);

                // Обрабатываем голосовые команды после активации
                if let Err(e) = handle_voice_commands(&sounds_directory, &mut frame_buffer) {
                    error!("Voice command processing failed: {}", e);
                    // Продолжаем работу даже если команда не выполнилась
                }
            },
            None => {
                // Wake-word не обнаружен, продолжаем слушать
                continue;
            }
        }
    }

    info!("Main loop completed");
    Ok(())
}

/// Обработка голосовых команд после активации wake-word
fn handle_voice_commands(
    sounds_directory: &std::path::PathBuf,
    frame_buffer: &mut [i16]
) -> JarvisResult<()> {
    let start_time = SystemTime::now();

    // Воспроизводим приветственную фразу
    let greet_phrase = config::ASSISTANT_GREET_PHRASES
        .choose(&mut rand::thread_rng())
        .unwrap_or(&"greet1");

    if let Err(e) = audio::play_sound(&sounds_directory.join(format!("{}.wav", greet_phrase))) {
        warn!("Failed to play greeting sound: {}", e);
    }

    info!("Listening for voice commands...");

    // Цикл распознавания голосовых команд
    'voice_recognition: loop {
        // Проверяем флаг завершения
        if should_shutdown() {
            break 'voice_recognition;
        }

        // Читаем данные с микрофона
        if let Err(e) = recorder::read_microphone(frame_buffer) {
            error!("Failed to read from microphone during voice recognition: {}", e);
            continue;
        }

        // STT обработка (без частичных результатов)
        match stt::recognize(frame_buffer, false) {
            Some(mut recognized_voice) => {
                if recognized_voice.trim().is_empty() {
                    continue; // Пустое распознавание, продолжаем слушать
                }

                info!("Recognized voice: '{}'", recognized_voice);

                // Фильтруем распознанный текст
                recognized_voice = filter_recognized_voice(recognized_voice);

                if recognized_voice.trim().is_empty() {
                    info!("Voice filtered to empty string, ignoring");
                    continue;
                }

                info!("Filtered voice: '{}'", recognized_voice);

                // Ищем подходящую команду
                if let Some((cmd_path, cmd_config)) = find_matching_command(&recognized_voice)? {
                    info!("Command found: {:?}", cmd_path);
                    info!("Executing command...");

                    // Выполняем команду
                    match execute_found_command(cmd_path, cmd_config, sounds_directory) {
                        Ok(should_continue_chain) => {
                            info!("Command executed successfully");

                            if should_continue_chain {
                                // Продолжаем цепочку команд - сбрасываем таймер
                                continue 'voice_recognition;
                            } else {
                                // Команда завершена, возвращаемся к wake-word
                                break 'voice_recognition;
                            }
                        }
                        Err(e) => {
                            error!("Command execution failed: {}", e);
                            // Продолжаем работу даже если команда не выполнилась
                            break 'voice_recognition;
                        }
                    }
                } else {
                    info!("No matching command found for: '{}'", recognized_voice);
                    // Можно добавить звук "команда не найдена"
                }

                // После обработки команды возвращаемся к wake-word
                break 'voice_recognition;
            }
            None => {
                // STT ничего не распознал, продолжаем слушать
            }
        }

        // Проверяем таймаут голосовых команд
        if let Ok(elapsed) = start_time.elapsed() {
            if elapsed > config::CMS_WAIT_DELAY {
                info!("Voice command timeout reached, returning to wake-word detection");
                break 'voice_recognition;
            }
        }
    }

    Ok(())
}

/// Фильтрация распознанного голоса от служебных фраз
fn filter_recognized_voice(mut voice: String) -> String {
    voice = voice.to_lowercase();

    // Удаляем служебные фразы ассистента
    for phrase_to_remove in &config::ASSISTANT_PHRASES_TBR {
        voice = voice.replace(phrase_to_remove, "");
    }

    // Убираем лишние пробелы
    voice.trim().to_string()
}

/// Поиск подходящей команды
fn find_matching_command(voice: &str) -> JarvisResult<Option<(&std::path::PathBuf, &commands::structs::Config)>> {
    let commands_list = COMMANDS_LIST.get()
        .ok_or_else(|| JarvisError::CommandError(error::CommandError::CommandNotFound(
            "Commands list not initialized".to_string()
        )))?;

    Ok(commands::fetch_command(voice, commands_list))
}
use db::structs;
/// Выполнение найденной команды
fn execute_found_command(
    cmd_path: &std::path::PathBuf,
    cmd_config: &commands::structs::Config,
    sounds_directory: &std::path::PathBuf,
) -> JarvisResult<bool> {
    match commands::execute_command(cmd_path, cmd_config) {
        Ok(should_chain) => {
            // Воспроизводим звук успешного выполнения
            if let Some(random_sound) = cmd_config.voice.sounds.choose(&mut rand::thread_rng()) {
                let sound_file = sounds_directory.join(format!("{}.wav", random_sound));
                if let Err(e) = audio::play_sound(&sound_file) {
                    warn!("Failed to play command success sound: {}", e);
                }
            }

            Ok(should_chain)
        }
        Err(e) => {
            error!("Command execution error: {}", e);

            // Воспроизводим звук ошибки (если есть)
            let error_sound = sounds_directory.join("error.wav");
            if error_sound.exists() {
                if let Err(sound_err) = audio::play_sound(&error_sound) {
                    warn!("Failed to play error sound: {}", sound_err);
                }
            }

            Err(JarvisError::CommandError(error::CommandError::ExecutionFailed(
                e.to_string()
            )))
        }
    }
}

/// Graceful остановка главного цикла
pub fn stop() -> JarvisResult<()> {
    info!("Stopping main application loop...");

    // Устанавливаем флаг завершения
    crate::request_shutdown();

    // Останавливаем запись
    if recorder::is_recording() {
        recorder::stop_recording()?;
    }

    info!("Main application loop stopped");
    Ok(())
}

/// Проверка состояния главного цикла
pub fn is_running() -> bool {
    recorder::is_recording() && !should_shutdown()
}

/// Получение статистики главного цикла
pub fn get_main_loop_stats() -> serde_json::Value {
    serde_json::json!({
        "is_running": is_running(),
        "is_recording": recorder::is_recording(),
        "shutdown_requested": should_shutdown(),
        "commands_available": COMMANDS_LIST.get().map(|c| c.len()).unwrap_or(0),
    })
}

/// Функция для принудительного завершения (legacy совместимость)
pub fn close(code: i32) -> ! {
    warn!("Legacy close function called with code: {}", code);
    info!("Initiating application shutdown...");

    // Пытаемся graceful shutdown
    if let Err(e) = crate::perform_graceful_shutdown() {
        error!("Graceful shutdown failed: {}", e);
    }

    info!("Application closing with code: {}", code);
    std::process::exit(code);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_recognized_voice() {
        let input = "Джарвис включи музыку сэр".to_string();
        let filtered = filter_recognized_voice(input);
        assert_eq!(filtered, "включи музыку");
    }

    #[test]
    fn test_filter_multiple_phrases() {
        let input = "Джарвис скажи время да сэр".to_string();
        let filtered = filter_recognized_voice(input);
        assert_eq!(filtered, "время");
    }

    #[test]
    fn test_main_loop_stats() {
        let stats = get_main_loop_stats();
        assert!(stats.get("is_running").is_some());
        assert!(stats.get("is_recording").is_some());
        assert!(stats.get("shutdown_requested").is_some());
        assert!(stats.get("commands_available").is_some());
    }
}