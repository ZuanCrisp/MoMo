import { h, svg, clear } from "./dom";
import { ICONS } from "./icons";
import { smoothSelect, syncSelect } from "./select";
import { Bridge, type ChatContext, type ConversationSummary } from "../core/bridge";
import { providerInfo } from "../core/ai";
import { Sound } from "../core/sound";
import { State, type ChatMessage } from "../core/state";
import type { ViewHost } from "./views";

let nextId = 1;

function bubble(message: ChatMessage): HTMLElement {
  if (message.role === "user") {
    return h("div", { class: "chat-row user" }, h("div", { class: "bubble", text: message.content }));
  }
  const route = message.route;
  return h("div", { class: "chat-row" }, h("div", { class: "chat-answer" },
    route ? h("div", { class: "chat-route", text: `${route.profileName} · ${route.model}${route.usedFallback ? " · fallback" : ""}`,
      title: route.keyLabel ? `Key: ${route.keyLabel}` : "Local model" }) : null,
    h("div", { class: "reply", text: message.content }),
    ...(route?.actions || []).map(action => h("div", { class: `chat-action${action.success ? "" : " failed"}`, text: `${action.success ? "✓" : "!"} ${action.detail}`, title: action.path || "" }))));
}

function typingDots(): HTMLElement {
  return h("div", { class: "chat-row chat-working" }, h("div", { class: "typing" }, h("i"), h("i"), h("i")), h("span", { text: "Preparing your reply…", class: "chat-working-label" }));
}

function contextChip(label: string): HTMLElement {
  const chip = h("div", { class: "chip" }, h("i", { class: "chip-dot" }), h("span", { text: label }));
  requestAnimationFrame(() => chip.classList.add("settled"));
  return chip;
}

export function buildPrompt(onHeightChange: () => void): ViewHost {
  const chipRow = h("div", { class: "chip-row" });
  const log = h("div", { class: "chat-log", "aria-live": "polite" });
  const error = h("div", { class: "chat-error", role: "alert" });
  const profile = h("select", { class: "chat-model", "aria-label": "Chat AI model" });
  const manage = h("button", { class: "chat-manage", text: "Manage AI", onclick: () => void Bridge.openSettingsWindow() });
  const history = h("button", { class: "chat-manage", text: "History", "aria-label": "Chat history", "aria-expanded": "false" });
  const newChat = h("button", { class: "chat-new", title: "New chat", "aria-label": "New chat" }, svg(ICONS.plus, 12));
  const archive = h("div", { class: "chat-archive", hidden: true });
  const filter = h("input", { class: "history-search", placeholder: "Search conversations…", "aria-label": "Search chat history", type: "search" });
  const conversations = h("div", { class: "history-list" });
  archive.append(filter, conversations);
  const input = h("input", { type: "text", class: "chat-input", placeholder: "Ask me anything…", spellcheck: "false", maxlength: 32000, "aria-label": "Message" });
  const send = h("button", { class: "send-btn", title: "Send", "aria-label": "Send message" }, svg(ICONS.arrowUp, 11));
  const el = h("div", { class: "view" },
    h("div", { class: "card wash chat-card" },
      h("div", { class: "chat-toolbar" }, smoothSelect(profile), h("div", { class: "chat-toolbar-actions" }, history, newChat, manage)),
      h("div", { class: "chat-body" }, chipRow, archive, log, error, h("div", { class: "chat-bar" }, input, send))));
  (el.querySelector(".card") as HTMLElement).style.setProperty("--wash", "rgba(99,102,241,0.5)");

  let sending = false;
  let selecting = false;
  let renderedCount = -1;
  let profileSignature = "";
  let renderedHistory: ChatMessage[] | null = null;
  let showingHistory = false;
  let historyBusy = false;
  let saved: ConversationSummary[] = [];

  function showHistory(show: boolean) {
    showingHistory = show; archive.hidden = !show; log.hidden = show;
    (el.querySelector(".chat-bar") as HTMLElement).hidden = show;
    history.textContent = show ? "Back to chat" : "History";
    history.setAttribute("aria-expanded", String(show));
    State.notify(); onHeightChange();
    if (show) filter.focus(); else input.focus();
  }
  function drawHistory() {
    clear(conversations);
    const items = saved.filter(c => c.title.toLowerCase().includes(filter.value.toLowerCase()));
    if (!items.length) conversations.append(h("p", { class: "chat-empty-history", text: saved.length ? "No matching conversations." : "Your completed conversations will appear here." }));
    for (const conversation of items) {
      const open = h("button", { class: "history-open", type: "button", "aria-label": `Open chat: ${conversation.title}` },
        h("span", { class: "history-title", text: conversation.title }), h("span", { class: "history-date", text: `${new Date(conversation.updatedAt * 1000).toLocaleDateString()} · ${conversation.messageCount / 2} replies` }));
      const remove = h("button", { class: "history-delete", text: "×", title: "Delete conversation", "aria-label": `Delete chat: ${conversation.title}` });
      let confirm = false;
      open.addEventListener("click", async () => {
        if (historyBusy || sending) return;
        historyBusy = true; State.notify();
        try {
          const chat = await Bridge.chatOpen(conversation.id);
          State.chatHistory = chat.messages; State.chatConversationId = chat.id; State.droppedFile = null;
          nextId = Math.max(nextId, ...chat.messages.map(m => m.id + 1));
          error.textContent = ""; showHistory(false);
        } catch (err) { error.textContent = String(err).replace(/^Error:\s*/, ""); }
        finally { historyBusy = false; State.notify(); }
      });
      remove.addEventListener("click", async () => {
        if (historyBusy || sending) return;
        if (!confirm) { confirm = true; remove.textContent = "Delete?"; remove.classList.add("confirm"); return; }
        historyBusy = true;
        try {
          await Bridge.chatDelete(conversation.id); saved = saved.filter(c => c.id !== conversation.id);
          if (State.chatConversationId === conversation.id) { State.chatHistory = []; State.chatConversationId = null; State.droppedFile = null; }
          drawHistory();
        } catch (err) { error.textContent = String(err).replace(/^Error:\s*/, ""); }
        finally { historyBusy = false; State.notify(); }
      });
      conversations.append(h("div", { class: "history-item" }, open, remove));
    }
  }
  filter.addEventListener("input", drawHistory);
  history.addEventListener("click", async () => {
    if (sending || historyBusy) return;
    if (showingHistory) { showHistory(false); return; }
    historyBusy = true; error.textContent = ""; State.notify();
    try { saved = await Bridge.chatHistory(); drawHistory(); showHistory(true); }
    catch (err) { error.textContent = String(err).replace(/^Error:\s*/, ""); }
    finally { historyBusy = false; State.notify(); }
  });
  newChat.addEventListener("click", async () => {
    if (sending || historyBusy) return;
    historyBusy = true;
    try {
      await Bridge.chatReset(); State.chatHistory = []; State.chatConversationId = null; State.droppedFile = null;
      error.textContent = ""; input.value = ""; showHistory(false);
    } finally { historyBusy = false; State.notify(); }
  });

  profile.addEventListener("change", async () => {
    selecting = true; profile.disabled = true; error.textContent = "";
    try { State.settings = await Bridge.aiSetActive(profile.value); }
    catch (err) { error.textContent = String(err).replace(/^Error:\s*/, ""); }
    finally { selecting = false; State.notify(); }
  });

  async function submit() {
    if (sending) {
      send.disabled = true;
      try { await Bridge.chatCancel(); }
      catch (err) { error.textContent = String(err).replace(/^Error:\s*/, ""); send.disabled = false; }
      return;
    }
    const query = input.value.trim();
    if (!query || selecting) return;
    input.value = ""; error.textContent = ""; sending = true;
    Sound.play("send");
    const id = nextId++;
    State.chatHistory.push({ id, role: "user", content: query });
    State.stateOverride = "thinking";
    State.notify(); onHeightChange();
    const file = State.droppedFile;
    const context: ChatContext | null = State.chatHistory.length === 1 && file ? { kind: "file", name: file.name, path: file.path } : null;
    try {
      const { text, ...route } = await Bridge.chatSend(query, context);
      // Closing/resetting chat must not resurrect the previous conversation.
      if (State.chatHistory.some(message => message.id === id)) {
        State.chatHistory.push({ id: nextId++, role: "assistant", content: text, route });
        State.chatConversationId = route.conversationId ?? null;
        if (route.historySaved === false) error.textContent = "Reply completed, but history could not be saved. Check available disk space and folder permissions.";
        Sound.play("finish");
      }
    } catch (err) {
      if (State.chatHistory.some(message => message.id === id)) {
        State.chatHistory = State.chatHistory.filter(message => message.id !== id);
        input.value = query;
        error.textContent = String(err).replace(/^Error:\s*/, "");
        Sound.play("error");
      }
    } finally {
      sending = false; State.stateOverride = null;
      State.notify(); onHeightChange(); input.focus();
    }
  }

  send.addEventListener("click", () => void submit());
  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter") { e.preventDefault(); void submit(); }
    e.stopPropagation();
  });

  return {
    el,
    sync() {
      const ai = State.settings.ai;
      const signature = JSON.stringify(ai.profiles.map(p => [p.id, p.name, p.model, p.provider]));
      if (signature !== profileSignature) {
        profileSignature = signature; clear(profile);
        if (!ai.profiles.length) profile.append(h("option", { value: "", text: "Set up an AI model" }));
        for (const p of ai.profiles) profile.append(h("option", { value: p.id,
          text: `${providerInfo(p.provider).local ? "Local · " : ""}${p.name === p.model ? p.model : `${p.name} · ${p.model || "setup needed"}`}` }));
      }
      profile.value = ai.activeProfileId;
      profile.disabled = sending || selecting || !ai.profiles.length;
      syncSelect(profile);
      const active = ai.profiles.find(p => p.id === ai.activeProfileId);
      const ready = !!active?.model;
      const wantChip = State.droppedFile?.name ?? "";
      if (chipRow.dataset.label !== wantChip) {
        chipRow.dataset.label = wantChip; clear(chipRow);
        if (wantChip) chipRow.append(contextChip(wantChip));
      }
      const thinking = State.stateOverride === "thinking";
      const count = State.chatHistory.length + (thinking ? 0.5 : 0);
      if (count !== renderedCount || renderedHistory !== State.chatHistory) {
        renderedCount = count; renderedHistory = State.chatHistory; clear(log);
        for (const m of State.chatHistory) log.append(bubble(m));
        if (thinking) log.append(typingDots());
        if (!count) log.append(h("div", { class: "chat-welcome" }, h("strong", { text: "What can I help you with?" }), h("span", { text: "Ask a question, open an app, or create a note." })));
        log.scrollTop = log.scrollHeight;
      }
      input.placeholder = !ready ? "Choose a model in Manage AI…" : State.chatHistory.length ? "Continue…" : "Ask me anything…";
      input.disabled = sending || !ready;
      history.disabled = sending || historyBusy; newChat.disabled = sending || historyBusy;
      send.disabled = selecting || !ready;
      send.title = sending ? "Stop request" : "Send";
      send.setAttribute("aria-label", sending ? "Stop request" : "Send message");
      clear(send);
      send.append(sending ? h("span", { class: "chat-stop", text: "■" }) : svg(ICONS.arrowUp, 11));
    },
    focus() { input.focus(); input.select(); },
  };
}
