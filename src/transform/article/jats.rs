//! JATS full-text extraction and markdown rendering helpers.

use std::sync::OnceLock;

use regex::Regex;
use roxmltree::{Document, Node, NodeType};

use super::collapse_whitespace;

mod refs;

use self::refs::render_references;

pub fn extract_text_from_xml(xml: &str) -> String {
    try_extract_jats_markdown(xml).unwrap_or_else(|| strip_xml_tags_fallback(xml))
}

fn try_extract_jats_markdown(xml: &str) -> Option<String> {
    if let Some(rendered) = parse_and_render_jats(xml) {
        return Some(rendered);
    }

    let sanitized = strip_doctype_declaration(xml);
    if sanitized.as_str() == xml {
        return None;
    }

    parse_and_render_jats(&sanitized)
}

fn parse_and_render_jats(xml: &str) -> Option<String> {
    let doc = Document::parse(xml).ok()?;
    let root = doc.root_element();
    if root.tag_name().name() != "article" || !has_jats_content_anchor(root) {
        return None;
    }

    let mut blocks = Vec::new();
    convert_front(root, &mut blocks);
    convert_body(root, &mut blocks);
    if let Some(references) = render_references(root) {
        blocks.push(references);
    }

    let rendered = join_blocks(blocks);
    if rendered.is_empty() {
        None
    } else {
        Some(rendered)
    }
}

fn strip_doctype_declaration(xml: &str) -> String {
    static DOCTYPE_RE: OnceLock<Regex> = OnceLock::new();
    let re = DOCTYPE_RE
        .get_or_init(|| Regex::new(r#"(?is)<!DOCTYPE[^>]*>"#).expect("valid doctype regex"));
    re.replace(xml, "").to_string()
}

fn has_jats_content_anchor(root: Node<'_, '_>) -> bool {
    root.descendants().any(|node| {
        node.is_element() && matches!(node.tag_name().name(), "body" | "abstract" | "ref-list")
    })
}

fn convert_front(root: Node<'_, '_>, blocks: &mut Vec<String>) {
    let Some(front) = find_child(root, "front") else {
        return;
    };

    if let Some(title) = front
        .descendants()
        .find(|node| node.is_element() && node.has_tag_name("article-title"))
        .map(inline_text)
        .filter(|value| !value.is_empty())
    {
        blocks.push(format!("# {title}"));
    }

    if let Some(abstract_node) = front
        .descendants()
        .find(|node| node.is_element() && node.has_tag_name("abstract"))
    {
        blocks.push("## Abstract".into());
        append_content_blocks(abstract_node, 2, blocks);
    }
}

fn convert_body(root: Node<'_, '_>, blocks: &mut Vec<String>) {
    let Some(body) = find_child(root, "body") else {
        return;
    };
    append_content_blocks(body, 2, blocks);
}

fn append_content_blocks(node: Node<'_, '_>, heading_level: usize, blocks: &mut Vec<String>) {
    for child in node.children().filter(|child| child.is_element()) {
        match child.tag_name().name() {
            "title" | "label" => {}
            "p" => {
                if let Some(paragraph) = convert_paragraph(child) {
                    blocks.push(paragraph);
                }
            }
            "sec" => convert_section(child, heading_level, blocks),
            "fig" => {
                if let Some(figure) = convert_figure(child) {
                    blocks.push(figure);
                }
            }
            "table-wrap" => blocks.extend(convert_table_wrap(child)),
            "list" => {
                if let Some(list) = convert_list(child) {
                    blocks.push(list);
                }
            }
            _ => {}
        }
    }
}

fn convert_section(section: Node<'_, '_>, heading_level: usize, blocks: &mut Vec<String>) {
    if let Some(title) = find_child(section, "title")
        .map(inline_text)
        .filter(|value| !value.is_empty())
    {
        let level = heading_level.clamp(2, 6);
        blocks.push(format!("{} {}", "#".repeat(level), title));
    }
    append_content_blocks(section, heading_level + 1, blocks);
}

fn convert_paragraph(node: Node<'_, '_>) -> Option<String> {
    let text = inline_text(node);
    if text.is_empty() { None } else { Some(text) }
}

fn convert_figure(node: Node<'_, '_>) -> Option<String> {
    let label = find_child(node, "label").map(inline_text);
    let caption = find_child(node, "caption").and_then(caption_text);

    let mut parts = Vec::new();
    if let Some(label) = label.filter(|value| !value.is_empty()) {
        let suffix = if label.ends_with('.') { "" } else { "." };
        parts.push(format!("**{label}{suffix}**"));
    }
    if let Some(caption) = caption.filter(|value| !value.is_empty()) {
        parts.push(caption);
    }

    if parts.is_empty() {
        None
    } else {
        Some(format!("> {}", parts.join(" ")))
    }
}

fn convert_table_wrap(node: Node<'_, '_>) -> Vec<String> {
    let mut blocks = Vec::new();
    let label = find_child(node, "label").map(inline_text);
    let caption = find_child(node, "caption").and_then(caption_text);

    if label.is_some() || caption.is_some() {
        let mut parts = Vec::new();
        if let Some(label) = label.filter(|value| !value.is_empty()) {
            let suffix = if label.ends_with('.') { "" } else { "." };
            parts.push(format!("**{label}{suffix}**"));
        }
        if let Some(caption) = caption.filter(|value| !value.is_empty()) {
            parts.push(caption);
        }
        if !parts.is_empty() {
            blocks.push(parts.join(" "));
        }
    }

    let Some(table) = node
        .descendants()
        .find(|child| child.is_element() && child.has_tag_name("table"))
    else {
        return blocks;
    };

    if let Some(markdown) = convert_regular_table(table) {
        blocks.push(markdown);
    }
    blocks
}

fn convert_regular_table(table: Node<'_, '_>) -> Option<String> {
    let mut rows = Vec::new();

    for row in table
        .descendants()
        .filter(|node| node.is_element() && node.has_tag_name("tr"))
    {
        let mut cells = Vec::new();
        for cell in row.children().filter(|child| child.is_element()) {
            let tag = cell.tag_name().name();
            if !matches!(tag, "th" | "td") {
                continue;
            }
            if cell.attribute("rowspan").is_some() || cell.attribute("colspan").is_some() {
                return None;
            }
            cells.push(normalize_table_cell(&inline_text(cell)));
        }
        if !cells.is_empty() && cells.iter().any(|cell| !cell.is_empty()) {
            rows.push(cells);
        }
    }

    let first = rows.first()?;
    let width = first.len();
    if width == 0 || rows.iter().any(|row| row.len() != width) {
        return None;
    }

    let mut lines = Vec::with_capacity(rows.len() + 1);
    lines.push(format!("| {} |", first.join(" | ")));
    lines.push(format!("| {} |", vec!["---"; width].join(" | ")));
    for row in rows.iter().skip(1) {
        lines.push(format!("| {} |", row.join(" | ")));
    }
    Some(lines.join("\n"))
}

fn convert_list(node: Node<'_, '_>) -> Option<String> {
    let ordered = node
        .attribute("list-type")
        .is_some_and(|value| value.eq_ignore_ascii_case("order"));
    let mut items = Vec::new();

    for (index, item) in node
        .children()
        .filter(|child| child.is_element() && child.has_tag_name("list-item"))
        .enumerate()
    {
        let text = list_item_text(item);
        if text.is_empty() {
            continue;
        }
        if ordered {
            items.push(format!("{}. {text}", index + 1));
        } else {
            items.push(format!("- {text}"));
        }
    }

    if items.is_empty() {
        None
    } else {
        Some(items.join("\n"))
    }
}

fn list_item_text(node: Node<'_, '_>) -> String {
    let mut parts = Vec::new();
    for child in node.children().filter(|child| child.is_element()) {
        match child.tag_name().name() {
            "p" => {
                let text = inline_text(child);
                if !text.is_empty() {
                    parts.push(text);
                }
            }
            "list" => {
                if let Some(text) = convert_list(child) {
                    parts.push(text);
                }
            }
            _ => {
                let text = inline_text(child);
                if !text.is_empty() {
                    parts.push(text);
                }
            }
        }
    }

    if parts.is_empty() {
        inline_text(node)
    } else {
        parts.join(" ")
    }
}

fn caption_text(node: Node<'_, '_>) -> Option<String> {
    let mut parts = Vec::new();
    for child in node.children().filter(|child| child.is_element()) {
        match child.tag_name().name() {
            "title" | "p" => {
                let text = inline_text(child);
                if !text.is_empty() {
                    parts.push(text);
                }
            }
            "list" => {
                if let Some(list) = convert_list(child) {
                    parts.push(list);
                }
            }
            _ => {
                let text = inline_text(child);
                if !text.is_empty() {
                    parts.push(text);
                }
            }
        }
    }

    let joined = parts.join(" ");
    if joined.is_empty() {
        None
    } else {
        Some(joined)
    }
}

fn inline_text(node: Node<'_, '_>) -> String {
    let mut out = String::new();
    match node.node_type() {
        NodeType::Text => out.push_str(node.text().unwrap_or_default()),
        _ => {
            for child in node.children() {
                append_inline_node(child, &mut out);
            }
        }
    }
    collapse_whitespace(&out)
}

fn append_inline_node(node: Node<'_, '_>, out: &mut String) {
    match node.node_type() {
        NodeType::Root => {
            for child in node.children() {
                append_inline_node(child, out);
            }
        }
        NodeType::Element => {
            match node.tag_name().name() {
                "italic" => return append_wrapped_inline(node, "*", out),
                "bold" => return append_wrapped_inline(node, "**", out),
                "sup" => return append_wrapped_inline(node, "^", out),
                "sub" => return append_wrapped_inline(node, "~", out),
                "xref" => return append_xref(node, out),
                "ext-link" => return append_ext_link(node, out),
                _ => {}
            }

            for child in node.children() {
                append_inline_node(child, out);
            }
        }
        NodeType::Text => out.push_str(node.text().unwrap_or_default()),
        _ => {}
    }
}

fn append_wrapped_inline(node: Node<'_, '_>, marker: &str, out: &mut String) {
    let text = inline_text(node);
    if text.is_empty() {
        return;
    }
    out.push_str(marker);
    out.push_str(&text);
    out.push_str(marker);
}

fn append_xref(node: Node<'_, '_>, out: &mut String) {
    let text = inline_text(node);
    if text.is_empty() {
        return;
    }
    match node.attribute("ref-type") {
        Some("bibr") => {
            out.push('[');
            out.push_str(&text);
            out.push(']');
        }
        Some("fig") | Some("table") => {
            out.push('(');
            out.push_str(&text);
            out.push(')');
        }
        _ => out.push_str(&text),
    }
}

fn append_ext_link(node: Node<'_, '_>, out: &mut String) {
    const XLINK_NS: &str = "http://www.w3.org/1999/xlink";

    let text = inline_text(node);
    let url = node
        .attribute((XLINK_NS, "href"))
        .or_else(|| node.attribute("href"))
        .map(str::trim)
        .filter(|value| !value.is_empty());

    match (text.is_empty(), url) {
        (false, Some(url)) => {
            out.push('[');
            out.push_str(&text);
            out.push_str("](");
            out.push_str(url);
            out.push(')');
        }
        (false, None) => out.push_str(&text),
        (true, Some(url)) => out.push_str(url),
        (true, None) => {}
    }
}

fn find_child<'a, 'input>(node: Node<'a, 'input>, name: &str) -> Option<Node<'a, 'input>> {
    node.children()
        .find(|child| child.is_element() && child.has_tag_name(name))
}

fn normalize_table_cell(value: &str) -> String {
    collapse_whitespace(value).replace('|', "\\|")
}

fn join_blocks(blocks: Vec<String>) -> String {
    blocks
        .into_iter()
        .map(|block| block.trim().to_string())
        .filter(|block| !block.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn strip_xml_tags_fallback(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len().min(32_000));
    let mut in_tag = false;

    for ch in xml.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if in_tag => {}
            _ => out.push(ch),
        }
    }

    out = out.replace("\r\n", "\n");
    out = out.replace('\r', "\n");
    static EXCESS_NEWLINES_RE: OnceLock<Regex> = OnceLock::new();
    let re = EXCESS_NEWLINES_RE
        .get_or_init(|| Regex::new(r"\n{3,}").expect("valid excess-newlines regex"));
    out = re.replace_all(&out, "\n\n").into_owned();
    out.trim().to_string()
}

#[cfg(test)]
mod tests;
