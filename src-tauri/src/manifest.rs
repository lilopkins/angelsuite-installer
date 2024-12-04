use getset::Getters;
use semver::{Version, VersionReq};
use serde::Deserialize;

/// The remote manifest object
#[derive(Clone, Debug, Deserialize, Getters)]
#[getset(get = "pub")]
pub struct Manifest {
    /// A semver of the latest installer version
    latest_installer_version: Version,
    /// Available products
    products: Vec<Product>,
}

/// The available products.
#[derive(Clone, Debug, Deserialize, Getters)]
#[getset(get = "pub")]
pub struct Product {
    /// A unique, unchanged internal ID for this product
    id: String,
    /// The name of the product
    name: String,
    /// The description of the product
    description: String,
    /// The target directory to install to for this product. This should be unique as this is also the directory that is deleted for uninstall.
    install_directory: String,
    /// A list of files/directories to remove when upgrading from particular versions
    removals: Vec<Removals>,
    /// A list of available versions
    versions: Vec<ProductVersion>,
}

impl Product {
    /// Calculate the latest version available of this product
    pub fn latest_version(&self, allow_prerelease: bool) -> Version {
        let mut latest_version = Version::new(0, 0, 0);
        for version in self.versions() {
            let v = version.version();
            if allow_prerelease || v.pre.is_empty() {
                if *v > latest_version {
                    latest_version = v.clone();
                }
            }
        }
        latest_version
    }

    pub fn latest_version_data(&self, allow_prerelease: bool) -> Option<DownloadSpec> {
        for v in self.versions() {
            if *v.version() == self.latest_version(allow_prerelease) {
                if cfg!(target_os = "windows") {
                    return v.downloads().windows().clone();
                } else if cfg!(target_os = "macos") {
                    return v.downloads().mac().clone();
                } else if cfg!(target_os = "linux") {
                    return v.downloads().linux().clone();
                }
            }
        }
        None
    }
}

/// A list of files/directories to remove when upgrading from particular versions
#[derive(Clone, Debug, Deserialize, Getters)]
#[getset(get = "pub")]
pub struct Removals {
    /// Only remove files when upgrading from a version matching this requirement
    on_upgrade_from: VersionReq,
    /// List of files to delete
    files: Vec<String>,
}

/// An available version of a product.
#[derive(Clone, Debug, Deserialize, Getters)]
#[getset(get = "pub")]
pub struct ProductVersion {
    /// Semantic version
    version: Version,
    /// The downloads for this product
    downloads: ProductDownloads,
}

/// The downloads
#[derive(Clone, Debug, Deserialize, Getters)]
#[getset(get = "pub")]
pub struct ProductDownloads {
    /// The Windows download
    windows: Option<DownloadSpec>,
    /// The Mac download
    mac: Option<DownloadSpec>,
    /// The Linux download
    linux: Option<DownloadSpec>,
}

/// The specification of the download
#[derive(Clone, Debug, Deserialize, Getters)]
#[getset(get = "pub")]
pub struct DownloadSpec {
    /// The URL to download the data from
    url: String,
    /// The download/install strategy
    strategy: DownloadStrategy,
}

/// The possible download and install strategies
#[derive(Clone, Debug, Deserialize)]
pub enum DownloadStrategy {
    /// Download a single file. This file should remain unprocessed in the target directory
    File(String),
    /// Download a compressed ZIP file. This file should be unzipped in the target directory
    ZipFile,
}
