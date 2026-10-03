import { Bridge, type LocalAIStatus } from "../core/bridge";
import { h, clear } from "../views/dom";
import { smoothSelect, syncSelect } from "../views/select";

export function createLocalSection(onUse: (model: string, baseUrl: string) => void) {
  let status: LocalAIStatus | null = null;
  let busy = false;
  let generation = 0;
  const directory = h("input", { type: "text", spellcheck: "false", autocomplete: "off", placeholder: "Choose the Ollama folder containing blobs and manifests", "aria-label": "Local models folder" });
  const scan = h("button", { type: "button", text: "Check folder" });
  const start = h("button", { type: "button", class: "primary", text: "Start local AI" });
  const select = h("select", { "aria-label": "Downloaded local model" });
  const use = h("button", { type: "button", text: "Add model profile", disabled: true });
  const feedback = h("div", { class: "notice", "aria-live": "polite", text: "Checking your local model library…" });
  const el = h("section", { class: "local-ai-section", "aria-label": "Local AI setup" },
    h("h2", { text: "Local AI · offline" }),
    h("p", { class: "hint", text: "Reuse your downloaded Ollama models. MoMo starts its own local server; no API key or model download is required." }),
    h("label", { text: "1. Models folder" }, directory),
    h("div", { class: "row" }, scan, start), feedback,
    h("label", { text: "2. Choose a downloaded model" }, smoothSelect(select)),
    h("div", { class: "row" }, use, h("span", { class: "hint", text: "3. Save changes in AI & models below to start chatting." })),
  );
  function draw(next: LocalAIStatus) {
    status = next; directory.value = next.directory;
    const previous = select.value;
    clear(select);
    if (!next.models.length) select.append(h("option", { value: "", text: "No complete chat models found" }));
    for (const model of next.models) select.append(h("option", { value: model, text: model }));
    select.value = next.models.includes(previous) ? previous : next.models.find(m => m === "qwen3.5:4b") || next.models.find(m => m === "llama3.2:3b") || next.models[0] || "";
    feedback.className = `notice ${next.running ? "ok" : "warn"}`;
    feedback.textContent = next.running ? `Local AI is running · ${next.models.length} models · ${next.baseUrl}`
      : !next.runtimeInstalled ? "Install Ollama first, then click Start local AI. Your downloaded models stay in this folder."
      : next.models.length ? `${next.models.length} downloaded chat models found. Click Start local AI to connect.` : "Enter your model folder and click Check folder.";
    start.textContent = next.running ? "Local AI ready" : "Start local AI";
    update();
  }
  function update() {
    directory.disabled = busy; scan.disabled = busy; start.disabled = busy || !status?.runtimeInstalled || !status.models.length || status.running;
    select.disabled = busy || !status?.models.length; use.disabled = busy || !status?.running || !select.value;
    syncSelect(select);
  }
  async function request(launch: boolean) {
    if (busy) return;
    const epoch = ++generation; busy = true; update();
    feedback.className = "notice"; feedback.textContent = launch ? "Starting local AI from your folder…" : "Checking your model folder…";
    try {
      const next = launch ? await Bridge.localAIStart(directory.value.trim()) : await Bridge.localAIStatus(directory.value.trim() || null);
      if (epoch === generation) draw(next);
    } catch (error) {
      status = null; feedback.className = "notice err"; feedback.textContent = String(error).replace(/^Error:\s*/, "");
    } finally { busy = false; update(); }
  }
  directory.addEventListener("input", () => { status = null; update(); });
  scan.addEventListener("click", () => void request(false));
  start.addEventListener("click", () => void request(true));
  use.addEventListener("click", () => { if (status?.running && select.value) onUse(select.value, status.baseUrl); });
  void request(false); return el;
}
