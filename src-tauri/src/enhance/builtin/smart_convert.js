// Builtin script: Convert proxy groups to Smart type
// Only runs when Smart core is active and user has enabled the feature
// eslint-disable-next-line unused-imports/no-unused-vars
function main(config, _name) {
  // Set Smart data collector size limit (MB)
  if (!config.profile) {
    config.profile = {};
  }
  if (config.profile["smart-collector-size"] === undefined) {
    config.profile["smart-collector-size"] = 100;
  }

  if (Array.isArray(config["proxy-groups"])) {
    config["proxy-groups"].forEach(function (group, i) {
      var type = (group.type || "").toLowerCase();
      if (
        type === "url-test" ||
        type === "fallback" ||
        type === "load-balance"
      ) {
        config["proxy-groups"][i].type = "smart";
        // Enable data collection for ML training (if not explicitly set)
        if (config["proxy-groups"][i].collectdata === undefined) {
          config["proxy-groups"][i].collectdata = true;
        }
        // Enable LightGBM ML model (if not explicitly set)
        if (config["proxy-groups"][i].uselightgbm === undefined) {
          config["proxy-groups"][i].uselightgbm = true;
        }
        // Enable LightGBM auto-update (if not explicitly set)
        if (config["proxy-groups"][i]["lgbm-auto-update"] === undefined) {
          config["proxy-groups"][i]["lgbm-auto-update"] = true;
        }
        // Enable ASN affinity (if not explicitly set)
        if (config["proxy-groups"][i]["prefer-asn"] === undefined) {
          config["proxy-groups"][i]["prefer-asn"] = true;
        }
        // Set full data sampling rate (if not explicitly set)
        if (config["proxy-groups"][i]["sample-rate"] === undefined) {
          config["proxy-groups"][i]["sample-rate"] = 1;
        }
        // Use sticky-sessions strategy (if not explicitly set)
        if (config["proxy-groups"][i].strategy === undefined) {
          config["proxy-groups"][i].strategy = "sticky-sessions";
        }
      }
    });
  }
  return config;
}
