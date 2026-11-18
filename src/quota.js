import { escapeHtml as html } from "./format.js";

export function usageColor(used) {
  return used >= 90 ? "var(--danger)" : used >= 75 ? "var(--amber)" : "var(--mint)";
}

// The panel has a single meaning: bar length and percentage both represent USED quota.
// Remaining-quota rings belong exclusively to the native system tray.
export function quotaMarkup(usage, { translate: t, locale, resetTime }) {
  if (!usage?.windows.length) return `<p class="caption">${html(t("noData"))}</p>`;
  const number = new Intl.NumberFormat(locale, { maximumFractionDigits: 1 });
  return `<div class="quota-rows">${usage.windows.map((window) => {
    const used = Math.max(0, Math.min(100, window.used));
    const label = window.minutes === 300 ? t("fiveHours") : window.minutes === 10080 ? t("weekly") : window.minutes ? `${window.minutes} min` : t("limit");
    const color = usageColor(used);
    return `<div class="quota-row">
      <div class="row"><span class="quota-label">${html(label)}</span><strong style="color:${color}">${html(number.format(used))}<small>%</small></strong></div>
      <div class="bar" role="progressbar" aria-label="${html(label)}: ${html(t("usedQuota"))}" aria-valuemin="0" aria-valuemax="100" aria-valuenow="${used}"><div style="width:${used}%;background:${color}"></div></div>
      <div class="quota-meta"><span>${html(t("usageCaption"))}</span><span>${html(window.resetsAt ? t("resetIn", { time: resetTime(window.resetsAt) }) : t("resetUnknown"))}</span></div>
    </div>`;
  }).join("")}</div>`;
}
