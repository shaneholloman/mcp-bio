use super::*;

pub(super) fn article_annotation_command(
    bucket: ArticleAnnotationBucket,
    text: &str,
) -> Option<String> {
    let text = text.trim();
    let quoted = quote_arg(text);
    if quoted.is_empty() {
        return None;
    }

    Some(match bucket {
        ArticleAnnotationBucket::Gene => format!("biomcp search gene -q {quoted}"),
        ArticleAnnotationBucket::Disease => format!("biomcp search disease --query {quoted}"),
        ArticleAnnotationBucket::Chemical => format!("biomcp get drug {quoted}"),
        ArticleAnnotationBucket::Mutation => format!("biomcp get variant {quoted}"),
    })
}

pub(super) fn ranked_article_annotation_commands(
    title: &str,
    rows: &[AnnotationCount],
    bucket: ArticleAnnotationBucket,
    limit: usize,
) -> Vec<String> {
    if limit == 0 {
        return Vec::new();
    }

    let normalized_title = normalize_match_text(title);
    let mut ranked = rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| {
            let command = article_annotation_command(bucket, &row.text)?;
            let normalized_text = normalize_match_text(&row.text);
            let title_hit =
                !normalized_text.is_empty() && normalized_title.contains(normalized_text.as_str());
            Some((title_hit, row.count, index, command))
        })
        .collect::<Vec<_>>();

    ranked.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| b.1.cmp(&a.1))
            .then_with(|| a.2.cmp(&b.2))
    });

    ranked
        .into_iter()
        .take(limit)
        .map(|(_, _, _, command)| command)
        .collect()
}

pub(super) fn trial_results_search_command(trial: &Trial) -> Option<String> {
    let nct_id = trial.nct_id.trim();
    if nct_id.is_empty() {
        return None;
    }

    let title_seed = trial
        .title
        .split_whitespace()
        .take(6)
        .collect::<Vec<_>>()
        .join(" ");
    let seed = if title_seed.is_empty() {
        nct_id.to_string()
    } else {
        format!("{nct_id} {title_seed}")
    };
    let seed_q = format!("\"{}\"", seed.replace('\"', "\\\""));
    if seed_q.is_empty() {
        return None;
    }

    if let Some(intervention) = trial.interventions.first().map(String::as_str) {
        let intervention_q = quote_arg(intervention);
        if !intervention_q.is_empty() {
            return Some(format!(
                "biomcp search article --drug {intervention_q} -q {seed_q} --limit 5"
            ));
        }
    }

    Some(format!("biomcp search article -q {seed_q} --limit 5"))
}

pub(super) fn article_related_id(paper: &ArticleRelatedPaper) -> String {
    paper
        .pmid
        .as_deref()
        .or(paper.doi.as_deref())
        .or(paper.arxiv_id.as_deref())
        .or(paper.paper_id.as_deref())
        .map(markdown_cell)
        .unwrap_or_else(|| "-".to_string())
}

pub(super) fn article_related_label(paper: &ArticleRelatedPaper) -> String {
    paper
        .pmid
        .as_deref()
        .map(|pmid| format!("PMID {pmid}"))
        .or_else(|| paper.doi.as_deref().map(|doi| format!("DOI {doi}")))
        .or_else(|| {
            paper
                .arxiv_id
                .as_deref()
                .map(|arxiv| format!("arXiv {arxiv}"))
        })
        .or_else(|| {
            paper
                .paper_id
                .as_deref()
                .map(|paper_id| format!("paper {paper_id}"))
        })
        .unwrap_or_else(|| markdown_cell(&paper.title))
}

pub(super) fn related_article(article: &Article) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Some(pmid) = article
        .pmid
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        && article.annotations.is_some()
    {
        out.push(format!("biomcp article entities {pmid}"));
    }
    if let Some(ann) = article.annotations.as_ref() {
        out.extend(ranked_article_annotation_commands(
            &article.title,
            &ann.genes,
            ArticleAnnotationBucket::Gene,
            2,
        ));
        out.extend(ranked_article_annotation_commands(
            &article.title,
            &ann.diseases,
            ArticleAnnotationBucket::Disease,
            2,
        ));
        out.extend(ranked_article_annotation_commands(
            &article.title,
            &ann.chemicals,
            ArticleAnnotationBucket::Chemical,
            2,
        ));
    }
    if let Some(pmid) = article
        .pmid
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        out.push(format!("biomcp article references {pmid} --limit 3"));
        out.push(format!("biomcp article citations {pmid} --limit 3"));
        out.push(format!("biomcp article recommendations {pmid} --limit 3"));
    }
    dedupe_markdown_commands(out)
}
