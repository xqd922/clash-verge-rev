// Builtin script: Convert proxy groups to Smart type
// Only runs when Smart core is active and user has enabled the feature
// eslint-disable-next-line unused-imports/no-unused-vars
function main(config, _name) {
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
      }
    });
  }
  return config;
}
