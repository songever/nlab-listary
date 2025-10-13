#![allow(non_snake_case)]

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

static CSS: Asset = asset!("/assets/styles.css");
static TAURI_ICON: Asset = asset!("/assets/tauri.svg");
static DIOXUS_ICON: Asset = asset!("/assets/dioxus.png");

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

#[derive(Serialize, Deserialize)]
struct GreetArgs<'a> {
    name: &'a str,
}
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

    let search_targets = move |_: FormEvent| {};

    rsx! {
        input {
            id: "search_bar",
            r#type: "text",
            placeholder: "Search pages in Nlab",
            oninput: search_targets,
        }
    }
}
