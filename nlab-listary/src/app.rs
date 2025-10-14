
#![allow(non_snake_case)]

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

static CSS: Asset = asset!("/assets/styles.css");
static TAURI_ICON: Asset = asset!("/assets/tauri.svg");
static DIOXUS_ICON: Asset = asset!("/assets/dioxus.png");

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
    // TODO: Implement Tauri backend integration
    // For now, return mock data
    if query.is_empty() {
        return Ok(vec![]);
    }
    
    // Mock search results
    Ok(vec![
        SearchIndex {
            title: format!("Mock result for: {}", query),
            url: "https://ncatlab.org/nlab/show/category".to_string(),
        },
        SearchIndex {
            title: "Another mock result".to_string(),
            url: "https://ncatlab.org/nlab/show/functor".to_string(),
        },
    ])
}

async fn open_url(url: &str) {
    // TODO: Implement Tauri backend integration
    println!("Would open URL: {}", url);
}

pub fn App() -> Element {
    let mut is_ready = use_signal(|| true); // Always ready for now
    let mut input_value = use_signal(|| String::from(""));
    
    // TODO: Add proper initialization check with Tauri backend

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
    search_results: Resource<Result<Vec<SearchIndex>, String>>
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
                    open_url(&url).await;
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
