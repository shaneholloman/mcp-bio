//! WHO structured-search bridge coverage.

use super::*;

#[tokio::test]
async fn structured_who_search_stops_after_one_extra_match_and_reports_unknown_total() {
    let filters = DrugSearchFilters {
        indication: Some("malaria".into()),
        ..Default::default()
    };
    let fetch_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let fetch_count_for_closure = fetch_count.clone();

    let page = search_structured_who_page_with(
        &filters,
        2,
        0,
        move |_, _, page_offset| {
            let fetch_count = fetch_count_for_closure.clone();
            async move {
                fetch_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                match page_offset {
                    0 => Ok(SearchPage::offset(
                        vec![mychem_row("candidate-a"), mychem_row("candidate-b")],
                        Some(100),
                    )),
                    _ => Ok(SearchPage::offset(Vec::new(), Some(100))),
                }
            }
        },
        |name| match name {
            "candidate-a" => vec![who_row("W1", "Artemether"), who_row("W2", "Lumefantrine")],
            "candidate-b" => vec![who_row("W3", "Artesunate")],
            _ => Vec::new(),
        },
    )
    .await
    .expect("structured WHO search");

    assert_eq!(fetch_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(page.total, None);
    assert_eq!(page.results.len(), 2);
    assert_eq!(page.results[0].who_reference_number, "W1");
    assert_eq!(page.results[1].who_reference_number, "W2");
}

#[tokio::test]
async fn structured_who_search_reports_exact_total_when_mychem_is_exhausted() {
    let filters = DrugSearchFilters {
        indication: Some("malaria".into()),
        ..Default::default()
    };

    let page = search_structured_who_page_with(
        &filters,
        5,
        0,
        |_, _, page_offset| async move {
            match page_offset {
                0 => Ok(SearchPage::offset(vec![mychem_row("candidate-a")], Some(1))),
                _ => Ok(SearchPage::offset(Vec::new(), Some(1))),
            }
        },
        |name| match name {
            "candidate-a" => vec![who_row("W1", "Artemether/Lumefantrine")],
            _ => Vec::new(),
        },
    )
    .await
    .expect("structured WHO search");

    assert_eq!(page.total, Some(1));
    assert_eq!(page.results.len(), 1);
    assert_eq!(page.results[0].who_reference_number, "W1");
}
