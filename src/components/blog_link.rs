use crate::components::app::Route;
use dioxus::prelude::*;
use dioxus_router::prelude::*;

#[component]
pub fn BlogLink(slug: &'static str, title: &'static str) -> Element {
    rsx! {
        li {
            key: "{slug}",
            Link {
                to: Route::Post { slug: slug.into() },
                "{title}"
            }
        }
    }
}
