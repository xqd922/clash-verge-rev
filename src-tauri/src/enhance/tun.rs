use serde_yaml_ng::{Mapping, Value};

#[cfg(target_os = "macos")]
use crate::process::AsyncHandler;

macro_rules! revise {
    ($map: expr, $key: expr, $val: expr) => {
        let ret_key = Value::String($key.into());
        $map.insert(ret_key, Value::from($val));
    };
}

// if key not exists then append value
#[allow(unused_macros)]
macro_rules! append {
    ($map: expr, $key: expr, $val: expr) => {
        let ret_key = Value::String($key.into());
        if !$map.contains_key(&ret_key) {
            $map.insert(ret_key, Value::from($val));
        }
    };
}

pub fn use_tun(mut config: Mapping, enable: bool) -> Mapping {
    let tun_key = Value::from("tun");
    let tun_val = config.get(&tun_key);
    let mut tun_val = tun_val.map_or_else(Mapping::new, |val| {
        val.as_mapping().cloned().unwrap_or_else(Mapping::new)
    });

    if enable {
        // 读取DNS配置
        let dns_key = Value::from("dns");
        let dns_val = config.get(&dns_key);
        let mut dns_val = dns_val.map_or_else(Mapping::new, |val| {
            val.as_mapping().cloned().unwrap_or_else(Mapping::new)
        });
        let ipv6_key = Value::from("ipv6");
        let ipv6_val = config.get(&ipv6_key).and_then(|v| v.as_bool()).unwrap_or(false);

        // 检查现有的 enhanced-mode 设置
        let current_mode = dns_val
            .get(Value::from("enhanced-mode"))
            .and_then(|v| v.as_str())
            .unwrap_or("fake-ip");

        // 只有当 enhanced-mode 是 fake-ip 或未设置时才修改 DNS 配置
        if current_mode == "fake-ip" || !dns_val.contains_key(Value::from("enhanced-mode")) {
            revise!(dns_val, "enable", true);
            revise!(dns_val, "ipv6", ipv6_val);

            if !dns_val.contains_key(Value::from("enhanced-mode")) {
                revise!(dns_val, "enhanced-mode", "fake-ip");
            }

            if !dns_val.contains_key(Value::from("fake-ip-range")) {
                revise!(dns_val, "fake-ip-range", "198.18.0.1/16");
            }

            // Without a v6 fake-ip range, mihomo can't answer AAAA lookups once TUN owns the
            // traffic, so every IPv6 name fails to resolve. (upstream #7811 / #7373)
            if ipv6_val && !dns_val.contains_key(Value::from("fake-ip-range6")) {
                revise!(dns_val, "fake-ip-range6", "2001:2::0/64");
            }

            #[cfg(target_os = "macos")]
            {
                AsyncHandler::spawn(move || async move {
                    crate::utils::resolve::dns::restore_public_dns().await;
                    crate::utils::resolve::dns::set_public_dns("114.114.114.114".to_string()).await;
                });
            }
        }

        // 当TUN启用时，将修改后的DNS配置写回
        revise!(config, "dns", dns_val);
    } else {
        // TUN未启用时，仅恢复系统DNS，不修改配置文件中的DNS设置
        #[cfg(target_os = "macos")]
        AsyncHandler::spawn(move || async move {
            crate::utils::resolve::dns::restore_public_dns().await;
        });
    }

    // 更新TUN配置
    revise!(tun_val, "enable", enable);
    revise!(config, "tun", tun_val);

    config
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::use_tun;

    fn parse(yaml: &str) -> serde_yaml_ng::Mapping {
        serde_yaml_ng::from_str(yaml).expect("test yaml must parse")
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn fake_ip_v6_range_appears_when_ipv6_enabled() {
        let config = use_tun(parse(r"{ipv6: true, tun: {enable: false}}"), true);
        let dns = config
            .get("dns")
            .and_then(serde_yaml_ng::Value::as_mapping)
            .expect("dns added");

        assert_eq!(
            dns.get("fake-ip-range6").and_then(serde_yaml_ng::Value::as_str),
            Some("2001:2::0/64")
        );
        assert_eq!(
            dns.get("fake-ip-range").and_then(serde_yaml_ng::Value::as_str),
            Some("198.18.0.1/16")
        );
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn fake_ip_v6_range_respects_existing_and_ipv6_disabled() {
        let custom = use_tun(parse(r"{ipv6: true, dns: {fake-ip-range6: '2001:db8::/64'}}"), true);
        let dns = custom
            .get("dns")
            .and_then(serde_yaml_ng::Value::as_mapping)
            .expect("dns kept");
        assert_eq!(
            dns.get("fake-ip-range6").and_then(serde_yaml_ng::Value::as_str),
            Some("2001:db8::/64")
        );

        let no_v6 = use_tun(parse(r"{ipv6: false}"), true);
        let dns = no_v6
            .get("dns")
            .and_then(serde_yaml_ng::Value::as_mapping)
            .expect("dns added");
        assert!(!dns.contains_key("fake-ip-range6"), "no v6 range when ipv6 is off");
    }

    #[test]
    fn disabled_tun_leaves_dns_untouched() {
        let config = use_tun(parse(r"{ipv6: true, dns: {enable: false}}"), false);
        let dns = config
            .get("dns")
            .and_then(serde_yaml_ng::Value::as_mapping)
            .expect("dns kept");
        assert!(!dns.contains_key("fake-ip-range6"));
    }
}
