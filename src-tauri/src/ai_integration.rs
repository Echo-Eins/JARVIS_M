// app/src/ai_integration.rs - Интеграция с OpenRouter API и AI сервисами

use std::collections::HashMap;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use reqwest::{Client, header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE}};
use tokio::time::timeout;

use crate::error::{JarvisResult, JarvisError};
use crate::{db, tts};
use log::{info, warn};
// Конфигурация AI сервисов
#[derive(Debug, Clone)]
pub struct AiConfig {
    pub openrouter_api_key: String,
    pub openai_api_key: String,
    pub preferred_model: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub timeout_seconds: u64,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            openrouter_api_key: String::new(),
            openai_api_key: String::new(),
            preferred_model: "anthropic/claude-3-haiku".to_string(),
            max_tokens: 1000,
            temperature: 0.7,
            timeout_seconds: 30,
        }
    }
}

// Структуры для OpenRouter API
#[derive(Debug, Serialize)]
struct OpenRouterRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: u32,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenRouterResponse {
    id: String,
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
    finish_reason: Option<String>,
    index: u32,
}

#[derive(Debug, Deserialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

// Типы AI запросов
#[derive(Debug, Clone)]
pub enum AiRequestType {
    Question(String),           // Простой вопрос
    Command(String),           // Команда для выполнения
    Conversation(String),      // Продолжение разговора
    DocumentRequest(String),   // Запрос документа
    Translation(String, String), // Перевод (текст, язык)
}

// Результат AI обработки
#[derive(Debug, Clone)]
pub struct AiResponse {
    pub text: String,
    pub response_type: AiResponseType,
    pub tokens_used: Option<u32>,
    pub model_used: String,
    pub confidence: f32,
}

#[derive(Debug, Clone)]
pub enum AiResponseType {
    TextResponse,              // Обычный текстовый ответ
    ActionCommand(String),     // Команда для выполнения
    DocumentPath(String),      // Путь к документу для открытия
    WebSearch(String),         // Поисковый запрос
    SystemControl(String),     // Системная команда
}

// Менеджер AI интеграции
pub struct AiManager {
    config: AiConfig,
    client: Client,
    conversation_history: Vec<Message>,
}

impl AiManager {
    pub fn new() -> JarvisResult<Self> {
        let config = Self::load_config()?;

        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .map_err(|e| JarvisError::Generic(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            config,
            client,
            conversation_history: Vec::new(),
        })
    }

    /// Загрузка конфигурации из настроек
    fn load_config() -> JarvisResult<AiConfig> {
        let mut config = AiConfig::default();

        if let Some(db) = db.get() {
            // Загружаем API ключи
            config.openai_api_key = db.api_keys.openai.clone();

            // Загружаем ключ OpenRouter (если добавлен в БД)
            // config.openrouter_api_key = db.api_keys.openrouter.clone();

            // Для демонстрации используем настройки по умолчанию
            // В реальном проекте эти настройки должны быть в БД
        }

        // Проверяем наличие хотя бы одного API ключа
        if config.openrouter_api_key.is_empty() && config.openai_api_key.is_empty() {
            return Err(JarvisError::Generic(
                "No AI API keys configured. Please set OpenRouter or OpenAI API key in settings.".to_string()
            ));
        }

        Ok(config)
    }

    /// Определение типа запроса на основе текста
    pub fn classify_request(&self, text: &str) -> AiRequestType {
        let text_lower = text.to_lowercase();

        // Ключевые слова для определения типа запроса
        if text_lower.contains("открой") || text_lower.contains("покажи") ||
            text_lower.contains("найди файл") || text_lower.contains("документ") {
            return AiRequestType::DocumentRequest(text.to_string());
        }

        if text_lower.contains("переведи") || text_lower.contains("перевод") {
            return AiRequestType::Translation(text.to_string(), "ru".to_string());
        }

        if text_lower.contains("выполни") || text_lower.contains("запусти") ||
            text_lower.contains("включи") || text_lower.contains("выключи") {
            return AiRequestType::Command(text.to_string());
        }

        // По умолчанию считаем обычным вопросом
        AiRequestType::Question(text.to_string())
    }

    /// Основная функция обработки AI запроса
    pub async fn process_request(&mut self, request: AiRequestType) -> JarvisResult<AiResponse> {
        match request {
            AiRequestType::Question(text) => self.handle_question(&text).await,
            AiRequestType::Command(text) => self.handle_command(&text).await,
            AiRequestType::Conversation(text) => self.handle_conversation(&text).await,
            AiRequestType::DocumentRequest(text) => self.handle_document_request(&text).await,
            AiRequestType::Translation(text, lang) => self.handle_translation(&text, &lang).await,
        }
    }

    /// Обработка обычного вопроса
    async fn handle_question(&mut self, question: &str) -> JarvisResult<AiResponse> {
        info!("Processing AI question: {}", question);

        let system_prompt = "Ты JARVIS - голосовой AI ассистент. Отвечай кратко и по делу на русском языке. \
                            Если вопрос требует выполнения команды системы, укажи это в ответе.";

        let messages = vec![
            Message {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            Message {
                role: "user".to_string(),
                content: question.to_string(),
            }
        ];

        let response = self.send_openrouter_request(messages).await?;

        // Добавляем в историю разговора
        self.conversation_history.push(Message {
            role: "user".to_string(),
            content: question.to_string(),
        });
        self.conversation_history.push(Message {
            role: "assistant".to_string(),
            content: response.text.clone(),
        });

        // Ограничиваем историю последними 10 сообщениями
        if self.conversation_history.len() > 10 {
            self.conversation_history.drain(0..self.conversation_history.len() - 10);
        }

        Ok(response)
    }

    /// Обработка команды
    async fn handle_command(&mut self, command: &str) -> JarvisResult<AiResponse> {
        info!("Processing AI command: {}", command);

        let system_prompt = "Ты JARVIS - голосовой AI ассистент. Пользователь просит выполнить команду. \
                            Определи, что именно нужно сделать, и дай конкретный ответ. \
                            Если это системная команда, начни ответ с 'SYSTEM_COMMAND:'. \
                            Если нужно открыть документ, начни с 'OPEN_DOCUMENT:'. \
                            Если нужен поиск в интернете, начни с 'WEB_SEARCH:'.";

        let messages = vec![
            Message {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            Message {
                role: "user".to_string(),
                content: command.to_string(),
            }
        ];

        let mut response = self.send_openrouter_request(messages).await?;

        // Определяем тип ответа по префиксу
        response.response_type = if response.text.starts_with("SYSTEM_COMMAND:") {
            let cmd = response.text.strip_prefix("SYSTEM_COMMAND:").unwrap_or("").trim();
            AiResponseType::SystemControl(cmd.to_string())
        } else if response.text.starts_with("OPEN_DOCUMENT:") {
            let path = response.text.strip_prefix("OPEN_DOCUMENT:").unwrap_or("").trim();
            AiResponseType::DocumentPath(path.to_string())
        } else if response.text.starts_with("WEB_SEARCH:") {
            let query = response.text.strip_prefix("WEB_SEARCH:").unwrap_or("").trim();
            AiResponseType::WebSearch(query.to_string())
        } else {
            AiResponseType::TextResponse
        };

        Ok(response)
    }

    /// Обработка продолжения разговора
    async fn handle_conversation(&mut self, text: &str) -> JarvisResult<AiResponse> {
        info!("Processing AI conversation: {}", text);

        let mut messages = vec![
            Message {
                role: "system".to_string(),
                content: "Ты JARVIS - голосовой AI ассистент. Продолжай разговор естественно.".to_string(),
            }
        ];

        // Добавляем историю разговора
        messages.extend(self.conversation_history.clone());

        // Добавляем новое сообщение
        messages.push(Message {
            role: "user".to_string(),
            content: text.to_string(),
        });

        let response = self.send_openrouter_request(messages).await?;

        // Обновляем историю
        self.conversation_history.push(Message {
            role: "user".to_string(),
            content: text.to_string(),
        });
        self.conversation_history.push(Message {
            role: "assistant".to_string(),
            content: response.text.clone(),
        });

        Ok(response)
    }

    /// Обработка запроса документа
    async fn handle_document_request(&mut self, request: &str) -> JarvisResult<AiResponse> {
        info!("Processing document request: {}", request);

        let system_prompt = "Пользователь просит найти или открыть документ. \
                            Определи, какой именно файл или тип файла нужен, и ответь в формате: \
                            'DOCUMENT_SEARCH: название_файла_или_тип' \
                            Например: 'DOCUMENT_SEARCH: презентация.pptx' или 'DOCUMENT_SEARCH: *.pdf'";

        let messages = vec![
            Message {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            Message {
                role: "user".to_string(),
                content: request.to_string(),
            }
        ];

        let response = self.send_openrouter_request(messages).await?;

        // Извлекаем поисковый запрос
        let search_query = if response.text.starts_with("DOCUMENT_SEARCH:") {
            response.text.strip_prefix("DOCUMENT_SEARCH:").unwrap_or("").trim().to_string()
        } else {
            request.to_string()
        };

        // Ищем документ в системе
        let found_documents = self.search_documents(&search_query)?;

        let final_response = if found_documents.is_empty() {
            AiResponse {
                text: format!("Документ '{}' не найден. Проверьте правильность названия.", search_query),
                response_type: AiResponseType::TextResponse,
                tokens_used: response.tokens_used,
                model_used: response.model_used,
                confidence: 0.5,
            }
        } else {
            let doc_path = &found_documents[0];
            AiResponse {
                text: format!("Открываю документ: {}", doc_path),
                response_type: AiResponseType::DocumentPath(doc_path.clone()),
                tokens_used: response.tokens_used,
                model_used: response.model_used,
                confidence: 0.9,
            }
        };

        Ok(final_response)
    }

    /// Обработка перевода
    async fn handle_translation(&mut self, text: &str, target_lang: &str) -> JarvisResult<AiResponse> {
        info!("Processing translation: {} -> {}", text, target_lang);

        let system_prompt = format!(
            "Переведи следующий текст на язык: {}. Отвечай только переводом без дополнительных комментариев.",
            target_lang
        );

        let messages = vec![
            Message {
                role: "system".to_string(),
                content: system_prompt,
            },
            Message {
                role: "user".to_string(),
                content: text.to_string(),
            }
        ];

        self.send_openrouter_request(messages).await
    }

    /// Отправка запроса к OpenRouter API
    async fn send_openrouter_request(&self, messages: Vec<Message>) -> JarvisResult<AiResponse> {
        // Выбираем API для использования
        let (api_url, api_key, model) = if !self.config.openrouter_api_key.is_empty() {
            (
                "https://openrouter.ai/api/v1/chat/completions",
                &self.config.openrouter_api_key,
                &self.config.preferred_model
            )
        } else if !self.config.openai_api_key.is_empty() {
            (
                "https://api.openai.com/v1/chat/completions",
                &self.config.openai_api_key,
                "gpt-3.5-turbo"
            )
        } else {
            return Err(JarvisError::Generic("No API key available".to_string()));
        };

        // Создаем заголовки
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", api_key))
                .map_err(|e| JarvisError::Generic(format!("Invalid API key format: {}", e)))?
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        // Если используем OpenRouter, добавляем дополнительные заголовки
        if api_url.contains("openrouter") {
            headers.insert(
                "HTTP-Referer",
                HeaderValue::from_static("https://github.com/jarvis-voice-assistant")
            );
            headers.insert(
                "X-Title",
                HeaderValue::from_static("JARVIS Voice Assistant")
            );
        }

        // Создаем тело запроса
        let request_body = OpenRouterRequest {
            model: model.to_string(),
            messages,
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
            stream: Some(false),
        };

        info!("Sending AI request to: {}", api_url);
        info!("Using model: {}", model);

        // Отправляем запрос с таймаутом
        let response = timeout(
            Duration::from_secs(self.config.timeout_seconds),
            self.client
                .post(api_url)
                .headers(headers)
                .json(&request_body)
                .send()
        ).await
            .map_err(|_| JarvisError::Generic("AI request timeout".to_string()))?
            .map_err(|e| JarvisError::Generic(format!("AI request failed: {}", e)))?;

        // Проверяем статус ответа
        if !response.status().is_success() {
            let error_text = response.text().await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(JarvisError::Generic(format!("AI API error {}: {}", response.status(), error_text)));
        }

        // Парсим ответ
        let ai_response: OpenRouterResponse = response
            .json()
            .await
            .map_err(|e| JarvisError::Generic(format!("Failed to parse AI response: {}", e)))?;

        // Извлекаем текст ответа
        let response_text = ai_response.choices
            .first()
            .map(|choice| choice.message.content.clone())
            .unwrap_or_else(|| "Не удалось получить ответ от AI".to_string());

        info!("AI response received: {} characters", response_text.len());

        Ok(AiResponse {
            text: response_text,
            response_type: AiResponseType::TextResponse,
            tokens_used: ai_response.usage.map(|u| u.total_tokens),
            model_used: model.to_string(),
            confidence: 0.8,
        })
    }

    /// Поиск документов в системе
    fn search_documents(&self, query: &str) -> JarvisResult<Vec<String>> {
        let mut found_documents = Vec::new();

        // Директории для поиска
        let search_dirs = vec![
            std::env::var("USERPROFILE").unwrap_or_else(|_| std::env::var("HOME").unwrap_or(".".to_string())),
            format!("{}/Documents", std::env::var("USERPROFILE").unwrap_or_else(|_| std::env::var("HOME").unwrap_or(".".to_string()))),
            format!("{}/Desktop", std::env::var("USERPROFILE").unwrap_or_else(|_| std::env::var("HOME").unwrap_or(".".to_string()))),
        ];

        // Поддерживаемые расширения документов
        let doc_extensions = vec![".pdf", ".docx", ".doc", ".pptx", ".ppt", ".xlsx", ".xls", ".txt"];

        for search_dir in search_dirs {
            if let Ok(entries) = std::fs::read_dir(&search_dir) {
                for entry in entries.flatten() {
                    if let Ok(file_name) = entry.file_name().into_string() {
                        // Проверяем, содержит ли имя файла поисковый запрос
                        if file_name.to_lowercase().contains(&query.to_lowercase()) {
                            // Проверяем расширение
                            if doc_extensions.iter().any(|ext| file_name.to_lowercase().ends_with(ext)) {
                                found_documents.push(entry.path().to_string_lossy().to_string());
                            }
                        }
                    }
                }
            }
        }

        // Ограничиваем количество результатов
        found_documents.truncate(5);

        info!("Found {} documents matching '{}'", found_documents.len(), query);
        Ok(found_documents)
    }

    /// Очистка истории разговора
    pub fn clear_conversation_history(&mut self) {
        self.conversation_history.clear();
        info!("Conversation history cleared");
    }

    /// Обновление конфигурации
    pub fn update_config(&mut self, new_config: AiConfig) -> JarvisResult<()> {
        self.config = new_config;
        info!("AI configuration updated");
        Ok(())
    }
}

// Глобальный AI менеджер
use once_cell::sync::OnceCell;
use tokio::sync::Mutex as AsyncMutex;

static AI_MANAGER: OnceCell<AsyncMutex<AiManager>> = OnceCell::new();

/// Инициализация AI системы
pub async fn init_ai() -> JarvisResult<()> {
    info!("Initializing AI integration...");

    let manager = AiManager::new()?;

    AI_MANAGER.set(AsyncMutex::new(manager))
        .map_err(|_| JarvisError::Generic("AI manager already initialized".to_string()))?;

    info!("AI integration initialized successfully");
    Ok(())
}

/// Обработка голосовой команды через AI
pub async fn process_voice_command(command: &str) -> JarvisResult<AiResponse> {
    let manager_mutex = AI_MANAGER.get()
        .ok_or_else(|| JarvisError::Generic("AI manager not initialized".to_string()))?;

    let mut manager = manager_mutex.lock().await;

    // Классифицируем запрос
    let request_type = manager.classify_request(command);

    // Обрабатываем запрос
    let response = manager.process_request(request_type).await?;

    info!("AI processed command: '{}' -> '{}'", command, response.text);

    // Озвучиваем ответ
    if let Err(e) = tts::speak(&response.text) {
        warn!("Failed to speak AI response: {}", e);
    }

    // Выполняем действие если нужно
    match &response.response_type {
        AiResponseType::DocumentPath(path) => {
            if let Err(e) = open_document(path) {
                warn!("Failed to open document {}: {}", path, e);
            }
        }
        AiResponseType::SystemControl(cmd) => {
            if let Err(e) = execute_system_command(cmd) {
                warn!("Failed to execute system command {}: {}", cmd, e);
            }
        }
        AiResponseType::WebSearch(query) => {
            if let Err(e) = perform_web_search(query) {
                warn!("Failed to perform web search {}: {}", query, e);
            }
        }
        _ => {}
    }

    Ok(response)
}

/// Открытие документа системным приложением
fn open_document(path: &str) -> JarvisResult<()> {
    info!("Opening document: {}", path);

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(&["/C", "start", "", path])
            .spawn()
            .map_err(|e| JarvisError::Generic(format!("Failed to open document: {}", e)))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| JarvisError::Generic(format!("Failed to open document: {}", e)))?;
    }

    Ok(())
}

/// Выполнение системной команды
fn execute_system_command(cmd: &str) -> JarvisResult<()> {
    info!("Executing system command: {}", cmd);

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(&["/C", cmd])
            .spawn()
            .map_err(|e| JarvisError::Generic(format!("Failed to execute command: {}", e)))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("sh")
            .args(&["-c", cmd])
            .spawn()
            .map_err(|e| JarvisError::Generic(format!("Failed to execute command: {}", e)))?;
    }

    Ok(())
}

/// Выполнение веб-поиска
fn perform_web_search(query: &str) -> JarvisResult<()> {
    info!("Performing web search: {}", query);

    let search_url = format!("https://www.google.com/search?q={}",
                             urlencoding::encode(query));

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(&["/C", "start", "", &search_url])
            .spawn()
            .map_err(|e| JarvisError::Generic(format!("Failed to open search: {}", e)))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&search_url)
            .spawn()
            .map_err(|e| JarvisError::Generic(format!("Failed to open search: {}", e)))?;
    }

    Ok(())
}

/// Graceful shutdown AI системы
pub async fn shutdown_ai() -> JarvisResult<()> {
    if let Some(manager_mutex) = AI_MANAGER.get() {
        let mut manager = manager_mutex.lock().await;
        manager.clear_conversation_history();
    }
    info!("AI system shutdown completed");
    Ok(())
}