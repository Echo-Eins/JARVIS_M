// app/src/document_search.rs - Модуль поиска и управления документами

use std::path::{Path, PathBuf};
use std::fs;
use std::process::Command;
use std::collections::HashMap;

use crate::error::{JarvisResult, JarvisError};
use log::{info, warn};
use tempfile::*;
// Поддерживаемые типы документов
const DOCUMENT_EXTENSIONS: &[&str] = &[
    ".pdf", ".doc", ".docx", ".txt", ".rtf",
    ".ppt", ".pptx", ".xls", ".xlsx", ".csv",
    ".odt", ".ods", ".odp", ".pages", ".numbers", ".key"
];

// Поддерживаемые изображения
const IMAGE_EXTENSIONS: &[&str] = &[
    ".jpg", ".jpeg", ".png", ".gif", ".bmp", ".tiff", ".svg", ".webp"
];

// Поддерживаемые видео
const VIDEO_EXTENSIONS: &[&str] = &[
    ".mp4", ".avi", ".mkv", ".mov", ".wmv", ".flv", ".webm", ".m4v"
];

// Поддерживаемые аудио
const AUDIO_EXTENSIONS: &[&str] = &[
    ".mp3", ".wav", ".flac", ".aac", ".ogg", ".wma", ".m4a"
];

/// Структура найденного файла
#[derive(Debug, Clone)]
pub struct FoundDocument {
    pub path: PathBuf,
    pub name: String,
    pub extension: String,
    pub size: u64,
    pub modified: Option<std::time::SystemTime>,
    pub file_type: DocumentType,
}

/// Типы документов
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DocumentType {
    Document,
    Image,
    Video,
    Audio,
    Other,
}

/// Инициализация модуля поиска документов
pub fn init() -> JarvisResult<()> {
    info!("Initializing document search module...");

    // Проверяем доступность системных команд для открытия файлов
    validate_system_commands()?;

    info!("Document search module initialized successfully");
    Ok(())
}

/// Проверка доступности системных команд
fn validate_system_commands() -> JarvisResult<()> {
    #[cfg(target_os = "windows")]
    {
        // На Windows всегда доступны start команды
        Ok(())
    }

    #[cfg(target_os = "linux")]
    {
        // Проверяем наличие xdg-open
        let output = Command::new("which")
            .arg("xdg-open")
            .output();

        match output {
            Ok(result) if result.status.success() => Ok(()),
            _ => {
                warn!("xdg-open not found, document opening may not work");
                Ok(()) // Не критическая ошибка
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        // На macOS всегда доступна команда open
        Ok(())
    }
}

/// Основная функция поиска файлов
pub fn search_files(query: &str) -> JarvisResult<Vec<String>> {
    info!("Searching for files with query: '{}'", query);

    let search_directories = get_search_directories();
    let mut found_files = Vec::new();

    for search_dir in search_directories {
        if let Ok(files) = search_in_directory(&search_dir, query) {
            for file in files {
                found_files.push(file.path.to_string_lossy().to_string());

                // Ограничиваем количество результатов
                if found_files.len() >= 20 {
                    break;
                }
            }
        }

        if found_files.len() >= 20 {
            break;
        }
    }

    // Сортируем по релевантности (простая сортировка по совпадению имени)
    found_files.sort_by(|a, b| {
        let a_score = calculate_relevance_score(a, query);
        let b_score = calculate_relevance_score(b, query);
        b_score.partial_cmp(&a_score).unwrap_or(std::cmp::Ordering::Equal)
    });

    info!("Found {} files matching query '{}'", found_files.len(), query);
    Ok(found_files)
}

/// Получение директорий для поиска
fn get_search_directories() -> Vec<PathBuf> {
    let mut directories = Vec::new();

    // Получаем домашнюю директорию
    if let Some(home_dir) = dirs::home_dir() {
        directories.push(home_dir.clone());
        directories.push(home_dir.join("Documents"));
        directories.push(home_dir.join("Desktop"));
        directories.push(home_dir.join("Downloads"));

        #[cfg(target_os = "windows")]
        {
            directories.push(home_dir.join("OneDrive"));
            directories.push(home_dir.join("OneDrive - Personal"));
        }

        #[cfg(target_os = "macos")]
        {
            directories.push(home_dir.join("iCloud Drive"));
        }

        #[cfg(target_os = "linux")]
        {
            directories.push(home_dir.join("Documents"));
            directories.push(home_dir.join("Pictures"));
            directories.push(home_dir.join("Videos"));
            directories.push(home_dir.join("Music"));
        }
    }

    // Добавляем текущую директорию
    if let Ok(current_dir) = std::env::current_dir() {
        directories.push(current_dir);
    }

    directories
}

/// Поиск в конкретной директории
fn search_in_directory(dir: &Path, query: &str) -> JarvisResult<Vec<FoundDocument>> {
    let mut found_documents = Vec::new();

    if !dir.exists() || !dir.is_dir() {
        return Ok(found_documents);
    }

    // Читаем содержимое директории
    let entries = fs::read_dir(dir).map_err(|e| {
        JarvisError::Generic(format!("Failed to read directory {}: {}", dir.display(), e))
    })?;

    for entry in entries {
        if let Ok(entry) = entry {
            let path = entry.path();

            if path.is_file() {
                if let Some(found_doc) = check_file_match(&path, query)? {
                    found_documents.push(found_doc);
                }
            } else if path.is_dir() {
                // Рекурсивный поиск только на 2 уровня вглубь для производительности
                if let Some(parent) = dir.parent() {
                    if parent != dir {
                        if let Ok(mut sub_files) = search_in_directory(&path, query) {
                            found_documents.append(&mut sub_files);
                        }
                    }
                }
            }
        }
    }

    Ok(found_documents)
}

/// Проверка соответствия файла запросу
fn check_file_match(path: &Path, query: &str) -> JarvisResult<Option<FoundDocument>> {
    let file_name = path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");

    // Проверяем, содержит ли имя файла поисковый запрос
    if !file_name.to_lowercase().contains(&query.to_lowercase()) {
        return Ok(None);
    }

    // Получаем расширение файла
    let extension = path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| format!(".{}", ext.to_lowercase()))
        .unwrap_or_default();

    // Проверяем, является ли файл документом, изображением и т.д.
    let file_type = determine_file_type(&extension);

    // Получаем метаданные файла
    let metadata = fs::metadata(path).ok();
    let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
    let modified = metadata.and_then(|m| m.modified().ok());

    let found_doc = FoundDocument {
        path: path.to_path_buf(),
        name: file_name.to_string(),
        extension,
        size,
        modified,
        file_type,
    };

    Ok(Some(found_doc))
}

/// Определение типа файла по расширению
fn determine_file_type(extension: &str) -> DocumentType {
    if DOCUMENT_EXTENSIONS.contains(&extension) {
        DocumentType::Document
    } else if IMAGE_EXTENSIONS.contains(&extension) {
        DocumentType::Image
    } else if VIDEO_EXTENSIONS.contains(&extension) {
        DocumentType::Video
    } else if AUDIO_EXTENSIONS.contains(&extension) {
        DocumentType::Audio
    } else {
        DocumentType::Other
    }
}

/// Расчет релевантности найденного файла
fn calculate_relevance_score(file_path: &str, query: &str) -> f64 {
    let file_name = Path::new(file_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_lowercase();

    let query_lower = query.to_lowercase();

    let mut score = 0.0;

    // Точное совпадение названия (без расширения)
    if let Some(stem) = Path::new(&file_name).file_stem() {
        if let Some(stem_str) = stem.to_str() {
            if stem_str == query_lower {
                score += 10.0;
            }
        }
    }

    // Начинается с запроса
    if file_name.starts_with(&query_lower) {
        score += 5.0;
    }

    // Содержит запрос
    if file_name.contains(&query_lower) {
        score += 2.0;
    }

    // Бонус за тип файла (документы приоритетнее)
    let extension = Path::new(file_path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| format!(".{}", ext.to_lowercase()))
        .unwrap_or_default();

    match determine_file_type(&extension) {
        DocumentType::Document => score += 1.0,
        DocumentType::Image => score += 0.5,
        _ => {}
    }

    score
}

/// Открытие файла системным приложением
pub fn open_file(file_path: &str) -> JarvisResult<()> {
    let path = Path::new(file_path);

    if !path.exists() {
        return Err(JarvisError::Generic(format!("File not found: {}", file_path)));
    }

    info!("Opening file: {}", file_path);

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(&["/C", "start", "", file_path])
            .spawn()
            .map_err(|e| JarvisError::Generic(format!("Failed to open file on Windows: {}", e)))?;
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(file_path)
            .spawn()
            .map_err(|e| JarvisError::Generic(format!("Failed to open file on Linux: {}", e)))?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(file_path)
            .spawn()
            .map_err(|e| JarvisError::Generic(format!("Failed to open file on macOS: {}", e)))?;
    }

    info!("File opened successfully: {}", file_path);
    Ok(())
}

/// Расширенный поиск с фильтрами
pub fn search_files_advanced(
    query: &str,
    file_type: Option<DocumentType>,
    max_results: Option<usize>
) -> JarvisResult<Vec<FoundDocument>> {
    info!("Advanced search for: '{}', type: {:?}", query, file_type);

    let search_directories = get_search_directories();
    let mut found_documents = Vec::new();
    let max_results = max_results.unwrap_or(50);

    for search_dir in search_directories {
        if let Ok(mut files) = search_in_directory(&search_dir, query) {
            // Фильтруем по типу файла если указан
            if let Some(ref filter_type) = file_type {
                files.retain(|doc| &doc.file_type == filter_type);
            }

            found_documents.extend(files);

            if found_documents.len() >= max_results {
                break;
            }
        }
    }

    // Сортируем по релевантности и времени изменения
    found_documents.sort_by(|a, b| {
        let a_score = calculate_relevance_score(&a.path.to_string_lossy(), query);
        let b_score = calculate_relevance_score(&b.path.to_string_lossy(), query);

        match b_score.partial_cmp(&a_score) {
            Some(std::cmp::Ordering::Equal) => {
                // При равных очках сортируем по времени изменения
                b.modified.cmp(&a.modified)
            }
            other => other.unwrap_or(std::cmp::Ordering::Equal)
        }
    });

    // Ограничиваем количество результатов
    found_documents.truncate(max_results);

    info!("Advanced search found {} documents", found_documents.len());
    Ok(found_documents)
}

/// Получение статистики по типам файлов в результатах поиска
pub fn get_search_statistics(documents: &[FoundDocument]) -> HashMap<DocumentType, usize> {
    let mut stats = HashMap::new();

    for doc in documents {
        *stats.entry(doc.file_type.clone()).or_insert(0) += 1;
    }

    stats
}

/// Проверка доступности файла для открытия
pub fn can_open_file(file_path: &str) -> bool {
    let path = Path::new(file_path);
    path.exists() && path.is_file()
}

/// Получение информации о файле
pub fn get_file_info(file_path: &str) -> JarvisResult<FoundDocument> {
    let path = Path::new(file_path);

    if !path.exists() {
        return Err(JarvisError::Generic(format!("File not found: {}", file_path)));
    }

    let file_name = path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_string();

    let extension = path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| format!(".{}", ext.to_lowercase()))
        .unwrap_or_default();

    let file_type = determine_file_type(&extension);

    let metadata = fs::metadata(path)
        .map_err(|e| JarvisError::Generic(format!("Failed to get file metadata: {}", e)))?;

    let size = metadata.len();
    let modified = metadata.modified().ok();

    Ok(FoundDocument {
        path: path.to_path_buf(),
        name: file_name,
        extension,
        size,
        modified,
        file_type,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn test_determine_file_type() {
        assert_eq!(determine_file_type(".pdf"), DocumentType::Document);
        assert_eq!(determine_file_type(".jpg"), DocumentType::Image);
        assert_eq!(determine_file_type(".mp4"), DocumentType::Video);
        assert_eq!(determine_file_type(".mp3"), DocumentType::Audio);
        assert_eq!(determine_file_type(".xyz"), DocumentType::Other);
    }

    #[test]
    fn test_calculate_relevance_score() {
        let score1 = calculate_relevance_score("/path/to/test.pdf", "test");
        let score2 = calculate_relevance_score("/path/to/another.pdf", "test");

        assert!(score1 > score2);
    }

    #[test]
    fn test_search_in_temporary_directory() {
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("test_document.pdf");
        File::create(&test_file).unwrap();

        let results = search_in_directory(temp_dir.path(), "test").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "test_document.pdf");
    }
}