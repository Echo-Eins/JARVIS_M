// app/src/recorder.rs - Рефакторинг с улучшенной обработкой ошибок

pub mod pvrecorder;
// mod cpal;     // TODO: Implement later
// mod portaudio; // TODO: Implement later

use once_cell::sync::OnceCell;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::{DB, config, config::structs::RecorderType};
use crate::error::{JarvisResult, JarvisError, RecorderError};
use log::{info, warn, error};
static RECORDER_TYPE: OnceCell<RecorderType> = OnceCell::new();
static FRAME_LENGTH: OnceCell<u32> = OnceCell::new();
static IS_INITIALIZED: AtomicBool = AtomicBool::new(false);
static IS_RECORDING: AtomicBool = AtomicBool::new(false);

/// Инициализация записывающего устройства с улучшенной обработкой ошибок
pub fn init() -> JarvisResult<()> {
    // Проверяем, не инициализирован ли уже рекордер
    if IS_INITIALIZED.load(Ordering::SeqCst) {
        info!("Recorder already initialized");
        return Ok(());
    }

    // Устанавливаем тип рекордера (пока только PvRecorder реализован)
    let recorder_type = get_preferred_recorder_type()?;
    RECORDER_TYPE.set(recorder_type).map_err(|_| {
        JarvisError::RecorderError(RecorderError::InitializationFailed(
            "RECORDER_TYPE already set".to_string()
        ))
    })?;

    info!("Selected recorder type: {:?}", recorder_type);

    // Инициализируем выбранный рекордер
    match recorder_type {
        RecorderType::PvRecorder => {
            init_pvrecorder()?;
        },
        RecorderType::PortAudio => {
            return Err(JarvisError::RecorderError(RecorderError::InitializationFailed(
                "PortAudio recorder not yet implemented".to_string()
            )));
        },
        RecorderType::Cpal => {
            return Err(JarvisError::RecorderError(RecorderError::InitializationFailed(
                "CPAL recorder not yet implemented".to_string()
            )));
        }
    }

    IS_INITIALIZED.store(true, Ordering::SeqCst);
    info!("Recorder initialization completed successfully");
    Ok(())
}

/// Определение предпочтительного типа рекордера
fn get_preferred_recorder_type() -> JarvisResult<RecorderType> {
    // В будущем здесь можно добавить логику выбора лучшего доступного рекордера
    // На данный момент возвращаем PvRecorder как единственный реализованный
    Ok(config::DEFAULT_RECORDER_TYPE)
}

/// Инициализация PvRecorder
fn init_pvrecorder() -> JarvisResult<()> {
    info!("Initializing PvRecorder recording backend");

    // PvRecorder требует буфер кадров размером 512
    FRAME_LENGTH.set(512u32).map_err(|_| {
        JarvisError::RecorderError(RecorderError::InitializationFailed(
            "FRAME_LENGTH already set".to_string()
        ))
    })?;

    let microphone_index = get_selected_microphone_index()?;
    let frame_length = *FRAME_LENGTH.get().unwrap();

    // Инициализируем микрофон с обработкой ошибок
    match pvrecorder::init_microphone(microphone_index, frame_length) {
        Ok(()) => {
            info!("PvRecorder initialized successfully with microphone index: {}", microphone_index);
            Ok(())
        }
        Err(e) => {
            error!("PvRecorder initialization failed: {}", e);
            Err(JarvisError::RecorderError(RecorderError::InitializationFailed(
                format!("PvRecorder error: {}", e)
            )))
        }
    }
}

/// Проверка инициализации
fn ensure_initialized() -> JarvisResult<()> {
    if !IS_INITIALIZED.load(Ordering::SeqCst) {
        return Err(JarvisError::RecorderError(RecorderError::InitializationFailed(
            "Recorder not initialized. Call init() first".to_string()
        )));
    }
    Ok(())
}

/// Безопасное чтение данных с микрофона
pub fn read_microphone(frame_buffer: &mut [i16]) -> JarvisResult<()> {
    ensure_initialized()?;

    if !IS_RECORDING.load(Ordering::SeqCst) {
        return Err(JarvisError::RecorderError(RecorderError::RecordingFailed(
            "Recording not started. Call start_recording() first".to_string()
        )));
    }

    let recorder_type = RECORDER_TYPE.get()
        .ok_or_else(|| JarvisError::RecorderError(RecorderError::InitializationFailed(
            "Recorder type not set".to_string()
        )))?;

    match recorder_type {
        RecorderType::PvRecorder => {
            pvrecorder::read_microphone(frame_buffer)
                .map_err(|e| JarvisError::RecorderError(RecorderError::RecordingFailed(
                    format!("PvRecorder read error: {}", e)
                )))
        },
        RecorderType::PortAudio => {
            Err(JarvisError::RecorderError(RecorderError::InitializationFailed(
                "PortAudio not implemented".to_string()
            )))
        },
        RecorderType::Cpal => {
            Err(JarvisError::RecorderError(RecorderError::InitializationFailed(
                "CPAL should be used via callback assignment".to_string()
            )))
        }
    }
}

/// Начало записи с проверками
pub fn start_recording() -> JarvisResult<()> {
    ensure_initialized()?;

    if IS_RECORDING.load(Ordering::SeqCst) {
        warn!("Recording already in progress");
        return Ok(());
    }

    let recorder_type = RECORDER_TYPE.get()
        .ok_or_else(|| JarvisError::RecorderError(RecorderError::InitializationFailed(
            "Recorder type not set".to_string()
        )))?;

    let microphone_index = get_selected_microphone_index()?;
    let frame_length = get_frame_length()?;

    match recorder_type {
        RecorderType::PvRecorder => {
            pvrecorder::start_recording(microphone_index, frame_length)
                .map_err(|e| JarvisError::RecorderError(RecorderError::RecordingFailed(
                    format!("Failed to start PvRecorder: {}", e)
                )))?;
        },
        RecorderType::PortAudio => {
            return Err(JarvisError::RecorderError(RecorderError::InitializationFailed(
                "PortAudio not implemented".to_string()
            )));
        },
        RecorderType::Cpal => {
            return Err(JarvisError::RecorderError(RecorderError::InitializationFailed(
                "CPAL not implemented".to_string()
            )));
        }
    }

    IS_RECORDING.store(true, Ordering::SeqCst);
    info!("Recording started successfully");
    Ok(())
}

/// Остановка записи с проверками
pub fn stop_recording() -> JarvisResult<()> {
    ensure_initialized()?;

    if !IS_RECORDING.load(Ordering::SeqCst) {
        warn!("Recording not in progress");
        return Ok(());
    }

    let recorder_type = RECORDER_TYPE.get()
        .ok_or_else(|| JarvisError::RecorderError(RecorderError::InitializationFailed(
            "Recorder type not set".to_string()
        )))?;

    match recorder_type {
        RecorderType::PvRecorder => {
            pvrecorder::stop_recording()
                .map_err(|e| JarvisError::RecorderError(RecorderError::RecordingFailed(
                    format!("Failed to stop PvRecorder: {}", e)
                )))?;
        },
        RecorderType::PortAudio => {
            return Err(JarvisError::RecorderError(RecorderError::InitializationFailed(
                "PortAudio not implemented".to_string()
            )));
        },
        RecorderType::Cpal => {
            return Err(JarvisError::RecorderError(RecorderError::InitializationFailed(
                "CPAL not implemented".to_string()
            )));
        }
    }

    IS_RECORDING.store(false, Ordering::SeqCst);
    info!("Recording stopped successfully");
    Ok(())
}

/// Получение индекса выбранного микрофона с проверкой
pub fn get_selected_microphone_index() -> JarvisResult<i32> {
    let db = DB.get()
        .ok_or_else(|| JarvisError::RecorderError(RecorderError::InitializationFailed(
            "Database not initialized".to_string()
        )))?;

    Ok(db.microphone)
}

/// Получение длины кадра с проверкой
pub fn get_frame_length() -> JarvisResult<u32> {
    FRAME_LENGTH.get()
        .copied()
        .ok_or_else(|| JarvisError::RecorderError(RecorderError::InitializationFailed(
            "Frame length not set".to_string()
        )))
}

/// Проверка статуса записи
pub fn is_recording() -> bool {
    IS_RECORDING.load(Ordering::SeqCst)
}

/// Проверка инициализации
pub fn is_initialized() -> bool {
    IS_INITIALIZED.load(Ordering::SeqCst)
}

/// Получение доступных аудио устройств
pub fn get_available_devices() -> JarvisResult<Vec<(i32, String)>> {
    ensure_initialized()?;

    let recorder_type = RECORDER_TYPE.get()
        .ok_or_else(|| JarvisError::RecorderError(RecorderError::InitializationFailed(
            "Recorder type not set".to_string()
        )))?;

    match recorder_type {
        RecorderType::PvRecorder => {
            pvrecorder::get_audio_devices()
                .map_err(|e| JarvisError::RecorderError(RecorderError::InitializationFailed(
                    format!("Failed to get audio devices: {}", e)
                )))
        },
        _ => {
            Err(JarvisError::RecorderError(RecorderError::InitializationFailed(
                "Audio device enumeration not implemented for this recorder type".to_string()
            )))
        }
    }
}

/// Валидация микрофона по индексу
pub fn validate_microphone_index(index: i32) -> JarvisResult<bool> {
    let available_devices = get_available_devices()?;

    let is_valid = available_devices.iter().any(|(device_index, _)| *device_index == index);

    if !is_valid {
        warn!("Microphone with index {} not found in available devices", index);
        warn!("Available devices: {:?}", available_devices);
    }

    Ok(is_valid)
}

/// Graceful shutdown рекордера
pub fn shutdown() -> JarvisResult<()> {
    if IS_RECORDING.load(Ordering::SeqCst) {
        info!("Stopping recording before shutdown");
        stop_recording()?;
    }

    if IS_INITIALIZED.load(Ordering::SeqCst) {
        // Здесь можно добавить дополнительную логику очистки ресурсов
        IS_INITIALIZED.store(false, Ordering::SeqCst);
        info!("Recorder shutdown completed");
    }

    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_not_initialized_error() {
        // Сброс состояния для теста
        IS_INITIALIZED.store(false, Ordering::SeqCst);

        let mut buffer = vec![0i16; 512];
        let result = read_microphone(&mut buffer);

        assert!(result.is_err());
        if let Err(JarvisError::RecorderError(RecorderError::InitializationFailed(msg))) = result {
            assert!(msg.contains("not initialized"));
        } else {
            panic!("Expected InitializationFailed error");
        }
    }

    #[test]
    fn test_recording_not_started_error() {
        IS_INITIALIZED.store(true, Ordering::SeqCst);
        IS_RECORDING.store(false, Ordering::SeqCst);

        let mut buffer = vec![0i16; 512];
        let result = read_microphone(&mut buffer);

        assert!(result.is_err());
        if let Err(JarvisError::RecorderError(RecorderError::RecordingFailed(msg))) = result {
            assert!(msg.contains("Recording not started"));
        } else {
            panic!("Expected RecordingFailed error");
        }
    }
}