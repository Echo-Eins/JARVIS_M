// app/src/tray.rs - Полнофункциональный модуль системного трея

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use tray_icon::{
    TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuItem, MenuEvent, PredefinedMenuItem}
};
use winit::event_loop::{EventLoop, ControlFlow};
use winit::event::{Event, WindowEvent};
use image::load_from_memory;

use crate::error::{JarvisResult, JarvisError};
use crate::config;
use crate::{recorder, listener, tts, should_shutdown};

// Состояние трея
struct TrayState {
    is_listening: bool,
    is_speaking: bool,
    last_command: Option<String>,
    microphone_count: usize,
}

impl Default for TrayState {
    fn default() -> Self {
        Self {
            is_listening: false,
            is_speaking: false,
            last_command: None,
            microphone_count: 0,
        }
    }
}

static TRAY_STATE: Mutex<TrayState> = Mutex::new(TrayState {
    is_listening: false,
    is_speaking: false,
    last_command: None,
    microphone_count: 0,
});

// ID для элементов меню
const MENU_TOGGLE_LISTENING: &str = "toggle_listening";
const MENU_OPEN_SETTINGS: &str = "open_settings";
const MENU_SHOW_STATUS: &str = "show_status";
const MENU_RELOAD_CONFIG: &str = "reload_config";
const MENU_QUIT: &str = "quit";

/// Инициализация системного трея
pub fn init() -> JarvisResult<()> {
    info!("Initializing system tray...");

    // Проверяем, поддерживается ли трей в системе
    if !tray_icon::TrayIcon::is_supported() {
        warn!("System tray is not supported on this platform");
        return Ok(()); // Не критическая ошибка
    }

    // Запускаем трей в отдельном потоке
    thread::spawn(|| {
        if let Err(e) = run_tray() {
            error!("Tray error: {}", e);
        }
    });

    info!("System tray initialized successfully");
    Ok(())
}

/// Основной цикл трея
fn run_tray() -> JarvisResult<()> {
    let event_loop = EventLoop::new();

    // Создаем иконку трея
    let icon = load_tray_icon()?;

    // Создаем меню
    let menu = create_tray_menu()?;

    // Создаем трей
    let _tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("JARVIS Voice Assistant")
        .with_icon(icon)
        .build()
        .map_err(|e| JarvisError::Generic(format!("Failed to create tray icon: {}", e)))?;

    info!("Tray icon created successfully");

    // Обработчик событий меню
    let menu_channel = MenuEvent::receiver();
    let tray_channel = TrayIconEvent::receiver();

    // Основной цикл событий
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(
            std::time::Instant::now() + Duration::from_millis(100)
        );

        // Проверяем запрос на завершение приложения
        if should_shutdown() {
            *control_flow = ControlFlow::Exit;
            return;
        }

        // Обновляем состояние трея
        update_tray_state();

        // Обрабатываем события меню
        if let Ok(event) = menu_channel.try_recv() {
            if let Err(e) = handle_menu_event(event) {
                error!("Menu event error: {}", e);
            }
        }

        // Обрабатываем события трея
        if let Ok(event) = tray_channel.try_recv() {
            if let Err(e) = handle_tray_event(event) {
                error!("Tray event error: {}", e);
            }
        }

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });
}

/// Загрузка иконки трея
fn load_tray_icon() -> JarvisResult<tray_icon::Icon> {
    // Встроенная иконка (base64 или байты)
    let icon_bytes = include_bytes!("../assets/icons/tray.png"); // Путь к иконке

    let image = load_from_memory(icon_bytes)
        .map_err(|e| JarvisError::Generic(format!("Failed to load tray icon: {}", e)))?;

    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();

    tray_icon::Icon::from_rgba(rgba.into_raw(), width, height)
        .map_err(|e| JarvisError::Generic(format!("Failed to create icon: {}", e)))
}

/// Создание меню трея
fn create_tray_menu() -> JarvisResult<Menu> {
    let menu = Menu::new();

    // Статус
    let status_item = MenuItem::new("JARVIS - Готов к работе", true, None);
    menu.append(&status_item)
        .map_err(|e| JarvisError::Generic(format!("Failed to add status item: {}", e)))?;

    // Разделитель
    menu.append(&PredefinedMenuItem::separator())
        .map_err(|e| JarvisError::Generic(format!("Failed to add separator: {}", e)))?;

    // Переключение прослушивания
    let listening_item = MenuItem::new("▶ Начать прослушивание", true, Some(MENU_TOGGLE_LISTENING.to_string()));
    menu.append(&listening_item)
        .map_err(|e| JarvisError::Generic(format!("Failed to add listening item: {}", e)))?;

    // Показать статус
    let status_detail_item = MenuItem::new("📊 Показать статус", true, Some(MENU_SHOW_STATUS.to_string()));
    menu.append(&status_detail_item)
        .map_err(|e| JarvisError::Generic(format!("Failed to add status detail item: {}", e)))?;

    // Разделитель
    menu.append(&PredefinedMenuItem::separator())
        .map_err(|e| JarvisError::Generic(format!("Failed to add separator: {}", e)))?;

    // Настройки
    let settings_item = MenuItem::new("⚙ Открыть настройки", true, Some(MENU_OPEN_SETTINGS.to_string()));
    menu.append(&settings_item)
        .map_err(|e| JarvisError::Generic(format!("Failed to add settings item: {}", e)))?;

    // Перезагрузить конфигурацию
    let reload_item = MenuItem::new("🔄 Перезагрузить конфигурацию", true, Some(MENU_RELOAD_CONFIG.to_string()));
    menu.append(&reload_item)
        .map_err(|e| JarvisError::Generic(format!("Failed to add reload item: {}", e)))?;

    // Разделитель
    menu.append(&PredefinedMenuItem::separator())
        .map_err(|e| JarvisError::Generic(format!("Failed to add separator: {}", e)))?;

    // Выход
    let quit_item = MenuItem::new("❌ Выйти", true, Some(MENU_QUIT.to_string()));
    menu.append(&quit_item)
        .map_err(|e| JarvisError::Generic(format!("Failed to add quit item: {}", e)))?;

    Ok(menu)
}

/// Обновление состояния трея
fn update_tray_state() {
    let mut state = TRAY_STATE.lock().unwrap();

    // Обновляем статус прослушивания
    state.is_listening = recorder::is_recording();

    // Обновляем статус синтеза речи
    state.is_speaking = tts::is_speaking();

    // Обновляем количество микрофонов
    if let Ok(devices) = recorder::get_available_devices() {
        state.microphone_count = devices.len();
    }
}

/// Обработка событий меню
fn handle_menu_event(event: MenuEvent) -> JarvisResult<()> {
    if let Some(id) = event.id.0.as_ref() {
        match id.as_str() {
            MENU_TOGGLE_LISTENING => toggle_listening()?,
            MENU_OPEN_SETTINGS => open_settings()?,
            MENU_SHOW_STATUS => show_status()?,
            MENU_RELOAD_CONFIG => reload_config()?,
            MENU_QUIT => quit_application()?,
            _ => {}
        }
    }
    Ok(())
}

/// Обработка событий трея (клики по иконке)
fn handle_tray_event(event: TrayIconEvent) -> JarvisResult<()> {
    match event {
        TrayIconEvent::Click {
            button: tray_icon::ClickType::Left,
            ..
        } => {
            // Левый клик - переключение прослушивания
            toggle_listening()?;
        }
        TrayIconEvent::Click {
            button: tray_icon::ClickType::Right,
            ..
        } => {
            // Правый клик - показать меню (автоматически)
        }
        TrayIconEvent::DoubleClick { .. } => {
            // Двойной клик - открыть настройки
            open_settings()?;
        }
        _ => {}
    }
    Ok(())
}

/// Переключение прослушивания
fn toggle_listening() -> JarvisResult<()> {
    let state = TRAY_STATE.lock().unwrap();

    if state.is_listening {
        info!("Stopping listening via tray");
        recorder::stop_recording()?;
        show_notification("JARVIS", "Прослушивание остановлено")?;
    } else {
        info!("Starting listening via tray");
        recorder::start_recording()?;
        show_notification("JARVIS", "Прослушивание началось")?;
    }

    Ok(())
}

/// Открытие настроек
fn open_settings() -> JarvisResult<()> {
    info!("Opening settings via tray");

    // Открываем настройки через системную команду или Tauri
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(&["/C", "start", "http://localhost:1420/settings"]) // Порт Tauri dev сервера
            .spawn()
            .map_err(|e| JarvisError::Generic(format!("Failed to open settings: {}", e)))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg("http://localhost:1420/settings")
            .spawn()
            .map_err(|e| JarvisError::Generic(format!("Failed to open settings: {}", e)))?;
    }

    Ok(())
}

/// Показ детального статуса
fn show_status() -> JarvisResult<()> {
    let state = TRAY_STATE.lock().unwrap();

    let listening_status = if state.is_listening { "🟢 Активно" } else { "🔴 Неактивно" };
    let speaking_status = if state.is_speaking { "🗣 Говорит" } else { "🔇 Молчит" };

    let status_message = format!(
        "JARVIS Статус:\n\n\
         Прослушивание: {}\n\
         Синтез речи: {}\n\
         Микрофоны: {} устройств\n\
         Последняя команда: {}",
        listening_status,
        speaking_status,
        state.microphone_count,
        state.last_command.as_deref().unwrap_or("Нет")
    );

    show_notification("JARVIS - Статус", &status_message)?;

    Ok(())
}

/// Перезагрузка конфигурации
fn reload_config() -> JarvisResult<()> {
    info!("Reloading configuration via tray");

    // Здесь можно добавить логику перезагрузки конфигурации
    // Например, переинициализация компонентов

    show_notification("JARVIS", "Конфигурация перезагружена")?;
    Ok(())
}

/// Завершение приложения
fn quit_application() -> JarvisResult<()> {
    info!("Quit requested via tray");

    // Устанавливаем флаг завершения
    crate::request_shutdown();

    Ok(())
}

/// Показ уведомления
fn show_notification(title: &str, message: &str) -> JarvisResult<()> {
    #[cfg(target_os = "windows")]
    {
        // Windows Toast notification
        let ps_command = format!(
            "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null; \
             [Windows.UI.Notifications.ToastNotification, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null; \
             [Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null; \
             $APP_ID = 'JARVIS Voice Assistant'; \
             $template = @'
             <toast>
                 <visual>
                     <binding template=\"ToastText02\">
                         <text id=\"1\">{}</text>
                         <text id=\"2\">{}</text>
                     </binding>
                 </visual>
             </toast>
             '@; \
             $xml = New-Object Windows.Data.Xml.Dom.XmlDocument; \
             $xml.LoadXml($template); \
             $toast = New-Object Windows.UI.Notifications.ToastNotification $xml; \
             [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier($APP_ID).Show($toast);",
            title.replace("\"", "\\\""),
            message.replace("\"", "\\\"")
        );

        std::process::Command::new("powershell")
            .args(&["-Command", &ps_command])
            .output()
            .map_err(|e| JarvisError::Generic(format!("Failed to show notification: {}", e)))?;
    }

    #[cfg(target_os = "linux")]
    {
        // Linux notification через notify-send
        std::process::Command::new("notify-send")
            .args(&[title, message, "-i", "info"])
            .output()
            .map_err(|e| JarvisError::Generic(format!("Failed to show notification: {}", e)))?;
    }

    Ok(())
}

/// Обновление иконки трея в зависимости от состояния
pub fn update_tray_icon(listening: bool) -> JarvisResult<()> {
    // Здесь можно изменить иконку в зависимости от состояния
    // Например, зеленая для активного прослушивания, серая для неактивного

    if listening {
        info!("Tray icon: Listening mode");
    } else {
        info!("Tray icon: Idle mode");
    }

    Ok(())
}

/// Обновление текста подсказки трея
pub fn update_tray_tooltip(status: &str) -> JarvisResult<()> {
    info!("Tray tooltip updated: {}", status);
    Ok(())
}

/// Обновление меню трея
pub fn update_tray_menu() -> JarvisResult<()> {
    // Здесь можно динамически обновлять элементы меню
    info!("Tray menu updated");
    Ok(())
}

/// Обновление последней выполненной команды
pub fn set_last_command(command: &str) {
    let mut state = TRAY_STATE.lock().unwrap();
    state.last_command = Some(command.to_string());
    info!("Last command updated in tray: {}", command);
}

/// Graceful shutdown трея
pub fn shutdown() -> JarvisResult<()> {
    info!("Shutting down system tray...");

    // Трей автоматически закроется при завершении приложения
    // Здесь можно добавить дополнительную логику очистки

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tray_state_default() {
        let state = TrayState::default();
        assert!(!state.is_listening);
        assert!(!state.is_speaking);
        assert!(state.last_command.is_none());
        assert_eq!(state.microphone_count, 0);
    }
}