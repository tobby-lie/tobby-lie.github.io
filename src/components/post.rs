use crate::components::markdown::RenderMarkdown;
use dioxus::prelude::*;

#[component]
pub fn Post(slug: String) -> Element {
    let content = get_post_content(&slug);
    rsx! {
        div { id: "post",
            h1 { "{slug}" }
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
