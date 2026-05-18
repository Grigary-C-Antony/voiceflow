import { useState, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";

const MAX_RECORDING_MS = 120_000;
const MODELS_LIST = [
  { id: "tiny.en", name: "tiny.en (~75MB)" },
  { id: "tiny", name: "tiny (~75MB)" },
  { id: "base.en", name: "base.en (~142MB)" },
  { id: "base", name: "base (~142MB)" },
  { id: "small.en", name: "small.en (~466MB)" },
  { id: "small", name: "small (~466MB)" },
  { id: "medium.en", name: "medium.en (~1.5GB)" },
  { id: "medium", name: "medium (~1.5GB)" },
  { id: "large-v3", name: "large-v3 (~2.9GB)" },
];

const LANGUAGES = [
  { code: "en", name: "English" },
  { code: "es", name: "Spanish" },
  { code: "fr", name: "French" },
  { code: "de", name: "German" },
  { code: "it", name: "Italian" },
  { code: "ja", name: "Japanese" },
  { code: "zh", name: "Chinese" },
  { code: "ru", name: "Russian" },
  { code: "auto", name: "Auto Detect" }
];

type View = "main" | "settings";

interface AppConfig {
  active_model: string;
  output_method: string;
  typing_speed: number;
  translate: boolean;
  language: string;
  hotkey: string;
  always_on_top: boolean;
  grammar_enabled: boolean;
  openrouter_api_key: string;
  openrouter_model: string;
}

function App() {
  const [view, setView] = useState<View>("main");
  const [recording, setRecording] = useState(false);
  const [text, setText] = useState("Voice");
  
  const [config, setConfig] = useState<AppConfig>({
    active_model: "tiny.en",
    output_method: "type",
    typing_speed: 20,
    translate: false,
    language: "en",
    hotkey: "Control+Shift+Space",
    always_on_top: true,
    grammar_enabled: false,
    openrouter_api_key: "",
    openrouter_model: "openai/gpt-4o-mini"
  });

  const [availableModels, setAvailableModels] = useState<string[]>([]);
  const [downloadProgress, setDownloadProgress] = useState<Record<string, number>>({});
  const [downloadError, setDownloadError] = useState<string | null>(null);

  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const resetTextTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const mouseDownPos = useRef<{ x: number; y: number } | null>(null);
  const isDragging = useRef(false);

  useEffect(() => {
    invoke<string[]>("get_available_models").then(setAvailableModels).catch(console.error);
    invoke<AppConfig>("get_config").then(setConfig).catch(console.error);

    const unlistenStart = listen("recording-started", () => {
      if (resetTextTimeoutRef.current) clearTimeout(resetTextTimeoutRef.current);
      setRecording(true);
      setText("Listening...");
      timeoutRef.current = setTimeout(() => handleStop(), MAX_RECORDING_MS);
    });

    const unlistenStop = listen<string>("recording-stopped", (event) => {
      clearFallback();
      setRecording(false);
      setText(event.payload || "Done");
      if (resetTextTimeoutRef.current) clearTimeout(resetTextTimeoutRef.current);
      resetTextTimeoutRef.current = setTimeout(() => setText("Voice"), 3000);
    });

    const unlistenProgress = listen<{ model: string; progress: number }>("download-progress", (event) => {
      setDownloadProgress(prev => ({ ...prev, [event.payload.model]: event.payload.progress }));
    });
    
    const unlistenComplete = listen<string>("download-complete", (event) => {
      setDownloadProgress(prev => {
        const next = { ...prev };
        delete next[event.payload];
        return next;
      });
      setAvailableModels(prev => [...prev.filter(m => m !== event.payload), event.payload]);
    });
    
    const unlistenError = listen<{ model: string; error: string }>("download-error", (event) => {
      setDownloadError(`Failed to download ${event.payload.model}: ${event.payload.error}`);
      setDownloadProgress(prev => {
        const next = { ...prev };
        delete next[event.payload.model];
        return next;
      });
      setTimeout(() => setDownloadError(null), 5000);
    });

    const unlistenStatus = listen<string>("status-update", (event) => {
      setText(event.payload);
    });

    return () => {
      unlistenStart.then((f) => f());
      unlistenStop.then((f) => f());
      unlistenProgress.then((f) => f());
      unlistenComplete.then((f) => f());
      unlistenError.then((f) => f());
      unlistenStatus.then((f) => f());
    };
  }, []);

  const updateConfig = async (newConfig: Partial<AppConfig>) => {
    const updated = { ...config, ...newConfig };
    setConfig(updated);
    try {
      await invoke("set_config", { config: updated });
      if (newConfig.always_on_top !== undefined) {
        await invoke("set_always_on_top", { alwaysOnTop: newConfig.always_on_top });
      }
    } catch (e) {
      console.error(e);
    }
  };

  const handleHotkeyChange = async (e: React.KeyboardEvent) => {
    e.preventDefault();
    e.stopPropagation();
    
    // Ignore lonely modifier presses
    if (["Control", "Shift", "Alt", "Meta"].includes(e.key)) return;

    const parts = [];
    if (e.ctrlKey || e.metaKey) parts.push("Control");
    if (e.shiftKey) parts.push("Shift");
    if (e.altKey) parts.push("Alt");

    let key = e.key;
    if (key === " ") key = "Space";
    else if (key.length === 1) key = key.toUpperCase();

    parts.push(key);
    const hotkeyStr = parts.join("+");

    try {
      await invoke("update_hotkey", { hotkey: hotkeyStr });
      updateConfig({ hotkey: hotkeyStr });
    } catch (err) {
      console.error("Failed to bind hotkey", err);
    }
  };

  const clearFallback = () => {
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
      timeoutRef.current = null;
    }
  };

  const handleStop = async () => {
    clearFallback();
    setRecording(false);
    setText("Transcribing...");
    try {
      const result = await invoke<string>("stop_recording");
      setText(result || "Done");
    } catch {
      setText("Error");
    }
    if (resetTextTimeoutRef.current) clearTimeout(resetTextTimeoutRef.current);
    resetTextTimeoutRef.current = setTimeout(() => setText("Voice"), 3000);
  };

  const handleStart = async () => {
    if (resetTextTimeoutRef.current) clearTimeout(resetTextTimeoutRef.current);
    setRecording(true);
    setText("Listening...");
    try {
      await invoke("start_recording");
      timeoutRef.current = setTimeout(() => handleStop(), MAX_RECORDING_MS);
    } catch {
      setRecording(false);
      setText("Error");
      if (resetTextTimeoutRef.current) clearTimeout(resetTextTimeoutRef.current);
      resetTextTimeoutRef.current = setTimeout(() => setText("Voice"), 2000);
    }
  };

  const handleMicClick = () => {
    if (isDragging.current) return;
    recording ? handleStop() : handleStart();
  };

  const handleSettingsClick = async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (isDragging.current) return;
    try {
      await invoke("resize_window", { width: 380, height: 600 });
      setView("settings");
    } catch (e) {
      console.error(e);
    }
  };

  const handleBackClick = async () => {
    try {
      await invoke("resize_window", { width: 380, height: 100 });
      setView("main");
    } catch (e) {
      console.error(e);
    }
  };

  const handleSelectModel = async (model: string) => {
    updateConfig({ active_model: model });
  };

  const handleDownloadModel = async (model: string) => {
    try {
      setDownloadError(null);
      await invoke("download_model", { modelName: model });
      setDownloadProgress(prev => ({ ...prev, [model]: 0 }));
    } catch (e) {
      console.error(e);
    }
  };

  const handlePillMouseDown = (e: React.MouseEvent) => {
    mouseDownPos.current = { x: e.clientX, y: e.clientY };
    isDragging.current = false;
  };

  const handlePillMouseMove = (e: React.MouseEvent) => {
    if (!mouseDownPos.current) return;
    const dx = Math.abs(e.clientX - mouseDownPos.current.x);
    const dy = Math.abs(e.clientY - mouseDownPos.current.y);
    if (dx > 4 || dy > 4) isDragging.current = true;
  };

  const handlePillMouseUp = () => {
    mouseDownPos.current = null;
    setTimeout(() => {
      isDragging.current = false;
    }, 50);
  };

  if (view === "settings") {
    return (
      <div className="app-shell" data-tauri-drag-region>
        <div className="settings-panel" data-tauri-drag-region>
          <div className="settings-header">
            <button className="icon-btn" onClick={handleBackClick}>
              ⬅
            </button>
            <h2 className="settings-title">Settings</h2>
            <div style={{ width: 32 }} />
          </div>
          
          <div className="settings-content">
            {/* General Section */}
            <div className="settings-section">
              <h3>General</h3>
              <div className="setting-row">
                <label>Global Hotkey</label>
                <input 
                  type="text" 
                  className="hotkey-input"
                  value={config.hotkey}
                  readOnly
                  onKeyDown={handleHotkeyChange}
                  placeholder="Press keys..."
                />
              </div>
              <div className="setting-row">
                <label>Language</label>
                <select 
                  className="dropdown"
                  value={config.language} 
                  onChange={e => updateConfig({ language: e.target.value })}
                >
                  {LANGUAGES.map(lang => (
                    <option key={lang.code} value={lang.code}>{lang.name}</option>
                  ))}
                </select>
              </div>
              <div className="setting-row">
                <label>Always on Top</label>
                <input 
                  type="checkbox" 
                  checked={config.always_on_top}
                  onChange={e => updateConfig({ always_on_top: e.target.checked })}
                />
              </div>
              <div className="setting-row">
                <label>Translate to English</label>
                <input 
                  type="checkbox" 
                  checked={config.translate}
                  onChange={e => updateConfig({ translate: e.target.checked })}
                />
              </div>
            </div>

            {/* Grammar Engine Section */}
            <div className="settings-section">
              <h3>Grammar Engine</h3>
              <div className="setting-row">
                <label>Enable OpenRouter Cleanup</label>
                <input 
                  type="checkbox" 
                  checked={config.grammar_enabled}
                  onChange={e => updateConfig({ grammar_enabled: e.target.checked })}
                />
              </div>
              
              {config.grammar_enabled && (
                <>
                  <div className="setting-row vertical">
                    <label>OpenRouter API Key</label>
                    <input 
                      type="password" 
                      className="text-input"
                      value={config.openrouter_api_key}
                      onChange={e => updateConfig({ openrouter_api_key: e.target.value })}
                      placeholder="sk-or-v1-..."
                    />
                  </div>
                  <div className="setting-row vertical">
                    <label>OpenRouter Model</label>
                    <input 
                      type="text" 
                      className="text-input"
                      value={config.openrouter_model}
                      onChange={e => updateConfig({ openrouter_model: e.target.value })}
                      placeholder="openai/gpt-4o-mini"
                    />
                  </div>
                </>
              )}
            </div>

            {/* Output Section */}
            <div className="settings-section">
              <h3>Output</h3>
              <div className="setting-row">
                <label>Output Method</label>
                <select 
                  className="dropdown"
                  value={config.output_method} 
                  onChange={e => updateConfig({ output_method: e.target.value })}
                >
                  <option value="type">Type Text (Keyboard)</option>
                  <option value="clipboard">Copy to Clipboard</option>
                </select>
              </div>
              {config.output_method === "type" && (
                <div className="setting-row vertical">
                  <label>Typing Speed (Delay: {config.typing_speed}ms)</label>
                  <input 
                    type="range" 
                    min="0" 
                    max="100" 
                    value={config.typing_speed}
                    onChange={e => updateConfig({ typing_speed: parseInt(e.target.value) })}
                  />
                </div>
              )}
            </div>

            {/* Models Section */}
            <div className="settings-section">
              <h3>Whisper Models</h3>
              {downloadError && <div className="error-text">{downloadError}</div>}
              <div className="model-list">
                {MODELS_LIST.map((model) => {
                  const isAvailable = availableModels.includes(model.id);
                  const isActive = config.active_model === model.id;
                  const progress = downloadProgress[model.id];
                  const isDownloading = progress !== undefined;

                  return (
                    <div key={model.id} className={`model-item ${isActive ? 'active' : ''}`}>
                      <div className="model-info">
                        <span className="model-name">{model.name}</span>
                        {isDownloading && (
                          <div className="progress-bar-bg">
                            <div className="progress-bar-fill" style={{ width: `${progress}%` }} />
                          </div>
                        )}
                      </div>
                      <div className="model-actions">
                        {isActive ? (
                          <span className="status-badge active">Selected</span>
                        ) : isAvailable ? (
                          <button className="action-btn select" onClick={() => handleSelectModel(model.id)}>
                            Select
                          </button>
                        ) : isDownloading ? (
                          <span className="status-badge downloading">{progress.toFixed(0)}%</span>
                        ) : (
                          <button className="action-btn download" onClick={() => handleDownloadModel(model.id)}>
                            Download
                          </button>
                        )}
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="app-shell" data-tauri-drag-region>
      <div
        className={`floating-pill ${(recording || text !== "Voice") ? "expanded recording" : ""}`}
        data-tauri-drag-region
        onMouseDown={handlePillMouseDown}
        onMouseMove={handlePillMouseMove}
        onMouseUp={handlePillMouseUp}
      >
        <button
          className="mic-btn"
          onMouseDown={(e) => e.stopPropagation()}
          onClick={handleMicClick}
        >
          {recording ? "⏹" : "🎙"}
        </button>

        <div className="status-text" data-tauri-drag-region>
          {text === "Listening..." ? (
            <div className="listening-animation">
              <span /><span /><span /><span />
            </div>
          ) : (
            text
          )}
        </div>
        
        {!recording && (
          <button 
            className="icon-btn settings-btn" 
            onMouseDown={(e) => e.stopPropagation()}
            onClick={handleSettingsClick}
          >
            ⚙
          </button>
        )}
      </div>
    </div>
  );
}

export default App;
