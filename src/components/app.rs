use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::components::{home::Home, post::Post};

static CSS: Asset = asset!("/assets/styles/main.css");

#[derive(Routable, Clone)]
pub enum Route {
    #[route("/")]
    Home {},
    #[route("/:slug")]
    Post { slug: String },
}

#[component]
pub fn App() -> Element {
    rsx! {
        document::Stylesheet { href: CSS }
        Router::<Route> {}
    }
}
