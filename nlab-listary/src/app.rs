#![allow(non_snake_case)]
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

static CSS: Asset = asset!("/assets/styles.css");
// static TAURI_ICON: Asset = asset!("/assets/tauri.svg");
// static DIOXUS_ICON: Asset = asset!("/assets/dioxus.png");

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"] )]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = "invoke" )]
    async fn invoke_without_args(cmd: &str) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "event"])]
    async fn listen(event: &str, handler: &js_sys::Function) -> JsValue;
}

#[derive(Serialize, Deserialize)]
struct SearchArgs {
    query: String,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct SearchIndex {
    title: String,
    url: String,
}
async fn get_search_results(query: &str) -> Result<Vec<SearchIndex>, String> {
    if query.is_empty() {
        return Ok(vec![]);
    }

    let args = serde_wasm_bindgen::to_value(&SearchArgs {
        query: query.to_string(),
    })
    .map_err(|e| format!("Failed to serialize args: {:?}", e))?;

    // invoke returns Result<JsValue, JsValue>
    let ret = invoke("get_search_results", args).await;

    // check if the result is an error message
    if ret.is_undefined() || ret.is_null() {
        return Err("Received invalid response from backend".to_string());
    }

    // try to parse the result as an error message
    if let Ok(error_msg) = serde_wasm_bindgen::from_value::<String>(ret.clone()) {
        return Err(error_msg);
    }

    // parse the result as search results
    let results: Vec<SearchIndex> = serde_wasm_bindgen::from_value(ret)
        .map_err(|e| format!("Failed to parse search results: {:?}", e))?;

    Ok(results)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct OpenArgs {
    url: String,
}
async fn open_url(url: &str) -> Result<(), String> {
    let args = serde_wasm_bindgen::to_value(&OpenArgs {
        url: url.to_string(),
    })
    .map_err(|e| format!("Failed to serialize args: {:?}", e))?;

    let ret = invoke("open_url", args).await;
    if let Some(err) = ret.as_string() {
        eprintln!("Error opening URL: {}", err);
        return Err(err);
    }

    Ok(())
}

async fn event_listener(
    mut is_ready: Signal<bool>,
    mut init_status: Signal<String>,
    mut init_error: Signal<Option<String>>,
) {
    let status_closure = Closure::wrap(Box::new(move |event: JsValue| {
        if let Ok(payload) = js_sys::Reflect::get(&event, &JsValue::from_str("payload")) {
            if let Some(status) = payload.as_string() {
                init_status.set(status);
            }
        }
    }) as Box<dyn FnMut(JsValue)>);

    let _ = listen("init-status", status_closure.as_ref().unchecked_ref()).await;
    status_closure.forget();

    let complete_closure = Closure::wrap(Box::new(move |_: JsValue| {
        is_ready.set(true);
    }) as Box<dyn FnMut(JsValue)>);

    let _ = listen("init-complete", complete_closure.as_ref().unchecked_ref()).await;
    complete_closure.forget();

    let error_closure = Closure::wrap(Box::new(move |event: JsValue| {
        if let Ok(payload) = js_sys::Reflect::get(&event, &JsValue::from_str("payload")) {
            if let Some(error) = payload.as_string() {
                init_error.set(Some(error));
            }
        }
    }) as Box<dyn FnMut(JsValue)>);

    let _ = listen("init-error", error_closure.as_ref().unchecked_ref()).await;
    error_closure.forget();
}

pub fn App() -> Element {
    let is_ready = use_signal(|| false); // Always ready for now
    let init_status = use_signal(|| String::from("Initializing..."));
    let init_error = use_signal(|| Option::<String>::None);
    let mut input_value = use_signal(|| String::from(""));

    use_effect(move || {
        spawn(async move {
            event_listener(is_ready, init_status, init_error).await;
        });
    });

    let search_results = use_resource(move || {
        let query = input_value.read().clone();
        async move {
            if query.is_empty() {
                return Ok(vec![]);
            }
            get_search_results(&query).await
        }
    });

    rsx! {
        document::Link { rel: "stylesheet", href: CSS }
        document::Title { "nLab-listary" }

        div { class: "app",
            if let Some(error) = init_error() {
                div { class: "status-message error",
                    "Failed to initialize: {error}"
                }
            }

            if !is_ready() && init_error().is_none() {
                div { class: "loading-banner",
                    "{init_status()}"
                }
            }

            SearchBox {
                is_ready: is_ready(),
                input_value: input_value(),
                oninput: move |event: FormEvent| input_value.set(event.value())
            }

            SearchResultsList {
                is_ready: is_ready(),
                input_value: input_value(),
                search_results: search_results
            }
        }
    }
}

#[component]
fn SearchBox(is_ready: bool, input_value: String, oninput: EventHandler<FormEvent>) -> Element {
    rsx! {
        div { class: "search-container",
            input {
                id: "search_bar",
                class: "search-input",
                r#type: "text",
                placeholder: if is_ready { "Search pages in nLab..." } else { "Initializing..." },
                disabled: !is_ready,
                value: "{input_value}",
                autofocus: true,
                oninput: move |event| oninput.call(event),
            }
        }
    }
}

#[component]
fn SearchResultsList(
    is_ready: bool,
    input_value: String,
    search_results: Resource<Result<Vec<SearchIndex>, String>>,
) -> Element {
    rsx! {
        div { class: "results",
            if !is_ready {
                div { class: "status-message",
                    "⏳ Initializing..."
                }
            } else if input_value.is_empty() {
                div { class: "status-message hint",
                    "Type to search nLab pages!"
                }
            } else {
                match &*search_results.read_unchecked() {
                    Some(Ok(results)) => rsx! {
                        if results.is_empty() {
                            div { class: "status-message",
                                "No results found"
                            }
                        } else {
                            for result in results {
                                ResultItem {
                                    result: result.clone()
                                }
                            }
                        }
                    },
                    Some(Err(error)) => rsx! {
                        div { class: "status-message error",
                            "Error: {error}"
                        }
                    },
                    None => rsx! {
                        div { class: "status-message",
                            "Searching..."
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ResultItem(result: SearchIndex) -> Element {
    rsx! {
        div {
            class: "result-item",
            onclick: move |_| {
                let url = result.url.clone();
                spawn(async move {
                    if let Err(e) = open_url(&url).await {
                        web_sys::window()
                            .unwrap()
                            .alert_with_message(&format!("Failed to open URL: {}", e))
                            .ok();

                    }
                });
            },

            div { class: "result-title",
                "{result.title}"
            }
            div { class: "result-url",
                "{result.url}"
            }
        }
    }
}
