// src-tauri/src/audio/monitor.rs - Исправленный аудио монитор

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// ДОБАВИТЬ ИМПОРТЫ ДЛЯ ЛОГИРОВАНИЯ
use log::{info, warn, error};

use crate::error::{JarvisResult, JarvisError, AudioError};

// Структуры остаются без изменений
#[derive(Debug, Clone, PartialEq)]
pub struct AudioDevice {
    pub id: i32,
    pub name: String,
    pub is_input: bool,
    pub is_output: bool,
    pub is_default: bool,
    pub is_available: bool,
}

#[derive(Debug, Clone)]
pub enum AudioDeviceEvent {
    DeviceAdded(AudioDevice),
    DeviceRemoved(i32),
    DeviceChanged(AudioDevice),
}

pub type AudioDeviceCallback = Box<dyn Fn(AudioDeviceEvent) + Send + Sync>;

pub struct AudioMonitor {
    devices: Arc<Mutex<HashMap<i32, AudioDevice>>>,
    callbacks: Arc<Mutex<Vec<Arc<AudioDeviceCallback>>>>,
    is_running: Arc<Mutex<bool>>,
    update_interval: Duration,
}

impl AudioMonitor {
    pub fn new() -> JarvisResult<Self> {
        Ok(Self {
            devices: Arc::new(Mutex::new(HashMap::new())),
            callbacks: Arc::new(Mutex::new(Vec::new())),
            is_running: Arc::new(Mutex::new(false)),
            update_interval: Duration::from_secs(2),
        })
    }

    pub fn start(&self) -> JarvisResult<()> {
        let mut is_running = self.is_running.lock().map_err(|e| {
            JarvisError::AudioError(AudioError::InitializationFailed(
                format!("Failed to acquire running lock: {}", e)
            ))
        })?;

        if *is_running {
            warn!("Audio monitor is already running");
            return Ok(());
        }

        *is_running = true;
        info!("Starting audio device monitor...");

        // Инициальное сканирование устройств
        self.scan_devices_initial()?;

        // Запускаем мониторинг в отдельном потоке
        let devices_clone = Arc::clone(&self.devices);
        let callbacks_clone = Arc::clone(&self.callbacks);
        let is_running_clone = Arc::clone(&self.is_running);
        let update_interval = self.update_interval;

        thread::spawn(move || {
            while *is_running_clone.lock().unwrap() {
                if let Err(e) = Self::scan_and_update_devices(&devices_clone, &callbacks_clone) {
                    error!("Device scan error: {}", e);
                }
                thread::sleep(update_interval);
            }
            info!("Audio monitor thread stopped");
        });

        info!("Audio monitor started successfully");
        Ok(())
    }

    pub fn stop(&self) -> JarvisResult<()> {
        let mut is_running = self.is_running.lock().map_err(|e| {
            JarvisError::AudioError(AudioError::InitializationFailed(
                format!("Failed to acquire running lock: {}", e)
            ))
        })?;

        if !*is_running {
            warn!("Audio monitor is not running");
            return Ok(());
        }

        *is_running = false;
        info!("Audio monitor stopped");
        Ok(())
    }

    pub fn get_devices(&self) -> JarvisResult<Vec<AudioDevice>> {
        let devices = self.devices.lock().map_err(|e| {
            JarvisError::AudioError(AudioError::InitializationFailed(
                format!("Failed to acquire devices lock: {}", e)
            ))
        })?;

        Ok(devices.values().cloned().collect())
    }

    pub fn add_callback(&self, callback: AudioDeviceCallback) -> JarvisResult<()> {
        let mut callbacks = self.callbacks.lock().map_err(|e| {
            JarvisError::AudioError(AudioError::InitializationFailed(
                format!("Failed to acquire callbacks lock: {}", e)
            ))
        })?;

        callbacks.push(Arc::new(callback));
        Ok(())
    }

    fn scan_devices_initial(&self) -> JarvisResult<()> {
        let devices = Self::get_system_devices_static()?;
        let mut devices_map = self.devices.lock().map_err(|e| {
            JarvisError::AudioError(AudioError::InitializationFailed(
                format!("Failed to acquire devices lock: {}", e)
            ))
        })?;

        for device in devices {
            info!("Found audio device: {} (ID: {})", device.name, device.id);
            devices_map.insert(device.id, device);
        }

        Ok(())
    }

    fn scan_and_update_devices(
        devices: &Arc<Mutex<HashMap<i32, AudioDevice>>>,
        callbacks: &Arc<Mutex<Vec<Arc<AudioDeviceCallback>>>>,
    ) -> JarvisResult<()> {
        let new_devices = Self::get_system_devices_static()?;
        let mut new_devices_map = HashMap::new();

        for device in new_devices {
            new_devices_map.insert(device.id, device);
        }

        let mut devices_map = devices.lock().map_err(|e| {
            JarvisError::AudioError(AudioError::InitializationFailed(
                format!("Failed to acquire devices lock: {}", e)
            ))
        })?;

        let callbacks_vec = callbacks.lock().map_err(|e| {
            JarvisError::AudioError(AudioError::InitializationFailed(
                format!("Failed to acquire callbacks lock: {}", e)
            ))
        })?.clone();

        // Находим новые устройства
        for (id, device) in &new_devices_map {
            if !devices_map.contains_key(id) {
                info!("Audio device added: {} (ID: {})", device.name, device.id);

                // Уведомляем коллбэки
                let event = AudioDeviceEvent::DeviceAdded(device.clone());
                for callback in callbacks_vec.iter() {
                    callback(event.clone());
                }
            }
        }

        // Находим удаленные устройства
        let removed_ids: Vec<i32> = devices_map.keys()
            .filter(|id| !new_devices_map.contains_key(id))
            .cloned()
            .collect();

        for id in removed_ids {
            if let Some(device) = devices_map.remove(&id) {
                info!("Audio device removed: {} (ID: {})", device.name, device.id);

                // Уведомляем коллбэки
                let event = AudioDeviceEvent::DeviceRemoved(id);
                for callback in callbacks_vec.iter() {
                    callback(event.clone());
                }
            }
        }

        // Находим измененные устройства
        for (id, new_device) in &new_devices_map {
            if let Some(old_device) = devices_map.get(id) {
                if old_device != new_device {
                    info!("Audio device changed: {} (ID: {})", new_device.name, new_device.id);

                    // Уведомляем коллбэки
                    let event = AudioDeviceEvent::DeviceChanged(new_device.clone());
                    for callback in callbacks_vec.iter() {
                        callback(event.clone());
                    }
                }
            }
        }

        // Обновляем карту устройств
        *devices_map = new_devices_map;

        Ok(())
    }

    /// Получение системных аудио устройств
    fn get_system_devices(&self) -> JarvisResult<Vec<AudioDevice>> {
        Self::get_system_devices_static()
    }

    /// Статическая версия получения системных устройств
    fn get_system_devices_static() -> JarvisResult<Vec<AudioDevice>> {
        let mut devices = Vec::new();

        // ИСПРАВИТЬ ИМПОРТ - используем относительный путь
        match crate::audio::recorder::get_available_devices() {
            Ok(recorder_devices) => {
                for (id, name) in recorder_devices {
                    let device = AudioDevice {
                        id,
                        name,
                        is_input: true,  // recorder возвращает только входные устройства
                        is_output: false,
                        is_default: id == crate::audio::recorder::get_selected_microphone_index().unwrap_or(-1),
                        is_available: true,
                    };
                    devices.push(device);
                }
            }
            Err(e) => {
                warn!("Failed to get recorder devices: {}", e);
            }
        }

        // Дополнительно получаем выходные устройства через системные API
        let output_devices = Self::get_output_devices()?;
        devices.extend(output_devices);

        Ok(devices)
    }

    /// Получение выходных аудио устройств
    fn get_output_devices() -> JarvisResult<Vec<AudioDevice>> {
        let mut devices = Vec::new();

        #[cfg(target_os = "windows")]
        {
            // Windows: используем WMI для получения аудио устройств
            let output = std::process::Command::new("powershell")
                .args(&["-Command",
                    "Get-WmiObject -Class Win32_SoundDevice | Select-Object Name, DeviceID | ConvertTo-Json"
                ])
                .output()
                .map_err(|e| JarvisError::AudioError(AudioError::InitializationFailed(
                    format!("Failed to execute PowerShell command: {}", e)
                )))?;

            if output.status.success() {
                let output_str = String::from_utf8_lossy(&output.stdout);
                // Здесь можно добавить парсинг JSON ответа
                info!("Windows audio devices detected");
            }
        }

        #[cfg(target_os = "linux")]
        {
            // Linux: используем aplay для получения устройств
            let output = std::process::Command::new("aplay")
                .args(&["-l"])
                .output()
                .map_err(|e| JarvisError::AudioError(AudioError::InitializationFailed(
                    format!("Failed to execute aplay: {}", e)
                )))?;

            if output.status.success() {
                let output_str = String::from_utf8_lossy(&output.stdout);
                // Парсинг вывода aplay
                info!("Linux audio devices detected");
            }
        }

        #[cfg(target_os = "macos")]
        {
            // macOS: используем system_profiler
            let output = std::process::Command::new("system_profiler")
                .args(&["SPAudioDataType", "-json"])
                .output()
                .map_err(|e| JarvisError::AudioError(AudioError::InitializationFailed(
                    format!("Failed to execute system_profiler: {}", e)
                )))?;

            if output.status.success() {
                let output_str = String::from_utf8_lossy(&output.stdout);
                // Парсинг JSON ответа
                info!("macOS audio devices detected");
            }
        }

        Ok(devices)
    }

    pub fn is_running(&self) -> bool {
        *self.is_running.lock().unwrap_or(&mut false)
    }
}

// Глобальный экземпляр монитора
static AUDIO_MONITOR: std::sync::OnceLock<Arc<Mutex<AudioMonitor>>> = std::sync::OnceLock::new();

/// Инициализация аудио монитора
pub fn init() -> JarvisResult<()> {
    let monitor = AudioMonitor::new()?;
    let monitor_arc = Arc::new(Mutex::new(monitor));

    AUDIO_MONITOR.set(monitor_arc.clone()).map_err(|_| {
        JarvisError::AudioError(AudioError::InitializationFailed(
            "Audio monitor already initialized".to_string()
        ))
    })?;

    // Запускаем мониторинг
    let monitor_guard = monitor_arc.lock().map_err(|e| {
        JarvisError::AudioError(AudioError::InitializationFailed(
            format!("Failed to acquire monitor lock: {}", e)
        ))
    })?;

    monitor_guard.start()?;

    info!("Audio monitor initialized successfully");
    Ok(())
}

/// Завершение работы аудио монитора
pub fn shutdown() -> JarvisResult<()> {
    if let Some(monitor_arc) = AUDIO_MONITOR.get() {
        let monitor_guard = monitor_arc.lock().map_err(|e| {
            JarvisError::AudioError(AudioError::InitializationFailed(
                format!("Failed to acquire monitor lock: {}", e)
            ))
        })?;

        monitor_guard.stop()?;
    }

    info!("Audio monitor shutdown completed");
    Ok(())
}

/// Получение глобального экземпляра монитора
pub fn get_audio_monitor() -> Option<Arc<Mutex<AudioMonitor>>> {
    AUDIO_MONITOR.get().cloned()
}

/// Получение всех устройств (для совместимости)
pub fn get_all_devices() -> JarvisResult<Vec<AudioDevice>> {
    if let Some(monitor_arc) = AUDIO_MONITOR.get() {
        let monitor_guard = monitor_arc.lock().map_err(|e| {
            JarvisError::AudioError(AudioError::InitializationFailed(
                format!("Failed to acquire monitor lock: {}", e)
            ))
        })?;

        monitor_guard.get_devices()
    } else {
        Ok(Vec::new())
    }
}

/// Проверка состояния мониторинга
pub fn is_running() -> bool {
    if let Some(monitor_arc) = AUDIO_MONITOR.get() {
        if let Ok(monitor_guard) = monitor_arc.lock() {
            return monitor_guard.is_running();
        }
    }
    false
}