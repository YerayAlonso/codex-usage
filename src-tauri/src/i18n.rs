use serde_json::{json, Value};
use std::{collections::HashMap, sync::LazyLock};

pub const LANGUAGES: [&str; 6] = ["ca", "es", "en", "fr", "de", "it"];
static SYSTEM_LANGUAGE: LazyLock<String> = LazyLock::new(|| {
    normalize(&sys_locale::get_locale().unwrap_or_else(|| "en".into())).to_owned()
});
static DICTIONARIES: LazyLock<HashMap<&'static str, HashMap<String, String>>> =
    LazyLock::new(|| {
        [
            ("en", include_str!("../../locales/en.json")),
            ("ca", include_str!("../../locales/ca.json")),
            ("es", include_str!("../../locales/es.json")),
            ("fr", include_str!("../../locales/fr.json")),
            ("de", include_str!("../../locales/de.json")),
            ("it", include_str!("../../locales/it.json")),
        ]
        .into_iter()
        .map(|(key, text)| {
            (
                key,
                serde_json::from_str(text).expect("Invalid translation"),
            )
        })
        .collect()
    });

pub fn normalize(value: &str) -> &str {
    let prefix = value.split(['-', '_']).next().unwrap_or("en");
    LANGUAGES
        .into_iter()
        .find(|lang| prefix.eq_ignore_ascii_case(lang))
        .unwrap_or("en")
}

pub fn language(setting: &str) -> &str {
    if setting == "auto" {
        SYSTEM_LANGUAGE.as_str()
    } else {
        normalize(setting)
    }
}

pub fn text<'a>(locale: &str, key: &'a str) -> &'a str {
    DICTIONARIES
        .get(normalize(locale))
        .and_then(|dict| dict.get(key))
        .or_else(|| DICTIONARIES["en"].get(key))
        .map(String::as_str)
        .unwrap_or(key)
}

pub fn msg(key: &str, params: Value) -> String {
    format!("@{key} {params}")
}

pub fn detail(key: &str, detail: impl ToString) -> String {
    msg(key, json!({"detail": detail.to_string()}))
}

pub fn render(locale: &str, source: &str) -> String {
    render_nested(locale, source, 0)
}

fn render_nested(locale: &str, source: &str, depth: u8) -> String {
    if !source.starts_with('@') || depth > 5 {
        return source.to_owned();
    }
    let (key, raw) = source[1..].split_once(' ').unwrap_or((&source[1..], "{}"));
    let mut result = text(locale, key).to_owned();
    if let Ok(Value::Object(params)) = serde_json::from_str(raw) {
        for (name, value) in params {
            let value = value
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| value.to_string());
            result = result.replace(
                &format!("{{{name}}}"),
                &render_nested(locale, &value, depth + 1),
            );
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_language_has_all_keys_and_placeholders() {
        let pattern = regex::Regex::new(r"\{\w+\}").unwrap();
        for locale in LANGUAGES {
            assert_eq!(DICTIONARIES[locale].len(), DICTIONARIES["en"].len());
            for (key, value) in &DICTIONARIES["en"] {
                assert!(DICTIONARIES[locale].contains_key(key), "{locale}: {key}");
                let mut expected: Vec<_> = pattern.find_iter(value).map(|m| m.as_str()).collect();
                let mut actual: Vec<_> = pattern
                    .find_iter(&DICTIONARIES[locale][key])
                    .map(|m| m.as_str())
                    .collect();
                expected.sort();
                actual.sort();
                assert_eq!(expected, actual, "{locale}: {key}");
            }
        }
    }
    #[test]
    fn renders_nested_errors_and_detects_language() {
        assert_eq!(normalize("FR_fr"), "fr");
        assert_eq!(normalize("zh-CN"), "en");
        let source = detail("error.connect", "@error.busy");
        assert!(render("it", &source).contains("Un’altra operazione"));
    }
}
