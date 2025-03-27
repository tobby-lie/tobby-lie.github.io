use crate::components::markdown::RenderMarkdown;
use crate::components::app::Route;
use dioxus::prelude::*;
use dioxus_router::prelude::*;

#[component]
pub fn Post(slug: String) -> Element {
    let content = get_post_content(&slug);
    rsx! {
        div { id: "post",
            div { class: "back-button-container",
                Link {
                    to: Route::Home {},
                    "< Back"
                }
            }
            h1 { "{get_post_title(&slug)}" }
            div { class: "post-date",
                "{get_post_date(&slug)}"
            }
            RenderMarkdown { source: content }
        }
    }
}

fn get_post_content(slug: &str) -> &'static str {
    match slug {
        "post1" => include_str!("../blog/post1.md"),
        _ => "Post not found.",
    }
}

fn get_post_title(slug: &str) -> &'static str {
    match slug {
        "post1" => "First Blog Post",
        _ => "Unknown Post",
    }
}

fn get_post_date(slug: &str) -> &'static str {
    match slug {
        "post1" => "March 27, 2025",
        _ => "",
    }
}
