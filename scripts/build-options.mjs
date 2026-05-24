export function shouldRunPreTauriCheck(env = process.env) {
  return !["1", "true", "yes"].includes(
    String(env.SKIP_PRE_TAURI_CHECK || "").toLowerCase()
  );
}
