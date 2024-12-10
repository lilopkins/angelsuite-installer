use std::collections::HashMap;
use std::io::{BufReader, Cursor};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::{fs, io::Write};

use install::Install;
use manifest::{DownloadStrategy, Manifest};
use semver::Version;
use serde::Serialize;
use tauri::{Manager, Runtime};
use tauri_plugin_updater::UpdaterExt;

mod gzip;
mod install;
mod manifest;

pub const MANIFEST_URL: &str = "https://gist.githubusercontent.com/lilopkins/a9a624367414e48f860f0fa0ef609c98/raw/manifest.json";

#[cfg(target_os = "windows")]
pub fn local_install_file() -> PathBuf {
    PathBuf::from("./installer.json")
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub fn local_install_file() -> PathBuf {
    let mut path = dirs::config_dir().unwrap();
    path.push("angelsuite.json");
    path
}

#[cfg(target_os = "windows")]
pub fn local_install_dir() -> PathBuf {
    PathBuf::from(".")
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub fn local_install_dir() -> PathBuf {
    let mut base = dirs::data_local_dir().unwrap();
    base.push("angelsuite");
    fs::create_dir_all(&base).unwrap();
    base
}

#[derive(Default)]
struct AppData {
    manifest: Mutex<Option<Manifest>>,
    install_data: Mutex<Install>,
}

#[derive(Serialize, Default)]
struct ManifestLoadResult {
    installer_update_available: Option<String>,
    products: Vec<ManifestLoadResultProduct>,
}

#[derive(Serialize)]
pub struct ManifestLoadResultProduct {
    /// The internal ID of this product
    pub id: String,
    /// The name of this product
    pub name: String,
    /// The local installed version of this product, if installed
    pub local_version: Option<String>,
    /// The latest remote version of this product, excluding prereleases
    pub remote_version: String,
    /// The latest remote version of this product, including prereleases
    pub remote_version_prerelease: String,
    /// The description of this product
    pub description: String,
    /// Is there a package available that matches this OS, excluding prereleases?
    pub has_os_match_prerelease: bool,
    /// Is there a package available that matches this OS, including prereleases?
    pub has_os_match: bool,
    /// Can this installation be started?
    pub can_start: bool,
    /// Prerelease enabled
    pub allow_prerelease: bool,
}

#[tauri::command]
async fn load_manifest<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppData>,
    _window: tauri::Window<R>,
) -> Result<ManifestLoadResult, String> {
    let mut result = ManifestLoadResult::default();

    result.installer_update_available = if let Ok(u) = app.updater() {
        if let Ok(Some(update)) = u.check().await {
            Some(update.version)
        } else {
            None
        }
    } else {
        None
    };

    // Check if `installer.json` exists. If not, create it.
    let install_data = if let Ok(f) = fs::File::open(local_install_file()) {
        let i: Install =
            serde_json::from_reader(BufReader::new(f)).expect("installer.json is invalid on disk");
        i
    } else {
        Install::default()
            .save()
            .expect("couldn't produce default installer.json");
        serde_json::from_reader(BufReader::new(
            fs::File::open(local_install_file()).unwrap(),
        ))
        .expect("installer.json is invalid on disk")
    };

    let res = reqwest::get(MANIFEST_URL).await;

    if res.is_err() || std::env::var("ANGEL_WORK_OFFLINE") == Ok("1".to_string()) {
        // Work offline
        // Load installed products
        for (prod_id, prod) in install_data.products() {
            if prod.version().is_none() {
                continue;
            }
            result.products.push(ManifestLoadResultProduct {
                id: prod_id.clone(),
                name: prod.name().clone(),
                local_version: prod.version().clone(),
                remote_version: "0.0.0".to_string(),
                remote_version_prerelease: "0.0.0".to_string(),
                description: prod.description().clone(),
                has_os_match_prerelease: prod.main_executable().is_some(),
                has_os_match: prod.main_executable().is_some(),
                can_start: prod.main_executable().is_some(),
                allow_prerelease: *prod.use_prerelease(),
            });
        }

        *state.install_data.lock().unwrap() = install_data;
        return Ok(result);
    }
    let body: Manifest = res
        .unwrap()
        .json()
        .await
        .map_err(|_| "Failed to read manifest".to_string())?;

    *state.manifest.lock().unwrap() = Some(body.clone());

    // Detect products to present to frontend, current install status and upgrade possibility and notify frontend
    for prod in body.products() {
        let install_prod = install_data.products().get(prod.id());
        result.products.push(ManifestLoadResultProduct {
            id: prod.id().clone(),
            name: prod.name().clone(),
            local_version: install_prod.and_then(|p| p.version().clone()),
            remote_version: prod.latest_version(false).to_string(),
            remote_version_prerelease: prod.latest_version(true).to_string(),
            description: prod.description().clone(),
            has_os_match: prod.latest_version_data(false).is_some(),
            has_os_match_prerelease: prod.latest_version_data(true).is_some(),
            can_start: install_prod
                .map(|p| p.main_executable().is_some())
                .unwrap_or(false),
            allow_prerelease: install_prod.map(|p| *p.use_prerelease()).unwrap_or(false),
        });
    }

    *state.install_data.lock().unwrap() = install_data;

    Ok(result)
}

#[tauri::command]
fn set_prerelease<R: Runtime>(
    _app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppData>,
    _window: tauri::Window<R>,
    id: String,
    allow_prerelease: bool,
) -> Result<(), String> {
    let mut install_data = state.install_data.lock().unwrap();
    let prod = install_data.get_mut_product_or_default(id);
    prod.set_use_prerelease(allow_prerelease);
    install_data.save().unwrap();
    Ok(())
}

#[tauri::command]
async fn install_app<R: Runtime>(
    _app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppData>,
    _window: tauri::Window<R>,
    id: String,
) -> Result<(), String> {
    let mut install = state.install_data.lock().unwrap().clone();
    let mf = {
        let mf_mutex = state.manifest.lock().unwrap();
        mf_mutex.clone().unwrap()
    };
    for prod in mf.products() {
        if *prod.id() == id {
            let mut install_directory = local_install_dir();
            install_directory.push(prod.install_directory());
            let install_directory = install_directory;

            let prod_install = install.get_mut_product_or_default(id);
            let current_version = prod_install
                .version()
                .clone()
                .map(|v| Version::parse(&v).unwrap());
            let version = prod.latest_version(*prod_install.use_prerelease());

            // Determine any removals
            if let Some(v) = current_version {
                let removals = prod
                    .removals()
                    .iter()
                    .filter(|maybe_removal| maybe_removal.on_upgrade_from().matches(&v));
                for removal in removals {
                    if let Some(target_oses) = removal.on() {
                        if cfg!(target_os = "windows")
                            && !target_oses.contains(&"windows".to_string())
                        {
                            continue;
                        }
                        if cfg!(target_os = "macos") && !target_oses.contains(&"mac".to_string()) {
                            continue;
                        }
                        if cfg!(target_os = "linux") && !target_oses.contains(&"linux".to_string())
                        {
                            continue;
                        }
                    }
                    for file in removal.files() {
                        let mut path = install_directory.clone();
                        path.push(file);
                        if let Ok(meta) = fs::symlink_metadata(&path) {
                            if meta.is_dir() {
                                let _ = fs::remove_dir_all(path);
                            } else {
                                let _ = fs::remove_file(path);
                            }
                        }
                    }
                }
            }

            // Install
            fs::create_dir_all(&install_directory).unwrap();

            let download = prod.latest_version_data(*prod_install.use_prerelease());
            if download.is_none() {
                return Err("Download not available for this operating system".to_string());
            }
            let download = download.unwrap();

            // Download file
            let req = reqwest::get(download.url())
                .await
                .map_err(|_| "Failed to get manifest".to_string())?
                .bytes()
                .await
                .map_err(|_| "Failed to download data".to_string())?;

            // Evaluate strategy
            match download.strategy() {
                DownloadStrategy::File { name, chmod } => {
                    let mut path = install_directory.clone();
                    path.push(name);
                    let mut f = fs::File::create(&path)
                        .map_err(|_| "Failed to create target file".to_string())?;
                    f.write_all(&req)
                        .map_err(|_| "Failed to write data".to_string())?;
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;

                        if *chmod {
                            let mut perms = fs::metadata(&path)
                                .map_err(|_| "Failed to set permissions".to_string())?
                                .permissions();
                            perms.set_mode(perms.mode() | 0o100);
                            fs::set_permissions(path, perms)
                                .map_err(|_| "Failed to set permissions".to_string())?;
                        }
                    }
                }
                DownloadStrategy::ZipFile => {
                    zip_extract::extract(Cursor::new(req), &install_directory, true)
                        .map_err(|_| "Failed to extract data".to_string())?;
                }
                DownloadStrategy::GzippedTarball => {
                    gzip::extract_tar_gz(Cursor::new(req), &install_directory)
                        .map_err(|_| "Failed to extract data".to_string())?;
                }
            }

            prod_install.set_name(prod.name().clone());
            prod_install.set_description(prod.description().clone());
            prod_install.set_version(Some(version.to_string()));
            if let Some(exec) = download.executable() {
                let mut main_exec_path = install_directory.clone();
                main_exec_path.push(exec);
                prod_install
                    .set_main_executable(Some(main_exec_path.to_string_lossy().to_string()));
                prod_install.set_execute_working_directory(Some(
                    install_directory.to_string_lossy().to_string(),
                ));
            }
            install
                .save()
                .expect("failed to update installer.json after uninstalling");
            *state.install_data.lock().unwrap() = install;
            return Ok(());
        }
    }
    Err("No matching product found".to_string())
}

#[tauri::command]
async fn remove_app<R: Runtime>(
    _app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppData>,
    _window: tauri::Window<R>,
    id: String,
) -> Result<(), String> {
    // Find install directory for app ID, then delete.
    let mf_mutex = state.manifest.lock().unwrap();
    let mut install = state.install_data.lock().unwrap();
    let mf = mf_mutex.as_ref().unwrap();
    for prod in mf.products() {
        if *prod.id() == id {
            let mut install_directory = local_install_dir();
            install_directory.push(prod.install_directory());
            let install_directory = install_directory;

            fs::remove_dir_all(install_directory)
                .expect("failed to delete a directory managed by the installer");
            let prod_install = install.get_mut_product_or_default(id);
            prod_install.set_version(None);
            prod_install.set_main_executable(None);
            prod_install.set_execute_working_directory(None);
            install
                .save()
                .expect("failed to update installer.json after uninstalling");
            return Ok(());
        }
    }
    Err("Product not found!".to_string())
}

#[tauri::command]
fn start_app<R: Runtime>(
    _app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppData>,
    _window: tauri::Window<R>,
    id: String,
) -> Result<(), String> {
    // Find install directory for app ID, then delete.
    let install = state.install_data.lock().unwrap();
    let prod = install.products().get(&id);
    if prod.is_none() {
        return Err("Product not found!".to_string());
    }
    let prod = prod.unwrap();

    // Read .env
    let mut env_map = HashMap::new();
    if let Ok(iter) = dotenvy::dotenv_iter() {
        for (key, val) in iter.flatten() {
            env_map.insert(key, val);
        }
    }

    if let Some(exec_path) = prod.main_executable() {
        let canonical_path = fs::canonicalize(exec_path).map_err(|e| e.to_string())?;
        Command::new(canonical_path)
            .current_dir(
                prod.execute_working_directory()
                    .clone()
                    .unwrap_or(".".to_string()),
            )
            .envs(env_map)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn update_installer<R: Runtime>(
    app: tauri::AppHandle<R>,
    _window: tauri::Window<R>,
) -> tauri_plugin_updater::Result<()> {
    let update = app.updater()?.check().await?.unwrap();
    let mut downloaded = 0;

    // alternatively we could also call update.download() and update.install() separately
    update
        .download_and_install(
            |chunk_length, content_length| {
                downloaded += chunk_length;
                println!("downloaded {downloaded} from {content_length:?}");
            },
            || {
                println!("download finished");
            },
        )
        .await?;

    println!("update installed");
    app.restart();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            app.manage(AppData::default());
            Ok(())
        })
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            load_manifest,
            set_prerelease,
            install_app,
            remove_app,
            start_app,
            update_installer,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
