# Changelog

## 0.4.0 — Windows / Linux

- Compact model selector aligned to the left, without duplicated profile/model names; chat history and New chat controls in the toolbar.
- Private local conversation archives with search, reopen, continuation and deletion. Reopening restores text context and action receipts without replaying actions. Completed turns survive app restarts; attachments are not copied into the archive.
- Ollama model inspection reports weight size, quantization, capabilities, context ceiling and supported reasoning controls. Adaptive reasoning chooses lighter settings for short messages and more reasoning for analysis, code and maths; unknown families retain model defaults.
- Balanced 8 GB VRAM preset: 4096 context tokens, 4096 output tokens, adaptive reasoning, 5-minute keep-alive. Qwen3.5 4B is selected when available in the local setup panel.
- MoMo's local runtime uses one loaded model and one parallel request, Flash Attention and q8_0 KV cache. A conservative context estimate retains recent complete turns while older text stays in the archive.
- Settings explains what each integration reads, which separate credential it needs and how to enable its status pill.
- Adaptive is the default for new local profiles and migrated local preferences. LM Studio models exposing supported effort levels use local Responses API with preserved function calls; unknown local APIs retain model defaults.
- If a local adaptive request produces thinking without a final answer, MoMo retries once at a supported lighter level on the same model and labels the adjustment. Recovery never replays desktop tool results.

Native macOS source is unchanged by this update.

## 0.3.0 — Windows / Linux

- Smooth themed dropdowns with keyboard navigation, selection markers and menus that fit the chat card.
- Taller native island window, softer frame and corners; the growing chat and bottom input no longer clip.
- Chat stays open while reading, loading a model or opening another app; Escape and navigation still close it normally.
- Removed hook installation controls and the missing-hooks warning from the user interface.
- Local AI setup detects a downloaded Ollama library, checks complete models and starts a separate loopback server using that folder. The model folder and large model formats are excluded from Git and installers.
- Optional desktop tools open supported apps and create new UTF-8 notes in Notepad on Windows, or available editors on Linux. Actual action results appear in chat.
- Tool calling for all six providers preserves call IDs, Gemini signatures and provider reasoning. Repeated calls within a turn are deduplicated; cancellation or an API error after an action reports the completed action without retrying another route.

Native macOS source is unchanged by this update.

## 0.2.0 — Windows / Linux

- AI model profiles for Claude, OpenAI / ChatGPT, Gemini, other OpenAI-compatible APIs, Ollama and local OpenAI-compatible servers.
- Provider model discovery and manual model IDs; multiple credential slots per profile with ordered key fallback.
- Explicit ordered model fallback, including a notice when a local profile can fall back to an online provider.
- Draft-based settings editor, masked key fields, labels and reorder controls. Keys stay in the OS credential store; existing Claude preferences and key account migrate automatically.
- Chat profile selector, reply routing labels, Stop button and recoverable failed messages.
- HTTP/vault tests and browser UI tests; Windows installer packaging and Linux build validation.
- One Windows installer per version, with WebView2 included and a README beside it.

Native macOS source is unchanged by this feature update.

## 0.1.2 — October 2, 2026

- Codex support (GitHub build): sessions show up live on the Codex pill, and permission requests get Allow and Deny in the notch. Install from Settings → Codex Hooks, then trust the hooks once with /hooks in Codex (#130) — thanks @lacatu5
- Cursor: Claude Code started in Cursor's terminal shows up on the Cursor pill, and you can answer its permission requests from the notch (#120).
- Pick your main coding tool in Settings → Active pills: VS Code, Cursor, Codex or Antigravity (Codex and Antigravity: GitHub build). It stays on and no longer takes one of the 4 slots (#120).
- The permission card stays in the notch until you answer it: the mouse no longer folds it, and reopening the island shows the request again (#117).
- The permission card also shows when the island is already open, and the pill you were on comes back once you answer (#120).

## 0.1.1 — October 2, 2026

- Declare the tools you use in Settings: Gemini CLI, Antigravity, Anthropic, Google AI and OpenAI pills join the existing ones (Cursor and Codex pills are coming soon), and you pick the main pill.
- Chat now supports Google AI (Gemini) and OpenAI in addition to Anthropic; switch provider and model by clicking the model name in the chat view, on macOS.
- Linux version: the Tauri app now builds for Linux too (AppImage, .deb, .rpm), with the island as a layer-shell overlay on Wayland and Claude Code hooks over a private Unix socket (#21) — thanks @Davy133
- Compact island on screens without a notch (#22) — thanks @Kamasoutra
- Only web links (http/https) open from the notch; other kinds of links from Claude or integrations are ignored (#16) — thanks @Cris1670
- Hook socket limited to your own user account, with size and time limits; logs no longer keep commands, n8n data or full URLs, and stay under 1 MB (#16) — thanks @Cris1670 and @Vignesh-Thangamariappan
- The island always reopens after folding, and Settings opens below it, resizable — thanks @rouderz
- Choose the Claude model for the chat in Settings; the list comes from your Anthropic account, and Claude Sonnet 4.6 stays the default — thanks @rouderz
- Windows build artifacts are now downloadable from a manual CI run — thanks @MysJofR
- Any agent can talk to Mochi: tag a hook payload with `momo_agent` (e.g. `nb-hook --agent my-agent`) and it gets its own pill in the island (#7, #9) — thanks @lacatu5
- Gemini CLI and Antigravity (agy) hook support on macOS: install from Settings and their sessions show up in the island — thanks @corefusiion
