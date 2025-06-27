// app/src/errors.rs
use std::fmt;

/// Главный тип ошибки для всего приложения
#[derive(Debug)]
pub enum JarvisError {
    // Ошибки инициализации
    ConfigError(ConfigError),
    DatabaseError(DatabaseError),

    // Ошибки аудио системы
    RecorderError(RecorderError),
    AudioError(AudioError),

    // Ошибки распознавания
    ListenerError(ListenerError),
    SttError(SttError),

    // Ошибки команд
    CommandError(CommandError),

    // Системные ошибки
    IoError(std::io::Error),
    SerializationError(String),

    // Общие ошибки
    Generic(String),
}

/// Ошибки конфигурации
#[derive(Debug)]
pub enum ConfigError {
    DirectoryCreationFailed(String),
    InvalidConfiguration(String),
    MissingRequiredSetting(String),
    FileNotFound(String),
}

/// Ошибки базы данных
#[derive(Debug)]
pub enum DatabaseError {
    InitializationFailed(String),
    ReadError(String),
    WriteError(String),
    CorruptedData(String),
}

/// Ошибки записи аудио
#[derive(Debug)]
pub enum RecorderError {
    InitializationFailed(String),
    DeviceNotFound(i32),
    PermissionDenied,
    RecordingFailed(String),
    UnsupportedFormat,
    DeviceInUse,
}

/// Ошибки воспроизведения аудио
#[derive(Debug)]
pub enum AudioError {
    InitializationFailed(String),
    FileNotFound(String),
    UnsupportedFormat(String),
    PlaybackFailed(String),
    VolumeControlError,
}

/// Ошибки распознавания wake-word
#[derive(Debug)]
pub enum ListenerError {
    EngineInitializationFailed(String),
    ApiKeyMissing,
    ApiKeyInvalid,
    ModelLoadingFailed(String),
    ProcessingError(String),
    NetworkError(String),
}

/// Ошибки Speech-to-Text
#[derive(Debug)]
pub enum SttError {
    InitializationFailed(String),
    ModelNotFound(String),
    RecognitionFailed(String),
    UnsupportedLanguage(String),
}

/// Ошибки команд
#[derive(Debug)]
pub enum CommandError {
    ParseError(String),
    ExecutionFailed(String),
    CommandNotFound(String),
    InvalidArguments(String),
    PermissionDenied(String),
    Timeout,
}

impl fmt::Display for JarvisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JarvisError::ConfigError(e) => write!(f, "Configuration error: {}", e),
            JarvisError::DatabaseError(e) => write!(f, "Database error: {}", e),
            JarvisError::RecorderError(e) => write!(f, "Recorder error: {}", e),
            JarvisError::AudioError(e) => write!(f, "Audio error: {}", e),
            JarvisError::ListenerError(e) => write!(f, "Listener error: {}", e),
            JarvisError::SttError(e) => write!(f, "Speech-to-text error: {}", e),
            JarvisError::CommandError(e) => write!(f, "Command error: {}", e),
            JarvisError::IoError(e) => write!(f, "IO error: {}", e),
            JarvisError::SerializationError(e) => write!(f, "Serialization error: {}", e),
            JarvisError::Generic(e) => write!(f, "Error: {}", e),
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::DirectoryCreationFailed(path) =>
                write!(f, "Failed to create directory: {}", path),
            ConfigError::InvalidConfiguration(msg) =>
                write!(f, "Invalid configuration: {}", msg),
            ConfigError::MissingRequiredSetting(setting) =>
                write!(f, "Missing required setting: {}", setting),
            ConfigError::FileNotFound(file) =>
                write!(f, "Configuration file not found: {}", file),
        }
    }
}

impl fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DatabaseError::InitializationFailed(msg) =>
                write!(f, "Database initialization failed: {}", msg),
            DatabaseError::ReadError(key) =>
                write!(f, "Failed to read from database, key: {}", key),
            DatabaseError::WriteError(key) =>
                write!(f, "Failed to write to database, key: {}", key),
            DatabaseError::CorruptedData(msg) =>
                write!(f, "Database contains corrupted data: {}", msg),
        }
    }
}

impl fmt::Display for RecorderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecorderError::InitializationFailed(msg) =>
                write!(f, "Recorder initialization failed: {}", msg),
            RecorderError::DeviceNotFound(id) =>
                write!(f, "Audio device not found, ID: {}", id),
            RecorderError::PermissionDenied =>
                write!(f, "Permission denied to access microphone"),
            RecorderError::RecordingFailed(msg) =>
                write!(f, "Recording failed: {}", msg),
            RecorderError::UnsupportedFormat =>
                write!(f, "Unsupported audio format"),
            RecorderError::DeviceInUse =>
                write!(f, "Audio device is already in use"),
        }
    }
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioError::InitializationFailed(msg) =>
                write!(f, "Audio system initialization failed: {}", msg),
            AudioError::FileNotFound(file) =>
                write!(f, "Audio file not found: {}", file),
            AudioError::UnsupportedFormat(format) =>
                write!(f, "Unsupported audio format: {}", format),
            AudioError::PlaybackFailed(msg) =>
                write!(f, "Audio playback failed: {}", msg),
            AudioError::VolumeControlError =>
                write!(f, "Failed to control volume"),
        }
    }
}

impl fmt::Display for ListenerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ListenerError::EngineInitializationFailed(engine) =>
                write!(f, "Wake-word engine initialization failed: {}", engine),
            ListenerError::ApiKeyMissing =>
                write!(f, "API key is missing"),
            ListenerError::ApiKeyInvalid =>
                write!(f, "API key is invalid"),
            ListenerError::ModelLoadingFailed(model) =>
                write!(f, "Failed to load model: {}", model),
            ListenerError::ProcessingError(msg) =>
                write!(f, "Processing error: {}", msg),
            ListenerError::NetworkError(msg) =>
                write!(f, "Network error: {}", msg),
        }
    }
}

impl fmt::Display for SttError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SttError::InitializationFailed(msg) =>
                write!(f, "STT initialization failed: {}", msg),
            SttError::ModelNotFound(model) =>
                write!(f, "STT model not found: {}", model),
            SttError::RecognitionFailed(msg) =>
                write!(f, "Speech recognition failed: {}", msg),
            SttError::UnsupportedLanguage(lang) =>
                write!(f, "Unsupported language: {}", lang),
        }
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::ParseError(msg) =>
                write!(f, "Command parse error: {}", msg),
            CommandError::ExecutionFailed(cmd) =>
                write!(f, "Command execution failed: {}", cmd),
            CommandError::CommandNotFound(cmd) =>
                write!(f, "Command not found: {}", cmd),
            CommandError::InvalidArguments(args) =>
                write!(f, "Invalid arguments: {}", args),
            CommandError::PermissionDenied(cmd) =>
                write!(f, "Permission denied for command: {}", cmd),
            CommandError::Timeout =>
                write!(f, "Command execution timeout"),
        }
    }
}

impl std::error::Error for JarvisError {}
impl std::error::Error for ConfigError {}
impl std::error::Error for DatabaseError {}
impl std::error::Error for RecorderError {}
impl std::error::Error for AudioError {}
impl std::error::Error for ListenerError {}
impl std::error::Error for SttError {}
impl std::error::Error for CommandError {}

// Конверсии из стандартных ошибок
impl From<std::io::Error> for JarvisError {
    fn from(error: std::io::Error) -> Self {
        JarvisError::IoError(error)
    }
}

impl From<ConfigError> for JarvisError {
    fn from(error: ConfigError) -> Self {
        JarvisError::ConfigError(error)
    }
}

impl From<DatabaseError> for JarvisError {
    fn from(error: DatabaseError) -> Self {
        JarvisError::DatabaseError(error)
    }
}

impl From<RecorderError> for JarvisError {
    fn from(error: RecorderError) -> Self {
        JarvisError::RecorderError(error)
    }
}

impl From<AudioError> for JarvisError {
    fn from(error: AudioError) -> Self {
        JarvisError::AudioError(error)
    }
}

impl From<ListenerError> for JarvisError {
    fn from(error: ListenerError) -> Self {
        JarvisError::ListenerError(error)
    }
}

impl From<SttError> for JarvisError {
    fn from(error: SttError) -> Self {
        JarvisError::SttError(error)
    }
}

impl From<CommandError> for JarvisError {
    fn from(error: CommandError) -> Self {
        JarvisError::CommandError(error)
    }
}

// Макро для упрощения создания ошибок
#[macro_export]
macro_rules! jarvis_error {
    ($kind:ident, $msg:expr) => {
        JarvisError::$kind($msg.to_string())
    };
    ($kind:ident::$variant:ident, $msg:expr) => {
        JarvisError::$kind(crate::errors::$kind::$variant($msg.to_string()))
    };
}

// Тип Result для удобства
pub type JarvisResult<T> = Result<T, JarvisError>;