use serde_json::Value;
use std::sync::OnceLock;

pub const EN: &str = include_str!("../ui/locales/en.json");
pub const ZH_HANT: &str = include_str!("../ui/locales/zh-Hant.json");

/// Share the embedded browser pack with native menus and dialog titles.
pub fn text<'a>(language: &str, key: &'a str) -> &'a str {
    static ZH: OnceLock<Value> = OnceLock::new();
    if language == "zh-Hant" {
        ZH.get_or_init(|| serde_json::from_str(ZH_HANT).expect("valid bundled language pack"))
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or(key)
    } else {
        key
    }
}
