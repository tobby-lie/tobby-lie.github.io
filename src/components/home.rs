use crate::components::app::Route;
use crate::components::markdown::RenderMarkdown;
use dioxus::prelude::*;
use dioxus_router::prelude::*;

#[component]
pub fn Home() -> Element {
    rsx! {
        div {
            id: "main",
            header {
                class: "site-header",
                "Tobby Lie"
            }
            ul {
                li {
                    Link {
                        to: Route::Post { slug: "post1".into() },
                        "First Blog Post"
                    }
                }
                // li {
                //     Link {
                //         to: Route::Post { slug: "post2".into() },
                //         "Second Blog Post"
                //     }
                // },
                // li {
                //     Link {
                //         to: Route::Post { slug: "post3".into() },
                //         "Third Blog Post"
                //     }
                // }
            }
        }
    }
}

#[component]
pub fn Post(slug: String) -> Element {
    let content = get_post_content(&slug);

    rsx! {
        div {
            id: "post",
            h1 { "{get_post_title(&slug)}" }
            RenderMarkdown { source: content }
        }
    }
}

fn get_post_content(slug: &str) -> &'static str {
    match slug {
        "post1" => include_str!("../blog/post1.md"),
        // "post2" => include_str!("../blog/post2.md"),
        // "post3" => include_str!("../blog/post3.md"),
        _ => "Post not found.",
    }
}

fn get_post_title(slug: &str) -> &'static str {
    match slug {
        "post1" => "First Blog Post",
        "post2" => "Second Blog Post",
        "post3" => "Third Blog Post",
        _ => "Unknown Post",
    }
}
