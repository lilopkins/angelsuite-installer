use std::collections::HashMap;

use getset::{Getters, Setters};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Getters, Default)]
#[getset(get = "pub")]
pub struct Install {
    products: HashMap<String, InstalledProduct>,
}

impl Install {
    pub fn save(&self) -> std::io::Result<()> {
        serde_json::to_writer(
            std::io::BufWriter::new(std::fs::File::create(super::LOCAL_INSTALL_FILE)?),
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

#[derive(Serialize, Deserialize, Getters, Setters, Default)]
#[getset(get = "pub", set = "pub")]
pub struct InstalledProduct {
    /// The installed version, if the product is installed.
    version: Option<String>,
    /// Should this product use prerelease versions?
    use_prerelease: bool,
}
