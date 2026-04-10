use std::collections::HashMap;
use std::sync::OnceLock;

use regex::Regex;
use roxmltree::{Document, Node, NodeType};

use crate::entities::article::{
    AnnotationCount, Article, ArticleAnnotations, ArticleSearchResult, ArticleSource,
};
use crate::sources::europepmc::EuropePmcResult;
use crate::sources::pubmed::ESummaryEntry;
use crate::sources::pubtator::{PubTatorDocument, PubTatorSearchResult};

fn truncate_utf8(s: &str, max_bytes: usize, suffix: &str) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }

    let mut boundary = max_bytes;
    while boundary > 0 && !s.is_char_boundary(boundary) {
        boundary -= 1;
    }
    let mut out = s[..boundary].trim_end().to_string();
    out.push_str(suffix);
    out
}

fn decode_html_entities(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
}

fn strip_inline_html_tags(value: &str) -> String {
    static HTML_TAG_RE: OnceLock<Regex> = OnceLock::new();
    let re = HTML_TAG_RE.get_or_init(|| Regex::new(r"(?is)<[^>]+>").expect("valid regex"));
    re.replace_all(value, "").to_string()
}

fn normalize_compound_hyphens(value: &str) -> String {
    static COMPOUND_HYPHEN_RE: OnceLock<Regex> = OnceLock::new();
    let re = COMPOUND_HYPHEN_RE
        .get_or_init(|| Regex::new(r"([a-z])-(\d)").expect("valid compound-hyphen regex"));
    re.replace_all(value, "${1}${2}").into_owned()
}

pub fn clean_title(value: &str) -> String {
    strip_inline_html_tags(&decode_html_entities(value))
        .trim()
        .to_string()
}

pub fn clean_abstract(value: &str) -> String {
    strip_inline_html_tags(&decode_html_entities(value))
        .trim()
        .to_string()
}

pub fn normalize_article_search_text(value: &str) -> String {
    let base = collapse_whitespace(&clean_abstract(value)).to_ascii_lowercase();
    if !base.contains('-') {
        return base;
    }
    normalize_compound_hyphens(&base)
}

fn truncate_title(title: &str) -> String {
    const MAX_TITLE_BYTES: usize = 60;
    truncate_utf8(&clean_title(title), MAX_TITLE_BYTES, "…")
}

pub fn article_search_fallback_title(text: &str) -> String {
    truncate_title(text)
}

pub fn truncate_abstract(text: &str) -> String {
    const MAX_ABSTRACT_BYTES: usize = 1500;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.len() <= MAX_ABSTRACT_BYTES {
        return trimmed.to_string();
    }

    let short = truncate_utf8(trimmed, MAX_ABSTRACT_BYTES, "...");
    let total = trimmed.chars().count();
    format!("{short}\n\n(truncated, {total} chars total)")
}

pub fn article_search_abstract_snippet(text: &str) -> Option<String> {
    const MAX_ABSTRACT_BYTES: usize = 240;
    let cleaned = clean_abstract(text);
    if cleaned.is_empty() {
        return None;
    }
    let snippet = if cleaned.len() <= MAX_ABSTRACT_BYTES {
        cleaned
    } else {
        truncate_utf8(&cleaned, MAX_ABSTRACT_BYTES, "...")
    };
    Some(snippet)
}

pub fn truncate_authors(authors: &[String]) -> Vec<String> {
    if authors.len() <= 4 {
        return authors.to_vec();
    }
    match (authors.first(), authors.last()) {
        (Some(first), Some(last)) if first != last => vec![first.clone(), last.clone()],
        _ => authors.iter().take(2).cloned().collect(),
    }
}

pub fn from_pubtator_document(doc: &PubTatorDocument) -> Article {
    let mut title: Option<String> = None;
    let mut abstract_text: Option<String> = None;
    for p in &doc.passages {
        let kind = p
            .infons
            .as_ref()
            .and_then(|i| i.kind.as_deref())
            .unwrap_or("");
        let text = p.text.as_deref().unwrap_or("").trim();
        if text.is_empty() {
            continue;
        }
        match kind {
            "title" if title.is_none() => title = Some(text.to_string()),
            "abstract" if abstract_text.is_none() => abstract_text = Some(text.to_string()),
            _ => {}
        }
    }

    Article {
        pmid: doc.pmid.map(|v| v.to_string()),
        pmcid: doc.pmcid.clone(),
        doi: None,
        title: title.unwrap_or_default().trim().to_string(),
        authors: truncate_authors(&doc.authors),
        journal: doc
            .journal
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        date: doc
            .date
            .as_deref()
            .and_then(|d| d.get(0..10))
            .map(|s| s.to_string()),
        citation_count: None,
        publication_type: None,
        open_access: None,
        abstract_text: abstract_text
            .map(|t| truncate_abstract(&t))
            .filter(|t| !t.is_empty()),
        full_text_path: None,
        full_text_note: None,
        annotations: None,
        semantic_scholar: None,
        pubtator_fallback: false,
    }
}

fn parse_citation_count(value: Option<&serde_json::Value>) -> Option<u64> {
    let value = value?;
    match value {
        serde_json::Value::Number(n) => n.as_u64(),
        serde_json::Value::String(s) => s.trim().parse::<u64>().ok(),
        _ => None,
    }
}

fn normalize_publication_type(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    let mapped = if lower.contains("meta-analysis") {
        "Meta-Analysis".to_string()
    } else if lower.contains("review") {
        "Review".to_string()
    } else if lower.contains("case report") {
        "Case Report".to_string()
    } else if lower.contains("research-article") || lower.contains("journal article") {
        "Research Article".to_string()
    } else {
        trimmed.to_string()
    };
    Some(mapped)
}

fn collect_publication_types_from_value(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) => {
            for token in s.split(';') {
                let token = token.trim();
                if !token.is_empty() {
                    out.push(token.to_string());
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                if let Some(text) = item.as_str().map(str::trim).filter(|v| !v.is_empty()) {
                    out.push(text.to_string());
                    continue;
                }
                if let Some(text) = item
                    .as_object()
                    .and_then(|o| o.get("name"))
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                {
                    out.push(text.to_string());
                    continue;
                }
                collect_publication_types_from_value(item, out);
            }
        }
        serde_json::Value::Object(obj) => {
            for value in obj.values() {
                collect_publication_types_from_value(value, out);
            }
        }
        _ => {}
    };
}

fn publication_types(hit: &EuropePmcResult) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(value) = hit.pub_type.as_ref() {
        collect_publication_types_from_value(value, &mut out);
    }
    if let Some(value) = hit.pub_type_list.as_ref() {
        collect_publication_types_from_value(value, &mut out);
    }

    let mut deduped = Vec::new();
    for value in out {
        if deduped
            .iter()
            .any(|v: &String| v.eq_ignore_ascii_case(&value))
        {
            continue;
        }
        deduped.push(value);
    }
    deduped
}

fn parse_publication_type(hit: &EuropePmcResult) -> Option<String> {
    publication_types(hit)
        .into_iter()
        .find_map(|v| normalize_publication_type(&v))
}

fn is_retracted_publication(hit: &EuropePmcResult) -> bool {
    publication_types(hit)
        .into_iter()
        .any(|value| value.to_ascii_lowercase().contains("retracted publication"))
}

fn parse_open_access(value: Option<&serde_json::Value>) -> Option<bool> {
    let value = value?;
    match value {
        serde_json::Value::Bool(v) => Some(*v),
        serde_json::Value::String(v) => match v.trim().to_ascii_uppercase().as_str() {
            "Y" | "YES" | "TRUE" | "1" => Some(true),
            "N" | "NO" | "FALSE" | "0" => Some(false),
            _ => None,
        },
        serde_json::Value::Number(v) => v.as_u64().map(|n| n > 0),
        _ => None,
    }
}

fn split_author_string(value: &str) -> Vec<String> {
    let v = value.trim();
    if v.is_empty() {
        return vec![];
    }
    v.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .take(10)
        .collect()
}

pub fn from_europepmc_result(hit: &EuropePmcResult) -> Article {
    Article {
        pmid: hit.pmid.clone(),
        pmcid: hit.pmcid.clone(),
        doi: hit.doi.clone(),
        title: clean_title(hit.title.as_deref().unwrap_or_default()),
        authors: hit
            .author_string
            .as_deref()
            .map(split_author_string)
            .unwrap_or_default(),
        journal: hit
            .journal_title
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        date: hit
            .first_publication_date
            .as_ref()
            .or(hit.pub_year.as_ref())
            .map(|s| s.get(0..10).unwrap_or(s).to_string()),
        citation_count: parse_citation_count(hit.cited_by_count.as_ref()),
        publication_type: parse_publication_type(hit),
        open_access: parse_open_access(hit.is_open_access.as_ref()),
        abstract_text: hit
            .abstract_text
            .as_deref()
            .map(clean_abstract)
            .map(|text| truncate_abstract(&text))
            .filter(|text| !text.is_empty()),
        full_text_path: None,
        full_text_note: None,
        annotations: None,
        semantic_scholar: None,
        pubtator_fallback: false,
    }
}

pub fn merge_europepmc_metadata(article: &mut Article, hit: &EuropePmcResult) {
    if article.doi.is_none() {
        article.doi = hit.doi.clone();
    }
    if article.pmcid.is_none() {
        article.pmcid = hit.pmcid.clone();
    }
    if article.journal.is_none() {
        article.journal = hit
            .journal_title
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
    }
    if article.date.is_none() {
        article.date = hit
            .first_publication_date
            .as_ref()
            .or(hit.pub_year.as_ref())
            .map(|s| s.get(0..10).unwrap_or(s).to_string());
    }

    article.citation_count = parse_citation_count(hit.cited_by_count.as_ref());
    article.publication_type = parse_publication_type(hit);
    article.open_access = parse_open_access(hit.is_open_access.as_ref());
    if article.abstract_text.is_none() {
        article.abstract_text = hit
            .abstract_text
            .as_deref()
            .map(clean_abstract)
            .map(|text| truncate_abstract(&text))
            .filter(|text| !text.is_empty());
    }
}

pub fn from_europepmc_search_result(hit: &EuropePmcResult) -> Option<ArticleSearchResult> {
    let pmid = hit
        .pmid
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())?
        .to_string();
    let title = clean_title(hit.title.as_deref().unwrap_or_default());
    let abstract_text = hit.abstract_text.as_deref().map(clean_abstract);
    Some(ArticleSearchResult {
        pmid,
        pmcid: hit
            .pmcid
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        doi: hit
            .doi
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        title,
        journal: hit
            .journal_title
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        date: hit
            .first_publication_date
            .as_ref()
            .or(hit.pub_year.as_ref())
            .map(|s| s.get(0..10).unwrap_or(s).to_string()),
        citation_count: parse_citation_count(hit.cited_by_count.as_ref()),
        influential_citation_count: None,
        source: ArticleSource::EuropePmc,
        score: None,
        is_retracted: Some(is_retracted_publication(hit)),
        abstract_snippet: abstract_text
            .as_deref()
            .and_then(article_search_abstract_snippet),
        ranking: None,
        matched_sources: vec![ArticleSource::EuropePmc],
        normalized_title: normalize_article_search_text(hit.title.as_deref().unwrap_or_default()),
        normalized_abstract: abstract_text
            .as_deref()
            .map(normalize_article_search_text)
            .unwrap_or_default(),
        publication_type: parse_publication_type(hit),
        source_local_position: 0,
    })
}

pub fn from_pubtator_search_result(hit: &PubTatorSearchResult) -> Option<ArticleSearchResult> {
    let pmid = hit
        .pmid
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())?
        .to_string();
    Some(ArticleSearchResult {
        pmid,
        pmcid: None,
        doi: None,
        title: clean_title(hit.title.as_deref().unwrap_or_default()),
        journal: hit
            .journal
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(|v| v.to_string()),
        date: hit
            .date
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(|v| v.get(0..10).unwrap_or(v).to_string()),
        citation_count: None,
        influential_citation_count: None,
        source: ArticleSource::PubTator,
        score: hit.score,
        is_retracted: None,
        abstract_snippet: None,
        ranking: None,
        matched_sources: vec![ArticleSource::PubTator],
        normalized_title: normalize_article_search_text(hit.title.as_deref().unwrap_or_default()),
        normalized_abstract: String::new(),
        publication_type: None,
        source_local_position: 0,
    })
}

fn parse_sortpubdate(value: &str) -> Option<String> {
    let prefix = value.trim().get(0..10)?;
    let bytes = prefix.as_bytes();
    if bytes[4] != b'/'
        || bytes[7] != b'/'
        || !bytes[0..4].iter().all(|byte| byte.is_ascii_digit())
        || !bytes[5..7].iter().all(|byte| byte.is_ascii_digit())
        || !bytes[8..10].iter().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    Some(prefix.replace('/', "-"))
}

fn pubmed_month_number(value: &str) -> Option<&'static str> {
    match value
        .trim_matches(|ch: char| !ch.is_ascii_alphabetic())
        .to_ascii_lowercase()
        .as_str()
    {
        "jan" | "january" => Some("01"),
        "feb" | "february" => Some("02"),
        "mar" | "march" => Some("03"),
        "apr" | "april" => Some("04"),
        "may" => Some("05"),
        "jun" | "june" => Some("06"),
        "jul" | "july" => Some("07"),
        "aug" | "august" => Some("08"),
        "sep" | "sept" | "september" => Some("09"),
        "oct" | "october" => Some("10"),
        "nov" | "november" => Some("11"),
        "dec" | "december" => Some("12"),
        _ => None,
    }
}

fn parse_pubdate(value: &str) -> Option<String> {
    let mut parts = value.split_whitespace();
    let year = parts.next()?.trim();
    if year.len() != 4 || !year.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }

    let Some(month) = parts.next().and_then(pubmed_month_number) else {
        return Some(year.to_string());
    };
    let Some(day_token) = parts.next() else {
        return Some(format!("{year}-{month}"));
    };
    let day = day_token
        .trim_matches(|ch: char| !ch.is_ascii_digit())
        .parse::<u8>()
        .ok()
        .filter(|day| (1..=31).contains(day));
    match day {
        Some(day) => Some(format!("{year}-{month}-{day:02}")),
        None => Some(format!("{year}-{month}")),
    }
}

pub fn from_pubmed_esummary_entry(entry: &ESummaryEntry) -> Option<ArticleSearchResult> {
    let title = clean_title(&entry.title);
    if title.is_empty() {
        return None;
    }

    let journal = entry
        .fulljournalname
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .or_else(|| {
            entry
                .source
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.to_string())
        });
    let date = entry
        .sortpubdate
        .as_deref()
        .and_then(parse_sortpubdate)
        .or_else(|| entry.pubdate.as_deref().and_then(parse_pubdate));

    Some(ArticleSearchResult {
        pmid: entry.uid.clone(),
        pmcid: None,
        doi: None,
        title: title.clone(),
        journal,
        date,
        citation_count: None,
        influential_citation_count: None,
        source: ArticleSource::PubMed,
        matched_sources: vec![ArticleSource::PubMed],
        score: None,
        is_retracted: None,
        abstract_snippet: None,
        ranking: None,
        normalized_title: normalize_article_search_text(&title),
        normalized_abstract: String::new(),
        publication_type: None,
        source_local_position: 0,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnnotationKind {
    Gene,
    Disease,
    Chemical,
    Mutation,
}

fn annotation_kind(kind: &str) -> Option<AnnotationKind> {
    let k = kind.trim().to_ascii_lowercase();
    if k.is_empty() {
        return None;
    }
    if k.contains("gene") {
        return Some(AnnotationKind::Gene);
    }
    if k.contains("disease") {
        return Some(AnnotationKind::Disease);
    }
    if k.contains("chemical") || k.contains("drug") {
        return Some(AnnotationKind::Chemical);
    }
    if k.contains("mutation") || k.contains("variant") {
        return Some(AnnotationKind::Mutation);
    }
    None
}

fn push_annotation_count(
    map: &mut HashMap<String, (String, u32, usize)>,
    text: &str,
    order: usize,
) {
    let t = text.trim();
    if t.is_empty() || t.len() > 128 {
        return;
    }
    let key = t.to_ascii_lowercase();
    let entry = map.entry(key).or_insert_with(|| (t.to_string(), 0, order));
    entry.1 += 1;
}

fn finalize_counts(map: HashMap<String, (String, u32, usize)>) -> Vec<AnnotationCount> {
    let mut out = map
        .into_values()
        .map(|(text, count, first_seen_order)| (AnnotationCount { text, count }, first_seen_order))
        .collect::<Vec<_>>();
    out.sort_by(|(a, a_order), (b, b_order)| {
        b.count.cmp(&a.count).then_with(|| a_order.cmp(b_order))
    });
    out.truncate(8);
    out.into_iter().map(|(row, _)| row).collect()
}

pub fn extract_annotations(doc: &PubTatorDocument) -> Option<ArticleAnnotations> {
    let mut genes: HashMap<String, (String, u32, usize)> = HashMap::new();
    let mut diseases: HashMap<String, (String, u32, usize)> = HashMap::new();
    let mut chemicals: HashMap<String, (String, u32, usize)> = HashMap::new();
    let mut mutations: HashMap<String, (String, u32, usize)> = HashMap::new();
    let mut next_order = 0usize;

    for passage in &doc.passages {
        for ann in &passage.annotations {
            let Some(text) = ann.text.as_deref() else {
                continue;
            };
            let Some(kind) = ann
                .infons
                .as_ref()
                .and_then(|i| i.kind.as_deref())
                .and_then(annotation_kind)
            else {
                continue;
            };

            match kind {
                AnnotationKind::Gene => push_annotation_count(&mut genes, text, next_order),
                AnnotationKind::Disease => push_annotation_count(&mut diseases, text, next_order),
                AnnotationKind::Chemical => push_annotation_count(&mut chemicals, text, next_order),
                AnnotationKind::Mutation => push_annotation_count(&mut mutations, text, next_order),
            }
            next_order += 1;
        }
    }

    let annotations = ArticleAnnotations {
        genes: finalize_counts(genes),
        diseases: finalize_counts(diseases),
        chemicals: finalize_counts(chemicals),
        mutations: finalize_counts(mutations),
    };

    if annotations.genes.is_empty()
        && annotations.diseases.is_empty()
        && annotations.chemicals.is_empty()
        && annotations.mutations.is_empty()
    {
        None
    } else {
        Some(annotations)
    }
}

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

fn render_references(root: Node<'_, '_>) -> Option<String> {
    let references = reference_nodes(root);
    if references.is_empty() {
        return None;
    }

    let items = references
        .into_iter()
        .enumerate()
        .map(|(index, ref_node)| render_reference(ref_node, index + 1))
        .collect::<Vec<_>>();

    Some(format!("## References\n\n{}", items.join("\n")))
}

fn reference_nodes<'a, 'input>(root: Node<'a, 'input>) -> Vec<Node<'a, 'input>> {
    root.descendants()
        .filter(|node| node.is_element() && node.has_tag_name("ref"))
        .filter(|node| {
            node.ancestors()
                .any(|ancestor| ancestor.is_element() && ancestor.has_tag_name("ref-list"))
        })
        .collect()
}

fn render_reference(ref_node: Node<'_, '_>, ordinal: usize) -> String {
    let mut citation = if let Some(mixed_citation) = find_child(ref_node, "mixed-citation") {
        render_mixed_citation(mixed_citation)
    } else if let Some(element_citation) = find_child(ref_node, "element-citation") {
        render_element_citation(element_citation)
    } else {
        fallback_reference_text(ref_node)
    };

    if let Some(label) = find_child(ref_node, "label")
        .map(reference_inline_text)
        .filter(|value| !value.is_empty())
        .filter(|value| value.parse::<usize>().is_err())
    {
        let marker = if label.starts_with('[') && label.ends_with(']') {
            label
        } else {
            format!("[{label}]")
        };
        citation = if citation.is_empty() {
            marker
        } else {
            format!("{marker} {citation}")
        };
    }

    if citation.is_empty() {
        format!("{ordinal}.")
    } else {
        format!("{ordinal}. {citation}")
    }
}

fn render_mixed_citation(node: Node<'_, '_>) -> String {
    reference_inline_text(node)
}

fn render_element_citation(node: Node<'_, '_>) -> String {
    let mut parts = Vec::new();

    if let Some(authors) = render_author_segment(node) {
        parts.push(authors);
    }

    if let Some(title) = find_child(node, "article-title")
        .or_else(|| find_child(node, "chapter-title"))
        .map(reference_inline_text)
        .filter(|value| !value.is_empty())
    {
        parts.push(title);
    }

    if let Some(source) = find_child(node, "source")
        .map(reference_inline_text)
        .filter(|value| !value.is_empty())
    {
        parts.push(source);
    }

    if let Some(bibliographic) = render_bibliographic_segment(node) {
        parts.push(bibliographic);
    }

    parts.extend(render_identifier_segment(node));
    parts.extend(render_extra_reference_links(node));

    let rendered = join_reference_parts(parts);
    if rendered.is_empty() {
        reference_inline_text(node)
    } else {
        rendered
    }
}

fn render_author_segment(node: Node<'_, '_>) -> Option<String> {
    let person_group = node
        .children()
        .find(|child| {
            child.is_element()
                && child.has_tag_name("person-group")
                && child
                    .attribute("person-group-type")
                    .is_some_and(|value| value.eq_ignore_ascii_case("author"))
        })
        .or_else(|| {
            node.children()
                .find(|child| child.is_element() && child.has_tag_name("person-group"))
        });

    if let Some(person_group) = person_group {
        let mut authors = Vec::new();
        for child in person_group.children().filter(|child| child.is_element()) {
            match child.tag_name().name() {
                "name" => {
                    let name = render_person_name(child);
                    if !name.is_empty() {
                        authors.push(name);
                    }
                }
                "string-name" | "collab" => {
                    let text = reference_inline_text(child);
                    if !text.is_empty() {
                        authors.push(text);
                    }
                }
                "etal" => authors.push("et al.".to_string()),
                _ => {}
            }
        }
        if !authors.is_empty() {
            return Some(authors.join(", "));
        }
    }

    let mut authors = Vec::new();
    for child in node.children().filter(|child| child.is_element()) {
        match child.tag_name().name() {
            "name" => {
                let name = render_person_name(child);
                if !name.is_empty() {
                    authors.push(name);
                }
            }
            "string-name" | "collab" => {
                let text = reference_inline_text(child);
                if !text.is_empty() {
                    authors.push(text);
                }
            }
            "etal" => authors.push("et al.".to_string()),
            _ => {}
        }
    }

    if authors.is_empty() {
        None
    } else {
        Some(authors.join(", "))
    }
}

fn render_person_name(node: Node<'_, '_>) -> String {
    let surname = find_child(node, "surname")
        .map(reference_inline_text)
        .filter(|value| !value.is_empty());
    let given_names = find_child(node, "given-names")
        .map(reference_inline_text)
        .filter(|value| !value.is_empty());

    match (surname, given_names) {
        (Some(surname), Some(given_names)) => format!("{surname} {given_names}"),
        (Some(surname), None) => surname,
        (None, Some(given_names)) => given_names,
        (None, None) => reference_inline_text(node),
    }
}

fn render_bibliographic_segment(node: Node<'_, '_>) -> Option<String> {
    let year = find_child(node, "year")
        .map(reference_inline_text)
        .filter(|value| !value.is_empty());
    let volume = find_child(node, "volume")
        .map(reference_inline_text)
        .filter(|value| !value.is_empty());
    let issue = find_child(node, "issue")
        .map(reference_inline_text)
        .filter(|value| !value.is_empty());
    let comment = find_child(node, "comment")
        .map(reference_inline_text)
        .filter(|value| !value.is_empty());

    let pages = match (
        find_child(node, "fpage")
            .map(reference_inline_text)
            .filter(|value| !value.is_empty()),
        find_child(node, "lpage")
            .map(reference_inline_text)
            .filter(|value| !value.is_empty()),
    ) {
        (Some(fpage), Some(lpage)) if fpage == lpage => Some(fpage),
        (Some(fpage), Some(lpage)) => Some(format!("{fpage}-{lpage}")),
        (Some(fpage), None) => Some(fpage),
        (None, Some(lpage)) => Some(lpage),
        (None, None) => find_child(node, "elocation-id")
            .map(reference_inline_text)
            .filter(|value| !value.is_empty()),
    };

    let mut out = String::new();
    if let Some(year) = year {
        out.push_str(&year);
    }
    match (volume, issue, pages) {
        (Some(volume), issue, pages) => {
            if !out.is_empty() {
                out.push(';');
            }
            out.push_str(&volume);
            if let Some(issue) = issue {
                out.push('(');
                out.push_str(&issue);
                out.push(')');
            }
            if let Some(pages) = pages {
                out.push(':');
                out.push_str(&pages);
            }
        }
        (None, Some(issue), pages) => {
            if !out.is_empty() {
                out.push(';');
            }
            out.push('(');
            out.push_str(&issue);
            out.push(')');
            if let Some(pages) = pages {
                out.push(':');
                out.push_str(&pages);
            }
        }
        (None, None, Some(pages)) => {
            if !out.is_empty() {
                out.push(':');
            }
            out.push_str(&pages);
        }
        (None, None, None) => {}
    }

    if let Some(comment) = comment {
        if !out.is_empty() {
            out.push_str(". ");
        }
        out.push_str(&comment);
    }

    if out.is_empty() { None } else { Some(out) }
}

fn render_identifier_segment(node: Node<'_, '_>) -> Vec<String> {
    let mut identifiers = Vec::new();

    for child in node
        .children()
        .filter(|child| child.is_element() && child.has_tag_name("pub-id"))
    {
        let mut text = String::new();
        for grandchild in child.children() {
            append_reference_inline_node(grandchild, &mut text);
        }
        let text = collapse_whitespace(&text);
        if text.is_empty() {
            continue;
        }

        match child.attribute("pub-id-type") {
            Some(value) if value.eq_ignore_ascii_case("doi") => {
                let doi = normalize_doi(&text).unwrap_or(text);
                identifiers.push(format_doi_link(&doi));
            }
            Some(value) if value.eq_ignore_ascii_case("pmid") => {
                identifiers.push(format!("PMID: {text}"));
            }
            Some(value) if value.eq_ignore_ascii_case("pmcid") => {
                identifiers.push(format!("PMCID: {text}"));
            }
            _ => {}
        }
    }

    identifiers
}

fn render_extra_reference_links(node: Node<'_, '_>) -> Vec<String> {
    const XLINK_NS: &str = "http://www.w3.org/1999/xlink";

    let mut links = Vec::new();

    for child in node
        .descendants()
        .filter(|child| child.is_element() && child.has_tag_name("ext-link"))
    {
        let text = reference_inline_text(child);
        let url = child
            .attribute((XLINK_NS, "href"))
            .or_else(|| child.attribute("href"))
            .map(str::trim)
            .filter(|value| !value.is_empty());

        if child
            .attribute("ext-link-type")
            .is_some_and(|value| value.eq_ignore_ascii_case("doi"))
            || url.and_then(normalize_doi).is_some()
        {
            continue;
        }

        let rendered = match (text.is_empty(), url) {
            (false, Some(url)) => format!("[{text}]({url})"),
            (false, None) => text,
            (true, Some(url)) => url.to_string(),
            (true, None) => String::new(),
        };

        if !rendered.is_empty() && !links.contains(&rendered) {
            links.push(rendered);
        }
    }

    links
}

fn fallback_reference_text(ref_node: Node<'_, '_>) -> String {
    reference_inline_text(ref_node)
}

fn reference_inline_text(node: Node<'_, '_>) -> String {
    let mut out = String::new();
    match node.node_type() {
        NodeType::Text => out.push_str(node.text().unwrap_or_default()),
        _ => {
            for child in node.children() {
                append_reference_inline_node(child, &mut out);
            }
        }
    }
    collapse_whitespace(&out)
}

fn append_reference_inline_node(node: Node<'_, '_>, out: &mut String) {
    match node.node_type() {
        NodeType::Root => {
            for child in node.children() {
                append_reference_inline_node(child, out);
            }
        }
        NodeType::Element => {
            match node.tag_name().name() {
                "label" => return,
                "italic" => return append_wrapped_reference_inline(node, "*", out),
                "bold" => return append_wrapped_reference_inline(node, "**", out),
                "sup" => return append_wrapped_reference_inline(node, "^", out),
                "sub" => return append_wrapped_reference_inline(node, "~", out),
                "ext-link" => return append_reference_ext_link(node, out),
                "pub-id" => return append_reference_pub_id(node, out),
                _ => {}
            }

            for child in node.children() {
                append_reference_inline_node(child, out);
            }
        }
        NodeType::Text => out.push_str(node.text().unwrap_or_default()),
        _ => {}
    }
}

fn append_wrapped_reference_inline(node: Node<'_, '_>, marker: &str, out: &mut String) {
    let text = reference_inline_text(node);
    if text.is_empty() {
        return;
    }
    out.push_str(marker);
    out.push_str(&text);
    out.push_str(marker);
}

fn append_reference_ext_link(node: Node<'_, '_>, out: &mut String) {
    const XLINK_NS: &str = "http://www.w3.org/1999/xlink";

    let text = reference_inline_text(node);
    let url = node
        .attribute((XLINK_NS, "href"))
        .or_else(|| node.attribute("href"))
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if node
        .attribute("ext-link-type")
        .is_some_and(|value| value.eq_ignore_ascii_case("doi"))
        && let Some(doi) = url.and_then(normalize_doi).or_else(|| normalize_doi(&text))
    {
        out.push_str(&format_doi_link(&doi));
        return;
    }

    if let Some(doi) = url.and_then(normalize_doi) {
        out.push_str(&format_doi_link(&doi));
        return;
    }

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

fn append_reference_pub_id(node: Node<'_, '_>, out: &mut String) {
    let mut text = String::new();
    for child in node.children() {
        append_reference_inline_node(child, &mut text);
    }
    let text = collapse_whitespace(&text);
    if text.is_empty() {
        return;
    }

    match node.attribute("pub-id-type") {
        Some(value) if value.eq_ignore_ascii_case("doi") => {
            let doi = normalize_doi(&text).unwrap_or(text);
            out.push_str(&format_doi_link(&doi));
        }
        Some(value) if value.eq_ignore_ascii_case("pmid") => {
            out.push_str("PMID: ");
            out.push_str(&text);
        }
        Some(value) if value.eq_ignore_ascii_case("pmcid") => {
            out.push_str("PMCID: ");
            out.push_str(&text);
        }
        _ => out.push_str(&text),
    }
}

fn normalize_doi(value: &str) -> Option<String> {
    let trimmed = collapse_whitespace(value);
    if trimmed.is_empty() {
        return None;
    }

    let lower = trimmed.to_ascii_lowercase();
    for prefix in [
        "https://doi.org/",
        "http://doi.org/",
        "https://dx.doi.org/",
        "http://dx.doi.org/",
        "doi:",
    ] {
        if lower.starts_with(prefix) {
            return normalize_doi(&trimmed[prefix.len()..]);
        }
    }

    if lower.starts_with("10.") && trimmed.contains('/') {
        Some(trimmed)
    } else {
        None
    }
}

fn format_doi_link(doi: &str) -> String {
    format!("[{doi}](https://doi.org/{doi})")
}

fn join_reference_parts(parts: Vec<String>) -> String {
    let mut out = String::new();

    for part in parts.into_iter().filter(|part| !part.is_empty()) {
        if out.is_empty() {
            out.push_str(&part);
            continue;
        }

        if out.ends_with('.') || out.ends_with('?') || out.ends_with('!') {
            out.push(' ');
        } else {
            out.push_str(". ");
        }
        out.push_str(&part);
    }

    out
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

fn collapse_whitespace(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut last_was_space = false;

    for ch in value.chars() {
        if ch.is_whitespace() {
            if !last_was_space {
                out.push(' ');
                last_was_space = true;
            }
        } else {
            out.push(ch);
            last_was_space = false;
        }
    }

    out.trim().to_string()
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
mod tests {
    use super::*;
    use crate::sources::pubmed::ESummaryEntry;

    #[test]
    fn truncate_title_truncates_on_utf8_boundary() {
        let title = "€".repeat(100);
        let out = truncate_title(&title);
        assert!(out.ends_with('…'));
        assert!(out.len() <= 63);
    }

    #[test]
    fn truncate_title_strips_inline_html_and_entities() {
        let title = "KRAS&lt;sup&gt;G12C&lt;/sup&gt; and <i>melanoma</i>";
        let out = truncate_title(title);
        assert!(out.contains("KRAS"));
        assert!(!out.contains("&lt;"));
        assert!(!out.contains("<i>"));
    }

    #[test]
    fn normalize_article_search_text_compacts_compound_hyphens() {
        assert_eq!(normalize_article_search_text("LB-100"), "lb100");
        assert_eq!(normalize_article_search_text("LB100"), "lb100");
        assert_eq!(normalize_article_search_text("IL-2"), "il2");
        assert_eq!(
            normalize_article_search_text("meta-analysis"),
            "meta-analysis"
        );
    }

    #[test]
    fn truncate_abstract_keeps_full_text_until_limit() {
        let text = "Sentence one. Sentence two. Sentence three.";
        let out = truncate_abstract(text);
        assert_eq!(out, text);
    }

    #[test]
    fn truncate_authors_first_last() {
        let authors = vec![
            "A".to_string(),
            "B".to_string(),
            "C".to_string(),
            "D".to_string(),
            "E".to_string(),
        ];
        assert_eq!(truncate_authors(&authors), vec!["A", "E"]);
    }

    #[test]
    fn extract_annotations_counts_mentions() {
        let doc: PubTatorDocument = serde_json::from_value(serde_json::json!({
            "pmid": 123,
            "pmcid": "PMC1",
            "date": "2026-02-05",
            "journal": "Test",
            "authors": ["A"],
            "passages": [
                {
                    "infons": {"type": "title"},
                    "text": "BRAF V600E in melanoma",
                    "annotations": [
                        {"text": "BRAF", "infons": {"type": "Gene"}},
                        {"text": "V600E", "infons": {"type": "Mutation"}},
                        {"text": "melanoma", "infons": {"type": "Disease"}}
                    ]
                },
                {
                    "infons": {"type": "abstract"},
                    "text": "Vemurafenib targets BRAF V600E",
                    "annotations": [
                        {"text": "BRAF", "infons": {"type": "Gene"}},
                        {"text": "TP53", "infons": {"type": "Gene"}},
                        {"text": "V600E", "infons": {"type": "Mutation"}},
                        {"text": "vemurafenib", "infons": {"type": "Chemical"}}
                    ]
                }
            ]
        }))
        .expect("valid JSON");

        let ann = extract_annotations(&doc).expect("annotations should exist");
        assert_eq!(
            ann.genes,
            vec![
                AnnotationCount {
                    text: "BRAF".into(),
                    count: 2
                },
                AnnotationCount {
                    text: "TP53".into(),
                    count: 1
                }
            ]
        );
        assert_eq!(
            ann.mutations,
            vec![AnnotationCount {
                text: "V600E".into(),
                count: 2
            }]
        );
        assert_eq!(
            ann.diseases,
            vec![AnnotationCount {
                text: "melanoma".into(),
                count: 1
            }]
        );
        assert_eq!(
            ann.chemicals,
            vec![AnnotationCount {
                text: "vemurafenib".into(),
                count: 1
            }]
        );
    }

    #[test]
    fn extract_annotations_preserves_first_seen_order_for_equal_counts() {
        let doc: PubTatorDocument = serde_json::from_value(serde_json::json!({
            "pmid": 22663011,
            "passages": [
                {
                    "infons": {"type": "title"},
                    "text": "Example title",
                    "annotations": [
                        {"text": "TP53", "infons": {"type": "Gene"}},
                        {"text": "BRAF", "infons": {"type": "Gene"}},
                        {"text": "TP53", "infons": {"type": "Gene"}},
                        {"text": "BRAF", "infons": {"type": "Gene"}}
                    ]
                }
            ]
        }))
        .expect("valid JSON");

        let ann = extract_annotations(&doc).expect("annotations should exist");
        assert_eq!(
            ann.genes,
            vec![
                AnnotationCount {
                    text: "TP53".into(),
                    count: 2
                },
                AnnotationCount {
                    text: "BRAF".into(),
                    count: 2
                }
            ]
        );
    }

    #[test]
    fn article_sections_maps_egfr_review() {
        let hit: EuropePmcResult = serde_json::from_value(serde_json::json!({
            "id": "39876543",
            "pmid": "39876543",
            "title": "EGFR &lt;i&gt;targeted&lt;/i&gt; therapy in NSCLC",
            "journalTitle": "Cancer Reviews",
            "firstPublicationDate": "2025-03-01",
            "authorString": "A. One, B. Two, C. Three",
            "citedByCount": "24",
            "pubType": "Review Article",
            "isOpenAccess": "Y",
            "abstractText": "EGFR inhibition improves progression-free survival in selected cohorts."
        }))
        .expect("valid Europe PMC hit");

        let article = from_europepmc_result(&hit);
        assert_eq!(article.pmid.as_deref(), Some("39876543"));
        assert!(article.title.contains("EGFR targeted therapy"));
        assert_eq!(article.publication_type.as_deref(), Some("Review"));
        assert_eq!(article.open_access, Some(true));
        assert!(
            article
                .abstract_text
                .as_deref()
                .is_some_and(|text| text.contains("EGFR inhibition"))
        );
        assert!(!article.pubtator_fallback);
    }

    #[test]
    fn article_sections_maps_brca1_study() {
        let doc: PubTatorDocument = serde_json::from_value(serde_json::json!({
            "pmid": 22663011,
            "pmcid": "PMC1234567",
            "date": "2024-09-20",
            "journal": "J Clin Oncol",
            "authors": ["Author A", "Author B", "Author C", "Author D", "Author E"],
            "passages": [
                {"infons": {"type": "title"}, "text": "BRCA1 pathogenic variants in breast cancer"},
                {"infons": {"type": "abstract"}, "text": "Study of BRCA1 germline alterations and PARP response."}
            ]
        }))
        .expect("valid PubTator document");

        let article = from_pubtator_document(&doc);
        assert_eq!(article.pmid.as_deref(), Some("22663011"));
        assert_eq!(article.pmcid.as_deref(), Some("PMC1234567"));
        assert!(article.title.contains("BRCA1"));
        assert_eq!(article.authors, vec!["Author A", "Author E"]);
        assert!(!article.pubtator_fallback);
    }

    #[test]
    fn publication_type_detection_reads_pub_type_list_for_retractions() {
        let hit: EuropePmcResult = serde_json::from_value(serde_json::json!({
            "id": "1",
            "pmid": "1",
            "title": "Retracted paper",
            "pubTypeList": {
                "pubType": ["Journal Article", "Retracted Publication"]
            }
        }))
        .expect("valid Europe PMC hit");

        let row = from_europepmc_search_result(&hit).expect("search row should map");
        assert_eq!(row.is_retracted, Some(true));
    }

    #[test]
    fn from_pubtator_search_result_maps_source_and_score() {
        let hit: PubTatorSearchResult = serde_json::from_value(serde_json::json!({
            "_id": "22663011",
            "pmid": 22663011,
            "title": "BRAF in melanoma",
            "journal": "J Clin Oncol",
            "date": "2024-01-20T00:00:00Z",
            "score": 255.9
        }))
        .expect("valid pubtator search row");

        let row = from_pubtator_search_result(&hit).expect("row should map");
        assert_eq!(row.pmid, "22663011");
        assert_eq!(row.source, ArticleSource::PubTator);
        assert_eq!(row.score, Some(255.9));
        assert_eq!(row.citation_count, None);
        assert_eq!(row.is_retracted, None);
    }

    #[test]
    fn parse_sortpubdate_extracts_ymd() {
        assert_eq!(
            parse_sortpubdate("2023/01/15 00:00"),
            Some("2023-01-15".to_string())
        );
    }

    #[test]
    fn parse_pubdate_extracts_full_date() {
        assert_eq!(parse_pubdate("2023 Jan 15"), Some("2023-01-15".to_string()));
    }

    #[test]
    fn parse_pubdate_extracts_year_month() {
        assert_eq!(parse_pubdate("2023 Jan"), Some("2023-01".to_string()));
    }

    #[test]
    fn parse_pubdate_extracts_year() {
        assert_eq!(parse_pubdate("2023 Spring"), Some("2023".to_string()));
    }

    #[test]
    fn from_pubmed_esummary_entry_hydrates_all_fields() {
        let row = from_pubmed_esummary_entry(&ESummaryEntry {
            uid: "12345".into(),
            title: "BRAF &lt;i&gt;targeted&lt;/i&gt; therapy".into(),
            sortpubdate: Some("2023/01/15 00:00".into()),
            pubdate: Some("2023 Jan 15".into()),
            fulljournalname: Some("Nature Medicine".into()),
            source: Some("Nat Med".into()),
        })
        .expect("pubmed row should map");

        assert_eq!(row.pmid, "12345");
        assert_eq!(row.title, "BRAF targeted therapy");
        assert_eq!(row.date.as_deref(), Some("2023-01-15"));
        assert_eq!(row.journal.as_deref(), Some("Nature Medicine"));
        assert_eq!(row.source, ArticleSource::PubMed);
        assert_eq!(row.matched_sources, vec![ArticleSource::PubMed]);
        assert_eq!(row.normalized_title, "braf targeted therapy");
        assert_eq!(row.normalized_abstract, "");
        assert_eq!(row.publication_type, None);
    }

    #[test]
    fn from_pubmed_esummary_entry_falls_back_to_source_journal() {
        let row = from_pubmed_esummary_entry(&ESummaryEntry {
            uid: "12345".into(),
            title: "PubMed fallback title".into(),
            sortpubdate: None,
            pubdate: Some("2023 Jan".into()),
            fulljournalname: Some("   ".into()),
            source: Some("Nat Med".into()),
        })
        .expect("pubmed row should map");

        assert_eq!(row.date.as_deref(), Some("2023-01"));
        assert_eq!(row.journal.as_deref(), Some("Nat Med"));
    }

    #[test]
    fn from_pubmed_esummary_entry_returns_none_for_blank_title() {
        let row = from_pubmed_esummary_entry(&ESummaryEntry {
            uid: "12345".into(),
            title: "   ".into(),
            sortpubdate: Some("2023/01/15 00:00".into()),
            pubdate: Some("2023 Jan 15".into()),
            fulljournalname: Some("Nature Medicine".into()),
            source: Some("Nat Med".into()),
        });

        assert!(row.is_none());
    }

    #[test]
    fn extract_text_from_jats_preserves_structure_and_renders_references() {
        let xml = r#"<!DOCTYPE article PUBLIC "-//NLM//DTD JATS (Z39.96) Journal Archiving and Interchange DTD v1.4 20241031//EN" "JATS-archivearticle1.dtd">
<article xmlns:xlink="http://www.w3.org/1999/xlink">
  <front>
    <journal-meta>
      <journal-title-group>
        <journal-title>Noise Journal</journal-title>
      </journal-title-group>
      <issn>1234-5678</issn>
    </journal-meta>
    <article-meta>
      <title-group>
        <article-title>Precision oncology in melanoma</article-title>
      </title-group>
      <permissions>
        <license><license-p>Creative Commons text that should not leak.</license-p></license>
      </permissions>
      <abstract>
        <p>Abstract text with <xref ref-type="bibr" rid="ref1">1</xref> and <italic>signal</italic>.</p>
      </abstract>
    </article-meta>
  </front>
  <body>
    <sec>
      <title>Introduction</title>
      <p>Body paragraph with <bold>important</bold> findings and <ext-link xlink:href="https://example.org/resource">external evidence</ext-link>.</p>
      <fig id="f1">
        <label>Figure 1</label>
        <caption>
          <title>Response overview</title>
          <p>Treatment response summary.</p>
        </caption>
      </fig>
      <table-wrap id="t1">
        <label>Table 1</label>
        <caption><title>Patient characteristics</title></caption>
        <table>
          <thead>
            <tr><th>Gene</th><th>Count</th></tr>
          </thead>
          <tbody>
            <tr><td>BRAF</td><td>12</td></tr>
            <tr><td>NRAS</td><td>4</td></tr>
          </tbody>
        </table>
      </table-wrap>
      <sec>
        <title>Methods</title>
        <list list-type="order">
          <list-item><p>Collect tumor samples</p></list-item>
          <list-item><p>Sequence genes</p></list-item>
        </list>
      </sec>
    </sec>
  </body>
  <back>
    <ref-list>
      <ref id="ref1"><label>1</label><mixed-citation>Reference one.</mixed-citation></ref>
      <ref id="ref2"><label>2</label><mixed-citation>Reference two.</mixed-citation></ref>
    </ref-list>
  </back>
</article>
"#;

        let out = extract_text_from_xml(xml);
        assert!(out.contains("# Precision oncology in melanoma"));
        assert!(out.contains("## Abstract"));
        assert!(out.contains("## Introduction"));
        assert!(out.contains("### Methods"));
        assert!(out.contains("Abstract text with [1] and *signal*."));
        assert!(out.contains("Body paragraph with **important** findings"));
        assert!(out.contains("[external evidence](https://example.org/resource)"));
        assert!(out.contains("> **Figure 1.** Response overview Treatment response summary."));
        assert!(out.contains("| Gene | Count |"));
        assert!(out.contains("| BRAF | 12 |"));
        assert!(out.contains("1. Collect tumor samples"));
        assert!(out.contains("## References"));
        assert!(out.contains("1. Reference one."));
        assert!(out.contains("2. Reference two."));
        assert!(!out.contains("references cited."));
        assert!(!out.contains("Noise Journal"));
        assert!(!out.contains("Creative Commons text that should not leak."));
    }

    #[test]
    fn extract_text_from_jats_renders_element_citation_fields_and_ids() {
        let xml = r#"
<article xmlns:xlink="http://www.w3.org/1999/xlink">
  <front>
    <article-meta>
      <title-group><article-title>Element citation article</article-title></title-group>
    </article-meta>
  </front>
  <back>
    <ref-list>
      <ref id="ref1">
        <element-citation publication-type="journal">
          <person-group person-group-type="author">
            <name><surname>Doe</surname><given-names>JA</given-names></name>
            <name><surname>Roe</surname><given-names>R</given-names></name>
            <etal/>
          </person-group>
          <article-title>Structured reference title</article-title>
          <source>Journal of Tests</source>
          <year>2024</year>
          <volume>12</volume>
          <issue>3</issue>
          <elocation-id>e45</elocation-id>
          <comment>Online ahead of print</comment>
          <pub-id pub-id-type="doi">10.1000/test-doi</pub-id>
          <pub-id pub-id-type="pmid">123456</pub-id>
          <pub-id pub-id-type="pmcid">PMC123456</pub-id>
          <ext-link ext-link-type="uri" xlink:href="https://example.org/dataset">Dataset</ext-link>
        </element-citation>
      </ref>
    </ref-list>
  </back>
</article>
"#;

        let out = extract_text_from_xml(xml);
        assert!(out.contains(
            "1. Doe JA, Roe R, et al. Structured reference title. Journal of Tests. 2024;12(3):e45. Online ahead of print. [10.1000/test-doi](https://doi.org/10.1000/test-doi). PMID: 123456. PMCID: PMC123456. [Dataset](https://example.org/dataset)"
        ));
    }

    #[test]
    fn extract_text_from_jats_renders_mixed_citation_doi_links() {
        let xml = r#"
<article xmlns:xlink="http://www.w3.org/1999/xlink">
  <front>
    <article-meta>
      <title-group><article-title>Mixed citation article</article-title></title-group>
    </article-meta>
  </front>
  <back>
    <ref-list>
      <ref id="ref1">
        <mixed-citation>Alpha study. <pub-id pub-id-type="doi">10.1000/alpha</pub-id></mixed-citation>
      </ref>
      <ref id="ref2">
        <mixed-citation>Beta study. <ext-link ext-link-type="doi" xlink:href="10.1000/beta">doi</ext-link></mixed-citation>
      </ref>
    </ref-list>
  </back>
</article>
"#;

        let out = extract_text_from_xml(xml);
        assert!(out.contains("1. Alpha study. [10.1000/alpha](https://doi.org/10.1000/alpha)"));
        assert!(out.contains("2. Beta study. [10.1000/beta](https://doi.org/10.1000/beta)"));
    }

    #[test]
    fn extract_text_from_jats_reference_fallback_omits_duplicate_label() {
        let xml = r#"
<article>
  <front>
    <article-meta>
      <title-group><article-title>Fallback citation article</article-title></title-group>
    </article-meta>
  </front>
  <back>
    <ref-list>
      <ref id="ref1">
        <label>S1</label>
        <note><p>Supplemental dataset companion</p></note>
      </ref>
    </ref-list>
  </back>
</article>
"#;

        let out = extract_text_from_xml(xml);
        assert!(out.contains("1. [S1] Supplemental dataset companion"));
        assert!(!out.contains("1. [S1] S1 Supplemental dataset companion"));
    }

    #[test]
    fn extract_text_from_jats_merges_multiple_ref_lists() {
        let xml = r#"
<article>
  <front>
    <article-meta>
      <title-group><article-title>Multiple ref-list article</article-title></title-group>
    </article-meta>
  </front>
  <back>
    <ref-list>
      <ref id="ref1"><mixed-citation>First reference.</mixed-citation></ref>
    </ref-list>
    <sec>
      <title>Supplement</title>
      <ref-list>
        <ref id="ref2"><mixed-citation>Second reference.</mixed-citation></ref>
      </ref-list>
    </sec>
  </back>
</article>
"#;

        let out = extract_text_from_xml(xml);
        let first = out.find("1. First reference.").expect("first ref present");
        let second = out
            .find("2. Second reference.")
            .expect("second ref present");
        assert!(first < second);
    }

    #[test]
    fn extract_text_from_jats_omits_irregular_tables() {
        let xml = r#"
<article>
  <front>
    <article-meta>
      <title-group><article-title>Irregular table article</article-title></title-group>
    </article-meta>
  </front>
  <body>
    <table-wrap>
      <label>Table 7</label>
      <caption><title>Irregular measurements</title></caption>
      <table>
        <tbody>
          <tr><th rowspan="2">Marker</th><th>Value</th></tr>
          <tr><td>42</td></tr>
        </tbody>
      </table>
    </table-wrap>
  </body>
</article>
"#;

        let out = extract_text_from_xml(xml);
        assert!(out.contains("Table 7"));
        assert!(out.contains("Irregular measurements"));
        assert!(!out.contains("| Marker | Value |"));
    }

    #[test]
    fn extract_text_from_xml_falls_back_for_non_jats_and_malformed_xml() {
        let non_jats = "<root><meta>ignored?</meta><p>Alpha</p><p>Beta</p></root>";
        let malformed = "<article><body><p>Broken";

        let non_jats_out = extract_text_from_xml(non_jats);
        let malformed_out = extract_text_from_xml(malformed);

        assert_eq!(non_jats_out, "ignored?AlphaBeta");
        assert_eq!(malformed_out, "Broken");
    }
}
