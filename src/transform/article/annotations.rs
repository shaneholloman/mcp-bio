//! PubTator annotation aggregation for article detail views.

use std::collections::HashMap;

use crate::entities::article::{AnnotationCount, ArticleAnnotations};
use crate::sources::pubtator::PubTatorDocument;

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

#[cfg(test)]
mod tests;
