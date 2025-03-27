use ammonia::clean;
use dioxus::prelude::*;
use pulldown_cmark::{html, Parser};

#[component]
pub fn RenderMarkdown(source: &'static str) -> Element {
    let mut html_output = String::new();
    let parser = Parser::new(source);
    html::push_html(&mut html_output, parser);

    // Sanitize HTML
    let safe_html = clean(&html_output);

    rsx! {
        div {
            class: "markdown-content",
            dangerous_inner_html: "{safe_html}"
        }
    }
}
