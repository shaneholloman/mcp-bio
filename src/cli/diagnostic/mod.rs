//! Diagnostic CLI payloads.

use clap::Args;

#[derive(Args, Debug)]
pub struct DiagnosticSearchArgs {
    /// Filter by gene symbol
    #[arg(short = 'g', long)]
    pub gene: Option<String>,
    /// Filter by disease name substring
    #[arg(short = 'd', long)]
    pub disease: Option<String>,
    /// Filter by exact diagnostic type
    #[arg(short = 't', long = "type")]
    pub test_type: Option<String>,
    /// Filter by manufacturer or lab substring
    #[arg(long)]
    pub manufacturer: Option<String>,
    /// Maximum results (default: 10)
    #[arg(short, long, default_value = "10")]
    pub limit: usize,
    /// Skip the first N results
    #[arg(long, default_value = "0")]
    pub offset: usize,
}

#[derive(Args, Debug)]
pub struct DiagnosticGetArgs {
    /// GTR accession version (e.g., GTR000000001.1)
    pub accession: String,
    /// Sections to include (genes, conditions, methods, all)
    #[arg(trailing_var_arg = true)]
    pub sections: Vec<String>,
}

mod dispatch;
pub(super) use self::dispatch::{handle_get, handle_search};

#[cfg(test)]
mod tests;
