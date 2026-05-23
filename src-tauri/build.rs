use std::{
    env,
    path::{Path, PathBuf},
};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("missing manifest dir"));
    let target = env::var("TARGET").expect("missing cargo target triple");

    ensure_runtime_artifacts(&manifest_dir, &target);
    tauri_build::build();
}

fn ensure_runtime_artifacts(manifest_dir: &Path, target: &str) {
    let is_windows = target.contains("windows");
    let mut missing = Vec::new();

    collect_missing(
        manifest_dir,
        "resources",
        &["Country.mmdb", "geoip.dat", "geosite.dat"],
        &mut missing,
    );

    if is_windows {
        collect_missing(
            manifest_dir,
            "resources",
            &[
                "clash-verge-service.exe",
                "install-service.exe",
                "uninstall-service.exe",
                "enableLoopback.exe",
            ],
            &mut missing,
        );
    } else {
        collect_missing(
            manifest_dir,
            "resources",
            &[
                "clash-verge-service",
                "install-service",
                "uninstall-service",
            ],
            &mut missing,
        );
    }

    let exe_ext = if is_windows { ".exe" } else { "" };
    let sidecars = [
        format!("verge-mihomo-{target}{exe_ext}"),
        format!("verge-mihomo-alpha-{target}{exe_ext}"),
    ];
    let sidecar_refs = sidecars.iter().map(String::as_str).collect::<Vec<_>>();

    collect_missing(manifest_dir, "sidecar", &sidecar_refs, &mut missing);

    if !missing.is_empty() {
        let mut message = String::from("missing required Tauri runtime artifacts:\n");
        for path in missing {
            message.push_str(" - ");
            message.push_str(&path.display().to_string());
            message.push('\n');
        }
        message.push_str("\nRun `pnpm check ");
        message.push_str(target);
        message.push_str(
            "` before building, or use `pnpm build` to prepare the required files automatically.\n",
        );
        panic!("{message}");
    }
}

fn collect_missing(manifest_dir: &Path, dir: &str, files: &[&str], missing: &mut Vec<PathBuf>) {
    for file in files {
        let path = manifest_dir.join(dir).join(file);
        if !path.is_file() {
            missing.push(path);
        }
    }
}
