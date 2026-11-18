import { invoke } from "@tauri-apps/api/core";
import { version } from "../package.json";
import { listen } from "@tauri-apps/api/event";
import { countdown, escapeHtml as html, stale } from "./format.js";
import { quotaMarkup } from "./quota.js";
import { language, languages, message, resolveLanguage, setLanguage, t } from "./i18n";
import "./style.css";

type Settings = { automation: boolean; model: string; codexPath: string; language: string };
type Usage = {
  windows: { used: number; minutes: number; resetsAt: number }[];
  credits: number | null; expiresAt: number | null; accountId: string | null; updatedAt: number;
};
type Snapshot = {
  locale: string; settings: Settings; usage: Usage | null; error: string | null; busy: string | null;
  lastPromptAt: number | null; nextPromptAt: number | null; pendingReset: boolean;
  autostart: boolean; logs: { at: number; level: string; message: string }[];
};
type View = "usage" | "settings" | "activity";
const root = document.querySelector<HTMLElement>("#app")!;
const el = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;
const btn = (id: string) => el<HTMLButtonElement>(id);
const input = (id: string) => el<HTMLInputElement>(id);
let state: Snapshot | undefined;
let draft: Settings = { automation: false, model: "gpt-6-luna", codexPath: "", language: "auto" };
let dirty = false;
let localBusy = false;
let view: View = "usage";
let logPage = 0;
let notice = "";
let noticeError = false;
let detailText = "";
let detailPage = 0;
let detailChunks: string[] = [];
const isNative = "__TAURI_INTERNALS__" in window;

function date(timestamp: number | null) {
  if (!timestamp) return t("neverRun");
  const value = new Date(timestamp * 1000);
  return Number.isNaN(value.getTime()) ? "—" :
    new Intl.DateTimeFormat(language(), { dateStyle: "short", timeStyle: "short" }).format(value);
}
const duration = (timestamp: number) => countdown(timestamp, undefined, t, language());
function notify(value: unknown, error = false) {
  notice = message(value);
  noticeError = error;
  renderStatus();
}
function renderStatus() {
  const value = notice || state?.error || state?.busy || "";
  btn("notice").textContent = value;
  btn("notice").title = value;
  btn("notice").classList.toggle("warning", notice ? noticeError : !!state?.error);
  btn("notice").disabled = !value;
}
function shell() {
  root.innerHTML = `
    <header class="header"><span class="brand-mark" aria-hidden="true">◎</span><div><h1>Codex Usage</h1><p>${html(t("tagline"))}</p></div></header>
    <nav class="tabs" role="tablist" aria-label="Codex Usage">
      ${(["usage", "settings", "activity"] as const).map((item) => `<button id="tab-${item}" role="tab" aria-controls="view-${item}" aria-selected="${view === item}" tabindex="${view === item ? 0 : -1}">${html(t(`${item}Tab`))}</button>`).join("")}
    </nav>
    <div class="views">
      <section id="view-usage" class="view usage" role="tabpanel" aria-labelledby="tab-usage" ${view !== "usage" ? "hidden" : ""}>
        <section class="card quota-card">
          <div class="row"><h2>${html(t("usedQuota"))}</h2><button id="refresh" class="text-button">↻ ${html(t("refresh"))}</button></div>
          <div id="quota"></div><p id="updated" class="caption"></p>
        </section>
        <section class="card cycle-card">
          <div class="row"><h2>${html(t("cycle"))}</h2><span id="cycle-badge" class="badge"></span></div>
          <dl class="facts"><div><dt>${html(t("lastPrompt"))}</dt><dd id="last-prompt"></dd></div><div><dt>${html(t("nextPrompt"))}</dt><dd id="next-prompt"></dd></div></dl>
          <button id="execute" class="button full">${html(t("execute"))}<span aria-hidden="true">↗</span></button>
          <p class="caption single-line" title="${html(t("promptHint"))}">${html(t("promptHint"))}</p>
        </section>
        <section class="card credit-card">
          <div class="row"><h2>${html(t("credits"))}</h2><strong id="credits"></strong></div>
          <p id="credit-detail" class="caption single-line"></p>
          <button id="reset" class="button full" disabled></button>
        </section>
      </section>
      <section id="view-settings" class="view settings-view" role="tabpanel" aria-labelledby="tab-settings" ${view !== "settings" ? "hidden" : ""}>
        <form id="settings">
          <label class="field">${html(t("language"))}<select id="language"><option value="auto">${html(t("automaticLanguage"))}</option>${Object.entries(languages).map(([key, name]) => `<option value="${key}">${name}</option>`).join("")}</select></label>
          <label class="toggle"><span>${html(t("automation"))}</span><input id="automation" type="checkbox" role="switch"></label>
          <p class="caption">${html(t("automationHint"))}</p>
          <label class="toggle"><span>${html(t("autostart"))}</span><input id="autostart" type="checkbox" role="switch"></label>
          <details id="advanced"><summary>${html(t("advanced"))}</summary>
            <label class="field">${html(t("model"))}<input id="model" maxlength="120" required autocomplete="off" spellcheck="false"></label>
            <label class="field">${html(t("codexPath"))}<input id="codex-path" maxlength="4096" autocomplete="off" spellcheck="false" placeholder="${html(t("detect"))}" title="${html(t("pathHint"))}"></label>
          </details>
          <button id="save" class="button primary full" type="submit">${html(t("save"))}</button>
        </form>
        <p class="caption background-hint">${html(t("backgroundHint"))}</p>
      </section>
      <section id="view-activity" class="view activity-view" role="tabpanel" aria-labelledby="tab-activity" ${view !== "activity" ? "hidden" : ""}>
        <div class="row"><h2>${html(t("activityTab"))}</h2><button id="logs-folder" class="text-button">${html(t("logsFolder"))} ↗</button></div>
        <ol id="logs" class="logs"></ol>
        <div class="pagination"><button id="previous" class="text-button">${html(t("previous"))}</button><span id="log-page" class="caption"></span><button id="next" class="text-button">${html(t("next"))}</button></div>
      </section>
    </div>
    <button id="notice" class="status-line" aria-live="polite"></button>
    <footer><span>${html(t("unofficial"))}</span><span>v${html(version)}</span></footer>
    <dialog id="confirmation" aria-labelledby="confirmation-title"><form method="dialog">
      <h2 id="confirmation-title"></h2><p id="confirmation-body"></p>
      <div class="dialog-actions"><button class="button" value="cancel">${html(t("cancel"))}</button><button id="confirm-action" class="button primary" value="confirm">${html(t("confirm"))}</button></div>
    </form></dialog>
    <dialog id="detail" aria-labelledby="detail-title"><h2 id="detail-title">${html(t("logDetails"))}</h2><pre id="detail-body"></pre>
      <div class="pagination"><button id="detail-previous" class="text-button">${html(t("previous"))}</button><span id="detail-page" class="caption"></span><button id="detail-next" class="text-button">${html(t("next"))}</button></div>
      <button id="detail-close" class="button full">${html(t("close"))}</button>
    </dialog>`;
  bind();
}
function renderQuota() {
  const usage = state?.usage;
  const old = stale(usage, state?.error);
  el("quota").innerHTML = quotaMarkup(usage, { translate: t, locale: language(), resetTime: duration });
  el("quota").classList.toggle("stale", old);
  el("updated").textContent = usage ? `${old ? `${t("stale")} · ` : ""}${t("updated", { time: date(usage.updatedAt) })}` : t("neverUpdated");
  el("updated").classList.toggle("warning", old);
}
function renderActivity() {
  const logs = [...(state?.logs ?? [])].reverse();
  const count = window.innerHeight < 540 ? 3 : 4;
  const total = Math.max(1, Math.ceil(logs.length / count));
  logPage = Math.min(logPage, total - 1);
  el("logs").innerHTML = logs.slice(logPage * count, (logPage + 1) * count).map((entry, index) =>
    `<li><button class="log-entry ${entry.level === "error" ? "log-error" : ""}" data-log="${logPage * count + index}"><time>${html(date(entry.at))}</time><span>${html(entry.message)}</span></button></li>`
  ).join("") || `<li class="caption">${html(t("noActivity"))}</li>`;
  btn("previous").disabled = logPage === 0;
  btn("next").disabled = logPage >= total - 1;
  el("log-page").textContent = t("page", { current: logPage + 1, total });
  for (const item of el("logs").querySelectorAll<HTMLButtonElement>("[data-log]")) {
    item.onclick = () => showDetail(logs[Number(item.dataset.log)].message);
  }
}
function render() {
  renderQuota();
  const busy = localBusy || !!state?.busy || !isNative;
  el("cycle-badge").textContent = state?.busy ?? t(state?.settings.automation ? "active" : "paused");
  el("cycle-badge").classList.toggle("active", !!state?.settings.automation);
  el("last-prompt").textContent = date(state?.lastPromptAt ?? null);
  el("next-prompt").textContent = state?.settings.automation
    ? state.nextPromptAt ? duration(state.nextPromptAt) : t("scheduled") : t("disabled");
  el("next-prompt").title = state?.nextPromptAt ? date(state.nextPromptAt) : "";
  el("credits").textContent = state?.usage?.credits == null ? "—" : String(state.usage.credits);
  const creditText = state?.pendingReset ? t("pendingReset") : state?.usage?.credits == null ? t("creditUnknown")
    : state.usage.expiresAt ? t("creditExpires", { time: date(state.usage.expiresAt) }) : t("creditHint");
  el("credit-detail").textContent = creditText;
  el("credit-detail").title = creditText;
  btn("reset").textContent = t(state?.pendingReset ? "retryReset" : "reset");
  btn("reset").disabled = busy || !state?.usage?.accountId || stale(state?.usage, state?.error) ||
    (!state?.pendingReset && !(state?.usage?.credits != null && state.usage.credits > 0));
  for (const id of ["refresh", "execute", "save"]) btn(id).disabled = busy;
  for (const id of ["automation", "model", "codex-path", "autostart"]) input(id).disabled = busy;
  el<HTMLSelectElement>("language").disabled = busy;
  input("automation").checked = draft.automation;
  input("model").value = draft.model;
  input("codex-path").value = draft.codexPath;
  el<HTMLSelectElement>("language").value = draft.language;
  input("autostart").checked = state?.autostart ?? false;
  renderActivity();
  renderStatus();
}
function acceptState(snapshot: Snapshot) {
  state = snapshot;
  if (!dirty) draft = { ...snapshot.settings };
  const next = resolveLanguage(dirty ? (draft.language === "auto" ? navigator.language : draft.language) : snapshot.locale);
  if (language() !== next) { notice = ""; setLanguage(next); shell(); }
  render();
}
async function action(command: string, args?: Record<string, unknown>, success?: string) {
  if (localBusy || state?.busy || !isNative) return false;
  localBusy = true;
  render();
  let ok = false;
  try {
    await invoke(command, args);
    ok = true;
    if (success) notify(t(success));
  } catch (error) { notify(error, true); }
  finally {
    localBusy = false;
    try { acceptState(await invoke<Snapshot>("get_state")); } catch (error) { notify(error, true); }
    render();
  }
  return ok;
}
function confirm(title: string, body: string, label: string): Promise<boolean> {
  const dialog = el<HTMLDialogElement>("confirmation");
  el("confirmation-title").textContent = t(title);
  el("confirmation-body").textContent = t(body);
  btn("confirm-action").textContent = t(label);
  dialog.returnValue = "";
  dialog.showModal();
  return new Promise((resolve) => dialog.addEventListener("close", () => resolve(dialog.returnValue === "confirm"), { once: true }));
}
function renderDetail() {
  const total = Math.max(1, detailChunks.length);
  el("detail-body").textContent = detailChunks[detailPage] ?? "";
  btn("detail-previous").disabled = detailPage === 0;
  btn("detail-next").disabled = detailPage >= total - 1;
  el("detail-page").textContent = t("page", { current: detailPage + 1, total });
}
function showDetail(value: string) {
  detailText = value;
  detailPage = 0;
  el<HTMLDialogElement>("detail").showModal();
  // Paginate by rendered height, including long lines and newline-heavy diagnostics.
  const body = el("detail-body");
  const maxHeight = Math.max(48, Math.min(270, window.innerHeight - 220));
  detailChunks = [];
  const chars = Array.from(detailText);
  let offset = 0;
  while (offset < chars.length) {
    let low = 1;
    let high = Math.min(1200, chars.length - offset);
    while (low < high) {
      const length = Math.ceil((low + high) / 2);
      body.textContent = chars.slice(offset, offset + length).join("");
      if (body.getBoundingClientRect().height <= maxHeight) low = length;
      else high = length - 1;
    }
    detailChunks.push(chars.slice(offset, offset + low).join(""));
    offset += low;
  }
  renderDetail();
}
function switchView(next: View, focus = false) {
  view = next;
  for (const name of ["usage", "settings", "activity"] as const) {
    el(`view-${name}`).hidden = name !== view;
    btn(`tab-${name}`).setAttribute("aria-selected", String(name === view));
    btn(`tab-${name}`).tabIndex = name === view ? 0 : -1;
  }
  if (focus) btn(`tab-${next}`).focus();
}
function bind() {
  const views: View[] = ["usage", "settings", "activity"];
  for (const name of views) {
    btn(`tab-${name}`).onclick = () => switchView(name);
    btn(`tab-${name}`).onkeydown = (event) => {
      if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
      event.preventDefault();
      const index = event.key === "Home" ? 0 : event.key === "End" ? 2 : (views.indexOf(view) + (event.key === "ArrowRight" ? 1 : 2)) % 3;
      switchView(views[index], true);
    };
  }
  btn("refresh").onclick = () => void action("refresh_usage", undefined, "log.refreshed");
  btn("execute").onclick = async () => {
    if (await confirm("executeTitle", "executeBody", "executeConfirm")) await action("execute_prompt");
  };
  btn("reset").onclick = async () => {
    const pending = state?.pendingReset;
    if (await confirm(pending ? "retryTitle" : "resetTitle", pending ? "retryBody" : "resetBody", pending ? "retryConfirm" : "resetConfirm"))
      await action("reset_quota", { confirmed: true });
  };
  btn("logs-folder").onclick = () => { void invoke("open_logs").catch((error) => notify(error, true)); };
  btn("previous").onclick = () => { logPage--; renderActivity(); };
  btn("next").onclick = () => { logPage++; renderActivity(); };
  btn("notice").onclick = () => showDetail(notice || state?.error || state?.busy || "");
  btn("detail-previous").onclick = () => { detailPage--; renderDetail(); };
  btn("detail-next").onclick = () => { detailPage++; renderDetail(); };
  btn("detail-close").onclick = () => el<HTMLDialogElement>("detail").close();
  el("settings").oninput = () => {
    dirty = true;
    draft = { language: el<HTMLSelectElement>("language").value, automation: input("automation").checked, model: input("model").value, codexPath: input("codex-path").value };
  };
  el<HTMLSelectElement>("language").onchange = () => {
    notice = "";
    setLanguage(draft.language === "auto" ? navigator.language : draft.language);
    shell();
    render();
  };
  el<HTMLFormElement>("settings").onsubmit = async (event) => {
    event.preventDefault();
    const settings = { ...draft, model: draft.model.trim(), codexPath: draft.codexPath.trim() };
    if (settings.automation && !state?.settings.automation && !await confirm("automationTitle", "automationBody", "automationConfirm")) return;
    if (await action("save_settings", { settings }, "log.settings")) {
      dirty = false;
      if (state) acceptState(state);
    }
  };
  input("autostart").onchange = () => void action("set_autostart", { enabled: input("autostart").checked });
}
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && !el<HTMLDialogElement>("confirmation").open && !el<HTMLDialogElement>("detail").open && isNative) {
    event.preventDefault();
    void invoke("close_panel").catch((error) => notify(error, true));
  }
});
window.addEventListener("resize", () => renderActivity());
shell();
render();
async function start() {
  if (!isNative) { notify(t("preview")); return; }
  await listen<Snapshot>("state-changed", (event) => acceptState(event.payload));
  acceptState(await invoke<Snapshot>("get_state"));
  // No webview timer survives closing the panel.
  window.setInterval(() => render(), 30_000);
}
void start().catch((error) => notify(t("error.connect", { detail: message(error) }), true));
