//! Request construction tests. Pure: build gRPC request structs and inspect
//! them. No network.

use super::*;

#[test]
fn score_variant_request_sets_interval_variant_and_scorers() {
    let request = score_variant_request("chr7", 140_753_336, "A", "T");
    let interval = request.interval.unwrap();
    let variant = request.variant.unwrap();

    assert_eq!(interval.chromosome, "chr7");
    assert_eq!(interval.start, 140_491_191);
    assert_eq!(interval.end, 141_015_479);
    assert_eq!(variant.chromosome, "chr7");
    assert_eq!(variant.position, 140_753_336);
    assert_eq!(variant.reference_bases, "A");
    assert_eq!(variant.alternate_bases, "T");
    assert_eq!(
        request.organism,
        alphagenome_proto::Organism::HomoSapiens as i32
    );
    assert_eq!(request.variant_scorers.len(), 3);
}

#[test]
fn make_interval_clamps_start_and_keeps_expected_width() {
    let interval = make_interval("chr7", 1);

    assert_eq!(interval.chromosome, "chr7");
    assert_eq!(interval.start, 0);
    assert_eq!(interval.end, 2 * INTERVAL_HALF);
}

#[test]
fn recommended_scorers_are_stable() {
    let scorers = recommended_scorers();

    assert_eq!(scorers.len(), 3);
}
