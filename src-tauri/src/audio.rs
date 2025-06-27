// app/src/audio.rs - Исправленный модуль аудио с недостающими функциями

mod rodio;
mod kira;

use std::cmp::Ordering;
use std::path::PathBuf;
use once_cell::sync::OnceCell;

use crate::{config, DB, SOUND_DIR};
use crate::config::structs::AudioType;
use crate::error::{JarvisResult, JarvisError, AudioError};


static AUDIO_TYPE: OnceCell<AudioType> = OnceCell::new();

/// Инициализация аудио системы
pub fn init() -> JarvisResult<()> {
    if AUDIO_TYPE.get().is_some() {
        info!("Audio system already initialized");
        return Ok(());
    }

    // Устанавливаем тип аудио системы
    let audio_type = config::DEFAULT_AUDIO_TYPE;
    AUDIO_TYPE.set(audio_type).map_err(|_| {
        JarvisError::AudioError(AudioError::InitializationFailed(
            "Audio type already set".to_string()
        ))
    })?;

    info!("Initializing audio backend: {:?}", audio_type);

    // Загружаем выбранный аудио бэкенд
    match audio_type {
        AudioType::Rodio => {
            info!("Initializing Rodio audio backend");
            rodio::init().map_err(|_| {
                JarvisError::AudioError(AudioError::InitializationFailed(
                    "Failed to initialize Rodio audio backend".to_string()
                ))
            })?;
            info!("Successfully initialized Rodio audio backend");
        },
        AudioType::Kira => {
            info!("Initializing Kira audio backend");
            kira::init().map_err(|_| {
                JarvisError::AudioError(AudioError::InitializationFailed(
                    "Failed to initialize Kira audio backend".to_string()
                ))
            })?;
            info!("Successfully initialized Kira audio backend");
        }
    }

    Ok(())
}

/// Воспроизведение звукового файла
pub fn play_sound(filename: &PathBuf) -> JarvisResult<()> {
    let audio_type = AUDIO_TYPE.get()
        .ok_or_else(|| JarvisError::AudioError(AudioError::InitializationFailed(
            "Audio system not initialized".to_string()
        )))?;

    if !filename.exists() {
        return Err(JarvisError::AudioError(AudioError::FileNotFound(
            filename.to_string_lossy().to_string()
        )));
    }

    info!("Playing sound: {}", filename.display());

    match audio_type {
        AudioType::Rodio => {
            rodio::play_sound(filename, true);
        },
        AudioType::Kira => {
            kira::play_sound(filename);
        }
    }

    Ok(())
}

/// Получение директории со звуками
/// Получение директории со звуками
pub fn get_sound_directory() -> JarvisResult<PathBuf> {
    // Сначала пытаемся получить из ресурсов
    if let Ok(resource_sound_dir) = config::get_sound_directory() {
        if resource_sound_dir.exists() {
            return Ok(resource_sound_dir);
        }
    }

    // Fallback для режима разработки
    let fallback_sound_dir = SOUND_DIR.get()
        .ok_or_else(|| JarvisError::AudioError(AudioError::InitializationFailed(
            "Sound directory not initialized".to_string()
        )))?;

    if fallback_sound_dir.exists() {
        Ok(fallback_sound_dir.clone())
    } else {
        Err(JarvisError::AudioError(AudioError::FileNotFound(
            format!("Sound directory not found: {}", fallback_sound_dir.display())
        )))
    }
}

/// Проверка инициализации аудио системы
pub fn is_initialized() -> bool {
    AUDIO_TYPE.get().is_some()
}

/// Установка громкости (если поддерживается бэкендом)
pub fn set_volume(volume: f32) -> JarvisResult<()> {
    let audio_type = AUDIO_TYPE.get()
        .ok_or_else(|| JarvisError::AudioError(AudioError::InitializationFailed(
            "Audio system not initialized".to_string()
        )))?;

    let clamped_volume = volume.clamp(0.0, 1.0);
    info!("Setting audio volume to: {:.2}", clamped_volume);

    match audio_type {
        AudioType::Rodio => {
            rodio::set_volume(clamped_volume);
        },
        AudioType::Kira => {
            kira::set_volume(clamped_volume);
        }
    }

    Ok(())
}

/// Остановка воспроизведения
pub fn stop_playback() -> JarvisResult<()> {
    let audio_type = AUDIO_TYPE.get()
        .ok_or_else(|| JarvisError::AudioError(AudioError::InitializationFailed(
            "Audio system not initialized".to_string()
        )))?;

    info!("Stopping audio playback");

    match audio_type {
        AudioType::Rodio => {
            rodio::stop_playback();
        },
        AudioType::Kira => {
            kira::stop_playback();
        }
    }

    Ok(())
}

/// Проверка возможности воспроизведения файла
pub fn can_play_file(filename: &PathBuf) -> bool {
    if !filename.exists() {
        return false;
    }

    // Проверяем расширение файла
    if let Some(extension) = filename.extension() {
        let ext_str = extension.to_string_lossy().to_lowercase();
        matches!(ext_str.as_str(), "wav" | "mp3" | "ogg" | "flac" | "m4a")
    } else {
        false
    }
}

/// Получение списка поддерживаемых аудио форматов
pub fn get_supported_formats() -> Vec<String> {
    vec![
        "wav".to_string(),
        "mp3".to_string(),
        "ogg".to_string(),
        "flac".to_string(),
        "m4a".to_string(),
    ]
}

/// Воспроизведение звука с настройками
pub fn play_sound_with_settings(
    filename: &PathBuf,
    volume: Option<f32>,
    blocking: Option<bool>
) -> JarvisResult<()> {
    let audio_type = AUDIO_TYPE.get()
        .ok_or_else(|| JarvisError::AudioError(AudioError::InitializationFailed(
            "Audio system not initialized".to_string()
        )))?;

    if !filename.exists() {
        return Err(JarvisError::AudioError(AudioError::FileNotFound(
            filename.to_string_lossy().to_string()
        )));
    }

    // Устанавливаем громкость если указана
    if let Some(vol) = volume {
        set_volume(vol)?;
    }

    let is_blocking = blocking.unwrap_or(false);

    info!("Playing sound: {} (blocking: {})", filename.display(), is_blocking);

    match audio_type {
        AudioType::Rodio => {
            rodio::play_sound(filename, is_blocking);
        },
        AudioType::Kira => {
            kira::play_sound(filename);
            // Kira не поддерживает блокирующее воспроизведение напрямую
            if is_blocking {
                // Можно добавить логику ожидания завершения
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
    }

    Ok(())
}

/// Graceful shutdown аудио системы
pub fn shutdown() -> JarvisResult<()> {
    info!("Shutting down audio system...");

    if let Some(audio_type) = AUDIO_TYPE.get() {
        // Останавливаем воспроизведение
        if let Err(e) = stop_playback() {
            warn!("Failed to stop playback during shutdown: {}", e);
        }

        // Очищаем ресурсы в зависимости от бэкенда
        match audio_type {
            AudioType::Rodio => {
                rodio::shutdown();
            },
            AudioType::Kira => {
                kira::shutdown();
            }
        }
    }

    info!("Audio system shutdown completed");
    Ok(())
}

/// Получение информации об аудио системе
pub fn get_audio_info() -> serde_json::Value {
    serde_json::json!({
        "initialized": is_initialized(),
        "backend": AUDIO_TYPE.get().map(|t| format!("{:?}", t)).unwrap_or("None".to_string()),
        "sound_directory": get_sound_directory().map(|p| p.to_string_lossy().to_string()).unwrap_or("Error".to_string()),
        "supported_formats": get_supported_formats(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn test_supported_formats() {
        let formats = get_supported_formats();
        assert!(formats.contains(&"wav".to_string()));
        assert!(formats.contains(&"mp3".to_string()));
    }

    #[test]
    fn test_can_play_file() {
        let temp_dir = tempdir().unwrap();
        let wav_file = temp_dir.path().join("test.wav");
        File::create(&wav_file).unwrap();

        assert!(can_play_file(&wav_file));

        let invalid_file = temp_dir.path().join("test.txt");
        File::create(&invalid_file).unwrap();

        assert!(!can_play_file(&invalid_file));
    }

    #[test]
    fn test_audio_info() {
        let info = get_audio_info();
        assert!(info.get("initialized").is_some());
        assert!(info.get("backend").is_some());
        assert!(info.get("supported_formats").is_some());
    }
}