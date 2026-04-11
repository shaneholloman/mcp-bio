use super::*;

#[test]
fn strict_target_family_label_accepts_numeric_suffix_family() {
    let targets = vec![
        "PARP1".to_string(),
        "PARP2".to_string(),
        "PARP3".to_string(),
    ];
    assert_eq!(
        strict_target_family_label(&targets).as_deref(),
        Some("PARP")
    );
}

#[test]
fn strict_target_family_label_handles_embedded_digits() {
    let targets = vec!["CYP2C9".to_string(), "CYP2C19".to_string()];
    assert_eq!(
        strict_target_family_label(&targets).as_deref(),
        Some("CYP2C")
    );
}

#[test]
fn strict_target_family_label_rejects_mixed_targets() {
    let targets = vec!["ABL1".to_string(), "KIT".to_string(), "PDGFRB".to_string()];
    assert!(strict_target_family_label(&targets).is_none());
}

#[test]
fn strict_target_family_label_rejects_single_target() {
    let targets = vec!["PDCD1".to_string()];
    assert!(strict_target_family_label(&targets).is_none());
}

#[test]
fn family_target_chembl_id_requires_single_matching_target_id() {
    let rows = vec![
        ChemblTarget {
            target: "PARP1".to_string(),
            action: "INHIBITOR".to_string(),
            mechanism: None,
            target_chembl_id: Some("CHEMBL3390820".to_string()),
        },
        ChemblTarget {
            target: "PARP2".to_string(),
            action: "INHIBITOR".to_string(),
            mechanism: None,
            target_chembl_id: Some("CHEMBL3390820".to_string()),
        },
    ];
    let displayed_targets = vec!["PARP1".to_string(), "PARP2".to_string()];
    assert_eq!(
        family_target_chembl_id(&rows, &displayed_targets).as_deref(),
        Some("CHEMBL3390820")
    );
}

#[test]
fn family_target_chembl_id_rejects_multiple_matching_target_ids() {
    let rows = vec![
        ChemblTarget {
            target: "PARP1".to_string(),
            action: "INHIBITOR".to_string(),
            mechanism: None,
            target_chembl_id: Some("CHEMBL3390820".to_string()),
        },
        ChemblTarget {
            target: "PARP2".to_string(),
            action: "INHIBITOR".to_string(),
            mechanism: None,
            target_chembl_id: Some("CHEMBL1234".to_string()),
        },
    ];
    let displayed_targets = vec!["PARP1".to_string(), "PARP2".to_string()];
    assert!(family_target_chembl_id(&rows, &displayed_targets).is_none());
}

#[test]
fn family_target_chembl_id_rejects_missing_matching_target_id() {
    let rows = vec![ChemblTarget {
        target: "PARP1".to_string(),
        action: "INHIBITOR".to_string(),
        mechanism: None,
        target_chembl_id: None,
    }];
    let displayed_targets = vec!["PARP1".to_string(), "PARP2".to_string()];
    assert!(family_target_chembl_id(&rows, &displayed_targets).is_none());
}

#[test]
fn family_target_chembl_id_accepts_mechanism_only_family_row() {
    let rows = vec![ChemblTarget {
        target: "Unknown target".to_string(),
        action: "INHIBITOR".to_string(),
        mechanism: Some("PARP 1, 2 and 3 inhibitor".to_string()),
        target_chembl_id: Some("CHEMBL3390820".to_string()),
    }];
    let displayed_targets = vec![
        "PARP1".to_string(),
        "PARP2".to_string(),
        "PARP3".to_string(),
    ];
    assert_eq!(
        family_target_chembl_id(&rows, &displayed_targets).as_deref(),
        Some("CHEMBL3390820")
    );
}

#[test]
fn derive_target_family_name_requires_complete_member_names() {
    let displayed_targets = vec!["PARP1".to_string(), "PARP2".to_string()];
    let opentargets_targets = vec![
        OpenTargetsTarget {
            approved_symbol: "PARP1".to_string(),
            approved_name: Some("poly(ADP-ribose) polymerase 1".to_string()),
        },
        OpenTargetsTarget {
            approved_symbol: "PARP2".to_string(),
            approved_name: None,
        },
    ];
    assert!(derive_target_family_name(&displayed_targets, &opentargets_targets).is_none());
}

#[test]
fn derive_target_family_name_trims_numeric_member_suffix() {
    let displayed_targets = vec!["PARP1".to_string(), "PARP2".to_string()];
    let opentargets_targets = vec![
        OpenTargetsTarget {
            approved_symbol: "PARP1".to_string(),
            approved_name: Some("poly(ADP-ribose) polymerase 1".to_string()),
        },
        OpenTargetsTarget {
            approved_symbol: "PARP2".to_string(),
            approved_name: Some("poly(ADP-ribose) polymerase 2".to_string()),
        },
    ];
    assert_eq!(
        derive_target_family_name(&displayed_targets, &opentargets_targets).as_deref(),
        Some("poly(ADP-ribose) polymerase")
    );
}

#[test]
fn derive_target_family_name_handles_non_ascii_without_panicking() {
    let displayed_targets = vec!["GENE1".to_string(), "GENE2".to_string()];
    let opentargets_targets = vec![
        OpenTargetsTarget {
            approved_symbol: "GENE1".to_string(),
            approved_name: Some("électron receptor 1".to_string()),
        },
        OpenTargetsTarget {
            approved_symbol: "GENE2".to_string(),
            approved_name: Some("èlectron receptor 2".to_string()),
        },
    ];
    assert!(derive_target_family_name(&displayed_targets, &opentargets_targets).is_none());
}
