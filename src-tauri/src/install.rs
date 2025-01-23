use std::collections::BTreeMap;

use getset::{Getters, Setters};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Getters, Default)]
#[getset(get = "pub")]
pub struct Install {
    products: BTreeMap<String, InstalledProduct>,
}

impl Install {
    pub fn save(&self) -> std::io::Result<()> {
        serde_json::to_writer(
            std::io::BufWriter::new(std::fs::File::create(super::local_install_file())?),
            self,
        )
        .unwrap();
        Ok(())
    }

    pub fn get_mut_product_or_default(&mut self, id: String) -> &mut InstalledProduct {
        if !self.products.contains_key(&id) {
            self.products
                .insert(id.clone(), InstalledProduct::default());
        }
        self.products.get_mut(&id).unwrap()
    }
}

#[derive(Clone, Serialize, Deserialize, Getters, Setters, Default)]
#[getset(get = "pub", set = "pub")]
pub struct InstalledProduct {
    /// The product name
    #[serde(default = "String::default")]
    name: String,
    /// The product description
    #[serde(default = "String::default")]
    description: String,
    /// The installed version, if the product is installed.
    version: Option<String>,
    /// The path to the working directory of this product, if it can be started.
    execute_working_directory: Option<String>,
    /// The path to the executable to start this product, if it can be started.
    main_executable: Option<String>,
    /// Should this product use prerelease versions?
    use_prerelease: bool,
}
