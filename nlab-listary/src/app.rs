#![allow(non_snake_case)]

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

static CSS: Asset = asset!("/assets/styles.css");
static TAURI_ICON: Asset = asset!("/assets/tauri.svg");
static DIOXUS_ICON: Asset = asset!("/assets/dioxus.png");

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = invoke)]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke_without_args(cmd: &str) -> JsValue;
}
#[derive(Serialize, Deserialize)]
struct SearchArgs {
    query: String,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
struct SearchIndex {
    title: String,
    url: String,
}
async fn get_search_results(query: &str) -> Result<Vec<SearchIndex>, String> {
    let args = serde_wasm_bindgen::to_value(&SearchArgs {
        query: query.to_string(),
    })
    .map_err(|e| format!("Serialization error: {:?}", e))?;

    let search_result = invoke("get_search_results", args).await;

    serde_wasm_bindgen::from_value(search_result)
        .map_err(|e| format!("Deserialization error: {:?}", e))
}

#[derive(Serialize, Deserialize)]
struct OpenArgs {
    url: String,
}
async fn open_url(url: &str) {
    let args = serde_wasm_bindgen::to_value(&OpenArgs {
        url: url.to_string(),
    })
    .unwrap();
    invoke("open_url", args).await;
}

// #[derive(Serialize, Deserialize)]
// struct GreetArgs<'a> {
//     name: &'a str,
// }
// let greet = move |_: FormEvent| async move {
//         if name.read().is_empty() {
//             return;
//         }

//         let name = name.read();
//         let args = serde_wasm_bindgen::to_value(&GreetArgs { name: &*name }).unwrap();
//         // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
//         let new_msg = invoke("greet", args).await.as_string().unwrap();
//         greet_msg.set(new_msg);
//     };

pub fn App() -> Element {
    let mut input_value = use_signal(|| String::from(""));

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
        input {
            id: "search_bar",
            r#type: "text",
            placeholder: "Search pages in Nlab",
            oninput: move |event| input_value.set(event.value()),
        }

        div {
            match search_results() {
                Some(Ok(results)) => rsx! {
                    for result in results {
                        div { "{result.title}" }
                        button {
                            onclick: {
                                move |_| {
                                    let url = result.url.to_owned();
                                    spawn(async move{
                                        open_url(&url).await
                                    });
                                }
                            }, 
                            "Open in browser"
                        }
                    }
                },
                Some(Err(_)) => rsx! { "Error" },
                None => rsx! { "Loading..." }
            }
        }
    }
}
