// This function is exported for use by the Clash core

function main(config, _name) {
  if (Array.isArray(config.proxies)) {
    config.proxies.forEach((p, i) => {
      if (p.type === "hysteria" && typeof p.alpn === "string") {
        config.proxies[i].alpn = [p.alpn];
      }
    });
  }
  return config;
}
