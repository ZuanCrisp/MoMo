import { test, expect, type Page } from "@playwright/test";
import { DEFAULT_SETTINGS } from "../../src/core/state";

async function fixture(page: Page) {
  await page.addInitScript((initial) => {
    const w = window as any;
    w.aiTest = { settings: initial, saves: [], failSave: false, chatError: false, pending: null, cancelled: 0 };
    w.__TAURI_INTERNALS__ = {
      transformCallback: () => 1,
      invoke: async (cmd: string, args: any = {}) => {
        const t = w.aiTest;
        switch (cmd) {
          case "boot": return { settings: t.settings, version: "0.2.0" };
          case "hooks_status": return { installed: false, hookReady: true, settingsPath: "", hookPath: "" };
          case "secret_present": return false;
          case "ai_key_status": return t.settings.ai.profiles.flatMap((p: any) => p.keys.map((k: any) => ({ profileId: p.id, keyId: k.id, present: true })));
          case "ai_list_models": return [{ id: "test-chat-model", label: "Test chat model" }];
          case "ai_save_config":
            if (t.failSave) throw "Could not save settings. Your previous keys were restored.";
            t.saves.push(structuredClone(args));
            t.settings.ai = structuredClone(args.config); return structuredClone(t.settings);
          case "ai_set_active": t.settings.ai.activeProfileId = args.profileId; return structuredClone(t.settings);
          case "chat_send":
            if (t.chatError) throw "API rate limit or quota reached (HTTP 429).";
            return new Promise((resolve, reject) => { t.pending = { resolve, reject }; });
          case "chat_cancel": t.cancelled++; t.pending?.reject("Request cancelled."); return;
          default: return null;
        }
      },
    };
  }, structuredClone(DEFAULT_SETTINGS));
}

test.beforeEach(async ({ page }) => { await fixture(page); });

test("multiple keys retain their identity when reordered; secrets never enter profile metadata", async ({ page }) => {
  await page.goto("/settings.html");
  await expect(page.getByLabel("API key 1", { exact: true })).toHaveAttribute("placeholder", "Saved securely · paste to replace");
  await page.getByRole("button", { name: "+ Add profile", exact: true }).click();
  await page.getByLabel("Profile name", { exact: true }).fill("Gemini work");
  await page.getByLabel("AI provider").selectOption("gemini");
  await page.getByLabel("API key 1", { exact: true }).fill("fake-main-only-for-test");
  await page.getByRole("button", { name: "Load models", exact: true }).click();
  await page.getByLabel("Available models").selectOption("test-chat-model");
  await page.getByRole("button", { name: "+ Add API key", exact: true }).click();
  await page.getByLabel("API key 2", { exact: true }).fill("fake-backup-only-for-test");
  await page.getByRole("button", { name: "Move API key 2 up", exact: true }).click();
  await expect(page.getByLabel("API key 1", { exact: true })).toHaveValue("fake-backup-only-for-test");
  await page.getByRole("button", { name: "Use as active model", exact: true }).click();
  await page.getByRole("button", { name: "Save changes", exact: true }).click();
  await expect(page.getByText("Profiles saved.", { exact: false })).toBeVisible();
  await expect(page.getByLabel("API key 1", { exact: true })).toHaveValue("");
  const payload = await page.evaluate(() => (window as any).aiTest.saves[0]);
  const profile = payload.config.profiles.find((p: any) => p.name === "Gemini work");
  expect(profile.model).toBe("test-chat-model");
  expect(payload.updates.find((k: any) => k.keyId === profile.keys[0].id).value).toBe("fake-backup-only-for-test");
  expect(JSON.stringify(payload.config)).not.toContain("fake-");
  await page.screenshot({ path: "test-results/ai-profiles.png", fullPage: true });
});

test("a failed save preserves the draft and Discard restores saved keys", async ({ page }) => {
  await page.goto("/settings.html");
  await page.getByLabel("API key 1", { exact: true }).fill("fake-replacement");
  await page.evaluate(() => { (window as any).aiTest.failSave = true; });
  await page.getByRole("button", { name: "Save changes", exact: true }).click();
  await expect(page.getByText("Could not save settings.", { exact: false })).toBeVisible();
  await expect(page.getByLabel("API key 1", { exact: true })).toHaveValue("fake-replacement");
  await page.getByLabel("AI provider").selectOption("openai");
  await expect(page.getByLabel("API key 1", { exact: true })).toHaveValue("");
  await page.getByRole("button", { name: "Discard", exact: true }).click();
  await expect(page.getByLabel("AI provider")).toHaveValue("anthropic");
  await expect(page.getByLabel("API key 1", { exact: true })).toHaveValue("");
});

test("local mode needs no key and online fallback is an explicit choice; narrow layout fits", async ({ page }) => {
  await page.setViewportSize({ width: 480, height: 780 });
  await page.goto("/settings.html");
  await page.getByRole("button", { name: "+ Add profile", exact: true }).click();
  await page.getByLabel("Profile name", { exact: true }).fill("Llama offline");
  await page.getByLabel("AI provider").selectOption("ollama");
  await expect(page.getByText("No key needed.", { exact: false })).toBeVisible();
  await page.getByLabel("Model ID", { exact: true }).fill("llama3.2");
  await page.getByRole("button", { name: "Use as active model", exact: true }).click();
  await page.getByRole("button", { name: "Fallback order", exact: true }).click();
  await expect(page.getByLabel("Automatic model fallback", { exact: true })).not.toBeChecked();
  await expect(page.getByLabel("Use Claude as fallback")).toBeDisabled();
  await page.getByLabel("Automatic model fallback", { exact: true }).check();
  await page.getByLabel("Use Claude as fallback").check();
  await expect(page.getByText("Online fallback is enabled", { exact: false })).toBeVisible();
  await page.getByRole("button", { name: "Save changes", exact: true }).click();
  await expect(page.getByText("Profiles saved.", { exact: false })).toBeVisible();
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth);
  expect(overflow).toBe(false);
  await page.screenshot({ path: "test-results/ai-fallback-narrow.png", fullPage: true });
});

async function chatFixture(page: Page) {
  await page.route("**/__chat-test.html", route => route.fulfill({ contentType: "text/html", body: `<!doctype html><html><body><script type="module">
    import "/src/style.css";
    import { buildPrompt } from "/src/views/chat.ts";
    import { State } from "/src/core/state.ts";
    State.settings = window.aiTest.settings;
    const view = buildPrompt(() => {});
    view.el.classList.add("on");
    document.body.append(view.el);
    view.el.style.cssText = "width: 520px; height: 350px; position: relative";
    State.subscribe(() => view.sync()); view.sync();
    window.chatState = State;
  </script></body></html>` }));
  await page.goto("/__chat-test.html");
}

test("chat failures preserve the question, Stop cancels, and replies identify fallback", async ({ page }) => {
  await chatFixture(page);
  await page.evaluate(() => { (window as any).aiTest.chatError = true; });
  await page.getByLabel("Message", { exact: true }).fill("hello");
  await page.getByLabel("Send message", { exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("HTTP 429");
  await expect(page.getByLabel("Message", { exact: true })).toHaveValue("hello");
  await expect(page.locator(".chat-row.user")).toHaveCount(0);
  await page.evaluate(() => { (window as any).aiTest.chatError = false; });
  await page.getByLabel("Send message", { exact: true }).click();
  await expect(page.getByLabel("Chat AI model")).toBeDisabled();
  await page.getByLabel("Stop request", { exact: true }).click();
  await expect(page.getByRole("alert")).toHaveText("Request cancelled.");
  await page.getByLabel("Send message", { exact: true }).click();
  await page.evaluate(() => (window as any).aiTest.pending.resolve({ text: "answer", profileId: "test", profileName: "Backup", model: "test-chat-model", usedFallback: true, keyLabel: "Backup key" }));
  await expect(page.locator(".chat-route")).toContainText("Backup · test-chat-model · fallback");
  await expect(page.locator(".chat-row.user")).toHaveCount(1);
  await page.screenshot({ path: "test-results/ai-chat.png" });
});

test("reset during a request does not restore the old conversation", async ({ page }) => {
  await chatFixture(page);
  await page.getByLabel("Message", { exact: true }).fill("old question");
  await page.getByLabel("Send message", { exact: true }).click();
  await expect(page.getByLabel("Stop request", { exact: true })).toBeVisible();
  await page.evaluate(() => {
    const w = window as any;
    w.chatState.chatHistory = []; w.chatState.notify();
    w.aiTest.pending.resolve({ text: "old answer" });
  });
  await expect(page.getByLabel("Send message", { exact: true })).toBeVisible();
  await expect(page.locator(".chat-row")).toHaveCount(0);
});
