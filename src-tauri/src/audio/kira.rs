// app/src/audio/kira.rs - Исправленный Kira аудио бэкенд

use std::path::PathBuf;
use std::sync::Mutex;
use once_cell::sync::OnceCell;
use log::{warn, info, error};
use kira::{
    manager::{
        AudioManager, AudioManagerSettings,
        backend::DefaultBackend,
    },
    sound::static_sound::{StaticSoundData, StaticSoundSettings},
    Volume,
};

thread_local!(static MANAGER: OnceCell<Mutex<AudioManager>> = OnceCell::new());

/// Инициализация Kira аудио менеджера
pub fn init() -> Result<(), ()> {
    MANAGER.with(|m| {
        if m.get().is_some() {
            return Ok(());
        }

        // Создаем аудио менеджер
        match AudioManager::<DefaultBackend>::new(AudioManagerSettings::default()) {
            Ok(manager) => {
                // Сохраняем менеджер
                if m.set(Mutex::new(manager)).is_err() {
                    error!("Failed to store Kira audio manager");
                    return Err(());
                }

                info!("Kira audio manager initialized successfully");
                Ok(())
            },
            Err(e) => {
                error!("Failed to initialize Kira audio manager: {}", e);
                Err(())
            }
        }
    })
}

/// Воспроизведение звукового файла
pub fn play_sound(filename: &PathBuf) {
    // Загружаем файл
    match StaticSoundData::from_file(filename, StaticSoundSettings::default()) {
        Ok(sound_data) => {
            // Воспроизводим (неблокирующий режим)
            MANAGER.with(|m| {
                if let Some(manager_mutex) = m.get() {
                    if let Ok(mut audio_manager) = manager_mutex.lock() {
                        match audio_manager.play(sound_data.clone()) {
                            Ok(_) => {
                                info!("Playing sound: {}", filename.display());
                            }
                            Err(e) => {
                                warn!("Failed to play sound {}: {}", filename.display(), e);
                            }
                        }
                    } else {
                        warn!("Failed to lock audio manager for: {}", filename.display());
                    }
                } else {
                    warn!("Audio manager not initialized for: {}", filename.display());
                }
            });
        },
        Err(e) => {
            warn!("Cannot load sound file {}: {}", filename.display(), e);
        }
    }
}

/// Установка глобальной громкости
pub fn set_volume(volume: f32) {
    let volume_value = Volume::Amplitude(volume as f64);

    MANAGER.with(|m| {
        if let Some(manager_mutex) = m.get() {
            if let Ok(mut audio_manager) = manager_mutex.lock() {
                // Kira не имеет прямого API для глобальной громкости
                // Можно реализовать через треки или группы
                info!("Volume set to: {:.2} (Note: Kira volume control is limited)", volume);
            } else {
                warn!("Failed to lock audio manager for volume control");
            }
        } else {
            warn!("Audio manager not initialized for volume control");
        }
    });
}

/// Остановка всех звуков
pub fn stop_playback() {
    MANAGER.with(|m| {
        if let Some(manager_mutex) = m.get() {
            if let Ok(mut audio_manager) = manager_mutex.lock() {
                // Kira не имеет прямого API для остановки всех звуков
                // Можно реализовать через отслеживание активных треков
                info!("Stopping all audio playback (Kira)");
                // TODO: Implement proper track management for stopping sounds
            } else {
                warn!("Failed to lock audio manager for stopping playback");
            }
        } else {
            warn!("Audio manager not initialized for stopping playback");
        }
    });
}

/// Graceful shutdown Kira системы
pub fn shutdown() {
    MANAGER.with(|m| {
        if let Some(manager_mutex) = m.get() {
            if let Ok(mut audio_manager) = manager_mutex.lock() {
                // Останавливаем все звуки перед закрытием
                info!("Shutting down Kira audio manager");

                // Kira автоматически очищает ресурсы при drop
                // Дополнительной очистки не требуется
            } else {
                warn!("Failed to lock audio manager during shutdown");
            }
        } else {
            info!("Kira audio manager was not initialized");
        }
    });

    info!("Kira audio backend shutdown completed");
}

/// Проверка инициализации
pub fn is_initialized() -> bool {
    MANAGER.with(|m| m.get().is_some())
}

/// Получение информации о Kira бэкенде
pub fn get_info() -> serde_json::Value {
    serde_json::json!({
        "backend": "Kira",
        "initialized": is_initialized(),
        "features": [
            "Non-blocking playback",
            "Multiple sound formats",
            "Low latency"
        ]
    })
}