use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    #[wasm_bindgen(catch)]
    async fn invoke(cmd: &str, args: JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI_PLUGIN_DIALOG__"])]
    fn dialog(s: &str, opts: JsValue);

    #[wasm_bindgen(js_namespace = ["window", "__TAURI_PLUGIN_DIALOG__"])]
    async fn confirm(s: &str, opts: JsValue) -> JsValue;
}

#[derive(Serialize)]
struct DialogOptions<'a> {
    title: &'a str,
    kind: &'a str,
}

#[derive(Deserialize, Default)]
struct ManifestLoadResult {
    can_auto_update: bool,
    installer_update_available: Option<String>,
    products: Vec<ManifestLoadResultProduct>,
}

#[derive(Clone, Properties, Deserialize, PartialEq)]
pub struct ManifestLoadResultProduct {
    /// The internal ID of this product
    pub id: String,
    /// The name of this product
    pub name: String,
    /// A base64 encoded icon at 64x64 size.
    pub icon: Option<String>,
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
                match invoke("load_manifest", JsValue::null()).await {
                    Ok(res) => {
                        manifest_load_result.set(serde_wasm_bindgen::from_value(res).unwrap());
                    }
                    Err(e) => {
                        dialog(
                            &format!("{} Please try again later.", e.as_string().unwrap()),
                            serde_wasm_bindgen::to_value(&DialogOptions {
                                title: "Failed to load manifest",
                                kind: "warning",
                            })
                            .unwrap(),
                        );
                    }
                }
            });
        });
    }

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

    let onclick_update = {
        let cb = cb_set_progress_message.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();

            cb.emit((Some("Updating...".to_string()), false));

            spawn_local(async move {
                let _ = invoke("update_installer", JsValue::null()).await;
            });
        })
    };

    let update_automatically_button = if manifest_load_result.can_auto_update {
        Some(html! {
            <a class="btn" href="#" onclick={ onclick_update }>{ "Update Automatically" }</a>
        })
    } else {
        None
    };
    let update_notification = manifest_load_result
        .installer_update_available
        .clone()
        .map(|v| {
            let href =
                format!("https://github.com/lilopkins/angelsuite-installer/releases/tag/app-v{v}");
            html! {
                <p class="update-notification">
                    { "An installer update is available (version " }{ v } { ") " }
                    { update_automatically_button }
                    <a class="btn" href={ href } target="_blank">{ "Update Manually" }</a>
                </p>
            }
        });

    let items: Vec<_> = manifest_load_result
        .products
        .iter()
        .map(|prod| {
            let prod = prod.clone();
            html! {
                <Item
                    id={ prod.id }
                    name={ prod.name }
                    icon={ prod.icon }
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
            <div style={ if progress_message.is_some() { "display:none" } else { "" } }>{ update_notification }</div>
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
    /// A base64 encoded icon at 64x64 size.
    pub icon: Option<String>,
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
    let install_error = use_state(String::new);

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
        if local_version == remote_version || local_version != "0.0.0" && remote_version == "0.0.0"
        {
            State::InstalledLatest(local_version.clone())
        } else {
            State::InstalledUpdate(local_version.clone(), remote_version.clone())
        }
    } else {
        State::NotInstalled(remote_version.clone())
    };

    let state_str = match &state {
        State::InstalledLatest(v) => format!("Installed v{v} (latest)"),
        State::InstalledUpdate(v, l) => format!("Installed v{v} (updatable to v{l})"),
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
        let install_error = install_error.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();

            cb.emit((Some("Installing...".to_string()), false));

            let id = id.clone();
            let cb = cb.clone();
            let install_error = install_error.clone();
            spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&StartInstallUpgradeRemoveArgs {
                    id: (*id).clone(),
                })
                .unwrap();
                let result = invoke("install_app", args).await;
                match result {
                    Ok(_) => cb.emit((None, true)),
                    Err(e) => {
                        install_error.set(e.as_string().unwrap());
                        cb.emit((None, false));
                    }
                }
            });
        })
    };

    let onclick_start = {
        let id = id.clone();
        let install_error = install_error.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();

            let id = id.clone();
            let install_error = install_error.clone();
            spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&StartInstallUpgradeRemoveArgs {
                    id: (*id).clone(),
                })
                .unwrap();
                let result = invoke("start_app", args).await;
                if let Err(e) = result {
                    install_error.set(e.as_string().unwrap());
                }
            });
        })
    };

    let onclick_remove = {
        let id = id.clone();
        let name = props.name.clone();
        let cb = props.set_progress_message.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();

            let id = id.clone();
            let name = name.clone();
            let cb = cb.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let response = confirm(
                    &format!("Are you sure you want to remove {name}?"),
                    serde_wasm_bindgen::to_value(&DialogOptions {
                        title: "Are you sure?",
                        kind: "warning",
                    })
                    .unwrap(),
                )
                .await;
                // SAFETY: confirm always returns bool
                if response.as_bool().unwrap() {
                    cb.emit((Some("Removing...".to_string()), false));

                    let id = id.clone();
                    let cb = cb.clone();
                    spawn_local(async move {
                        let args = serde_wasm_bindgen::to_value(&StartInstallUpgradeRemoveArgs {
                            id: (*id).clone(),
                        })
                        .unwrap();

                        // SAFETY: this exists
                        invoke("remove_app", args).await.unwrap();

                        cb.emit((None, true));
                    });
                }
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
                // SAFETY: function exists
                invoke("set_prerelease", args).await.unwrap();
            });
        });
    }

    let icon = props.icon.as_ref().map(|ic| {
        html! {
            <img class="item__icon" src={ ic.clone() } aria-hidden="true" />
        }
    });

    html! {
        <div class="scrolling-list__item item">
            <p class="item__name">{ icon }{ &props.name }</p>
            <p class="item__state">{ &state_str }</p>
            <p class="item__description">{ &props.description }</p>
            <label class="item__prerelease">
                <input type="checkbox" name="allow_prerelease" onchange={ onchange_prerelease } checked={*allow_prereleases} />
                { "Use Prerelease Versions" }
            </label>
            <p style="color: red;">{ &*install_error }</p>
            <button class="btn" onclick={ onclick_start } hidden={ hide_start }>{ "Start" }</button>
            <button class="btn" onclick={ onclick_install } hidden={ hide_install_upgrade }>{ install_uprade_txt }</button>
            <button class="btn" onclick={ onclick_remove } hidden={ hide_remove }>{ "Remove" }</button>
        </div>
    }
}
