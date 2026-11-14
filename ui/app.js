const copy = {
  en: {
    eyebrow: "Voice input",
    status: { idle: "Ready", starting: "Starting", listening: "Listening", finalizing: "Finishing" },
    legHotkey: "Hotkey",
    lblBinding: "Binding",
    hintBinding: "Right Alt is the default. Fn is not visible to Windows. On AltGr layouts, switch to Ctrl+Win.",
    lblActivation: "Activation",
    ptt: "Push to talk (hold)",
    toggle: "Toggle",
    legAsr: "Soniox",
    hintKey: "The key is stored in Windows Credential Manager, not in the JSON config.",
    lblKey: "API key",
    saveKey: "Save key",
    clearKey: "Clear",
    keyOn: "A key is saved.",
    keyOff: "No key saved.",
    lblModel: "Model",
    lblHints: "Language hints",
    lblSensitivity: "Endpoint sensitivity",
    lblMic: "Microphone",
    micDefault: "System default",
    legOutput: "Output",
    lblInject: "Injection",
    injectAuto: "Auto (Unicode, clipboard if needed)",
    injectUnicode: "Unicode only",
    injectClipboard: "Clipboard + Ctrl+V",
    lblPartials: "Show partial text near the cursor",
    hintUipi: "Elevated windows ignore injected input unless the app is signed with uiAccess. That is a known limitation.",
    save: "Save",
    saved: "Saved.",
    keySaved: "Key saved.",
    keyCleared: "Key cleared.",
  },
  zh: {
    eyebrow: "语音输入",
    status: { idle: "就绪", starting: "启动中", listening: "聆听中", finalizing: "结束中" },
    legHotkey: "热键",
    lblBinding: "按键",
    hintBinding: "默认是右 Alt。Fn 在 Windows 上通常不可见。如果右 Alt 是 AltGr，请改用 Ctrl+Win。",
    lblActivation: "触发方式",
    ptt: "按住说话",
    toggle: "切换",
    legAsr: "Soniox",
    hintKey: "密钥存在 Windows 凭据管理器里，不会写入 JSON 配置。",
    lblKey: "API 密钥",
    saveKey: "保存密钥",
    clearKey: "清除",
    keyOn: "已保存密钥。",
    keyOff: "尚未保存密钥。",
    lblModel: "模型",
    lblHints: "语言提示",
    lblSensitivity: "断句灵敏度",
    lblMic: "麦克风",
    micDefault: "系统默认",
    legOutput: "输出",
    lblInject: "注入方式",
    injectAuto: "自动（Unicode，失败再用剪贴板）",
    injectUnicode: "仅 Unicode",
    injectClipboard: "剪贴板 + Ctrl+V",
    lblPartials: "在光标附近显示中间结果",
    hintUipi: "未签名的应用无法向以管理员身份运行的窗口注入文字（UIPI）。这是已知限制。",
    save: "保存",
    saved: "已保存。",
    keySaved: "密钥已保存。",
    keyCleared: "密钥已清除。",
  },
};

const $ = (id) => document.getElementById(id);
let lang = "en";

function t() {
  return lang === "zh" ? copy.zh : copy.en;
}

function applyCopy() {
  const text = t();
  $("eyebrow").textContent = text.eyebrow;
  $("leg-hotkey").textContent = text.legHotkey;
  $("lbl-binding").textContent = text.lblBinding;
  $("hint-binding").textContent = text.hintBinding;
  $("lbl-activation").textContent = text.lblActivation;
  $("activation").options[0].textContent = text.ptt;
  $("activation").options[1].textContent = text.toggle;
  $("leg-asr").textContent = text.legAsr;
  $("hint-key").textContent = text.hintKey;
  $("lbl-key").textContent = text.lblKey;
  $("save-key").textContent = text.saveKey;
  $("clear-key").textContent = text.clearKey;
  $("lbl-model").textContent = text.lblModel;
  $("lbl-hints").textContent = text.lblHints;
  $("lbl-model").textContent = text.lblModel;
  $("lbl-mic").textContent = text.lblMic;
  $("leg-output").textContent = text.legOutput;
  $("lbl-inject").textContent = text.lblInject;
  $("injection").options[0].textContent = text.injectAuto;
  $("injection").options[1].textContent = text.injectUnicode;
  $("injection").options[2].textContent = text.injectClipboard;
  $("lbl-partials").textContent = text.lblPartials;
  $("hint-uipi").textContent = text.hintUipi;
  $("save").textContent = text.save;
  document.documentElement.lang = lang === "zh" ? "zh-CN" : "en";
  $("lang-en").classList.toggle("active", lang !== "zh");
  $("lang-zh").classList.toggle("active", lang === "zh");
  const mic = $("mic");
  if (mic.options.length) mic.options[0].textContent = text.micDefault;
}

function tauri() {
  return window.__TAURI__;
}

async function invoke(command, args) {
  const api = tauri();
  if (!api) throw new Error("Tauri bridge is unavailable");
  return api.core.invoke(command, args);
}

function setStatus(phase, error) {
  const text = t();
  const dot = $("status-dot");
  dot.className = "dot" + (error ? " error" : phase && phase !== "idle" ? " " + phase : "");
  $("status-text").textContent = error ? (lang === "zh" ? "出错" : "Error") : (text.status[phase] || text.status.idle);
  const err = $("status-error");
  err.hidden = !error;
  err.textContent = error || "";
}

function fill(settings) {
  $("binding").value = settings.binding;
  $("activation").value = settings.activation;
  $("injection").value = settings.injection_method;
  $("partials").checked = settings.show_partials;
  $("model").value = settings.model;
  $("hints").value = settings.language_hints;
  $("sensitivity").value = String(settings.endpoint_sensitivity);
  $("sensitivity-value").textContent = Number(settings.endpoint_sensitivity).toFixed(1);
  lang = String(settings.ui_language || "en").toLowerCase().startsWith("zh") ? "zh" : "en";
  applyCopy();
  $("key-state").textContent = settings.api_key_set ? t().keyOn : t().keyOff;
  $("config-path").textContent = settings.config_path;
  const mic = $("mic");
  const current = settings.input_device || "";
  for (const option of mic.options) option.selected = option.value === current;
}

async function loadMics(selected) {
  const mics = await invoke("list_microphones");
  const select = $("mic");
  const first = select.options[0];
  select.replaceChildren(first);
  for (const name of mics) {
    const option = document.createElement("option");
    option.value = name;
    option.textContent = name;
    if (name === selected) option.selected = true;
    select.appendChild(option);
  }
}

async function boot() {
  applyCopy();
  const settings = await invoke("get_settings");
  await loadMics(settings.input_device || "");
  fill(settings);
  const snap = await invoke("session_snapshot");
  setStatus(snap.phase, snap.error);
  const api = tauri();
  if (api) {
    await api.event.listen("session-update", (event) => {
      setStatus(event.payload.phase, event.payload.error);
    });
  }
}

$("lang-en").addEventListener("click", () => { lang = "en"; applyCopy(); });
$("lang-zh").addEventListener("click", () => { lang = "zh"; applyCopy(); });
$("sensitivity").addEventListener("input", () => {
  $("sensitivity-value").textContent = Number($("sensitivity").value).toFixed(1);
});
document.querySelectorAll("[data-binding]").forEach((button) => {
  button.addEventListener("click", () => { $("binding").value = button.dataset.binding; });
});

$("save-key").addEventListener("click", async () => {
  try {
    await invoke("set_api_key", { apiKey: $("api-key").value });
    $("api-key").value = "";
    $("key-state").textContent = t().keySaved;
  } catch (err) {
    $("key-state").textContent = String(err);
  }
});

$("clear-key").addEventListener("click", async () => {
  try {
    await invoke("clear_api_key");
    $("key-state").textContent = t().keyCleared;
  } catch (err) {
    $("key-state").textContent = String(err);
  }
});

$("settings").addEventListener("submit", async (event) => {
  event.preventDefault();
  const payload = {
    binding: $("binding").value,
    activation: $("activation").value,
    injection_method: $("injection").value,
    show_partials: $("partials").checked,
    input_device: $("mic").value || null,
    model: $("model").value,
    endpoint_sensitivity: Number($("sensitivity").value),
    language_hints: $("hints").value,
    ui_language: lang === "zh" ? "zh-CN" : "en",
  };
  try {
    const settings = await invoke("save_settings", { update: payload });
    fill(settings);
    $("save-state").textContent = t().saved;
  } catch (err) {
    $("save-state").textContent = String(err);
  }
});

boot().catch((err) => {
  $("status-error").hidden = false;
  $("status-error").textContent = String(err);
});
