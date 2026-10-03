import "./select.css";
import { h } from "./dom";

const controls = new WeakMap<HTMLSelectElement, () => void>();
let closeCurrent: (() => void) | null = null;

/** The native element keeps form state; the visible control owns interaction. */
export function smoothSelect(select: HTMLSelectElement): HTMLElement {
  const originalId = select.id || `select-${crypto.randomUUID()}`;
  select.id = `${originalId}-native`;
  select.hidden = true;
  select.setAttribute("aria-hidden", "true");
  select.tabIndex = -1;
  const label = h("span", { class: "select-label" });
  const button = h("button", { type: "button", id: originalId, class: "select-button", role: "combobox",
    "aria-haspopup": "listbox", "aria-expanded": "false", "aria-label": select.getAttribute("aria-label") || "Choose an option" },
    label, h("span", { class: "select-chevron", text: "⌄", "aria-hidden": "true" }));
  const root = h("div", { class: `smooth-select ${select.className}` }, select, button);
  select.removeAttribute("aria-label");
  let menu: HTMLElement | null = null;
  let index = 0;
  let prefix = "";
  let prefixTime = 0;
  let observer: MutationObserver | null = null;

  function sync() {
    label.textContent = select.selectedOptions[0]?.text || select.options[0]?.text || "Choose an option";
    button.disabled = select.disabled;
    if (menu && (button.disabled || !root.isConnected)) close();
  }
  function close() {
    if (!menu) return;
    menu.remove(); menu = null;
    button.setAttribute("aria-expanded", "false"); button.removeAttribute("aria-activedescendant");
    document.removeEventListener("pointerdown", outside, true);
    window.removeEventListener("resize", close); document.removeEventListener("scroll", onScroll, true);
    observer?.disconnect(); observer = null;
    if (closeCurrent === close) closeCurrent = null;
  }
  function outside(event: Event) {
    if (!root.contains(event.target as Node) && !menu?.contains(event.target as Node)) close();
  }
  function onScroll(event: Event) { if (!menu?.contains(event.target as Node)) placeMenu(); }
  function placeMenu() {
    if (!menu) return;
    const anchor = button.getBoundingClientRect();
    const bounds = root.closest(".chat-card")?.getBoundingClientRect();
    const bottom = Math.min(innerHeight - 10, bounds ? bounds.bottom - 8 : innerHeight - 10);
    const top = Math.max(10, bounds ? bounds.top + 8 : 10);
    const below = bottom - anchor.bottom - 6;
    const above = anchor.top - top - 6;
    const upwards = below < 100 && above > below;
    const height = Math.max(40, Math.min(248, upwards ? above : below));
    menu.style.cssText = `left:${anchor.left}px;width:${Math.min(anchor.width, innerWidth - anchor.left - 10)}px;max-height:${height}px;${upwards ? `bottom:${innerHeight - anchor.top + 6}px` : `top:${anchor.bottom + 6}px`}`;
  }
  function highlight(next: number) {
    if (!menu) return;
    const choices = [...menu.querySelectorAll<HTMLElement>("[role=option]")];
    index = Math.max(0, Math.min(choices.length - 1, next));
    choices.forEach((choice, i) => choice.classList.toggle("highlighted", i === index));
    if (choices[index]) {
      button.setAttribute("aria-activedescendant", choices[index].id);
      choices[index].scrollIntoView({ block: "nearest" });
    }
  }
  function choose() {
    const options = [...select.options].filter(option => !option.disabled);
    const chosen = options[index];
    if (!chosen) return;
    select.value = chosen.value; close(); sync();
    select.dispatchEvent(new Event("change", { bubbles: true })); button.focus();
  }
  function open() {
    if (button.disabled || menu || !root.isConnected) return;
    closeCurrent?.(); closeCurrent = close;
    const options = [...select.options].filter(option => !option.disabled);
    if (!options.length) return;
    const id = `${originalId}-list`;
    menu = h("div", { id, class: "select-menu", role: "listbox", "aria-label": button.getAttribute("aria-label") || "Options" });
    options.forEach((option, i) => {
      const row = h("div", { id: `${id}-${i}`, class: "select-option", role: "option",
        "aria-selected": String(option.value === select.value) },
        h("span", { text: option.text }), h("span", { class: "select-check", text: option.value === select.value ? "✓" : "" }));
      row.addEventListener("pointermove", () => highlight(i));
      row.addEventListener("pointerdown", event => event.preventDefault());
      row.addEventListener("click", () => { index = i; choose(); }); menu!.append(row);
    });
    document.body.append(menu);
    button.setAttribute("aria-expanded", "true"); button.setAttribute("aria-controls", id);
    placeMenu();
    highlight(Math.max(0, options.findIndex(option => option.value === select.value)));
    document.addEventListener("pointerdown", outside, true);
    window.addEventListener("resize", close); document.addEventListener("scroll", onScroll, true);
    observer = new MutationObserver(() => { if (!root.isConnected) close(); });
    observer.observe(document.body, { childList: true, subtree: true });
  }
  button.addEventListener("click", () => menu ? close() : open());
  button.addEventListener("keydown", event => {
    if (["ArrowDown", "ArrowUp", "Home", "End", "Enter", " ", "Escape", "Tab"].includes(event.key)) {
      if (event.key !== "Tab") event.preventDefault(); event.stopPropagation();
      if (event.key === "Escape" || event.key === "Tab") { close(); return; }
      if (!menu) { open(); return; }
      if (event.key === "Enter" || event.key === " ") { choose(); return; }
      highlight(event.key === "Home" ? 0 : event.key === "End" ? select.options.length - 1 : index + (event.key === "ArrowDown" ? 1 : -1));
    } else if (event.key.length === 1 && !event.ctrlKey && !event.metaKey && !event.altKey) {
      event.preventDefault(); event.stopPropagation(); open();
      if (performance.now() - prefixTime > 700) prefix = "";
      prefixTime = performance.now(); prefix += event.key.toLocaleLowerCase();
      const found = [...select.options].filter(option => !option.disabled).findIndex(option => option.text.toLocaleLowerCase().startsWith(prefix));
      if (found >= 0) highlight(found);
    }
  });
  select.addEventListener("change", sync);
  new MutationObserver(sync).observe(select, { attributes: true, childList: true, subtree: true });
  controls.set(select, sync); sync(); return root;
}

export function syncSelect(select: HTMLSelectElement) { controls.get(select)?.(); }
