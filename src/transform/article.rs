//! Transform adapters for article data from upstream API sources into CLI-facing entity models.

mod anchors;
mod annotations;
mod federation;
mod jats;

pub use self::anchors::{
    article_search_abstract_snippet, article_search_fallback_title, clean_abstract, clean_title,
    normalize_article_search_text,
};
#[allow(unused_imports)]
pub use self::anchors::{truncate_abstract, truncate_authors};
pub use self::annotations::extract_annotations;
pub use self::federation::{
    from_europepmc_result, from_europepmc_search_result, from_pubmed_esummary_entry,
    from_pubtator_document, from_pubtator_search_result, merge_europepmc_metadata,
};
pub use self::jats::extract_text_from_xml;

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
