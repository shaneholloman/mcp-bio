use std::path::Path;

mod construction;
mod parsing;

fn fixture_csv() -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("spec")
            .join("fixtures")
            .join("who-ivd")
            .join(super::WHO_IVD_CSV_FILE),
    )
    .expect("WHO IVD fixture should be readable")
}
