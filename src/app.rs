use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

#[derive(Deserialize, Default)]
struct ManifestLoadResult {
    installer_update_available: Option<String>,
    products: Vec<ManifestLoadResultProduct>,
}

#[derive(Clone, Properties, Deserialize, PartialEq)]
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

#[function_component(App)]
pub fn app() -> Html {
    let progress_message = use_state(|| None::<String>);
    let update_manifest = use_state(|| 0);
    let manifest_load_result = use_state(ManifestLoadResult::default);

    {
        let manifest_load_result = manifest_load_result.clone();
        let update_manifest = update_manifest.clone();
        use_effect_with(update_manifest, |_| {
            spawn_local(async move {
                let res = invoke("load_manifest", JsValue::null()).await;
                manifest_load_result.set(serde_wasm_bindgen::from_value(res).unwrap());
            });
        });
    }

    let update_notification = manifest_load_result.installer_update_available.clone().map(|v| html! {
        <p class="update-notification">{ "An update to the installer is available. (version " }{ v } { ")" }</p>
    });

    let cb_set_progress_message = {
        let progress_message = progress_message.clone();
        let update_manifest = update_manifest.clone();
        let manifest_load_result = manifest_load_result.clone();
        Callback::from(move |(name, update): (Option<String>, bool)| {
            if update {
                manifest_load_result.set(ManifestLoadResult::default());
                update_manifest.set(*update_manifest + 1);
            }
            progress_message.set(name);
        })
    };

    let items: Vec<_> = manifest_load_result
        .products
        .iter()
        .map(|prod| {
            let prod = prod.clone();
            html! {
                <Item
                    id={ prod.id }
                    name={ prod.name }
                    local_version={ prod.local_version }
                    remote_version={ prod.remote_version }
                    remote_version_prerelease={ prod.remote_version_prerelease }
                    description={ prod.description }
                    allow_prerelease={ prod.allow_prerelease }
                    has_os_match_prerelease={ prod.has_os_match_prerelease }
                    has_os_match={ prod.has_os_match }
                    can_start={ prod.can_start }
                    set_progress_message={ &cb_set_progress_message } />
            }
        })
        .collect();

    html! {
        <>
            <div class="title">
                <img src="/public/icon.png" aria-hidden="true" alt="" />
                <h1>{"AngelSuite"}</h1>
            </div>
            { update_notification }
            <p hidden={ progress_message.is_none() }>{ &*progress_message }</p>

            <div class="scrolling-list" style={ if progress_message.is_some() { "display:none" } else { "" } }>
                { items }
            </div>
        </>
    }
}

#[derive(Clone, Properties, PartialEq)]
pub struct ItemProps {
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
    /// Prerelease enabled
    pub allow_prerelease: bool,
    /// Is there a package available that matches this OS, excluding prereleases?
    pub has_os_match_prerelease: bool,
    /// Is there a package available that matches this OS, including prereleases?
    pub has_os_match: bool,
    /// Can this installation be started?
    pub can_start: bool,
    /// Update the progress message
    pub set_progress_message: Callback<(Option<String>, bool)>,
}

enum State {
    InstalledLatest(String),
    InstalledUpdate(String, String),
    NotInstalled(String),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetPrereleaseArgs {
    id: String,
    allow_prerelease: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartInstallUpgradeRemoveArgs {
    id: String,
}

#[function_component(Item)]
pub fn item(props: &ItemProps) -> Html {
    let id = use_state(|| props.id.clone());
    let allow_prereleases = use_state(|| props.allow_prerelease);

    let remote_version = if *allow_prereleases {
        &props.remote_version_prerelease
    } else {
        &props.remote_version
    };
    let has_os_match = if *allow_prereleases {
        props.has_os_match_prerelease
    } else {
        props.has_os_match
    };
    let state = if let Some(local_version) = props.local_version.as_ref() {
        if local_version == remote_version {
            State::InstalledLatest(local_version.clone())
        } else {
            State::InstalledUpdate(local_version.clone(), remote_version.clone())
        }
    } else {
        State::NotInstalled(remote_version.clone())
    };

    let state_str = match &state {
        State::InstalledLatest(v) => format!("Installed v{v}, latest"),
        State::InstalledUpdate(v, l) => format!("Installed v{v}, v{l} available"),
        State::NotInstalled(l) => {
            if l == "0.0.0" || !has_os_match {
                "Not available for your system".to_string()
            } else {
                format!("v{} available", l)
            }
        }
    };

    let hide_install_upgrade = match &state {
        State::InstalledLatest(_) => true,
        _ => remote_version == "0.0.0" || !has_os_match,
    };

    let hide_remove = matches!(&state, State::NotInstalled(_));
    
    let hide_start = match &state {
        State::NotInstalled(_) => true,
        _ => !props.can_start,
    };

    let install_uprade_txt = match &state {
        State::InstalledUpdate(_, _) => "Update",
        State::NotInstalled(_) => "Install",
        _ => "Woops!",
    };

    let onclick_install = {
        let id = id.clone();
        let cb = props.set_progress_message.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();

            cb.emit((Some("Installing...".to_string()), false));

            let id = id.clone();
            let cb = cb.clone();
            spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&StartInstallUpgradeRemoveArgs {
                    id: (*id).clone(),
                })
                .unwrap();
                invoke("install_app", args).await;

                cb.emit((None, true));
            });
        })
    };

    let onclick_start = {
        let id = id.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();

            let id = id.clone();
            spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&StartInstallUpgradeRemoveArgs {
                    id: (*id).clone(),
                })
                .unwrap();
                invoke("start_app", args).await;
            });
        })
    };

    let onclick_remove = {
        let id = id.clone();
        let cb = props.set_progress_message.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();

            cb.emit((Some("Removing...".to_string()), false));

            let id = id.clone();
            let cb = cb.clone();
            spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&StartInstallUpgradeRemoveArgs {
                    id: (*id).clone(),
                })
                .unwrap();
                invoke("remove_app", args).await;

                cb.emit((None, true));
            });
        })
    };

    let onchange_prerelease = {
        let allow_prereleases = allow_prereleases.clone();
        Callback::from(move |e: Event| {
            e.prevent_default();
            // Update available latest version
            allow_prereleases.set(!*allow_prereleases);
        })
    };

    {
        let allow_prereleases = allow_prereleases.clone();
        let id = id.clone();
        use_effect_with(allow_prereleases.clone(), move |_| {
            spawn_local(async move {
                // Trigger event to updated `installer.json`
                let args = serde_wasm_bindgen::to_value(&SetPrereleaseArgs {
                    id: (*id).clone(),
                    allow_prerelease: *allow_prereleases,
                })
                .unwrap();
                invoke("set_prerelease", args).await;
            });
        });
    }

    html! {
        <div class="scrolling-list__item item">
            <p class="item__name">{ &props.name }</p>
            <p class="item__state">{ &state_str }</p>
            <p class="item__description">{ &props.description }</p>
            <label class="item__prerelease">
                <input type="checkbox" name="allow_prerelease" onchange={ onchange_prerelease } checked={*allow_prereleases} />
                { "Use Prerelease Versions" }
            </label>
            <button class="item__install" onclick={ onclick_start } hidden={ hide_start }>{ "Start" }</button>
            <button class="item__install" onclick={ onclick_install } hidden={ hide_install_upgrade }>{ install_uprade_txt }</button>
            <button class="item__install" onclick={ onclick_remove } hidden={ hide_remove }>{ "Remove" }</button>
        </div>
    }
}
