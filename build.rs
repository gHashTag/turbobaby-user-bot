// Emits BUILD_VERSION env var available via env!("BUILD_VERSION") at compile time.
//
// Resolution order:
//   1. BUILD_VERSION_OVERRIDE env (set by Dockerfile ARG / CI)
//   2. `git rev-parse --short HEAD` (when .git is available locally)
//   3. fallback "dev"
//
// Format: <id>-<unix-ts>  e.g. "c49b56e-1747500123" or "docker-1747500123"

use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    println!("cargo:rerun-if-env-changed=BUILD_VERSION_OVERRIDE");
    println!("cargo:rerun-if-env-changed=RAILWAY_GIT_COMMIT_SHA");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads/main");

    let id = std::env::var("BUILD_VERSION_OVERRIDE")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            // Railway injects this automatically at build time.
            std::env::var("RAILWAY_GIT_COMMIT_SHA")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .map(|s| s.chars().take(7).collect::<String>())
        })
        .or_else(|| {
            Command::new("git")
                .args(["rev-parse", "--short", "HEAD"])
                .output()
                .ok()
                .and_then(|o| {
                    if o.status.success() {
                        let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                        if s.is_empty() { None } else { Some(s) }
                    } else {
                        None
                    }
                })
        })
        .unwrap_or_else(|| "dev".to_string());

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    println!("cargo:rustc-env=BUILD_VERSION={}-{}", id, ts);
}
