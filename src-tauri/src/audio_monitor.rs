// app/src/audio_monitor.rs - Мониторинг подключения/отключения аудио устройств

use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};
use std::collections::HashMap;

use crate::error::{JarvisResult, JarvisError, RecorderError, AudioError};
use crate::recorder;

// Информация об аудио устройстве
#[derive(Debug, Clone, PartialEq)]
pub struct AudioDevice {
    pub id: i32,
    pub name: String,
    pub is_input: bool,
    pub is_output: bool,
    pub is_default: bool,
    pub is_available: bool,
}

// События аудио устройств
#[derive(Debug, Clone)]
pub enum AudioDeviceEvent {
    DeviceAdded(AudioDevice),
    DeviceRemoved(i32),
    DeviceChanged(AudioDevice),
    DefaultDeviceChanged { device_id: i32, is_input: bool },
}

// Коллбэк для уведомлений об изменениях устройств
pub type DeviceCallback = Box<dyn Fn(AudioDeviceEvent) + Send + Sync>;

// Мониторинг аудио устройств
pub struct AudioMonitor {
    devices: Arc<Mutex<HashMap<i32, AudioDevice>>>,
    callbacks: Arc<Mutex<Vec<DeviceCallback>>>,
    is_running: Arc<Mutex<bool>>,
    update_interval: Duration,
}

impl AudioMonitor {
    pub fn new() -> Self {
        Self {
            devices: Arc::new(Mutex::new(HashMap::new())),
            callbacks: Arc::new(Mutex::new(Vec::new())),
            is_running: Arc::new(Mutex::new(false)),
            update_interval: Duration::from_millis(1000), // Проверяем каждую секунду
        }
    }

    /// Запуск мониторинга
    pub fn start(&self) -> JarvisResult<()> {
        let mut is_running = self.is_running.lock()
            .map_err(|e| JarvisError::RecorderError(RecorderError::InitializationFailed(
                format!("Failed to acquire running lock: {}", e)
            )))?;

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
            let mut last_scan = Instant::now();

            loop {
                // Проверяем флаг остановки
                {
                    let running = is_running_clone.lock().unwrap();
                    if !*running {
                        break;
                    }
                }

                // Ждем до следующего сканирования
                thread::sleep(Duration::from_millis(100));

                if last_scan.elapsed() >= update_interval {
                    if let Err(e) = Self::scan_devices_update(&devices_clone, &callbacks_clone) {
                        error!("Error during device scan: {}", e);
                    }
                    last_scan = Instant::now();
                }
            }

            info!("Audio monitor stopped");
        });

        info!("Audio device monitor started successfully");
        Ok(())
    }

    /// Остановка мониторинга
    pub fn stop(&self) -> JarvisResult<()> {
        let mut is_running = self.is_running.lock()
            .map_err(|e| JarvisError::RecorderError(RecorderError::InitializationFailed(
                format!("Failed to acquire running lock: {}", e)
            )))?;

        if !*is_running {
            info!("Audio monitor is already stopped");
            return Ok(());
        }

        *is_running = false;
        info!("Stopping audio device monitor...");
        Ok(())
    }

    /// Добавление коллбэка для уведомлений
    pub fn add_callback<F>(&self, callback: F) -> JarvisResult<()>
    where
        F: Fn(AudioDeviceEvent) + Send + Sync + 'static,
    {
        let mut callbacks = self.callbacks.lock()
            .map_err(|e| JarvisError::RecorderError(RecorderError::InitializationFailed(
                format!("Failed to acquire callbacks lock: {}", e)
            )))?;

        callbacks.push(Box::new(callback));
        info!("Audio device callback added");
        Ok(())
    }

    /// Получение текущего списка устройств
    pub fn get_devices(&self) -> JarvisResult<Vec<AudioDevice>> {
        let devices = self.devices.lock()
            .map_err(|e| JarvisError::RecorderError(RecorderError::InitializationFailed(
                format!("Failed to acquire devices lock: {}", e)
            )))?;

        Ok(devices.values().cloned().collect())
    }

    /// Получение устройства по ID
    pub fn get_device(&self, id: i32) -> JarvisResult<Option<AudioDevice>> {
        let devices = self.devices.lock()
            .map_err(|e| JarvisError::RecorderError(RecorderError::InitializationFailed(
                format!("Failed to acquire devices lock: {}", e)
            )))?;

        Ok(devices.get(&id).cloned())
    }

    /// Инициальное сканирование устройств
    fn scan_devices_initial(&self) -> JarvisResult<()> {
        info!("Performing initial device scan...");

        let current_devices = self.get_system_devices()?;

        let mut devices = self.devices.lock()
            .map_err(|e| JarvisError::RecorderError(RecorderError::InitializationFailed(
                format!("Failed to acquire devices lock: {}", e)
            )))?;

        for device in current_devices {
            info!("Found audio device: {} (ID: {})", device.name, device.id);
            devices.insert(device.id, device);
        }

        info!("Initial device scan completed, found {} devices", devices.len());
        Ok(())
    }

    /// Обновленное сканирование устройств
    fn scan_devices_update(
        devices: &Arc<Mutex<HashMap<i32, AudioDevice>>>,
        callbacks: &Arc<Mutex<Vec<DeviceCallback>>>,
    ) -> JarvisResult<()> {
        let current_devices = match Self::get_system_devices_static() {
            Ok(devices) => devices,
            Err(e) => {
                warn!("Failed to scan devices: {}", e);
                return Ok(()); // Не критическая ошибка
            }
        };

        let mut devices_map = devices.lock()
            .map_err(|e| JarvisError::RecorderError(RecorderError::InitializationFailed(
                format!("Failed to acquire devices lock: {}", e)
            )))?;

        let callbacks_vec = callbacks.lock()
            .map_err(|e| JarvisError::RecorderError(RecorderError::InitializationFailed(
                format!("Failed to acquire callbacks lock: {}", e)
            )))?;

        // Создаем новую карту устройств
        let mut new_devices: HashMap<i32, AudioDevice> = HashMap::new();
        for device in current_devices {
            new_devices.insert(device.id, device);
        }

        // Находим добавленные устройства
        for (id, device) in &new_devices {
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
        let mut removed_devices = Vec::new();
        for id in devices_map.keys() {
            if !new_devices.contains_key(id) {
                removed_devices.push(*id);
            }
        }

        for id in removed_devices {
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
        for (id, new_device) in &new_devices {
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
        *devices_map = new_devices;

        Ok(())
    }

    /// Получение системных аудио устройств
    fn get_system_devices(&self) -> JarvisResult<Vec<AudioDevice>> {
        Self::get_system_devices_static()
    }

    /// Статическая версия получения системных устройств
    fn get_system_devices_static() -> JarvisResult<Vec<AudioDevice>> {
        let mut devices = Vec::new();

        // Получаем устройства через recorder модуль
        match recorder::get_available_devices() {
            Ok(recorder_devices) => {
                for (id, name) in recorder_devices {
                    let device = AudioDevice {
                        id,
                        name,
                        is_input: true,  // recorder возвращает только входные устройства
                        is_output: false,
                        is_default: id == recorder::get_selected_microphone_index().unwrap_or(-1),
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
                    format!("Failed to get Windows audio devices: {}", e)
                )))?;

            if output.status.success() {
                let json_str = String::from_utf8_lossy(&output.stdout);
                // Здесь можно парсить JSON и создавать AudioDevice
                // Для простоты создаем заглушки
                let device = AudioDevice {
                    id: 1000, // Смещение для выходных устройств
                    name: "Default Speakers".to_string(),
                    is_input: false,
                    is_output: true,
                    is_default: true,
                    is_available: true,
                };
                devices.push(device);
            }
        }

        #[cfg(target_os = "linux")]
        {
            // Linux: используем pactl для получения аудио устройств
            let output = std::process::Command::new("pactl")
                .args(&["list", "short", "sinks"])
                .output();

            if let Ok(output) = output {
                if output.status.success() {
                    let output_str = String::from_utf8_lossy(&output.stdout);
                    for (index, line) in output_str.lines().enumerate() {
                        let parts: Vec<&str> = line.split('\t').collect();
                        if parts.len() >= 2 {
                            let device = AudioDevice {
                                id: 2000 + index as i32, // Смещение для выходных устройств
                                name: parts[1].to_string(),
                                is_input: false,
                                is_output: true,
                                is_default: index == 0,
                                is_available: true,
                            };
                            devices.push(device);
                        }
                    }
                }
            }
        }

        Ok(devices)
    }

    /// Проверка доступности устройства
    pub fn is_device_available(&self, device_id: i32) -> bool {
        if let Ok(device) = self.get_device(device_id) {
            device.map(|d| d.is_available).unwrap_or(false)
        } else {
            false
        }
    }

    /// Установка устройства по умолчанию (если поддерживается)
    pub fn set_default_device(&self, device_id: i32, is_input: bool) -> JarvisResult<()> {
        #[cfg(target_os = "windows")]
        {
            // Windows: требует дополнительных инструментов или API
            warn!("Setting default device not implemented on Windows");
        }

        #[cfg(target_os = "linux")]
        {
            if is_input {
                // Устанавливаем микрофон по умолчанию
                let output = std::process::Command::new("pactl")
                    .args(&["set-default-source", &device_id.to_string()])
                    .output()
                    .map_err(|e| JarvisError::AudioError(AudioError::InitializationFailed(
                        format!("Failed to set default input device: {}", e)
                    )))?;

                if !output.status.success() {
                    return Err(JarvisError::AudioError(AudioError::InitializationFailed(
                        "Failed to set default input device".to_string()
                    )));
                }
            } else {
                // Устанавливаем динамики по умолчанию
                let output = std::process::Command::new("pactl")
                    .args(&["set-default-sink", &device_id.to_string()])
                    .output()
                    .map_err(|e| JarvisError::AudioError(AudioError::InitializationFailed(
                        format!("Failed to set default output device: {}", e)
                    )))?;

                if !output.status.success() {
                    return Err(JarvisError::AudioError(AudioError::InitializationFailed(
                        "Failed to set default output device".to_string()
                    )));
                }
            }
        }

        info!("Default device set: ID {}, input: {}", device_id, is_input);
        Ok(())
    }
}

// Глобальный экземпляр монитора
use once_cell::sync::OnceCell;
static AUDIO_MONITOR: OnceCell<Arc<AudioMonitor>> = OnceCell::new();

/// Инициализация глобального монитора аудио устройств
pub fn init_audio_monitor() -> JarvisResult<()> {
    let monitor = Arc::new(AudioMonitor::new());

    AUDIO_MONITOR.set(monitor.clone())
        .map_err(|_| JarvisError::RecorderError(RecorderError::InitializationFailed(
            "Audio monitor already initialized".to_string()
        )))?;

    monitor.start()?;
    info!("Global audio monitor initialized");
    Ok(())
}

/// Получение глобального монитора
pub fn get_audio_monitor() -> Option<Arc<AudioMonitor>> {
    AUDIO_MONITOR.get().cloned()
}

/// Добавление коллбэка в глобальный монитор
pub fn add_device_callback<F>(callback: F) -> JarvisResult<()>
where
    F: Fn(AudioDeviceEvent) + Send + Sync + 'static,
{
    let monitor = get_audio_monitor()
        .ok_or_else(|| JarvisError::RecorderError(RecorderError::InitializationFailed(
            "Audio monitor not initialized".to_string()
        )))?;

    monitor.add_callback(callback)
}

/// Получение списка всех устройств
pub fn get_all_devices() -> JarvisResult<Vec<AudioDevice>> {
    let monitor = get_audio_monitor()
        .ok_or_else(|| JarvisError::RecorderError(RecorderError::InitializationFailed(
            "Audio monitor not initialized".to_string()
        )))?;

    monitor.get_devices()
}

/// Graceful shutdown монитора
pub fn shutdown_audio_monitor() -> JarvisResult<()> {
    if let Some(monitor) = get_audio_monitor() {
        monitor.stop()?;
    }
    info!("Audio monitor shutdown completed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_audio_device_creation() {
        let device = AudioDevice {
            id: 1,
            name: "Test Device".to_string(),
            is_input: true,
            is_output: false,
            is_default: false,
            is_available: true,
        };

        assert_eq!(device.id, 1);
        assert_eq!(device.name, "Test Device");
        assert!(device.is_input);
        assert!(!device.is_output);
    }

    #[test]
    fn test_audio_monitor_creation() {
        let monitor = AudioMonitor::new();
        assert!(monitor.get_devices().unwrap().is_empty());
    }

    #[test]
    fn test_callback_addition() {
        let monitor = AudioMonitor::new();
        let callback_called = Arc::new(AtomicUsize::new(0));
        let callback_called_clone = callback_called.clone();

        let callback = move |_event: AudioDeviceEvent| {
            callback_called_clone.fetch_add(1, Ordering::SeqCst);
        };

        assert!(monitor.add_callback(callback).is_ok());
    }
}