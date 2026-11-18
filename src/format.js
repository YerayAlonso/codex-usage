export function remaining(used) {
  return Math.max(0, Math.min(100, 100 - used));
}

export function countdown(timestamp, now = Date.now() / 1000, translate, locale = "ca") {
  if (!timestamp) return translate ? translate("resetUnknown") : "Hora desconeguda";
  const minutes = Math.max(0, Math.ceil((timestamp - now) / 60));
  if (!minutes) return translate ? translate("resetDue") : "Pendent d’actualitzar";
  const days = Math.floor(minutes / 1440);
  const hours = Math.floor((minutes % 1440) / 60);
  const rest = minutes % 60;
  if (!translate) return days ? `${days} d ${hours} h` : hours ? `${hours} h ${rest} min` : `${rest} min`;
  const part = (value, unit) => new Intl.NumberFormat(locale, { style: "unit", unit, unitDisplay: "narrow" }).format(value);
  return days ? `${part(days, "day")} ${part(hours, "hour")}` : hours ? `${part(hours, "hour")} ${part(rest, "minute")}` : part(rest, "minute");
}

export function stale(usage, error, now = Date.now() / 1000) {
  return !!error || !usage || now - usage.updatedAt > 600 ||
    usage.windows.some((window) => window.resetsAt > 0 && now >= window.resetsAt);
}

export function escapeHtml(value) {
  return String(value).replace(/[&<>"']/g, (char) => ({
    "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
  })[char]);
}
