use serde::Serialize;
use serde_yaml_ng::Result;

pub fn to_mihomo_config_string<T: Serialize>(data: &T) -> Result<String> {
    let yaml = serde_yaml_ng::to_string(data)?;
    Ok(quote_fake_ip_filter_wildcards(&yaml))
}

fn quote_fake_ip_filter_wildcards(yaml: &str) -> String {
    let mut result = String::with_capacity(yaml.len());
    let mut in_fake_ip_filter = false;
    let mut fake_ip_filter_indent = 0;

    for line in yaml.lines() {
        let indent = leading_spaces(line);
        let trimmed = &line[indent..];

        if in_fake_ip_filter && indent <= fake_ip_filter_indent && !trimmed.starts_with("- ") {
            in_fake_ip_filter = false;
        }

        if !in_fake_ip_filter && trimmed == "fake-ip-filter:" {
            in_fake_ip_filter = true;
            fake_ip_filter_indent = indent;
            result.push_str(line);
            result.push('\n');
            continue;
        }

        if in_fake_ip_filter && indent >= fake_ip_filter_indent && trimmed.starts_with("- ") {
            result.push_str(&line[..indent]);
            result.push_str("- ");
            result.push_str(&quote_wildcard_item(&trimmed[2..]));
            result.push('\n');
            continue;
        }

        result.push_str(line);
        result.push('\n');
    }

    result
}

fn leading_spaces(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

fn quote_wildcard_item(item: &str) -> String {
    if !item.starts_with('"') && !item.starts_with('\'') && contains_wildcard(item) {
        quote_string(item)
    } else {
        item.to_string()
    }
}

fn quote_string(s: &str) -> String {
    "\'".to_string() + s.replace('\'', "''").as_str() + "\'"
}

fn contains_wildcard(value: &str) -> bool {
    value.contains('*') || value.starts_with('+') || value.starts_with('.')
}

#[cfg(test)]
mod tests {
    use super::to_mihomo_config_string;

    fn roundtrip(input: &str) -> serde_yaml_ng::Result<()> {
        let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(input)?;
        let output = to_mihomo_config_string(&value)?;
        let reparsed: serde_yaml_ng::Value = serde_yaml_ng::from_str(&output)?;

        assert_eq!(reparsed, value);
        Ok(())
    }

    #[test]
    fn quotes_fake_ip_filter_wildcards_without_changing_values() -> serde_yaml_ng::Result<()> {
        let input = r#"
dns:
  fake-ip-filter:
    - "*.lan"
    - "+.market.xiaomi.com"
    - ".example.com"
    - "time.*.com"
    - "plain.example.com"
"#;

        roundtrip(input)?;

        let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(input)?;
        let output = to_mihomo_config_string(&value)?;

        assert!(output.contains("- '*.lan'"));
        assert!(output.contains("- '+.market.xiaomi.com'"));
        assert!(output.contains("- '.example.com'"));
        assert!(output.contains("- 'time.*.com'"));
        assert!(output.contains("- plain.example.com"));
        Ok(())
    }

    #[test]
    fn nested_multiline_strings_roundtrip() -> serde_yaml_ng::Result<()> {
        roundtrip(
            r"
items:
  - name: demo
    desc: |
      line1
      line2
",
        )
    }
}
