import en from "../locales/en.json";
import ca from "../locales/ca.json";
import es from "../locales/es.json";
import fr from "../locales/fr.json";
import de from "../locales/de.json";
import it from "../locales/it.json";

export const languages = { ca: "Català", es: "Español", en: "English", fr: "Français", de: "Deutsch", it: "Italiano" };
export type Language = keyof typeof languages;
export const dictionaries: Record<Language, Record<string, string>> = { en, ca, es, fr, de, it };
export function resolveLanguage(value: string): Language {
  const prefix = value.toLowerCase().split(/[-_]/)[0];
  return prefix in languages ? prefix as Language : "en";
}
let locale: Language = resolveLanguage(navigator.language);
export const language = () => locale;
export function setLanguage(value: string) {
  locale = resolveLanguage(value);
  document.documentElement.lang = locale;
}
export function t(key: string, params: Record<string, string | number> = {}): string {
  const template = dictionaries[locale][key] ?? en[key as keyof typeof en] ?? key;
  return template.replace(/\{(\w+)\}/g, (match, name: string) => String(params[name] ?? match));
}
export function message(value: unknown, depth = 0): string {
  const text = String(value);
  if (!text.startsWith("@") || depth > 5) return text;
  const split = text.indexOf(" ");
  const key = text.slice(1, split < 0 ? undefined : split);
  try {
    const params = split < 0 ? {} : JSON.parse(text.slice(split + 1));
    return t(key, Object.fromEntries(Object.entries(params).map(([name, item]) => [name, message(item, depth + 1)])));
  } catch { return t(key); }
}
