use super::*;

#[test]
fn adverse_event_markdown_includes_openfda_sections() {
    let event = AdverseEvent {
        report_id: "10329882".to_string(),
        drug: "ivacaftor".to_string(),
        reactions: vec!["Cough".to_string()],
        outcomes: vec!["Hospitalization".to_string()],
        patient: None,
        concomitant_medications: vec!["azithromycin".to_string()],
        reporter_type: None,
        reporter_country: None,
        indication: None,
        serious: true,
        date: Some("2024-01-01".to_string()),
    };

    let markdown = adverse_event_markdown(&event, &["all".to_string()]).expect("faers");
    assert!(markdown.contains("Source: OpenFDA"));
    assert!(markdown.contains("## Reactions (OpenFDA)"));
    assert!(markdown.contains("## Outcomes (OpenFDA)"));
    assert!(markdown.contains("## Concomitant Drugs (OpenFDA)"));
}

#[test]
fn adverse_event_search_markdown_renders_summary_and_filters() {
    let summary = AdverseEventSearchSummary {
        total_reports: 12,
        returned_report_count: 1,
        top_reactions: vec![
            crate::entities::adverse_event::AdverseEventReactionSummary {
                reaction: "Cough".to_string(),
                count: 4,
                percentage: 33.3,
            },
        ],
    };
    let results = vec![AdverseEventSearchResult {
        report_id: "1001".to_string(),
        drug: "ivacaftor".to_string(),
        reactions: vec!["Cough".to_string()],
        serious: true,
    }];

    let markdown = adverse_event_search_markdown("ivacaftor", &results, &summary).expect("search");
    assert!(markdown.contains("# Adverse Events: ivacaftor"));
    assert!(markdown.contains("## Summary"));
    assert!(markdown.contains("| Cough | 4 | 33.3% |"));
    assert!(markdown.contains("Use `get adverse-event <report_id>` for details."));
}

#[test]
fn adverse_event_count_markdown_renders_bucket_rows() {
    let markdown = adverse_event_count_markdown(
        "drug=ivacaftor",
        "reaction",
        &[AdverseEventCountBucket {
            value: "Cough".to_string(),
            count: 7,
        }],
    )
    .expect("count markdown");

    assert!(markdown.contains("# Adverse Event Counts"));
    assert!(markdown.contains("Count field: reaction"));
    assert!(markdown.contains("| Cough | 7 |"));
}

#[test]
fn device_event_renderers_include_openfda_content() {
    let event = DeviceEvent {
        report_id: "MDR-123".to_string(),
        report_number: None,
        device: "Infusion Pump".to_string(),
        manufacturer: Some("Example".to_string()),
        event_type: Some("Malfunction".to_string()),
        date: Some("2024-02-01".to_string()),
        description: Some("Description text.".to_string()),
    };

    let markdown = device_event_markdown(&event).expect("device");
    assert!(markdown.contains("Source: OpenFDA"));
    assert!(markdown.contains("## Description (OpenFDA)"));

    let search_markdown = device_event_search_markdown(
        "pump",
        &[DeviceEventSearchResult {
            report_id: event.report_id.clone(),
            device: event.device.clone(),
            event_type: event.event_type.clone(),
            date: event.date.clone(),
            description: event.description.clone(),
        }],
    )
    .expect("device search");
    assert!(search_markdown.contains("# Device Events: pump"));
    assert!(search_markdown.contains("|Report Key|Device|Event Type|Date|Description|"));
}

#[test]
fn recall_search_markdown_renders_result_table() {
    let markdown = recall_search_markdown(
        "insulin pump",
        &[RecallSearchResult {
            recall_number: "Z-1234-2025".to_string(),
            classification: "Class II".to_string(),
            product_description: "Infusion pump cartridge".to_string(),
            reason_for_recall: "Leak risk".to_string(),
            status: "Ongoing".to_string(),
            distribution_pattern: None,
            recall_initiation_date: Some("2025-01-01".to_string()),
        }],
    )
    .expect("recall search");

    assert!(markdown.contains("# Recalls"));
    assert!(markdown.contains("|Recall #|Classification|Product|Status|"));
    assert!(markdown.contains("|Z-1234-2025|Class II|Infusion pump cartridge|Ongoing|"));
}
