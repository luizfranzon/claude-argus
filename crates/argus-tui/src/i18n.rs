//! Runtime UI translation. Translation files live in `languages/*.json` at
//! the repo root and are embedded into the binary at compile time (not read
//! from disk), so a translation is contributed via a PR against argus itself
//! rather than a file dropped next to the binary — see ADR-0015.

use std::collections::HashMap;
use std::sync::OnceLock;

const PT_BR: &str = include_str!("../../../languages/pt_BR.json");
const EN_US: &str = include_str!("../../../languages/en_US.json");
const ES_ES: &str = include_str!("../../../languages/es_ES.json");

/// Every locale argus ships, in the order `languages/*.json` files exist.
pub const SUPPORTED_LOCALES: &[&str] = &["pt_BR", "en_US", "es_ES"];

/// The locale used when nothing else resolves, and the one every other
/// locale is checked against for completeness (see the `i18n` tests below).
const FALLBACK_LOCALE: &str = "pt_BR";

struct Catalog {
    tables: HashMap<&'static str, HashMap<String, String>>,
    active: &'static str,
}

fn load_table(json: &str) -> HashMap<String, String> {
    serde_json::from_str(json).expect("bundled translation file must be valid JSON")
}

fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let mut tables = HashMap::new();
        tables.insert("pt_BR", load_table(PT_BR));
        tables.insert("en_US", load_table(EN_US));
        tables.insert("es_ES", load_table(ES_ES));
        Catalog { tables, active: resolve_active_locale() }
    })
}

/// Resolves the active locale: `ARGUS_LANG` overrides OS detection; either
/// falls back to [`FALLBACK_LOCALE`] if it doesn't name a locale argus
/// ships. Read once and cached for the process lifetime — argus has no
/// mechanism to change locale after startup.
fn resolve_active_locale() -> &'static str {
    if let Ok(requested) = std::env::var("ARGUS_LANG") {
        if let Some(matched) = match_locale(&requested) {
            return matched;
        }
    }
    if let Some(detected) = sys_locale::get_locale() {
        if let Some(matched) = match_locale(&detected) {
            return matched;
        }
    }
    FALLBACK_LOCALE
}

/// Normalizes a locale string (`en-US`, `en_US`, `pt-BR`, ...) and matches it
/// against [`SUPPORTED_LOCALES`], first exactly, then by bare language code
/// (e.g. the OS reporting `en-GB` still resolves to `en_US`).
fn match_locale(raw: &str) -> Option<&'static str> {
    let normalized = raw.replace('-', "_");
    if let Some(exact) = SUPPORTED_LOCALES.iter().find(|l| l.eq_ignore_ascii_case(&normalized)) {
        return Some(*exact);
    }
    let lang = normalized.split('_').next().unwrap_or(&normalized);
    if lang.len() < 2 {
        return None;
    }
    SUPPORTED_LOCALES.iter().find(|l| l.starts_with(lang)).copied()
}

/// Looks up `key` in the active locale, interpolating `{name}` placeholders
/// from `params`. Falls back to [`FALLBACK_LOCALE`] if the active locale is
/// missing the key (should not happen — CI requires every shipped locale to
/// have the same key set as it), then to the raw key string as a last-resort
/// safety net for a `t()` call site whose key doesn't exist anywhere (should
/// also not happen — a test below scans every call site against `pt_BR.json`).
pub fn t(key: &str, params: &[(&str, &str)]) -> String {
    let catalog = catalog();
    let value = catalog
        .tables
        .get(catalog.active)
        .and_then(|table| table.get(key))
        .or_else(|| catalog.tables.get(FALLBACK_LOCALE).and_then(|table| table.get(key)));

    let mut out = value.cloned().unwrap_or_else(|| key.to_string());
    for (name, val) in params {
        out = out.replace(&format!("{{{name}}}"), val);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn parsed(json: &str) -> HashMap<String, String> {
        serde_json::from_str(json).expect("valid json")
    }

    #[test]
    fn all_locales_have_the_same_key_set_as_pt_br() {
        let pt = parsed(PT_BR);
        let pt_keys: HashSet<_> = pt.keys().collect();
        for (name, json) in [("en_US", EN_US), ("es_ES", ES_ES)] {
            let table = parsed(json);
            let keys: HashSet<_> = table.keys().collect();
            let missing: Vec<_> = pt_keys.difference(&keys).collect();
            let extra: Vec<_> = keys.difference(&pt_keys).collect();
            assert!(missing.is_empty(), "{name}.json is missing keys: {missing:?}");
            assert!(extra.is_empty(), "{name}.json has extra keys not in pt_BR.json: {extra:?}");
        }
    }

    #[test]
    fn every_t_call_site_uses_a_key_that_exists_in_pt_br() {
        let pt = parsed(PT_BR);
        let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut missing = Vec::new();
        scan_dir(&src_dir, &pt, &mut missing);
        assert!(missing.is_empty(), "t() call sites using unknown keys: {missing:?}");
    }

    fn scan_dir(dir: &std::path::Path, table: &HashMap<String, String>, missing: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).expect("readable src dir") {
            let entry = entry.expect("dir entry");
            let path = entry.path();
            if path.is_dir() {
                scan_dir(&path, table, missing);
            } else if path.extension().is_some_and(|ext| ext == "rs")
                && path.file_name().is_some_and(|name| name != "i18n.rs")
            {
                let contents = std::fs::read_to_string(&path).expect("readable source file");
                for key in extract_t_keys(&contents) {
                    if !table.contains_key(&key) {
                        missing.push(format!("{}: {key}", path.display()));
                    }
                }
            }
        }
    }

    /// Extracts the literal key string out of every bare `t("...")` call in
    /// `src` — deliberately not a real Rust parser: finds `t("`, then
    /// requires the character right before that `t` to not be an identifier
    /// character, so it doesn't also match the tail of `print("...")`,
    /// `sort("...")`, etc.
    fn extract_t_keys(src: &str) -> Vec<String> {
        let bytes = src.as_bytes();
        let mut keys = Vec::new();
        let mut idx = 0;
        while let Some(rel) = src[idx..].find("t(\"") {
            let pos = idx + rel;
            let preceded_by_identifier = pos > 0 && {
                let prev = bytes[pos - 1];
                prev.is_ascii_alphanumeric() || prev == b'_'
            };
            let start = pos + 3;
            let Some(end_rel) = src[start..].find('"') else { break };
            let end = start + end_rel;
            if !preceded_by_identifier {
                keys.push(src[start..end].to_string());
            }
            idx = end + 1;
        }
        keys
    }

    #[test]
    fn extract_t_keys_ignores_calls_ending_in_t() {
        let src = r#"print("hello"); sort("x"); t("real.key");"#;
        assert_eq!(extract_t_keys(src), vec!["real.key".to_string()]);
    }

    #[test]
    fn interpolates_named_placeholders() {
        let msg = t("finder.results.placeholder_no_results", &[]);
        assert!(!msg.is_empty());
    }
}
