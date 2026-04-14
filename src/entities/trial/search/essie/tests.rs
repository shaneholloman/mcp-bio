//! Tests for ESSIE trial search helpers.

use super::*;

#[test]
fn essie_escape_boolean_expression_preserves_or_operators() {
    assert_eq!(
        essie_escape_boolean_expression("dMMR OR MSI-H"),
        "\"dMMR\" OR \"MSI\\-H\""
    );
}

#[test]
fn essie_escape_boolean_expression_handles_leading_not() {
    assert_eq!(
        essie_escape_boolean_expression("NOT MSI-H"),
        "NOT \"MSI\\-H\""
    );
}

#[test]
fn essie_escape_boolean_expression_handles_and_not() {
    assert_eq!(
        essie_escape_boolean_expression("dMMR AND NOT MSI-H"),
        "\"dMMR\" AND NOT \"MSI\\-H\""
    );
}

#[test]
fn line_of_therapy_patterns_accepts_supported_values() {
    assert!(line_of_therapy_patterns("1L").is_some());
    assert!(line_of_therapy_patterns("2L").is_some());
    assert!(line_of_therapy_patterns("3L+").is_some());
    assert!(line_of_therapy_patterns("2l").is_some());
}

#[test]
fn line_of_therapy_patterns_rejects_invalid_values() {
    assert!(line_of_therapy_patterns("4L").is_none());
    assert!(line_of_therapy_patterns("frontline").is_none());
}
