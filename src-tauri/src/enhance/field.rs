use serde_yaml_ng::{Mapping, Value};
use smartstring::alias::String;

pub const HANDLE_FIELDS: [&str; 12] = [
    "mode",
    "redir-port",
    "tproxy-port",
    "mixed-port",
    "socks-port",
    "port",
    "allow-lan",
    "log-level",
    "ipv6",
    "external-controller",
    "secret",
    "unified-delay",
];

pub const DEFAULT_FIELDS: [&str; 5] = ["proxies", "proxy-providers", "proxy-groups", "rule-providers", "rules"];

pub fn use_lowercase(config: &Mapping) -> Mapping {
    let mut ret = Mapping::new();

    for (key, value) in config.into_iter() {
        if let Some(key_str) = key.as_str() {
            let mut key_str = String::from(key_str);
            key_str.make_ascii_lowercase();
            ret.insert(Value::from(key_str.as_str()), value.clone());
        }
    }
    ret
}

/// Lowercase the top-level keys, moving the values instead of cloning them.
///
/// `use_lowercase` clones every value, which reallocates the whole tree; when the caller
/// already owns the mapping it can hand it over and pay a single pass instead.
pub fn use_lowercase_owned(config: Mapping) -> Mapping {
    let mut ret = Mapping::new();

    for (key, value) in config {
        if let Some(key_str) = key.as_str() {
            let mut key_str = String::from(key_str);
            key_str.make_ascii_lowercase();
            ret.insert(Value::from(key_str.as_str()), value);
        }
    }
    ret
}

pub fn use_sort(mut config: Mapping) -> Mapping {
    // `remove` moves each value out; the previous `get` + `clone` reallocated every entry,
    // including the multi-megabyte `proxies`/`rules` sequences.
    let mut ret = Mapping::new();
    HANDLE_FIELDS.into_iter().for_each(|key| {
        let key = Value::from(key);
        if let Some(value) = config.remove(&key) {
            ret.insert(key, value);
        }
    });

    let mut default_field_values = Mapping::new();
    for (key, value) in config {
        if let Some(key_str) = key.as_str() {
            if DEFAULT_FIELDS.contains(&key_str) {
                default_field_values.insert(key, value);
            } else if !HANDLE_FIELDS.contains(&key_str) {
                ret.insert(key, value);
            }
        }
    }

    DEFAULT_FIELDS.into_iter().for_each(|key| {
        let key = Value::from(key);
        if let Some(value) = default_field_values.remove(&key) {
            ret.insert(key, value);
        }
    });

    ret
}

#[inline]
pub fn use_keys<'a>(config: &'a Mapping) -> impl Iterator<Item = String> + 'a {
    config.iter().filter_map(|(key, _)| key.as_str()).map(|s: &str| {
        let mut s: String = s.into();
        s.make_ascii_lowercase();
        s
    })
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{use_lowercase_owned, use_sort};
    use serde_yaml_ng::{Mapping, Value};

    fn parse(yaml: &str) -> Mapping {
        serde_yaml_ng::from_str(yaml).expect("test yaml should parse")
    }

    fn keys(mapping: &Mapping) -> Vec<&str> {
        mapping.keys().filter_map(Value::as_str).collect()
    }

    #[test]
    fn use_sort_orders_handled_fields_then_extras_then_defaults() {
        let config = parse(
            r"
custom-b: 2
rules: []
mode: rule
custom-a: 1
mixed-port: 7897
proxies: []
",
        );

        let sorted = use_sort(config);

        assert_eq!(
            keys(&sorted),
            vec!["mode", "mixed-port", "custom-b", "custom-a", "proxies", "rules"],
            "handle fields first, then non-default extras in input order, then default fields"
        );
    }

    #[test]
    fn use_sort_preserves_values() {
        let config = parse(
            r"
rules:
  - MATCH,DIRECT
proxies:
  - name: a
    type: ss
",
        );

        let sorted = use_sort(config);

        assert_eq!(sorted.get("rules").and_then(Value::as_sequence).map(Vec::len), Some(1));
        assert!(sorted.get("proxies").and_then(Value::as_sequence).is_some());
    }

    #[test]
    fn use_lowercase_owned_lowercases_keys_and_keeps_values() {
        let config = parse("MODE: rule\nProxies:\n  - name: a\n");

        let lowercased = use_lowercase_owned(config);

        assert_eq!(keys(&lowercased), vec!["mode", "proxies"]);
        assert_eq!(lowercased.get("mode").and_then(Value::as_str), Some("rule"));
        assert!(lowercased.get("proxies").and_then(Value::as_sequence).is_some());
    }

    #[test]
    fn use_lowercase_owned_skips_non_string_keys() {
        let mut config = Mapping::new();
        config.insert(Value::from(1), Value::from("numeric"));
        config.insert(Value::from("Keep"), Value::from("string"));

        let lowercased = use_lowercase_owned(config);

        assert_eq!(keys(&lowercased), vec!["keep"], "non-string keys are dropped");
    }
}
