use pulldown_cmark::{html, Options, Parser};

#[tauri::command]
pub async fn blog_render_preview(markdown: String) -> Result<String, String> {
    let opts = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_SMART_PUNCTUATION;

    let parser = Parser::new_ext(&markdown, opts);
    let mut html_output = String::with_capacity(markdown.len() * 2);
    html::push_html(&mut html_output, parser);
    Ok(html_output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn renders_gfm_table() {
        let md = "| A | B |\n|---|---|\n| 1 | 2 |";
        let html = blog_render_preview(md.to_string()).await.unwrap();
        assert!(html.contains("<table>"), "expected table tag, got: {html}");
    }

    #[tokio::test]
    async fn renders_strikethrough() {
        let md = "~~deleted~~";
        let html = blog_render_preview(md.to_string()).await.unwrap();
        assert!(html.contains("<del>"), "expected del tag");
    }

    #[tokio::test]
    async fn renders_task_list() {
        let md = "- [x] done\n- [ ] todo";
        let html = blog_render_preview(md.to_string()).await.unwrap();
        assert!(html.contains("checkbox"), "expected checkbox input");
    }

    #[tokio::test]
    async fn fenced_code_block_has_language_class() {
        let md = "```rust\nfn main() {}\n```";
        let html = blog_render_preview(md.to_string()).await.unwrap();
        assert!(html.contains("language-rust"), "expected language-rust class");
    }

    #[tokio::test]
    async fn mermaid_fence_preserves_class() {
        let md = "```mermaid\ngraph LR\n  A --> B\n```";
        let html = blog_render_preview(md.to_string()).await.unwrap();
        assert!(html.contains("language-mermaid"), "expected language-mermaid class");
    }

    #[tokio::test]
    async fn empty_input_returns_empty() {
        let html = blog_render_preview("".to_string()).await.unwrap();
        assert!(html.trim().is_empty());
    }
}
