pub(super) fn article_follow_up_workflow(
    article: &crate::entities::article::Article,
) -> Result<Option<crate::workflow_ladders::WorkflowMeta>, crate::error::BioMcpError> {
    let has_pmid = article
        .pmid
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let has_annotations = article.annotations.as_ref().is_some_and(|annotations| {
        !annotations.genes.is_empty()
            || !annotations.diseases.is_empty()
            || !annotations.chemicals.is_empty()
            || !annotations.mutations.is_empty()
    });

    (has_pmid && has_annotations)
        .then(|| {
            crate::workflow_ladders::meta_for(crate::workflow_ladders::Workflow::ArticleFollowUp)
        })
        .transpose()
}
