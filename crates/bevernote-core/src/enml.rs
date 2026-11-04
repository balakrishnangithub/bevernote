use std::collections::HashMap;

use regex::{Captures, Regex};
use scraper::Html;

pub fn enml_to_html(enml: &str) -> String {
    let without_declaration = strip_prefix(enml);
    let mut html = without_declaration
        .replace("<en-note>", "")
        .replace("</en-note>", "");
    html = replace_enote_open_tag(&html);
    html = expand_empty_custom_element(&html, "en-media");
    html = expand_empty_custom_element(&html, "en-todo");
    html = expand_empty_custom_element(&html, "en-crypt");
    html = convert_modern_checklists(&html);
    html.trim().to_owned()
}

fn strip_prefix(enml: &str) -> String {
    let declaration = Regex::new(r"(?s)<\?xml.*?\?>").expect("valid regex");
    let doctype = Regex::new(r"(?s)<!DOCTYPE[^>]*>").expect("valid regex");
    let value = declaration.replace_all(enml, "");
    doctype.replace_all(&value, "").into_owned()
}

fn replace_enote_open_tag(html: &str) -> String {
    Regex::new(r"<en-note(?:\s[^>]*)?>")
        .expect("valid regex")
        .replace_all(html, "")
        .into_owned()
}

fn expand_empty_custom_element(html: &str, tag: &str) -> String {
    let pattern = format!(r"<{tag}([^>]*?)/>");
    Regex::new(&pattern)
        .expect("valid regex")
        .replace_all(html, |captures: &Captures| {
            format!(
                "<{tag}{}></{tag}>",
                captures.get(1).map_or("", |m| m.as_str())
            )
        })
        .into_owned()
}

fn convert_modern_checklists(html: &str) -> String {
    let list = Regex::new(r"(?s)<ul([^>]*)>(.*?)</ul>").expect("valid regex");
    let item = Regex::new(r#"<li\s+style="([^"]*--en-todo\s*:\s*true[^"]*)"([^>]*)>"#)
        .expect("valid regex");
    list.replace_all(html, |captures: &Captures| {
        let body = captures.get(2).map_or("", |value| value.as_str());
        if !body.contains("--en-todo") {
            return captures
                .get(0)
                .map_or("", |value| value.as_str())
                .to_owned();
        }
        let body = item.replace_all(body, |item: &Captures| {
            let style = item.get(1).map_or("", |value| value.as_str());
            let rest = item.get(2).map_or("", |value| value.as_str());
            let checked = Regex::new(r"--en-checked\s*:\s*true")
                .expect("valid regex")
                .is_match(style);
            format!(
                "<li data-type=\"taskItem\" data-checked=\"{checked}\" style=\"{style}\"{rest}>"
            )
        });
        format!(
            "<ul{} data-type=\"taskList\">{}</ul>",
            captures.get(1).map_or("", |value| value.as_str()),
            body
        )
    })
    .into_owned()
}

pub fn html_to_enml(html: &str) -> String {
    let mut body = html.trim().to_owned();
    for tag in ["en-media", "en-todo", "en-crypt"] {
        let pattern = format!(r"<{tag}([^>]*)>\s*</{tag}>");
        body = Regex::new(&pattern)
            .expect("valid regex")
            .replace_all(&body, |captures: &Captures| {
                format!("<{tag}{} />", captures.get(1).map_or("", |m| m.as_str()))
            })
            .into_owned();
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\"?>\n<!DOCTYPE en-note SYSTEM \"http://xml.evernote.com/pub/enml2.dtd\"><en-note>{body}</en-note>"
    )
}

pub fn plain_text(html: &str) -> String {
    let document = Html::parse_fragment(html);
    let text = document.root_element().text().collect::<Vec<_>>().join(" ");
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn recognition_text(recognition_xml: &str) -> String {
    plain_text(recognition_xml)
}

pub fn rewrite_internal_links(html: &str, note_ids: &HashMap<String, String>) -> String {
    let link = Regex::new(
        r##"evernote:///view/[^/\s\"']+/[^/\s\"']+/([0-9a-fA-F-]{32,36})/[0-9a-fA-F-]{32,36}/?"##,
    )
    .expect("valid regex");
    link.replace_all(html, |captures: &Captures| {
        let guid = captures.get(1).map_or("", |m| m.as_str()).to_lowercase();
        note_ids.get(&guid).map_or_else(
            || captures.get(0).map_or("", |m| m.as_str()).to_owned(),
            |id| format!("bevernote://note/{id}"),
        )
    })
    .into_owned()
}

pub fn escape_cdata(value: &str) -> String {
    value.replace("]]>", "]]]]><![CDATA[>")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_evernote_elements_for_the_editor() {
        let enml = r#"<?xml version="1.0"?><!DOCTYPE en-note SYSTEM "x"><en-note><div>Hello <en-todo checked="true"/><en-media hash="abc" type="image/png"/></div></en-note>"#;
        let html = enml_to_html(enml);
        assert!(html.contains("<en-todo checked=\"true\"></en-todo>"));
        assert!(html.contains("<en-media hash=\"abc\" type=\"image/png\"></en-media>"));
        assert_eq!(plain_text(&html), "Hello");
        assert!(html_to_enml(&html).contains("<en-todo checked=\"true\" />"));
    }

    #[test]
    fn rewrites_only_known_internal_links() {
        let guid = "01234567-89ab-cdef-0123-456789abcdef";
        let map = HashMap::from([(guid.to_owned(), "local-id".to_owned())]);
        let input = format!("<a href=\"evernote:///view/u/s/{guid}/{guid}/\">Note</a>");
        assert_eq!(
            rewrite_internal_links(&input, &map),
            "<a href=\"bevernote://note/local-id\">Note</a>"
        );
    }
}
