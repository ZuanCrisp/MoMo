import { Bridge } from "../core/bridge";
import { PROVIDERS, providerInfo, LOCAL_DEFAULTS, LOCAL_BALANCED, type AIConfig, type AIProfile, type AIProvider, type AIModel, type AIKeyStatus } from "../core/ai";
import type { Settings } from "../core/state";
import { h, clear } from "../views/dom";
import { smoothSelect, syncSelect } from "../views/select";

const clone = <T>(value: T): T => structuredClone(value);
const message = (error: unknown) => String(error).replace(/^Error:\s*/, "");
const newId = () => crypto.randomUUID();

function field(label: string, input: HTMLInputElement | HTMLSelectElement, hint?: string) {
  input.id ||= `ai-${newId()}`;
  const id = input.id;
  return h("div", { class: "ai-field" }, h("label", { for: id, text: label }), input instanceof HTMLSelectElement ? smoothSelect(input) : input,
    hint ? h("span", { class: "hint", text: hint }) : null);
}

export function createAISection(initial: AIConfig, onSaved: (settings: Settings) => void) {
  let stored = clone(initial);
  let config = clone(initial);
  let selectedId = config.activeProfileId || config.profiles[0]?.id || "";
  let tab: "profiles" | "fallback" = "profiles";
  let dirty = false;
  let saving = false;
  let epoch = 0;
  let statuses: AIKeyStatus[] = [];
  let statusKnown = false;
  const values = new Map<string, string>();
  const models = new Map<string, AIModel[]>();

  const feedback = h("div", { class: "ai-feedback", "aria-live": "polite" });
  const state = h("span", { class: "hint", text: "Saved on this device" });
  const content = h("fieldset", { class: "ai-content" });
  const profilesTab = h("button", { type: "button", text: "Model profiles" });
  const fallbackTab = h("button", { type: "button", text: "Fallback order" });
  const save = h("button", { type: "button", class: "primary", text: "Save changes", disabled: true });
  const discard = h("button", { type: "button", text: "Discard", disabled: true });
  const add = h("button", { type: "button", class: "ai-add", text: "+ Add profile" });
  const el = h("section", { class: "ai-section", "aria-label": "AI and models" },
    h("div", { class: "ai-section-title" }, h("div", {}, h("h2", { text: "AI & models" }),
      h("p", { class: "hint", text: "Choose an online provider or a local model. Give each model its own keys and backups." })), add),
    h("div", { class: "ai-tabs", "aria-label": "AI settings views" }, profilesTab, fallbackTab),
    content, feedback,
    h("div", { class: "ai-save-bar" }, state, h("div", { class: "ai-actions" }, discard, save)),
  );

  function notice(text: string, kind: "ok" | "err" | "warn" = "ok") {
    clear(feedback); feedback.append(h("div", { class: `notice ${kind}`, text }));
  }
  function changed() {
    dirty = true; state.textContent = "Unsaved changes";
    save.disabled = saving; discard.disabled = saving;
    updateSidebar();
  }
  function hasKey(profile: AIProfile, id: string) {
    return statuses.some(status => status.profileId === profile.id && status.keyId === id && status.present);
  }
  function clearDraftKeys(profile: AIProfile) {
    for (const key of profile.keys) values.delete(`${profile.id}:${key.id}`);
  }
  function ready(profile: AIProfile) {
    return !!profile.model && (providerInfo(profile.provider).local && !profile.keys.length || profile.keys.some(key => hasKey(profile, key.id)));
  }
  let sidebar: HTMLElement | null = null;

  function updateSidebar() {
    if (!sidebar) return;
    clear(sidebar);
    for (const profile of config.profiles) {
      const local = providerInfo(profile.provider).local;
      const isActive = config.activeProfileId === profile.id;
      const button = h("button", { type: "button", class: `ai-profile-item${profile.id === selectedId ? " selected" : ""}`,
        "aria-pressed": String(profile.id === selectedId), "data-profile-id": profile.id },
        h("span", { class: "ai-profile-name", text: profile.name || "Untitled profile" }),
        h("span", { class: "ai-profile-model", text: profile.model || "Choose a model" }),
        h("span", { class: "ai-profile-badges" }, h("span", { class: `ai-badge${local ? " local" : ""}`, text: local ? "Local" : "Online" }),
          isActive ? h("span", { class: "ai-badge active", text: "Active" }) : null,
          !ready(profile) ? h("span", { class: "ai-badge draft", text: statusKnown ? "Setup" : "Check keys" }) : null),
      );
      button.addEventListener("click", () => { selectedId = profile.id; draw(); });
      sidebar.append(button);
    }
  }

  function draw() {
    epoch++;
    clear(content); sidebar = null;
    profilesTab.className = tab === "profiles" ? "selected" : "";
    fallbackTab.className = tab === "fallback" ? "selected" : "";
    profilesTab.setAttribute("aria-pressed", String(tab === "profiles"));
    fallbackTab.setAttribute("aria-pressed", String(tab === "fallback"));
    if (tab === "fallback") { drawFallback(); return; }
    sidebar = h("nav", { class: "ai-profiles", "aria-label": "Saved AI profiles" });
    const editor = h("div", { class: "ai-editor" });
    content.append(h("div", { class: "ai-workspace" }, sidebar, editor));
    updateSidebar();
    const profile = config.profiles.find(p => p.id === selectedId) || config.profiles[0];
    if (!profile) {
      editor.append(h("div", { class: "ai-empty" }, h("h3", { text: "Choose your first AI model" }),
        h("p", { class: "hint", text: "Add an online provider with an API key, or connect a downloaded model on this device." }),
        h("button", { type: "button", class: "primary", text: "Add profile", onclick: addProfile })));
      return;
    }
    selectedId = profile.id;
    drawEditor(editor, profile);
  }

  function addProfile() {
    if (config.profiles.length >= 32) { notice("You can save up to 32 profiles.", "warn"); return; }
    const profile: AIProfile = { id: newId(), name: `AI profile ${config.profiles.length + 1}`, provider: "openai", baseUrl: providerInfo("openai").url,
      model: "", keys: [{ id: newId(), label: "Main key" }], maxOutputTokens: 4096, timeoutSeconds: 120, webSearch: false, ...LOCAL_DEFAULTS };
    config.profiles.push(profile);
    if (!config.activeProfileId) config.activeProfileId = profile.id;
    selectedId = profile.id; tab = "profiles"; changed(); draw();
    content.querySelector<HTMLInputElement>("[data-field='profile-name']")?.focus();
  }

  function drawEditor(editor: HTMLElement, profile: AIProfile) {
    const info = providerInfo(profile.provider);
    const name = h("input", { type: "text", value: profile.name, maxlength: 80, "data-field": "profile-name" });
    name.addEventListener("input", () => { profile.name = name.value; changed(); });
    const provider = h("select", { "aria-label": "AI provider" });
    for (const item of PROVIDERS) provider.append(h("option", { value: item.id, text: item.label }));
    provider.value = profile.provider;
    provider.addEventListener("change", () => {
      clearDraftKeys(profile);
      profile.provider = provider.value as AIProvider;
      const next = providerInfo(profile.provider);
      profile.baseUrl = next.url; profile.model = ""; profile.webSearch = false;
      profile.reasoning = next.local ? "adaptive" : "default";
      profile.timeoutSeconds = next.local ? 300 : 120;
      profile.keys = next.local ? [] : [{ id: newId(), label: "Main key" }];
      models.delete(profile.id); changed(); draw();
      notice("Provider changed. Previous keys stay saved until you save these changes; enter keys for the new provider.", "warn");
    });
    editor.append(h("div", { class: "ai-editor-heading" }, h("h3", { text: "Connection" }),
      h("span", { class: `ai-badge${info.local ? " local" : ""}`, text: info.local ? "Local model" : "Online API" })),
      h("div", { class: "ai-form-grid" }, field("Profile name", name, "A name you recognize, such as Gemini work or Llama offline."), field("Provider", provider)),
      h("p", { class: "hint", text: info.hint }));

    if (info.local || profile.provider === "openaiCompatible") {
      const url = h("input", { type: "text", value: profile.baseUrl, placeholder: info.url || "https://api.example.com/v1", spellcheck: "false", autocomplete: "off" });
      url.addEventListener("change", () => {
        if (url.value.trim().replace(/\/$/, "") === profile.baseUrl) return;
        clearDraftKeys(profile); profile.baseUrl = url.value.trim();
        profile.keys = info.local ? [] : [{ id: newId(), label: "Main key" }];
        models.delete(profile.id); changed(); draw();
        notice("Server changed. Paste any keys needed by this server before saving.", "warn");
      });
      editor.append(field("API base URL", url, info.local ? "Local profiles connect only to this device (localhost / loopback)." : "Use the provider's HTTPS API base URL, including /v1 if required. Keep keys out of the URL."));
    } else {
      editor.append(h("div", { class: "ai-endpoint", text: `Official API · ${profile.baseUrl}` }));
    }

    const modelInput = h("input", { type: "text", value: profile.model, placeholder: "Load models or enter a model ID", spellcheck: "false", autocomplete: "off", "data-field": "model-id" });
    const chooser = h("select", { "aria-label": "Available models" });
    const modelHint = h("span", { class: "hint", "aria-live": "polite", text: "Model IDs are fetched from your provider or local server." });
    const load = h("button", { type: "button", text: "Load models", class: "ai-load" });
    function fillModels() {
      clear(chooser);
      const available = models.get(profile.id) || [];
      chooser.append(h("option", { value: "", text: available.length ? `Choose a model (${available.length})` : "Load models to see available choices" }));
      for (const item of available) chooser.append(h("option", { value: item.id, text: item.label === item.id ? item.id : `${item.label} · ${item.id}` }));
      chooser.value = available.some(model => model.id === profile.model) ? profile.model : "";
      chooser.disabled = !available.length;
      syncSelect(chooser);
    }
    fillModels();
    modelInput.addEventListener("input", () => { profile.model = modelInput.value; chooser.value = profile.model; syncSelect(chooser); changed(); });
    chooser.addEventListener("change", () => { if (chooser.value) { profile.model = chooser.value; modelInput.value = chooser.value; changed(); } });
    load.addEventListener("click", async () => {
      const loadingEpoch = epoch;
      load.disabled = true; load.textContent = "Loading…"; modelHint.textContent = "Connecting to this profile's server…";
      const draftKey = profile.keys.map(key => values.get(`${profile.id}:${key.id}`)?.trim()).find(Boolean) || null;
      try {
        const result = await Bridge.aiListModels(clone(profile), draftKey);
        if (loadingEpoch !== epoch) return;
        models.set(profile.id, result); fillModels();
        modelHint.textContent = result.length ? `Connected · ${result.length} models. Choose one above or enter a custom model ID.` : "Connected, but no chat models were listed. Enter a model ID manually or load a model in your local server.";
      } catch (error) {
        if (loadingEpoch === epoch) modelHint.textContent = message(error);
      } finally {
        if (load.isConnected) { load.disabled = false; load.textContent = "Load models"; }
      }
    });
    const modelGroup = h("div", { class: "ai-model-group" }, h("div", { class: "ai-model-label" }, h("h3", { text: "Model" }), load), smoothSelect(chooser), field("Model ID", modelInput), modelHint);

    const keys = h("div", { class: "ai-keys" });
    const addKey = h("button", { type: "button", text: "+ Add API key" });
    addKey.addEventListener("click", () => {
      if (profile.keys.length >= 16) return;
      profile.keys.push({ id: newId(), label: profile.keys.length ? `Backup ${profile.keys.length}` : "Main key" });
      changed(); drawKeys();
      keys.querySelectorAll<HTMLInputElement>("[data-key-value]")[profile.keys.length - 1]?.focus();
    });
    function drawKeys() {
      clear(keys); addKey.disabled = profile.keys.length >= 16;
      if (!profile.keys.length) keys.append(h("div", { class: "ai-key-empty hint", text: info.local ? "No key needed. Add one only if your local server requires authentication." : "No keys in this profile yet. Add a key to use this online model." }));
      for (const [index, key] of profile.keys.entries()) {
        const valueId = `${profile.id}:${key.id}`;
        const saved = hasKey(profile, key.id);
        const label = h("input", { type: "text", value: key.label, maxlength: 60, "aria-label": `Label for API key ${index + 1}` });
        label.addEventListener("input", () => { key.label = label.value; changed(); });
        const input = h("input", { type: "password", placeholder: saved ? "Saved securely · paste to replace" : info.keyHint, autocomplete: "off", spellcheck: "false", "data-key-value": key.id, "aria-label": `API key ${index + 1}` });
        input.value = values.get(valueId) || "";
        input.addEventListener("input", () => { values.set(valueId, input.value); changed(); });
        const reveal = h("button", { type: "button", text: "Show", "aria-label": `Show entered API key ${index + 1}`, "aria-pressed": "false" });
        reveal.addEventListener("click", () => {
          const show = input.type === "password"; input.type = show ? "text" : "password";
          reveal.textContent = show ? "Hide" : "Show"; reveal.setAttribute("aria-pressed", String(show));
          reveal.setAttribute("aria-label", `${show ? "Hide" : "Show"} entered API key ${index + 1}`);
        });
        const up = h("button", { type: "button", text: "↑", disabled: index === 0, "aria-label": `Move API key ${index + 1} up` });
        const down = h("button", { type: "button", text: "↓", disabled: index === profile.keys.length - 1, "aria-label": `Move API key ${index + 1} down` });
        const move = (offset: number) => { const [item] = profile.keys.splice(index, 1); profile.keys.splice(index + offset, 0, item); changed(); drawKeys(); };
        up.addEventListener("click", () => move(-1)); down.addEventListener("click", () => move(1));
        const remove = h("button", { type: "button", class: "danger", text: "Remove", "aria-label": `Remove API key ${index + 1}` });
        remove.addEventListener("click", () => { profile.keys.splice(index, 1); values.delete(valueId); changed(); drawKeys(); });
        keys.append(h("div", { class: "ai-key-card" },
          h("div", { class: "ai-key-title" }, h("strong", { text: index ? `Key fallback ${index}` : "Primary key" }),
            h("span", { class: "ai-key-state", text: saved ? "Stored" : "Not saved" }), h("div", { class: "ai-key-order" }, up, down)),
          field("Key label", label), h("div", { class: "ai-key-input" }, input, reveal),
          h("div", { class: "ai-key-footer" }, h("span", { class: "hint", text: index ? "Tried after the keys above it." : "Tried first for this model." }), remove)));
      }
    }
    drawKeys();
    editor.append(h("div", { class: "ai-key-heading" }, h("h3", { text: "API keys" }), addKey),
      h("p", { class: "hint", text: "Keys are tried top to bottom on connection, authentication or quota errors. Saved key values stay hidden and are stored securely on this device." }), keys, modelGroup);

    if (info.local) {
      const performance = h("div", { class: "ai-performance" });
      const inspect = h("button", { type: "button", text: "Inspect model" });
      const preset = h("button", { type: "button", text: "Balanced · 8 GB VRAM" });
      const result = h("p", { class: "hint", "aria-live": "polite", text: "Inspect to see this model's size, context limit and supported reasoning. Model defaults preserve its sampling settings." });
      const context = h("input", { type: "number", min: 1024, max: 131072, step: 1024, value: profile.localContextTokens ?? 4096, disabled: profile.provider !== "ollama" });
      const alive = h("input", { type: "number", min: 0, max: 30, value: profile.keepAliveMinutes ?? 5, disabled: profile.provider !== "ollama" });
      const reasoning = h("select", { "aria-label": "Local reasoning" });
      function choices(values: (string | boolean)[] = []) {
        clear(reasoning);
        reasoning.append(h("option", { value: "default", text: "Model default" }), h("option", { value: "adaptive", text: "Adaptive · automatic" }));
        for (const value of values) {
          const id = typeof value === "boolean" ? value ? "on" : "off" : value;
          if (!["on","off","low","medium","high"].includes(id)) continue;
          reasoning.append(h("option", { value: id, text: typeof value === "boolean" ? value ? "Thinking on" : "Thinking off" : `Reasoning: ${value}` }));
        }
        if (![...reasoning.options].some(o => o.value === profile.reasoning)) reasoning.append(h("option", { value: profile.reasoning, text: `${profile.reasoning} · inspect to verify` }));
        reasoning.value = profile.reasoning ?? "default"; syncSelect(reasoning);
      }
      choices();
      context.addEventListener("input", () => { profile.localContextTokens = Number(context.value); changed(); });
      alive.addEventListener("input", () => { profile.keepAliveMinutes = Number(alive.value); changed(); });
      reasoning.addEventListener("change", () => { profile.reasoning = reasoning.value; changed(); });
      preset.addEventListener("click", () => { Object.assign(profile, LOCAL_BALANCED); changed(); draw(); notice("8 GB preset staged: 4096 context, 2048 output, adaptive reasoning, 5 minute keep-alive. Save changes to apply. For daily use choose a 3B/4B model; 20B/30B models can still offload to CPU."); });
      inspect.addEventListener("click", async () => {
        const currentEpoch = epoch; inspect.disabled = true; result.textContent = "Reading model specifications…";
        try {
          const model = await Bridge.aiModelDetails(clone(profile)); if (currentEpoch !== epoch) return;
          choices(model.thinkingValues); if (model.maxContextTokens) context.max = String(Math.min(131072, model.maxContextTokens));
          const size = model.sizeBytes / 1024 ** 3;
          const warning = size > 8 ? "CPU/RAM offload expected on an 8 GB GPU; choose a smaller model for faster replies." : size > 5.5 ? "Limited VRAM headroom on 8 GB. Keep context small, or choose a 3B/4B model." : "Good weight-size headroom for an 8 GB GPU; actual usage also includes context and other apps.";
          result.textContent = `${model.parameters} · ${model.quantization}${size ? ` · ${size.toFixed(2)} GiB weights` : ""} · model context ceiling ${model.maxContextTokens || "unknown"} tokens. ${warning} Capabilities: ${model.capabilities.join(", ") || "unknown"}. ${model.thinkingValues.length ? model.legacyThinkingControls ? "Reasoning uses known family controls on this older runtime." : "Reasoning controls detected from the server." : "No selectable reasoning reported; adaptive keeps model defaults."}`;
        } catch (error) { if (currentEpoch === epoch) result.textContent = message(error); }
        finally { if (inspect.isConnected) inspect.disabled = false; }
      });
      performance.append(h("h3", { text: "Local performance & reasoning" }), h("div", { class: "row" }, inspect, preset), result,
        h("div", { class: "ai-form-grid" }, field("Context tokens", context, "4096 is a practical starting point for 8 GB. Longer chats retain recent turns in the model's context."), field("Keep loaded (minutes)", alive, "0 releases memory after each reply; 5 avoids repeated model loading.")),
        field("Reasoning", reasoning, "Adaptive applies to every local profile. Supported models use lighter/deeper controls; unknown or non-thinking models keep their defaults. LM Studio context and unload settings are managed in its server."));
      editor.append(performance);
    }
    const advanced = h("details", { class: "ai-advanced" }, h("summary", { text: "Advanced" }));
    const tokens = h("input", { type: "number", min: 16, max: 65536, value: profile.maxOutputTokens, step: 1 });
    const timeout = h("input", { type: "number", min: 10, max: 600, value: profile.timeoutSeconds, step: 1 });
    tokens.addEventListener("input", () => { profile.maxOutputTokens = Number(tokens.value); changed(); });
    timeout.addEventListener("input", () => { profile.timeoutSeconds = Number(timeout.value); changed(); });
    advanced.append(h("div", { class: "ai-form-grid" }, field("Output token limit", tokens), field("Timeout (seconds)", timeout, "Local models may need more time on the first request.")));
    if (info.search) {
      const search = h("input", { type: "checkbox" }); search.checked = profile.webSearch;
      search.addEventListener("change", () => { profile.webSearch = search.checked; changed(); });
      advanced.append(h("label", { class: "ai-check" }, search, h("span", { text: "Allow the provider's built-in web search (if supported by this model)." })));
    }
    editor.append(advanced);
    const active = h("button", { type: "button", text: config.activeProfileId === profile.id ? "Active model" : "Use as active model", disabled: config.activeProfileId === profile.id });
    active.addEventListener("click", () => { config.activeProfileId = profile.id; config.fallbackProfileIds = config.fallbackProfileIds.filter(id => id !== profile.id); changed(); draw(); });
    const remove = h("button", { type: "button", class: "danger", text: "Delete profile" });
    let confirming = false;
    remove.addEventListener("click", () => {
      if (!confirming) { confirming = true; remove.textContent = "Confirm delete"; notice("Delete this profile and its saved keys? Click Confirm delete, then Save changes. Discard can undo this before saving.", "warn"); return; }
      clearDraftKeys(profile); config.profiles = config.profiles.filter(p => p.id !== profile.id);
      config.fallbackProfileIds = config.fallbackProfileIds.filter(id => id !== profile.id);
      if (config.activeProfileId === profile.id) config.activeProfileId = config.profiles[0]?.id || "";
      config.fallbackProfileIds = config.fallbackProfileIds.filter(id => id !== config.activeProfileId);
      selectedId = config.activeProfileId; changed(); draw(); notice("Profile removal is staged. Save changes to remove it from this device.", "warn");
    });
    editor.append(h("div", { class: "ai-editor-footer" }, active, remove));
  }

  function drawFallback() {
    const primary = h("select", { "aria-label": "Active AI profile" });
    if (!config.profiles.length) primary.append(h("option", { text: "Add a profile first", value: "" }));
    for (const p of config.profiles) primary.append(h("option", { value: p.id, text: `${p.name} · ${p.model || "model not selected"}` }));
    primary.value = config.activeProfileId;
    primary.addEventListener("change", () => { config.activeProfileId = primary.value; config.fallbackProfileIds = config.fallbackProfileIds.filter(id => id !== primary.value); changed(); draw(); });
    const enabled = h("input", { type: "checkbox", "aria-label": "Automatic model fallback" }); enabled.checked = config.fallbackEnabled;
    enabled.addEventListener("change", () => { config.fallbackEnabled = enabled.checked; changed(); draw(); });
    const panel = h("div", { class: "ai-fallback-panel" }, field("Active profile", primary),
      h("label", { class: "ai-check ai-fallback-toggle" }, enabled, h("span", {}, h("strong", { text: "Automatic model fallback" }), h("span", { class: "hint", text: "After this model's keys are exhausted, try the profiles you select below." }))));
    content.append(panel);
    const active = config.profiles.find(p => p.id === config.activeProfileId);
    if (active && providerInfo(active.provider).local && config.fallbackEnabled && config.fallbackProfileIds.some(id => { const p = config.profiles.find(p => p.id === id); return p && !providerInfo(p.provider).local; })) {
      panel.append(h("div", { class: "notice warn", text: "Online fallback is enabled for your local model. If the local request fails, this conversation and attachments can be sent to the selected online provider." }));
    } else if (active && providerInfo(active.provider).local) {
      panel.append(h("div", { class: "notice ok", text: "Local routing: this profile connects only to this device. Online providers are used only if you enable and select them as fallbacks." }));
    }
    const list = h("div", { class: "ai-fallback-list" });
    const ordered = config.fallbackProfileIds.map(id => config.profiles.find(p => p.id === id)).filter((p): p is AIProfile => !!p);
    const others = config.profiles.filter(p => p.id !== config.activeProfileId && !config.fallbackProfileIds.includes(p.id));
    if (!ordered.length && !others.length) list.append(h("p", { class: "hint", text: "Add another model profile to choose a fallback. Each profile keeps its own provider and API keys." }));
    for (const p of [...ordered, ...others]) {
      const index = config.fallbackProfileIds.indexOf(p.id);
      const check = h("input", { type: "checkbox", "aria-label": `Use ${p.name} as fallback`, disabled: !config.fallbackEnabled }); check.checked = index >= 0;
      check.addEventListener("change", () => {
        if (check.checked) config.fallbackProfileIds.push(p.id);
        else config.fallbackProfileIds = config.fallbackProfileIds.filter(id => id !== p.id);
        changed(); draw();
      });
      const up = h("button", { type: "button", text: "↑", disabled: index <= 0 || !config.fallbackEnabled, "aria-label": `Move fallback ${p.name} up` });
      const down = h("button", { type: "button", text: "↓", disabled: index < 0 || index === ordered.length - 1 || !config.fallbackEnabled, "aria-label": `Move fallback ${p.name} down` });
      const move = (offset: number) => { const [id] = config.fallbackProfileIds.splice(index, 1); config.fallbackProfileIds.splice(index + offset, 0, id); changed(); draw(); };
      up.addEventListener("click", () => move(-1)); down.addEventListener("click", () => move(1));
      list.append(h("div", { class: `ai-fallback-item${index >= 0 ? " selected" : ""}` },
        h("label", { class: "ai-check" }, check, h("span", {}, h("strong", { text: `${index >= 0 ? `${index + 1}. ` : ""}${p.name}` }), h("span", { class: "hint", text: p.model || "Choose a model first" }))),
        h("span", { class: `ai-badge${providerInfo(p.provider).local ? " local" : ""}`, text: providerInfo(p.provider).local ? "Local" : "Online" }),
        h("div", { class: "ai-key-order" }, up, down)));
    }
    panel.append(h("h3", { text: "Model fallback order" }), list,
      h("p", { class: "hint", text: "Only selected profiles are used. Key fallback within a profile is always available. All retries share the same conversation; you can stop a request from the chat." }));
  }

  add.addEventListener("click", addProfile);
  profilesTab.addEventListener("click", () => { tab = "profiles"; draw(); });
  fallbackTab.addEventListener("click", () => { tab = "fallback"; draw(); });
  discard.addEventListener("click", () => {
    config = clone(stored); values.clear(); models.clear(); dirty = false;
    selectedId = config.activeProfileId; save.disabled = true; discard.disabled = true; state.textContent = "Saved on this device";
    clear(feedback); draw();
  });
  async function refreshStatus() {
    try {
      statuses = await Bridge.aiKeyStatus(); statusKnown = true;
      if (!dirty && !saving) draw(); else updateSidebar();
    }
    catch (error) { statusKnown = false; notice(message(error), "warn"); }
  }
  save.addEventListener("click", async () => {
    if (saving || !dirty) return;
    saving = true; save.disabled = true; discard.disabled = true; add.disabled = true; content.disabled = true;
    save.textContent = "Saving…"; clear(feedback);
    const updates = config.profiles.flatMap(profile => profile.keys.flatMap(key => {
      const value = values.get(`${profile.id}:${key.id}`)?.trim();
      return value ? [{ profileId: profile.id, keyId: key.id, value }] : [];
    }));
    try {
      const saved = await Bridge.aiSaveConfig(clone(config), updates);
      config = clone(saved.ai); stored = clone(saved.ai); values.clear(); dirty = false;
      onSaved(saved); await refreshStatus(); draw();
      state.textContent = "Saved on this device";
      notice("Profiles saved. Keys are secured on this device.");
    } catch (error) { notice(message(error), "err"); }
    finally {
      saving = false; content.disabled = false; add.disabled = false;
      save.textContent = "Save changes"; save.disabled = !dirty; discard.disabled = !dirty;
    }
  });
  draw();
  void refreshStatus();
  return {
    el,
    addLocalModel(model: string, baseUrl: string) {
      let profile = config.profiles.find(p => p.provider === "ollama" && p.baseUrl === baseUrl && p.model === model);
      if (!profile) {
        if (config.profiles.length >= 32) { notice("Delete an unused profile before adding another model.", "warn"); return; }
        profile = { id: newId(), name: model, provider: "ollama", baseUrl, model, keys: [], ...LOCAL_BALANCED, webSearch: false };
        config.profiles.push(profile);
      }
      config.activeProfileId = profile.id;
      config.fallbackProfileIds = config.fallbackProfileIds.filter(id => id !== profile.id);
      selectedId = profile.id; tab = "profiles"; changed(); draw();
      notice("Local model selected. Click Save changes to use it in chat. No API key is needed.");
      el.scrollIntoView({ block: "start", behavior: "smooth" });
    },
    update(next: AIConfig) {
      stored = clone(next);
      if (!dirty && !saving) { config = clone(next); if (!config.profiles.some(p => p.id === selectedId)) selectedId = config.activeProfileId; draw(); }
    },
  };
}
