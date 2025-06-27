<!-- gui/src/pages/settings.svelte - Обновленная страница настроек -->
<script lang="ts">
  // IMPORTS
  import { invoke } from "@tauri-apps/api/tauri"
  import { goto } from '@roxi/routify'
  import { onMount, onDestroy } from 'svelte'
  import { startListening, stopListening, showInExplorer } from "@/functions";
  import { setTimeout } from 'worker-timers';

  import { feedback_link, log_file_path } from "@/stores";

  // COMPONENTS & STUFF
  import HDivider from "@/components/elements/HDivider.svelte"
  import Footer from "@/components/Footer.svelte"

  import {
    Notification, Button, Text, Tabs, Space, Alert, Input, InputWrapper,
    NativeSelect, PasswordInput, Switch, Slider, Accordion, Badge
  } from '@svelteuidev/core';
  import {
    Check, Mix, Cube, Code, Gear, QuestionMarkCircled, CrossCircled,
    Robot, Globe, Microphone2, Speaker, Brain
  } from 'radix-icons-svelte';

  // VARIABLES
  let available_microphones = [];
  let available_speakers = [];
  let available_voices = [];
  let settings_saved = false;
  let save_button_disabled = false;
  let test_tts_playing = false;
  let ai_test_loading = false;

  // Основные настройки
  let assistant_voice_val = "";
  let selected_microphone = "";
  let selected_speaker = "";
  let selected_wake_word_engine = "";

  // AI настройки
  let api_key_picovoice = "";
  let api_key_openai = "";
  let api_key_openrouter = "";
  let ai_model = "anthropic/claude-3-haiku";
  let ai_temperature = 0.7;
  let ai_max_tokens = 1000;

  // TTS настройки
  let tts_engine = "system";
  let tts_voice = "default";
  let tts_speed = 1.0;
  let tts_volume = 0.8;

  // Продвинутые настройки
  let enable_conversation_mode = false;
  let enable_document_search = true;
  let auto_open_documents = true;
  let device_monitoring = true;

  // SHARED VALUES
  import { assistant_voice } from "@/stores"
  assistant_voice.subscribe(value => {
    assistant_voice_val = value;
  });

  // Модели для OpenRouter
  const ai_models = [
    { label: 'Claude 3 Haiku (Быстрый)', value: 'anthropic/claude-3-haiku' },
    { label: 'Claude 3 Sonnet (Сбалансированный)', value: 'anthropic/claude-3-sonnet' },
    { label: 'Claude 3 Opus (Лучший)', value: 'anthropic/claude-3-opus' },
    { label: 'GPT-4 Turbo', value: 'openai/gpt-4-turbo' },
    { label: 'GPT-3.5 Turbo', value: 'openai/gpt-3.5-turbo' },
    { label: 'Gemini Pro', value: 'google/gemini-pro' },
    { label: 'Llama 2 70B', value: 'meta-llama/llama-2-70b-chat' },
  ];

  const tts_engines = [
    { label: 'Системный TTS', value: 'system' },
    { label: 'OpenAI TTS (Требует ключ)', value: 'openai' },
    { label: 'Локальный TTS (В разработке)', value: 'local' }
  ];

  // FUNCTIONS
  async function save_settings() {
    save_button_disabled = true;
    settings_saved = false;

    try {
      // Основные настройки
      await invoke("db_write", {key: "assistant_voice", val: assistant_voice_val});
      await invoke("db_write", {key: "selected_microphone", val: selected_microphone});
      await invoke("db_write", {key: "selected_speaker", val: selected_speaker});
      await invoke("db_write", {key: "selected_wake_word_engine", val: selected_wake_word_engine});

      // API ключи
      await invoke("db_write", {key: "api_key_picovoice", val: api_key_picovoice});
      await invoke("db_write", {key: "api_key_openai", val: api_key_openai});
      await invoke("db_write", {key: "api_key_openrouter", val: api_key_openrouter});

      // AI настройки
      await invoke("db_write", {key: "ai_model", val: ai_model});
      await invoke("db_write", {key: "ai_temperature", val: ai_temperature});
      await invoke("db_write", {key: "ai_max_tokens", val: ai_max_tokens});

      // TTS настройки
      await invoke("db_write", {key: "tts_engine", val: tts_engine});
      await invoke("db_write", {key: "tts_voice", val: tts_voice});
      await invoke("db_write", {key: "tts_speed", val: tts_speed});
      await invoke("db_write", {key: "tts_volume", val: tts_volume});

      // Продвинутые настройки
      await invoke("db_write", {key: "enable_conversation_mode", val: enable_conversation_mode});
      await invoke("db_write", {key: "enable_document_search", val: enable_document_search});
      await invoke("db_write", {key: "auto_open_documents", val: auto_open_documents});
      await invoke("db_write", {key: "device_monitoring", val: device_monitoring});

      // Обновляем shared переменные
      assistant_voice.set(assistant_voice_val);

      // Применяем настройки
      await invoke("apply_settings");

      settings_saved = true;
      setTimeout(() => {
        settings_saved = false;
      }, 5000);

    } catch (error) {
      console.error("Failed to save settings:", error);
      alert("Ошибка сохранения настроек: " + error);
    }

    setTimeout(() => {
      save_button_disabled = false;
    }, 1000);

    // Перезапускаем прослушивание с новыми настройками
    if (device_monitoring) {
      stopListening(() => {
        setTimeout(() => {
          startListening();
        }, 1000);
      });
    }
  }

  async function test_tts() {
    if (test_tts_playing) return;

    test_tts_playing = true;
    try {
      await invoke("test_tts", {
        text: "Привет! Это тест синтеза речи JARVIS.",
        voice: tts_voice,
        speed: tts_speed,
        volume: tts_volume
      });
    } catch (error) {
      console.error("TTS test failed:", error);
      alert("Ошибка тестирования TTS: " + error);
    }
    test_tts_playing = false;
  }

  async function test_ai_connection() {
    if (ai_test_loading) return;

    ai_test_loading = true;
    try {
      const response = await invoke("test_ai_connection", {
        openai_key: api_key_openai,
        openrouter_key: api_key_openrouter,
        model: ai_model
      });

      alert("AI подключение успешно! Ответ: " + response);
    } catch (error) {
      console.error("AI test failed:", error);
      alert("Ошибка подключения к AI: " + error);
    }
    ai_test_loading = false;
  }

  async function refresh_audio_devices() {
    try {
      // Обновляем микрофоны
      let _available_microphones = await invoke("get_audio_input_devices");
      available_microphones = [];
      Object.entries(_available_microphones).forEach(entry => {
        const [k, v] = entry;
        available_microphones.push({
          label: v,
          value: k
        });
      });

      // Обновляем динамики
      let _available_speakers = await invoke("get_audio_output_devices");
      available_speakers = [];
      Object.entries(_available_speakers).forEach(entry => {
        const [k, v] = entry;
        available_speakers.push({
          label: v,
          value: k
        });
      });

      // Обновляем голоса
      let _available_voices = await invoke("get_available_voices");
      available_voices = _available_voices.map(voice => ({
        label: voice,
        value: voice
      }));

      // Обновляем UI
      available_microphones = available_microphones;
      available_speakers = available_speakers;
      available_voices = available_voices;

    } catch (error) {
      console.error("Failed to refresh devices:", error);
    }
  }

  // Автообновление устройств
  let deviceRefreshInterval;

  // CODE
  onMount(async () => {
    // Загружаем устройства
    await refresh_audio_devices();

    // Загружаем настройки из базы
    try {
      selected_microphone = await invoke("db_read", {key: "selected_microphone"}) || "";
      selected_speaker = await invoke("db_read", {key: "selected_speaker"}) || "";
      selected_wake_word_engine = await invoke("db_read", {key: "selected_wake_word_engine"}) || "rustpotter";

      api_key_picovoice = await invoke("db_read", {key: "api_key_picovoice"}) || "";
      api_key_openai = await invoke("db_read", {key: "api_key_openai"}) || "";
      api_key_openrouter = await invoke("db_read", {key: "api_key_openrouter"}) || "";

      ai_model = await invoke("db_read", {key: "ai_model"}) || "anthropic/claude-3-haiku";
      ai_temperature = await invoke("db_read", {key: "ai_temperature"}) || 0.7;
      ai_max_tokens = await invoke("db_read", {key: "ai_max_tokens"}) || 1000;

      tts_engine = await invoke("db_read", {key: "tts_engine"}) || "system";
      tts_voice = await invoke("db_read", {key: "tts_voice"}) || "default";
      tts_speed = await invoke("db_read", {key: "tts_speed"}) || 1.0;
      tts_volume = await invoke("db_read", {key: "tts_volume"}) || 0.8;

      enable_conversation_mode = await invoke("db_read", {key: "enable_conversation_mode"}) || false;
      enable_document_search = await invoke("db_read", {key: "enable_document_search"}) || true;
      auto_open_documents = await invoke("db_read", {key: "auto_open_documents"}) || true;
      device_monitoring = await invoke("db_read", {key: "device_monitoring"}) || true;

    } catch (error) {
      console.error("Failed to load settings:", error);
    }

    // Запускаем автообновление устройств если включено
    if (device_monitoring) {
      deviceRefreshInterval = setInterval(refresh_audio_devices, 3000);
    }
  });

  onDestroy(() => {
    if (deviceRefreshInterval) {
      clearInterval(deviceRefreshInterval);
    }
  });

  // Реактивность для автообновления устройств
  $: if (device_monitoring && !deviceRefreshInterval) {
    deviceRefreshInterval = setInterval(refresh_audio_devices, 3000);
  } else if (!device_monitoring && deviceRefreshInterval) {
    clearInterval(deviceRefreshInterval);
    deviceRefreshInterval = null;
  }
</script>

<Space h="xl" />

<Notification title='JARVIS v2.0 - Продвинутые настройки' icon={Robot} color='blue' withCloseButton={false}>
  Голосовой ассистент с поддержкой AI, TTS и горячего подключения устройств.<br />
  Сообщайте об ошибках в <a href="{feedback_link}" target="_blank">наш телеграм бот</a>.
  <Space h="sm" />
  <Button color="gray" radius="md" size="xs" uppercase on:click={() => {showInExplorer(log_file_path)}}>
    📁 Открыть папку с логами
  </Button>
  <Button color="blue" radius="md" size="xs" uppercase on:click={refresh_audio_devices}>
    🔄 Обновить устройства
  </Button>
</Notification>

<Space h="xl" />

{#if settings_saved}
<Notification title='Настройки сохранены!' icon={Check} color='teal' on:close="{() => {settings_saved = false}}">
  Все изменения применены. Ассистент перезапущен с новыми настройками.
</Notification>
<Space h="xl" />
{/if}

<Tabs class="form" color='#8AC832' position="left">

  <!-- Основные настройки -->
  <Tabs.Tab label='Основные' icon={Gear}>
    <Space h="sm" />

    <NativeSelect data={[
      { label: 'JARVIS Ремейк (от Хауди)', value: 'jarvis-remake' },
      { label: 'JARVIS Original (из фильмов)', value: 'jarvis-og' },
      { label: 'Пользовательский', value: 'custom' }
    ]}
    label="Пакет голоса ассистента"
    description="Звуковые сигналы и реакции ассистента."
    variant="filled"
    bind:value={assistant_voice_val}
    />

    <Space h="md" />

    <Switch
      bind:checked={enable_conversation_mode}
      label="Режим разговора"
      description="Ассистент запоминает контекст разговора"
    />

    <Space h="sm" />

    <Switch
      bind:checked={enable_document_search}
      label="Поиск документов"
      description="Ассистент может искать и открывать файлы"
    />

    <Space h="sm" />

    <Switch
      bind:checked={auto_open_documents}
      label="Автоматическое открытие"
      description="Найденные документы открываются автоматически"
    />
  </Tabs.Tab>

  <!-- Аудио устройства -->
  <Tabs.Tab label='Аудио' icon={Mix}>
    <Space h="sm" />

    <div class="device-section">
      <div class="device-header">
        <Microphone2 size={16} />
        <Text weight="bold">Устройства ввода</Text>
        {#if device_monitoring}
          <Badge color="green" size="xs">Мониторинг активен</Badge>
        {/if}
      </div>

      <NativeSelect data={available_microphones}
        label="Микрофон для прослушивания"
        description="Выберите микрофон для распознавания команд"
        variant="filled"
        bind:value={selected_microphone}
      />
    </div>

    <Space h="md" />

    <div class="device-section">
      <div class="device-header">
        <Speaker size={16} />
        <Text weight="bold">Устройства вывода</Text>
      </div>

      <NativeSelect data={available_speakers}
        label="Динамики для воспроизведения"
        description="Выберите устройство для воспроизведения звуков"
        variant="filled"
        bind:value={selected_speaker}
      />
    </div>

    <Space h="md" />

    <Switch
      bind:checked={device_monitoring}
      label="Мониторинг устройств"
      description="Автоматическое обнаружение новых аудио устройств"
    />
  </Tabs.Tab>

  <!-- Распознавание речи -->
  <Tabs.Tab label='Распознавание' icon={Cube}>
    <Space h="sm" />

    <NativeSelect data={[
      { label: 'Rustpotter (Рекомендуется)', value: 'rustpotter' },
      { label: 'Vosk (Медленный)', value: 'vosk' },
      { label: 'Picovoice Porcupine (API)', value: 'picovoice' }
    ]}
    label="Движок распознавания активации (Wake Word)"
    description="Выберите систему для распознавания фразы активации 'Джарвис'"
    variant="filled"
    bind:value={selected_wake_word_engine}
    />

    {#if selected_wake_word_engine === "picovoice"}
      <Space h="sm" />
      <Alert title="Требуется API ключ Picovoice" color="orange">
        Для работы Picovoice необходим API ключ. Получите его на
        <a href="https://console.picovoice.ai/" target="_blank">console.picovoice.ai</a>
      </Alert>
      <Space h="sm" />
      <PasswordInput
        label="API ключ Picovoice"
        placeholder="Введите ключ API..."
        bind:value={api_key_picovoice}
        description="Ключ хранится локально и не передается третьим лицам"
      />
    {/if}
  </Tabs.Tab>

  <!-- AI интеграция -->
  <Tabs.Tab label='AI Интеграция' icon={Brain}>
    <Space h="sm" />

    <Alert title="Настройка AI помощника" color="blue">
      JARVIS может использовать мощные языковые модели для ответов на вопросы,
      выполнения команд и поиска документов.
    </Alert>

    <Space h="md" />

    <Accordion multiple>
      <Accordion.Item value="openrouter">
        <svelte:fragment slot="control">
          <div class="ai-provider">
            <Globe size={20} />
            <div>
              <Text weight="bold">OpenRouter API</Text>
              <Text size="sm" color="dimmed">Доступ к Claude, GPT-4, Llama и другим моделям</Text>
            </div>
            {#if api_key_openrouter}
              <Badge color="green" size="xs">Настроен</Badge>
            {/if}
          </div>
        </svelte:fragment>

        <PasswordInput
          label="API ключ OpenRouter"
          placeholder="sk-or-v1-..."
          bind:value={api_key_openrouter}
          description="Получите ключ на openrouter.ai - доступ к лучшим AI моделям"
        />

        <Space h="sm" />

        <NativeSelect
          data={ai_models}
          label="Модель AI"
          description="Выберите языковую модель для обработки запросов"
          bind:value={ai_model}
        />
      </Accordion.Item>

      <Accordion.Item value="openai">
        <svelte:fragment slot="control">
          <div class="ai-provider">
            <Robot size={20} />
            <div>
              <Text weight="bold">OpenAI API</Text>
              <Text size="sm" color="dimmed">GPT-4, GPT-3.5 Turbo и TTS</Text>
            </div>
            {#if api_key_openai}
              <Badge color="green" size="xs">Настроен</Badge>
            {/if}
          </div>
        </svelte:fragment>

        <PasswordInput
          label="API ключ OpenAI"
          placeholder="sk-..."
          bind:value={api_key_openai}
          description="Получите ключ на platform.openai.com"
        />
      </Accordion.Item>
    </Accordion>

    <Space h="md" />

    <div class="ai-settings">
      <Text weight="bold" size="sm">Параметры генерации</Text>
      <Space h="xs" />

      <div class="slider-group">
        <Text size="sm">Творчество (Temperature): {ai_temperature}</Text>
        <Slider
          bind:value={ai_temperature}
          min={0}
          max={2}
          step={0.1}
          marks={[
            { value: 0, label: 'Точный' },
            { value: 1, label: 'Сбалансированный' },
            { value: 2, label: 'Творческий' }
          ]}
        />
      </div>

      <Space h="sm" />

      <div class="slider-group">
        <Text size="sm">Длина ответа (Токены): {ai_max_tokens}</Text>
        <Slider
          bind:value={ai_max_tokens}
          min={100}
          max={4000}
          step={100}
          marks={[
            { value: 100, label: 'Короткий' },
            { value: 1000, label: 'Средний' },
            { value: 4000, label: 'Длинный' }
          ]}
        />
      </div>
    </div>

    <Space h="md" />

    <Button
      loading={ai_test_loading}
      leftIcon={Brain}
      variant="light"
      color="blue"
      on:click={test_ai_connection}
    >
      Тестировать AI подключение
    </Button>
  </Tabs.Tab>

  <!-- Синтез речи -->
  <Tabs.Tab label='Синтез речи' icon={Speaker}>
    <Space h="sm" />

    <NativeSelect
      data={tts_engines}
      label="Движок синтеза речи"
      description="Выберите систему для озвучивания ответов"
      bind:value={tts_engine}
    />

    <Space h="sm" />

    <NativeSelect
      data={available_voices}
      label="Голос"
      description="Выберите голос для синтеза речи"
      bind:value={tts_voice}
    />

    <Space h="md" />

    <div class="tts-controls">
      <div class="slider-group">
        <Text size="sm">Скорость речи: {tts_speed}x</Text>
        <Slider
          bind:value={tts_speed}
          min={0.5}
          max={2}
          step={0.1}
          marks={[
            { value: 0.5, label: 'Медленно' },
            { value: 1, label: 'Нормально' },
            { value: 2, label: 'Быстро' }
          ]}
        />
      </div>

      <Space h="sm" />

      <div class="slider-group">
        <Text size="sm">Громкость: {Math.round(tts_volume * 100)}%</Text>
        <Slider
          bind:value={tts_volume}
          min={0}
          max={1}
          step={0.1}
          marks={[
            { value: 0, label: 'Тихо' },
            { value: 0.5, label: 'Средне' },
            { value: 1, label: 'Громко' }
          ]}
        />
      </div>
    </div>

    <Space h="md" />

    <Button
      loading={test_tts_playing}
      leftIcon={Speaker}
      variant="light"
      color="green"
      on:click={test_tts}
    >
      Тестировать синтез речи
    </Button>
  </Tabs.Tab>

</Tabs>

<Space h="xl" />

<div class="save-section">
  <Button
    disabled={save_button_disabled}
    size="lg"
    color="green"
    fullWidth
    uppercase
    on:click={save_settings}
  >
    {save_button_disabled ? 'Сохранение...' : '💾 Сохранить все настройки'}
  </Button>
</div>

<HDivider />
<Footer />

<style lang="scss">
.device-section {
  border: 1px solid #444;
  border-radius: 8px;
  padding: 12px;

  .device-header {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 12px;
  }
}

.ai-provider {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 100%;
}

.ai-settings {
  border: 1px solid #444;
  border-radius: 8px;
  padding: 16px;
}

.slider-group {
  margin-bottom: 16px;
}

.tts-controls {
  border: 1px solid #444;
  border-radius: 8px;
  padding: 16px;
}

.save-section {
  margin: 24px 0;
}

:global(.form) {
  :global(.svelteui-Tab-label) {
    font-size: 16px !important;
    font-weight: 600 !important;
  }

  :global(.svelteui-Tabs-tab[data-active="true"]) {
    border-bottom: 2px solid #8AC832 !important;
  }
}
</style>