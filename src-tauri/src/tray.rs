// src-tauri/src/tray.rs - Трей через Tauri API БЕЗ GTK

use tauri::{
    AppHandle, CustomMenuItem, Manager, SystemTray, SystemTrayEvent,
    SystemTrayMenu, SystemTrayMenuItem, SystemTraySubmenu
};

use crate::error::{JarvisResult, JarvisError};

// ID для элементов меню
const MENU_TOGGLE_LISTENING: &str = "toggle_listening";
const MENU_OPEN_SETTINGS: &str = "open_settings";
const MENU_SHOW_STATUS: &str = "show_status";
const MENU_RELOAD_CONFIG: &str = "reload_config";
const MENU_QUIT: &str = "quit";

/// Создание системного трея
pub fn create_system_tray() -> SystemTray {
    // Создаем элементы меню
    let toggle_listening = CustomMenuItem::new(MENU_TOGGLE_LISTENING.to_string(), "▶ Начать прослушивание");
    let show_status = CustomMenuItem::new(MENU_SHOW_STATUS.to_string(), "📊 Показать статус");
    let open_settings = CustomMenuItem::new(MENU_OPEN_SETTINGS.to_string(), "⚙ Настройки");
    let reload_config = CustomMenuItem::new(MENU_RELOAD_CONFIG.to_string(), "🔄 Перезагрузить конфигурацию");
    let quit = CustomMenuItem::new(MENU_QUIT.to_string(), "❌ Выйти");

    // Создаем меню
    let tray_menu = SystemTrayMenu::new()
        .add_item(toggle_listening)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(show_status)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(open_settings)
        .add_item(reload_config)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(quit);

    // Создаем трей
    SystemTray::new()
        .with_menu(tray_menu)
        .with_tooltip("JARVIS Voice Assistant")
}

/// Обработчик событий системного трея
pub fn handle_system_tray_event(app: &AppHandle, event: SystemTrayEvent) {
    match event {
        SystemTrayEvent::LeftClick {
            position: _,
            size: _,
            ..
        } => {
            // Левый клик - переключение прослушивания
            if let Err(e) = toggle_listening() {
                log::error!("Failed to toggle listening: {}", e);
            }
        }
        SystemTrayEvent::RightClick {
            position: _,
            size: _,
            ..
        } => {
            // Правый клик - показать меню (автоматически)
        }
        SystemTrayEvent::DoubleClick {
            position: _,
            size: _,
            ..
        } => {
            // Двойной клик - показать главное окно
            if let Some(window) = app.get_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        SystemTrayEvent::MenuItemClick { id, .. } => {
            // Обработка кликов по элементам меню
            if let Err(e) = handle_menu_click(&id, app) {
                log::error!("Menu click error: {}", e);
            }
        }
    }
}

/// Обработка кликов по элементам меню
fn handle_menu_click(id: &str, app: &AppHandle) -> JarvisResult<()> {
    match id {
        MENU_TOGGLE_LISTENING => toggle_listening()?,
        MENU_OPEN_SETTINGS => open_settings(app)?,
        MENU_SHOW_STATUS => show_status()?,
        MENU_RELOAD_CONFIG => reload_config()?,
        MENU_QUIT => quit_application(app)?,
        _ => {}
    }
    Ok(())
}

/// Переключение прослушивания
fn toggle_listening() -> JarvisResult<()> {
    log::info!("Toggle listening requested from tray");

    // Здесь должна быть логика переключения прослушивания
    // Пока заглушка
    show_notification("JARVIS", "Прослушивание переключено")?;

    Ok(())
}

/// Открытие настроек
fn open_settings(app: &AppHandle) -> JarvisResult<()> {
    log::info!("Opening settings via tray");

    // Показываем главное окно приложения
    if let Some(window) = app.get_window("main") {
        let _ = window.show();
        let _ = window.set_focus();

        // Переходим на страницу настроек
        let _ = window.emit("navigate-to", "/settings");
    }

    Ok(())
}

/// Показ детального статуса
fn show_status() -> JarvisResult<()> {
    log::info!("Showing status via tray");

    let status_message = format!(
        "JARVIS Статус:\n\n\
         Прослушивание: 🔴 Неактивно\n\
         Синтез речи: 🔇 Молчит\n\
         Версия: {}",
        env!("CARGO_PKG_VERSION")
    );

    show_notification("JARVIS - Статус", &status_message)?;

    Ok(())
}

/// Перезагрузка конфигурации
fn reload_config() -> JarvisResult<()> {
    log::info!("Reloading configuration via tray");

    // Здесь должна быть логика перезагрузки
    show_notification("JARVIS", "Конфигурация перезагружена")?;

    Ok(())
}

/// Завершение приложения
fn quit_application(app: &AppHandle) -> JarvisResult<()> {
    log::info!("Quit requested via tray");

    // Закрываем приложение
    app.exit(0);

    Ok(())
}

/// Показ уведомления (кроссплатформенно)
fn show_notification(title: &str, message: &str) -> JarvisResult<()> {
    // Используем простой println для начала
    // Позже можно заменить на более продвинутые уведомления
    println!("🔔 {}: {}", title, message);

    #[cfg(target_os = "windows")]
    {
        // Windows Toast notification через PowerShell
        let ps_command = format!(
            r#"
            Add-Type -AssemblyName System.Windows.Forms;
            [System.Windows.Forms.MessageBox]::Show('{}', '{}', 'OK', 'Information')
            "#,
            message.replace("'", "''"),
            title.replace("'", "''")
        );

        let _ = std::process::Command::new("powershell")
            .args(&["-Command", &ps_command])
            .output();
    }

    #[cfg(target_os = "linux")]
    {
        // Linux notification через notify-send
        let _ = std::process::Command::new("notify-send")
            .args(&[title, message, "-i", "info"])
            .output();
    }

    #[cfg(target_os = "macos")]
    {
        // macOS notification через osascript
        let script = format!(
            r#"display notification "{}" with title "{}""#,
            message.replace("\"", "\\\""),
            title.replace("\"", "\\\"")
        );

        let _ = std::process::Command::new("osascript")
            .args(&["-e", &script])
            .output();
    }

    Ok(())
}

/// Обновление иконки трея в зависимости от состояния
pub fn update_tray_icon(app: &AppHandle, listening: bool) -> JarvisResult<()> {
    // Обновляем tooltip
    let tooltip = if listening {
        "JARVIS - Прослушивание активно"
    } else {
        "JARVIS - Готов к работе"
    };

    if let Some(tray_handle) = app.tray_handle() {
        let _ = tray_handle.set_tooltip(tooltip);
    }

    log::info!("Tray icon updated: {}", if listening { "listening" } else { "idle" });

    Ok(())
}

/// Обновление элементов меню
pub fn update_menu_item(app: &AppHandle, item_id: &str, title: &str, enabled: bool) -> JarvisResult<()> {
    if let Some(tray_handle) = app.tray_handle() {
        let _ = tray_handle.get_item(item_id).set_title(title);
        let _ = tray_handle.get_item(item_id).set_enabled(enabled);
    }

    Ok(())
}

/// Инициализация (заглушка для совместимости)
pub fn init() -> JarvisResult<()> {
    log::info!("Tray module initialized (using Tauri system tray)");
    Ok(())
}

/// Graceful shutdown (заглушка для совместимости)
pub fn shutdown() -> JarvisResult<()> {
    log::info!("Tray module shutdown completed");
    Ok(())
}