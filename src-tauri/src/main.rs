// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tracing_subscriber_multi::*;

use std::path::PathBuf;
use std::{env, fs, sync::Mutex};

#[cfg(target_os = "windows")]
pub fn local_log_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or(PathBuf::from("."))
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub fn local_log_dir() -> PathBuf {
    let mut base = dirs::data_local_dir().unwrap();
    base.push("angelsuite");
    fs::create_dir_all(&base).unwrap();
    base
}

fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(
            if cfg!(debug_assertions) || env::var("ANGELSUITE_DEBUG").is_ok_and(|v| !v.is_empty()) {
                tracing::Level::TRACE
            } else {
                tracing::Level::INFO
            },
        )
        .with_ansi(true)
        .with_writer(Mutex::new(DualWriter::new(
            std::io::stderr(),
            AnsiStripper::new(RotatingFile::new(
                local_log_dir().join("angelsuite-installer.log"),
                AppendCount::new(3),
                ContentLimit::Lines(1000),
                Compression::OnRotate(0),
            )),
        )))
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("failed to initialise logger");

    angelsuite_installer_lib::run()
}
