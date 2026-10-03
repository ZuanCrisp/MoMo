export type AIProvider = "anthropic" | "openai" | "gemini" | "openaiCompatible" | "ollama" | "localOpenai";

export interface AIKey { id: string; label: string }
export interface AIProfile {
  id: string;
  name: string;
  provider: AIProvider;
  baseUrl: string;
  model: string;
  keys: AIKey[];
  maxOutputTokens: number;
  timeoutSeconds: number;
  webSearch: boolean;
  localContextTokens: number;
  reasoning: string;
  keepAliveMinutes: number;
}
export interface AIConfig {
  profiles: AIProfile[];
  activeProfileId: string;
  fallbackEnabled: boolean;
  fallbackProfileIds: string[];
}
export interface AIKeyStatus { profileId: string; keyId: string; present: boolean }
export interface AIKeyUpdate { profileId: string; keyId: string; value: string }
export interface AIModel { id: string; label: string }
export interface AIReply {
  text: string;
  profileId: string;
  profileName: string;
  model: string;
  usedFallback: boolean;
  keyLabel: string | null;
  actions?: { name: string; detail: string; success: boolean; path: string | null }[];
  historySaved?: boolean;
  conversationId?: string | null;
}

export interface ModelDetails {
  sizeBytes: number; maxContextTokens: number; parameters: string; quantization: string;
  capabilities: string[]; thinkingValues: (string | boolean)[]; legacyThinkingControls: boolean;
}
export const LOCAL_DEFAULTS = { localContextTokens: 4096, reasoning: "default", keepAliveMinutes: 5 };
export const LOCAL_BALANCED = { localContextTokens: 4096, reasoning: "adaptive", keepAliveMinutes: 5, maxOutputTokens: 2048, timeoutSeconds: 300 };

export const PROVIDERS: { id: AIProvider; label: string; url: string; hint: string; keyHint: string; local?: boolean; search?: boolean }[] = [
  { id: "anthropic", label: "Claude · Anthropic", url: "https://api.anthropic.com/v1", hint: "Use an Anthropic API key, then choose a Claude model.", keyHint: "Paste your Anthropic API key", search: true },
  { id: "openai", label: "OpenAI / ChatGPT", url: "https://api.openai.com/v1", hint: "Use an OpenAI API key, then choose a GPT model available to your account.", keyHint: "Paste your OpenAI API key", search: true },
  { id: "gemini", label: "Gemini · Google", url: "https://generativelanguage.googleapis.com/v1beta", hint: "Use a Google AI Studio API key, then choose a Gemini model.", keyHint: "Paste your Gemini API key", search: true },
  { id: "openaiCompatible", label: "Other OpenAI-compatible API", url: "", hint: "Enter the provider's API base URL including its API version, for example https://api.example.com/v1.", keyHint: "Paste this provider's API key" },
  { id: "ollama", label: "Ollama · local", url: "http://127.0.0.1:11434", hint: "Start Ollama on this device and download a local model first. Load models to choose one; cloud models are excluded.", keyHint: "Optional server API key", local: true },
  { id: "localOpenai", label: "LM Studio / local OpenAI server", url: "http://127.0.0.1:1234/v1", hint: "Start your local server, load a downloaded model and enable its API server. No API key is needed unless you enabled server authentication.", keyHint: "Optional server API key", local: true },
];

export function providerInfo(id: AIProvider) { return PROVIDERS.find(p => p.id === id)!; }
export function defaultAIConfig(): AIConfig {
  return {
    profiles: [{ id: "legacy-claude", name: "Claude", provider: "anthropic", baseUrl: PROVIDERS[0].url, model: "claude-opus-5", keys: [{ id: "legacy", label: "Main key" }], maxOutputTokens: 4096, timeoutSeconds: 120, webSearch: true, ...LOCAL_DEFAULTS }],
    activeProfileId: "legacy-claude", fallbackEnabled: false, fallbackProfileIds: [],
  };
}
