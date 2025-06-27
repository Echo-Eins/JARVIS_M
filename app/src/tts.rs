// app/src/tts.rs - Полнофункциональный Text-to-Speech модуль

use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use std::io::{Write, BufReader};
use std::process::{Command, Stdio};
use once_cell::sync::OnceCell;

use crate::error::{JarvisResult, JarvisError, AudioError};
use crate::config;
use crate::{DB, APP_CONFIG_DIR};

// Поддерживаемые TTS движки
#[derive(Debug, Clone)]
pub enum TtsEngine {
    System,      // Системный TTS (SAPI на Windows, espeak на Linux)
    Silero,      // Локальный Silero TTS (будущая реализация)
    OpenAI,      // OpenAI TTS API
    ElevenLabs,  // ElevenLabs API (будущая реализация)
}

// Настройки голоса
#[derive(Debug, Clone)]
pub struct VoiceSettings {
    pub voice_id: String,
    pub speed: f32,        // 0.5 - 2.0
    pub pitch: f32,        // 0.5 - 2.0
    pub volume: f32,       // 0.0 - 1.0
    pub language: String,  // "ru-RU", "en-US", etc.
}

impl Default for VoiceSettings {
    fn default() -> Self {
        Self {
            voice_id: "default".to_string(),
            speed: 1.0,
            pitch: 1.0,
            volume: 0.8,
            language: "ru-RU".to_string(),
        }
    }
}

// Основная структура TTS
pub struct TtsManager {
    engine: TtsEngine,
    settings: VoiceSettings,
    is_speaking: Arc<Mutex<bool>>,
    cache_dir: PathBuf,
}

static TTS_MANAGER: OnceCell<Arc<Mutex<TtsManager>>> = OnceCell::new();

/// Инициализация TTS системы
pub fn init() -> JarvisResult<()> {
    info!("Initializing Text-to-Speech system...");

    // Определяем cache директорию
    let cache_dir = APP_CONFIG_DIR.get()
        .ok_or_else(|| JarvisError::AudioError(AudioError::InitializationFailed(
            "Config directory not initialized".to_string()
        )))?
        .join("tts_cache");

    // Создаем cache директорию
    std::fs::create_dir_all(&cache_dir)
        .map_err(|e| JarvisError::AudioError(AudioError::InitializationFailed(
            format!("Failed to create TTS cache directory: {}", e)
        )))?;

    // Выбираем лучший доступный движок
    let engine = detect_best_engine()?;
    info!("Selected TTS engine: {:?}", engine);

    // Получаем настройки голоса из конфигурации
    let settings = load_voice_settings()?;

    let tts_manager = TtsManager {
        engine,
        settings,
        is_speaking: Arc::new(Mutex::new(false)),
        cache_dir,
    };

    // Тестируем TTS
    test_tts(&tts_manager)?;

    // Сохраняем в глобальной переменной
    TTS_MANAGER.set(Arc::new(Mutex::new(tts_manager)))
        .map_err(|_| JarvisError::AudioError(AudioError::InitializationFailed(
            "TTS_MANAGER already initialized".to_string()
        )))?;

    info!("TTS system initialized successfully");
    Ok(())
}

/// Определение лучшего доступного TTS движка
fn detect_best_engine() -> JarvisResult<TtsEngine> {
    // Проверяем OpenAI API ключ
    if let Some(db) = DB.get() {
        if !db.api_keys.openai.trim().is_empty() {
            info!("OpenAI API key found, testing OpenAI TTS...");
            if test_openai_tts().is_ok() {
                return Ok(TtsEngine::OpenAI);
            }
        }
    }

    // Проверяем системный TTS
    if test_system_tts().is_ok() {
        return Ok(TtsEngine::System);
    }

    Err(JarvisError::AudioError(AudioError::InitializationFailed(
        "No working TTS engine found".to_string()
    )))
}

/// Тест системного TTS
fn test_system_tts() -> JarvisResult<()> {
    #[cfg(target_os = "windows")]
    {
        // Тестируем Windows SAPI
        let output = Command::new("powershell")
            .args(&["-Command", "Add-Type -AssemblyName System.Speech; $speak = New-Object System.Speech.Synthesis.SpeechSynthesizer; $speak.GetInstalledVoices().Count"])
            .output()
            .map_err(|e| JarvisError::AudioError(AudioError::InitializationFailed(
                format!("Failed to test Windows SAPI: {}", e)
            )))?;

        if output.status.success() {
            let voice_count = String::from_utf8_lossy(&output.stdout).trim().parse::<i32>().unwrap_or(0);
            if voice_count > 0 {
                info!("Windows SAPI available with {} voices", voice_count);
                return Ok(());
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Тестируем espeak
        let output = Command::new("espeak")
            .args(&["--version"])
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                info!("espeak available: {}", String::from_utf8_lossy(&output.stdout));
                return Ok(());
            }
        }

        // Тестируем festival
        let output = Command::new("festival")
            .args(&["--version"])
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                info!("festival available");
                return Ok(());
            }
        }
    }

    Err(JarvisError::AudioError(AudioError::InitializationFailed(
        "No system TTS found".to_string()
    )))
}

/// Тест OpenAI TTS
fn test_openai_tts() -> JarvisResult<()> {
    // Получаем API ключ
    let api_key = DB.get()
        .and_then(|db| {
            if db.api_keys.openai.trim().is_empty() {
                None
            } else {
                Some(db.api_keys.openai.clone())
            }
        })
        .ok_or_else(|| JarvisError::AudioError(AudioError::InitializationFailed(
            "OpenAI API key not found".to_string()
        )))?;

    // Простой тест API (здесь можно добавить реальный HTTP запрос)
    if api_key.len() < 20 {
        return Err(JarvisError::AudioError(AudioError::InitializationFailed(
            "OpenAI API key too short".to_string()
        )));
    }

    Ok(())
}

/// Загрузка настроек голоса
fn load_voice_settings() -> JarvisResult<VoiceSettings> {
    let mut settings = VoiceSettings::default();

    if let Some(db) = DB.get() {
        // Загружаем voice из настроек
        if !db.voice.is_empty() {
            settings.voice_id = db.voice.clone();
        }

        // Здесь можно добавить загрузку других настроек из БД
        // settings.speed = db.tts_speed.unwrap_or(1.0);
        // settings.pitch = db.tts_pitch.unwrap_or(1.0);
        // settings.volume = db.tts_volume.unwrap_or(0.8);
    }

    Ok(settings)
}

/// Тестирование TTS
fn test_tts(manager: &TtsManager) -> JarvisResult<()> {
    info!("Testing TTS with a sample phrase...");

    match manager.engine {
        TtsEngine::System => test_system_speech("Тест системы синтеза речи"),
        TtsEngine::OpenAI => {
            info!("OpenAI TTS test will be performed on first use");
            Ok(())
        }
        _ => Ok(())
    }
}

/// Тест системной речи
fn test_system_speech(text: &str) -> JarvisResult<()> {
    #[cfg(target_os = "windows")]
    {
        let ps_command = format!(
            "Add-Type -AssemblyName System.Speech; \
             $speak = New-Object System.Speech.Synthesis.SpeechSynthesizer; \
             $speak.Rate = 0; \
             $speak.Volume = 50; \
             $speak.Speak('{}')",
            text
        );

        let output = Command::new("powershell")
            .args(&["-Command", &ps_command])
            .output()
            .map_err(|e| JarvisError::AudioError(AudioError::PlaybackFailed(
                format!("Windows TTS test failed: {}", e)
            )))?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(JarvisError::AudioError(AudioError::PlaybackFailed(
                format!("Windows TTS error: {}", error)
            )));
        }
    }

    #[cfg(target_os = "linux")]
    {
        let output = Command::new("espeak")
            .args(&["-s", "150", "-v", "ru", text])
            .output()
            .map_err(|e| JarvisError::AudioError(AudioError::PlaybackFailed(
                format!("espeak test failed: {}", e)
            )))?;

        if !output.status.success() {
            // Пробуем festival
            let output = Command::new("sh")
                .args(&["-c", &format!("echo '{}' | festival --tts", text)])
                .output()
                .map_err(|e| JarvisError::AudioError(AudioError::PlaybackFailed(
                    format!("festival test failed: {}", e)
                )))?;

            if !output.status.success() {
                return Err(JarvisError::AudioError(AudioError::PlaybackFailed(
                    "Both espeak and festival failed".to_string()
                )));
            }
        }
    }

    info!("TTS test completed successfully");
    Ok(())
}

/// Главная функция синтеза речи
pub fn speak(text: &str) -> JarvisResult<()> {
    let manager_arc = TTS_MANAGER.get()
        .ok_or_else(|| JarvisError::AudioError(AudioError::InitializationFailed(
            "TTS not initialized".to_string()
        )))?;

    let manager = manager_arc.lock()
        .map_err(|e| JarvisError::AudioError(AudioError::PlaybackFailed(
            format!("Failed to lock TTS manager: {}", e)
        )))?;

    // Проверяем, не говорим ли мы уже
    {
        let is_speaking = manager.is_speaking.lock()
            .map_err(|e| JarvisError::AudioError(AudioError::PlaybackFailed(
                format!("Failed to check speaking status: {}", e)
            )))?;

        if *is_speaking {
            warn!("TTS already speaking, skipping new request");
            return Ok(());
        }
    }

    info!("Speaking: '{}'", text);

    // Отмечаем, что начинаем говорить
    {
        let mut is_speaking = manager.is_speaking.lock()
            .map_err(|e| JarvisError::AudioError(AudioError::PlaybackFailed(
                format!("Failed to set speaking status: {}", e)
            )))?;
        *is_speaking = true;
    }

    let result = match manager.engine {
        TtsEngine::System => speak_system(text, &manager.settings),
        TtsEngine::OpenAI => speak_openai(text, &manager.settings),
        _ => Err(JarvisError::AudioError(AudioError::PlaybackFailed(
            "TTS engine not implemented".to_string()
        )))
    };

    // Сбрасываем флаг говорения
    {
        let mut is_speaking = manager.is_speaking.lock()
            .map_err(|e| JarvisError::AudioError(AudioError::PlaybackFailed(
                format!("Failed to reset speaking status: {}", e)
            )))?;
        *is_speaking = false;
    }

    result
}

/// Системный TTS
fn speak_system(text: &str, settings: &VoiceSettings) -> JarvisResult<()> {
    #[cfg(target_os = "windows")]
    {
        let rate = ((settings.speed - 1.0) * 10.0) as i32; // -10 to 10
        let volume = (settings.volume * 100.0) as i32; // 0 to 100

        let voice_selection = if settings.voice_id != "default" {
            format!("$speak.SelectVoice('{}'); ", settings.voice_id)
        } else {
            String::new()
        };

        let ps_command = format!(
            "Add-Type -AssemblyName System.Speech; \
             $speak = New-Object System.Speech.Synthesis.SpeechSynthesizer; \
             {}$speak.Rate = {}; \
             $speak.Volume = {}; \
             $speak.Speak('{}')",
            voice_selection, rate, volume, text.replace("'", "''")
        );

        let output = Command::new("powershell")
            .args(&["-Command", &ps_command])
            .output()
            .map_err(|e| JarvisError::AudioError(AudioError::PlaybackFailed(
                format!("Windows TTS failed: {}", e)
            )))?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(JarvisError::AudioError(AudioError::PlaybackFailed(
                format!("Windows TTS error: {}", error)
            )));
        }
    }

    #[cfg(target_os = "linux")]
    {
        let speed = (settings.speed * 150.0) as i32; // words per minute
        let voice = if settings.language.starts_with("ru") { "ru" } else { "en" };

        let output = Command::new("espeak")
            .args(&[
                "-s", &speed.to_string(),
                "-v", voice,
                text
            ])
            .output()
            .map_err(|e| JarvisError::AudioError(AudioError::PlaybackFailed(
                format!("espeak failed: {}", e)
            )))?;

        if !output.status.success() {
            // Fallback to festival
            let output = Command::new("sh")
                .args(&["-c", &format!("echo '{}' | festival --tts", text)])
                .output()
                .map_err(|e| JarvisError::AudioError(AudioError::PlaybackFailed(
                    format!("festival fallback failed: {}", e)
                )))?;

            if !output.status.success() {
                return Err(JarvisError::AudioError(AudioError::PlaybackFailed(
                    "Both espeak and festival failed".to_string()
                )));
            }
        }
    }

    Ok(())
}

/// OpenAI TTS
fn speak_openai(text: &str, settings: &VoiceSettings) -> JarvisResult<()> {
    let api_key = DB.get()
        .and_then(|db| {
            if db.api_keys.openai.trim().is_empty() {
                None
            } else {
                Some(db.api_keys.openai.clone())
            }
        })
        .ok_or_else(|| JarvisError::AudioError(AudioError::PlaybackFailed(
            "OpenAI API key not found".to_string()
        )))?;

    // Создаем JSON payload для OpenAI TTS API
    let payload = serde_json::json!({
        "model": "tts-1",
        "input": text,
        "voice": "nova", // alloy, echo, fable, onyx, nova, shimmer
        "speed": settings.speed,
        "response_format": "mp3"
    });

    // HTTP запрос к OpenAI API
    let client = std::process::Command::new("curl")
        .args(&[
            "-X", "POST",
            "https://api.openai.com/v1/audio/speech",
            "-H", "Content-Type: application/json",
            "-H", &format!("Authorization: Bearer {}", api_key),
            "-d", &payload.to_string(),
            "--output", "-"
        ])
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| JarvisError::AudioError(AudioError::PlaybackFailed(
            format!("Failed to start curl: {}", e)
        )))?;

    // Воспроизводим аудио напрямую через ffplay или другой плеер
    let mut player = std::process::Command::new("ffplay")
        .args(&["-nodisp", "-autoexit", "-"])
        .stdin(Stdio::piped())
        .spawn()
        .or_else(|_| {
            // Fallback to mpv
            std::process::Command::new("mpv")
                .args(&["--no-video", "--really-quiet", "-"])
                .stdin(Stdio::piped())
                .spawn()
        })
        .map_err(|e| JarvisError::AudioError(AudioError::PlaybackFailed(
            format!("No audio player found (tried ffplay, mpv): {}", e)
        )))?;

    // Перенаправляем вывод curl в плеер
    if let (Some(curl_stdout), Some(player_stdin)) = (client.stdout, player.stdin.as_mut()) {
        std::io::copy(&mut BufReader::new(curl_stdout), player_stdin)
            .map_err(|e| JarvisError::AudioError(AudioError::PlaybackFailed(
                format!("Failed to pipe audio: {}", e)
            )))?;
    }

    // Ждем завершения воспроизведения
    let _ = player.wait();

    Ok(())
}

/// Остановка текущего синтеза речи
pub fn stop() -> JarvisResult<()> {
    let manager_arc = TTS_MANAGER.get()
        .ok_or_else(|| JarvisError::AudioError(AudioError::InitializationFailed(
            "TTS not initialized".to_string()
        )))?;

    let manager = manager_arc.lock()
        .map_err(|e| JarvisError::AudioError(AudioError::PlaybackFailed(
            format!("Failed to lock TTS manager: {}", e)
        )))?;

    // Сбрасываем флаг говорения
    {
        let mut is_speaking = manager.is_speaking.lock()
            .map_err(|e| JarvisError::AudioError(AudioError::PlaybackFailed(
                format!("Failed to reset speaking status: {}", e)
            )))?;
        *is_speaking = false;
    }

    // TODO: Добавить логику принудительной остановки воспроизведения
    // Это зависит от используемого движка

    info!("TTS stopped");
    Ok(())
}

/// Проверка, говорит ли TTS в данный момент
pub fn is_speaking() -> bool {
    if let Some(manager_arc) = TTS_MANAGER.get() {
        if let Ok(manager) = manager_arc.lock() {
            if let Ok(is_speaking) = manager.is_speaking.lock() {
                return *is_speaking;
            }
        }
    }
    false
}

/// Получение доступных голосов
pub fn get_available_voices() -> JarvisResult<Vec<String>> {
    let manager_arc = TTS_MANAGER.get()
        .ok_or_else(|| JarvisError::AudioError(AudioError::InitializationFailed(
            "TTS not initialized".to_string()
        )))?;

    let manager = manager_arc.lock()
        .map_err(|e| JarvisError::AudioError(AudioError::PlaybackFailed(
            format!("Failed to lock TTS manager: {}", e)
        )))?;

    match manager.engine {
        TtsEngine::System => get_system_voices(),
        TtsEngine::OpenAI => Ok(vec![
            "alloy".to_string(),
            "echo".to_string(),
            "fable".to_string(),
            "onyx".to_string(),
            "nova".to_string(),
            "shimmer".to_string(),
        ]),
        _ => Ok(vec!["default".to_string()])
    }
}

/// Получение системных голосов
fn get_system_voices() -> JarvisResult<Vec<String>> {
    let mut voices = Vec::new();

    #[cfg(target_os = "windows")]
    {
        let output = Command::new("powershell")
            .args(&["-Command",
                "Add-Type -AssemblyName System.Speech; \
                 $speak = New-Object System.Speech.Synthesis.SpeechSynthesizer; \
                 $speak.GetInstalledVoices() | ForEach-Object { $_.VoiceInfo.Name }"
            ])
            .output()
            .map_err(|e| JarvisError::AudioError(AudioError::InitializationFailed(
                format!("Failed to get Windows voices: {}", e)
            )))?;

        if output.status.success() {
            let voice_list = String::from_utf8_lossy(&output.stdout);
            voices = voice_list.lines()
                .map(|line| line.trim().to_string())
                .filter(|line| !line.is_empty())
                .collect();
        }
    }

    #[cfg(target_os = "linux")]
    {
        // espeak voices
        let output = Command::new("espeak")
            .args(&["--voices"])
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let voice_list = String::from_utf8_lossy(&output.stdout);
                for line in voice_list.lines().skip(1) { // skip header
                    if let Some(voice_name) = line.split_whitespace().nth(3) {
                        voices.push(voice_name.to_string());
                    }
                }
            }
        }
    }

    if voices.is_empty() {
        voices.push("default".to_string());
    }

    Ok(voices)
}

/// Обновление настроек TTS
pub fn update_settings(new_settings: VoiceSettings) -> JarvisResult<()> {
    let manager_arc = TTS_MANAGER.get()
        .ok_or_else(|| JarvisError::AudioError(AudioError::InitializationFailed(
            "TTS not initialized".to_string()
        )))?;

    let mut manager = manager_arc.lock()
        .map_err(|e| JarvisError::AudioError(AudioError::PlaybackFailed(
            format!("Failed to lock TTS manager: {}", e)
        )))?;

    manager.settings = new_settings;
    info!("TTS settings updated");
    Ok(())
}

/// Graceful shutdown TTS
pub fn shutdown() -> JarvisResult<()> {
    if let Some(manager_arc) = TTS_MANAGER.get() {
        if let Ok(manager) = manager_arc.lock() {
            // Останавливаем текущий синтез
            {
                let mut is_speaking = manager.is_speaking.lock()
                    .map_err(|e| JarvisError::AudioError(AudioError::PlaybackFailed(
                        format!("Failed to access speaking status: {}", e)
                    )))?;
                *is_speaking = false;
            }
        }
    }

    info!("TTS system shutdown completed");
    Ok(())
}