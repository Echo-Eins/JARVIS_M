// app/src/audio/rodio.rs - Исправленный Rodio аудио бэкенд

use std::fs::File;
use std::path::PathBuf;
use std::io::BufReader;
use std::sync::{Arc, Mutex};
use once_cell::sync::OnceCell;

use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};

// Глобальные статические переменные для Rodio
static STREAM: OnceCell<OutputStream> = OnceCell::new();
static STREAM_HANDLE: OnceCell<OutputStreamHandle> = OnceCell::new();
static SINK: OnceCell<Arc<Mutex<Sink>>> = OnceCell::new();

/// Инициализация Rodio аудио системы
pub fn init() -> Result<(), ()> {
    if STREAM_HANDLE.get().is_some() {
        info!("Rodio already initialized");
        return Ok(());
    }

    // Получаем выходной поток для звукового устройства по умолчанию
    match OutputStream::try_default() {
        Ok((stream, stream_handle)) => {
            // Создаем sink для управления воспроизведением
            match Sink::try_new(&stream_handle) {
                Ok(sink) => {
                    info!("Rodio sink initialized successfully");

                    // Сохраняем компоненты
                    if STREAM.set(stream).is_err() {
                        error!("Failed to store Rodio stream");
                        return Err(());
                    }

                    if STREAM_HANDLE.set(stream_handle).is_err() {
                        error!("Failed to store Rodio stream handle");
                        return Err(());
                    }

                    if SINK.set(Arc::new(Mutex::new(sink))).is_err() {
                        error!("Failed to store Rodio sink");
                        return Err(());
                    }

                    info!("Rodio audio backend initialized successfully");
                    Ok(())
                },
                Err(e) => {
                    error!("Cannot create Rodio sink: {}", e);
                    Err(())
                }
            }
        },
        Err(e) => {
            error!("Failed to initialize Rodio audio stream: {}", e);
            Err(())
        }
    }
}

/// Воспроизведение звукового файла
pub fn play_sound(filename: &PathBuf, sleep: bool) {
    // Открываем и декодируем аудио файл
    let file = match File::open(filename) {
        Ok(f) => f,
        Err(e) => {
            warn!("Cannot open sound file {}: {}", filename.display(), e);
            return;
        }
    };

    let buf_reader = BufReader::new(file);
    let source = match Decoder::new(buf_reader) {
        Ok(s) => s,
        Err(e) => {
            warn!("Cannot decode sound file {}: {}", filename.display(), e);
            return;
        }
    };

    // Воспроизводим через sink
    if let Some(sink_arc) = SINK.get() {
        if let Ok(sink) = sink_arc.lock() {
            sink.append(source);

            info!("Playing sound: {} (blocking: {})", filename.display(), sleep);

            if sleep {
                // Блокирующий режим - ждем завершения воспроизведения
                sink.sleep_until_end();
            }
        } else {
            warn!("Failed to lock Rodio sink for: {}", filename.display());
        }
    } else {
        warn!("Rodio sink not initialized for: {}", filename.display());
    }
}

/// Установка громкости
pub fn set_volume(volume: f32) {
    if let Some(sink_arc) = SINK.get() {
        if let Ok(sink) = sink_arc.lock() {
            sink.set_volume(volume);
            info!("Rodio volume set to: {:.2}", volume);
        } else {
            warn!("Failed to lock Rodio sink for volume control");
        }
    } else {
        warn!("Rodio sink not initialized for volume control");
    }
}

/// Остановка воспроизведения
pub fn stop_playback() {
    if let Some(sink_arc) = SINK.get() {
        if let Ok(sink) = sink_arc.lock() {
            sink.stop();
            info!("Rodio playback stopped");
        } else {
            warn!("Failed to lock Rodio sink for stopping playback");
        }
    } else {
        warn!("Rodio sink not initialized for stopping playback");
    }
}

/// Пауза воспроизведения
pub fn pause_playback() {
    if let Some(sink_arc) = SINK.get() {
        if let Ok(sink) = sink_arc.lock() {
            sink.pause();
            info!("Rodio playback paused");
        } else {
            warn!("Failed to lock Rodio sink for pausing playback");
        }
    } else {
        warn!("Rodio sink not initialized for pausing playback");
    }
}

/// Возобновление воспроизведения
pub fn resume_playback() {
    if let Some(sink_arc) = SINK.get() {
        if let Ok(sink) = sink_arc.lock() {
            sink.play();
            info!("Rodio playback resumed");
        } else {
            warn!("Failed to lock Rodio sink for resuming playback");
        }
    } else {
        warn!("Rodio sink not initialized for resuming playback");
    }
}

/// Проверка состояния воспроизведения
pub fn is_paused() -> bool {
    if let Some(sink_arc) = SINK.get() {
        if let Ok(sink) = sink_arc.lock() {
            sink.is_paused()
        } else {
            false
        }
    } else {
        false
    }
}

/// Проверка, есть ли звуки в очереди
pub fn is_empty() -> bool {
    if let Some(sink_arc) = SINK.get() {
        if let Ok(sink) = sink_arc.lock() {
            sink.empty()
        } else {
            true
        }
    } else {
        true
    }
}

/// Получение текущей громкости
pub fn get_volume() -> f32 {
    if let Some(sink_arc) = SINK.get() {
        if let Ok(sink) = sink_arc.lock() {
            sink.volume()
        } else {
            0.0
        }
    } else {
        0.0
    }
}

/// Graceful shutdown Rodio системы
pub fn shutdown() {
    info!("Shutting down Rodio audio backend");

    // Останавливаем воспроизведение
    stop_playback();

    // Ждем завершения воспроизведения если есть активные треки
    if let Some(sink_arc) = SINK.get() {
        if let Ok(sink) = sink_arc.lock() {
            if !sink.empty() {
                info!("Waiting for Rodio playback to finish...");
                sink.sleep_until_end();
            }
        }
    }

    // Rodio автоматически освобождает ресурсы при drop
    // Дополнительной очистки не требуется

    info!("Rodio audio backend shutdown completed");
}

/// Проверка инициализации
pub fn is_initialized() -> bool {
    STREAM_HANDLE.get().is_some() && SINK.get().is_some()
}

/// Получение информации о Rodio бэкенде
pub fn get_info() -> serde_json::Value {
    let volume = get_volume();
    let is_playing = !is_empty();
    let paused = is_paused();

    serde_json::json!({
        "backend": "Rodio",
        "initialized": is_initialized(),
        "volume": volume,
        "is_playing": is_playing,
        "is_paused": paused,
        "features": [
            "Blocking/non-blocking playback",
            "Volume control",
            "Pause/resume support",
            "Multiple format support"
        ]
    })
}

/// Очистка очереди воспроизведения
pub fn clear_queue() {
    if let Some(sink_arc) = SINK.get() {
        if let Ok(sink) = sink_arc.lock() {
            sink.stop();
            info!("Rodio playback queue cleared");
        } else {
            warn!("Failed to lock Rodio sink for clearing queue");
        }
    } else {
        warn!("Rodio sink not initialized for clearing queue");
    }
}