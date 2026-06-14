use super::planning::{
    RareDiseaseTrialRequest, TrialPlanningMode, TrialQueryField, plan_rare_disease_trials,
};

fn phelan_shank3_request() -> RareDiseaseTrialRequest {
    RareDiseaseTrialRequest {
        raw_query: Some("Phelan-McDermid syndrome SHANK3 22q13 trial".to_string()),
        condition: Some("Phelan-McDermid syndrome".to_string()),
        gene: Some("SHANK3".to_string()),
        sponsor: None,
        strict_condition: false,
        mode: TrialPlanningMode::Search,
    }
}

#[test]
fn ticket_414_rare_disease_trial_planning_phelan_shank3_expands_to_bounded_trial_terms() {
    let plan = plan_rare_disease_trials(phelan_shank3_request())
        .expect("rare-disease trial planning should be pure and deterministic");

    assert!(
        plan.primary_condition_labels
            .iter()
            .any(|label| label.label == "Phelan-McDermid syndrome"),
        "primary condition labels should preserve the typed disease"
    );
    assert!(
        plan.gene_labels
            .iter()
            .any(|label| label.symbol == "SHANK3"),
        "gene labels should preserve the typed gene"
    );
    assert!(
        plan.expanded_condition_labels.iter().any(|expansion| {
            expansion.label == "22q13 deletion syndrome"
                && !expansion.source.trim().is_empty()
                && !expansion.reason.trim().is_empty()
        }),
        "bounded 22q13 synonym should carry source provenance and reason"
    );
    assert!(
        plan.query_terms.iter().any(|term| {
            term.term == "Phelan-McDermid syndrome" && term.field == TrialQueryField::Condition
        }),
        "the execution plan should include a CTGov condition term"
    );
    assert!(
        plan.query_terms
            .iter()
            .any(|term| term.term == "SHANK3" && term.field == TrialQueryField::Biomarker),
        "the execution plan should include a SHANK3 biomarker term"
    );
}

#[test]
fn ticket_414_rare_disease_trial_planning_rejects_noisy_broad_terms() {
    let request = RareDiseaseTrialRequest {
        raw_query: Some("Phelan-McDermid SHANK3 autism SHANK2 SHANK1".to_string()),
        condition: Some("Phelan-McDermid syndrome".to_string()),
        gene: Some("SHANK3".to_string()),
        sponsor: None,
        strict_condition: false,
        mode: TrialPlanningMode::Search,
    };

    let plan = plan_rare_disease_trials(request)
        .expect("noisy rare-disease trial planning should remain deterministic");

    let accepted_terms: Vec<&str> = plan
        .query_terms
        .iter()
        .map(|term| term.term.as_str())
        .chain(
            plan.expanded_condition_labels
                .iter()
                .map(|expansion| expansion.label.as_str()),
        )
        .collect();
    assert!(
        !accepted_terms
            .iter()
            .any(|term: &&str| term.eq_ignore_ascii_case("autism")),
        "broad autism labels should not become accepted trial terms"
    );
    assert!(
        !accepted_terms
            .iter()
            .any(|term: &&str| term.eq_ignore_ascii_case("SHANK1")
                || term.eq_ignore_ascii_case("SHANK2")),
        "unrelated SHANK-family genes should not become accepted trial terms"
    );
    assert!(
        plan.warnings.iter().any(|warning| {
            warning.term.eq_ignore_ascii_case("autism")
                && warning.reason.to_ascii_lowercase().contains("broad")
        }),
        "broad rejected labels should be visible as planning warnings"
    );
    assert!(
        plan.warnings.iter().any(|warning| {
            warning.term.eq_ignore_ascii_case("SHANK2")
                && warning.reason.to_ascii_lowercase().contains("unrelated")
        }),
        "unrelated family terms should be visible as planning warnings"
    );
}

#[test]
fn ticket_414_rare_disease_trial_planning_strict_mode_keeps_literal_condition() {
    let request = RareDiseaseTrialRequest {
        strict_condition: true,
        ..phelan_shank3_request()
    };

    let plan = plan_rare_disease_trials(request)
        .expect("strict rare-disease trial planning should be pure and deterministic");

    assert!(
        plan.query_terms.iter().any(|term| {
            term.term == "Phelan-McDermid syndrome" && term.field == TrialQueryField::Condition
        }),
        "strict mode should keep the literal condition term"
    );
    assert!(
        plan.expanded_condition_labels.is_empty(),
        "strict mode should not add non-literal condition expansions"
    );
    assert!(
        plan.query_terms
            .iter()
            .any(|term| term.term == "SHANK3" && term.field == TrialQueryField::Biomarker),
        "strict condition mode should not discard separately typed gene intent"
    );
}
