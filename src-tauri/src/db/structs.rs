// app/src/db/structs.rs - Обновленные структуры базы данных

use serde::{Deserialize, Serialize};
use crate::config;
use crate::config::structs::{WakeWordEngine, SpeechToTextEngine};

/// Основная структура настроек приложения
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Settings {
    // Аудио настройки
    pub microphone: i32,
    pub speaker: i32,
    pub voice: String,

    // Движки распознавания
    pub wake_word_engine: WakeWordEngine,
    pub speech_to_text_engine: SpeechToTextEngine,

    // API ключи
    pub api_keys: ApiKeys,

    // AI настройки
    pub ai_config: AiConfig,

    // TTS настройки
    pub tts_config: TtsConfig,

    // Дополнительные настройки
    pub advanced_settings: AdvancedSettings,

    // Метаданные
    pub version: String,
    pub last_updated: Option<chrono::DateTime<chrono::Utc>>,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            microphone: -1,
            speaker: -1,
            voice: config::DEFAULT_VOICE.to_string(),

            wake_word_engine: config::DEFAULT_WAKE_WORD_ENGINE,
            speech_to_text_engine: config::DEFAULT_SPEECH_TO_TEXT_ENGINE,

            api_keys: ApiKeys::default(),
            ai_config: AiConfig::default(),
            tts_config: TtsConfig::default(),
            advanced_settings: AdvancedSettings::default(),

            version: config::APP_VERSION.unwrap_or("unknown").to_string(),
            last_updated: Some(chrono::Utc::now()),
        }
    }
}

/// API ключи для различных сервисов
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ApiKeys {
    pub picovoice: String,
    pub openai: String,
    pub openrouter: String,
    pub elevenlabs: String,  // Для будущего использования
}

impl Default for ApiKeys {
    fn default() -> Self {
        Self {
            picovoice: String::new(),
            openai: String::new(),
            openrouter: String::new(),
            elevenlabs: String::new(),
        }
    }
}

/// Настройки AI системы
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AiConfig {
    pub preferred_model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub timeout_seconds: u64,
    pub enable_conversation_mode: bool,
    pub conversation_history_limit: usize,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            preferred_model: "anthropic/claude-3-haiku".to_string(),
            temperature: 0.7,
            max_tokens: 1000,
            timeout_seconds: 30,
            enable_conversation_mode: false,
            conversation_history_limit: 10,
        }
    }
}

/// Настройки синтеза речи
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TtsConfig {
    pub engine: TtsEngine,
    pub voice_id: String,
    pub speed: f32,
    pub pitch: f32,
    pub volume: f32,
    pub language: String,
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            engine: TtsEngine::System,
            voice_id: "default".to_string(),
            speed: 1.0,
            pitch: 1.0,
            volume: 0.8,
            language: "ru-RU".to_string(),
        }
    }
}

/// Типы TTS движков
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum TtsEngine {
    System,
    OpenAI,
    ElevenLabs,
    Local,  // Для будущих локальных движков
}

/// Продвинутые настройки
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AdvancedSettings {
    // Функциональность
    pub enable_document_search: bool,
    pub auto_open_documents: bool,
    pub device_monitoring: bool,
    pub enable_tray_icon: bool,

    // Производительность
    pub audio_buffer_size: usize,
    pub processing_threads: usize,
    pub enable_gpu_acceleration: bool,

    // Безопасность
    pub encrypt_api_keys: bool,
    pub enable_logging: bool,
    pub log_level: LogLevel,

    // UI настройки
    pub theme: UiTheme,
    pub language: String,
    pub startup_behavior: StartupBehavior,

    // Экспериментальные функции
    pub experimental_features: ExperimentalFeatures,
}

impl Default for AdvancedSettings {
    fn default() -> Self {
        Self {
            enable_document_search: true,
            auto_open_documents: true,
            device_monitoring: true,
            enable_tray_icon: true,

            audio_buffer_size: 512,
            processing_threads: 2,
            enable_gpu_acceleration: false,

            encrypt_api_keys: true,
            enable_logging: true,
            log_level: LogLevel::Info,

            theme: UiTheme::Dark,
            language: "ru".to_string(),
            startup_behavior: StartupBehavior::StartListening,

            experimental_features: ExperimentalFeatures::default(),
        }
    }
}

/// Уровни логирования
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

/// Темы интерфейса
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum UiTheme {
    Light,
    Dark,
    Auto,
}

/// Поведение при запуске
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum StartupBehavior {
    StartListening,
    OpenSettings,
    Minimized,
    Hidden,
}

/// Экспериментальные функции
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExperimentalFeatures {
    pub enable_local_ai: bool,
    pub enable_voice_training: bool,
    pub enable_emotion_detection: bool,
    pub enable_multi_language: bool,
    pub enable_plugin_system: bool,
}

impl Default for ExperimentalFeatures {
    fn default() -> Self {
        Self {
            enable_local_ai: false,
            enable_voice_training: false,
            enable_emotion_detection: false,
            enable_multi_language: false,
            enable_plugin_system: false,
        }
    }
}

/// Статистика использования (не сохраняется, только для runtime)
#[derive(Debug, Clone)]
pub struct UsageStats {
    pub total_commands: u64,
    pub successful_commands: u64,
    pub failed_commands: u64,
    pub total_ai_requests: u64,
    pub total_tts_requests: u64,
    pub session_start_time: chrono::DateTime<chrono::Utc>,
    pub last_activity: Option<chrono::DateTime<chrono::Utc>>,
}

impl Default for UsageStats {
    fn default() -> Self {
        Self {
            total_commands: 0,
            successful_commands: 0,
            failed_commands: 0,
            total_ai_requests: 0,
            total_tts_requests: 0,
            session_start_time: chrono::Utc::now(),
            last_activity: None,
        }
    }
}

/// Вспомогательные методы для Settings
impl Settings {
    /// Обновление времени последнего изменения
    pub fn touch(&mut self) {
        self.last_updated = Some(chrono::Utc::now());
    }

    /// Проверка корректности настроек
    pub fn validate(&self) -> Result<(), String> {
        // Проверка AI настроек
        if self.ai_config.temperature < 0.0 || self.ai_config.temperature > 2.0 {
            return Err("AI temperature must be between 0.0 and 2.0".to_string());
        }

        if self.ai_config.max_tokens == 0 || self.ai_config.max_tokens > 10000 {
            return Err("AI max_tokens must be between 1 and 10000".to_string());
        }

        // Проверка TTS настроек
        if self.tts_config.speed < 0.1 || self.tts_config.speed > 3.0 {
            return Err("TTS speed must be between 0.1 and 3.0".to_string());
        }

        if self.tts_config.volume < 0.0 || self.tts_config.volume > 1.0 {
            return Err("TTS volume must be between 0.0 and 1.0".to_string());
        }

        Ok(())
    }

    /// Получение активного AI API ключа
    pub fn get_active_ai_key(&self) -> Option<(&str, &str)> {
        if !self.api_keys.openrouter.is_empty() {
            Some(("openrouter", &self.api_keys.openrouter))
        } else if !self.api_keys.openai.is_empty() {
            Some(("openai", &self.api_keys.openai))
        } else {
            None
        }
    }

    /// Проверка наличия необходимых API ключей
    pub fn has_required_keys(&self) -> bool {
        !self.api_keys.openrouter.is_empty() || !self.api_keys.openai.is_empty()
    }

    /// Получение настроек для экспорта (без API ключей)
    pub fn get_exportable_settings(&self) -> Settings {
        let mut exported = self.clone();
        exported.api_keys = ApiKeys::default();
        exported
    }

    /// Слияние настроек с импортированными (сохраняя API ключи)
    pub fn merge_with_imported(&mut self, imported: Settings) {
        // Сохраняем текущие API ключи
        let current_keys = self.api_keys.clone();

        // Применяем импортированные настройки
        *self = imported;

        // Восстанавливаем API ключи если они не были в импорте
        if self.api_keys.openai.is_empty() && !current_keys.openai.is_empty() {
            self.api_keys.openai = current_keys.openai;
        }
        if self.api_keys.openrouter.is_empty() && !current_keys.openrouter.is_empty() {
            self.api_keys.openrouter = current_keys.openrouter;
        }
        if self.api_keys.picovoice.is_empty() && !current_keys.picovoice.is_empty() {
            self.api_keys.picovoice = current_keys.picovoice;
        }

        self.touch();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_default() {
        let settings = Settings::default();
        assert_eq!(settings.microphone, -1);
        assert_eq!(settings.ai_config.temperature, 0.7);
        assert!(settings.version.len() > 0);
    }

    #[test]
    fn test_settings_validation() {
        let mut settings = Settings::default();
        assert!(settings.validate().is_ok());

        settings.ai_config.temperature = -1.0;
        assert!(settings.validate().is_err());

        settings.ai_config.temperature = 0.7;
        settings.tts_config.volume = 2.0;
        assert!(settings.validate().is_err());
    }

    #[test]
    fn test_api_key_detection() {
        let mut settings = Settings::default();
        assert!(!settings.has_required_keys());

        settings.api_keys.openai = "test-key".to_string();
        assert!(settings.has_required_keys());
        assert_eq!(settings.get_active_ai_key(), Some(("openai", "test-key")));

        settings.api_keys.openrouter = "openrouter-key".to_string();
        assert_eq!(settings.get_active_ai_key(), Some(("openrouter", "openrouter-key")));
    }

    #[test]
    fn test_exportable_settings() {
        let mut settings = Settings::default();
        settings.api_keys.openai = "secret-key".to_string();

        let exportable = settings.get_exportable_settings();
        assert!(exportable.api_keys.openai.is_empty());
        assert_eq!(exportable.ai_config.temperature, settings.ai_config.temperature);
    }

    #[test]
    fn test_settings_merge() {
        let mut current = Settings::default();
        current.api_keys.openai = "existing-key".to_string();

        let mut imported = Settings::default();
        imported.ai_config.temperature = 0.9;
        // API ключ не указан в импорте

        current.merge_with_imported(imported);

        assert_eq!(current.ai_config.temperature, 0.9);
        assert_eq!(current.api_keys.openai, "existing-key"); // Ключ сохранился
    }
}