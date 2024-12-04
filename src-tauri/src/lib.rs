use std::path::PathBuf;
use std::process::Command;
use std::{fs, io::Write};
use std::io::{BufReader, Cursor};
use std::sync::Mutex;

use install::Install;
use manifest::{DownloadStrategy, Manifest};
use semver::Version;
use serde::Serialize;
use tauri::{Manager, Runtime};

mod install;
mod manifest;

pub const MANIFEST_URL: &'static str = "https://gist.githubusercontent.com/lilopkins/a9a624367414e48f860f0fa0ef609c98/raw/manifest.json";
pub const LOCAL_INSTALL_FILE: &'static str = "installer.json";

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
    /// Can this installation be started?
    pub can_start: bool,
    /// Prerelease enabled
    pub allow_prerelease: bool,
}

#[tauri::command]
async fn load_manifest<R: Runtime>(
    _app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppData>,
    _window: tauri::Window<R>,
) -> Result<ManifestLoadResult, String> {
    let body: Manifest = reqwest::get(MANIFEST_URL)
        .await
        .map_err(|_| "Failed to get manifest".to_string())?
        .json()
        .await
        .map_err(|_| "Failed to parse manifest".to_string())?;
    *state.manifest.lock().unwrap() = Some(body.clone());
    let mut result = ManifestLoadResult::default();

    // Check if installer needs updating and notify frontend
    if semver::Version::parse(env!("CARGO_PKG_VERSION")).unwrap() < *body.latest_installer_version()
    {
        result.installer_update_available = Some(body.latest_installer_version().to_string());
    }

    // Check if `installer.json` exists. If not, create it.
    let install_data = if let Ok(f) = fs::File::open(LOCAL_INSTALL_FILE) {
        let i: Install =
            serde_json::from_reader(BufReader::new(f)).expect("installer.json is invalid on disk");
        i
    } else {
        Install::default()
            .save()
            .expect("couldn't produce default installer.json");
        serde_json::from_reader(BufReader::new(fs::File::open(LOCAL_INSTALL_FILE).unwrap()))
            .expect("installer.json is invalid on disk")
    };

    // Detect products to present to frontend, current install status and upgrade possibility and notify frontend
    for prod in body.products() {
        let install_prod = install_data.products().get(prod.id());
        result.products.push(ManifestLoadResultProduct {
            id: prod.id().clone(),
            name: prod.name().clone(),
            local_version: install_prod.map(|p| p.version().clone()).flatten(),
            remote_version: prod.latest_version(false).to_string(),
            remote_version_prerelease: prod.latest_version(true).to_string(),
            description: prod.description().clone(),
            can_start: install_prod.map(|p| p.main_executable().is_some()).unwrap_or(false),
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
fn install_app<R: Runtime>(
    _app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppData>,
    _window: tauri::Window<R>,
    id: String,
) -> Result<(), String> {
    let mf_mutex = state.manifest.lock().unwrap();
    let mut install = state.install_data.lock().unwrap();
    let mf = mf_mutex.as_ref().unwrap();
    for prod in mf.products() {
        if *prod.id() == id {
            let install_directory = prod.install_directory();
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
                    for file in removal.files() {
                        let path = format!("{install_directory}/{file}");
                        let _ = fs::remove_file(path);
                    }
                }
            }

            // Install
            fs::create_dir_all(install_directory).unwrap();

            let download = prod.latest_version_data(*prod_install.use_prerelease());
            if download.is_none() {
                return Err("Download not available for this operating system".to_string());
            }
            let download = download.unwrap();

            // Download file
            let req = reqwest::blocking::get(download.url())
                .map_err(|_| "Failed to get manifest".to_string())?
                .bytes()
                .map_err(|_| "Failed to download data".to_string())?;

            // Evaluate strategy
            match download.strategy() {
                DownloadStrategy::File { name, chmod } => {
                    let path = format!("{}/{name}", prod.install_directory());
                    let mut f = fs::File::create(&path)
                        .map_err(|_| "Failed to create target file".to_string())?;
                    f.write_all(&*req)
                        .map_err(|_| "Failed to write data".to_string())?;
                    #[cfg(target_os = "linux")]
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
                },
                DownloadStrategy::ZipFile => {
                    zip_extract::extract(Cursor::new(req), &PathBuf::from(prod.install_directory()), true)
                        .map_err(|_| "Failed to extract data".to_string())?;
                },
            }

            prod_install.set_version(Some(version.to_string()));
            if let Some(exec) = download.executable() {
                prod_install.set_main_executable(Some(format!("{}/{exec}", prod.install_directory())));
                prod_install.set_execute_working_directory(Some(prod.install_directory().clone()));
            }
            install
                .save()
                .expect("failed to update installer.json after uninstalling");
            return Ok(());
        }
    }
    Err("No matching product found".to_string())
}

#[tauri::command]
fn remove_app<R: Runtime>(
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
            let install_directory = prod.install_directory();
            fs::remove_dir_all(install_directory)
                .expect("failed to delete a directory managed by the installer");
            let prod_install = install.get_mut_product_or_default(id);
            prod_install.set_version(None);
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

    if let Some(exec_path) = prod.main_executable() {
        let canonical_path = fs::canonicalize(exec_path).map_err(|e| e.to_string())?;
        Command::new(canonical_path)
            .current_dir(prod.execute_working_directory().clone().unwrap_or(".".to_string()))
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            app.manage(AppData::default());
            Ok(())
        })
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            load_manifest,
            set_prerelease,
            install_app,
            remove_app,
            start_app,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
