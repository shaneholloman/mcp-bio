use std::path::PathBuf;
use std::time::Instant;

use anyhow::Result;
use biomcp_kb_rust_probe::{run_html_file, run_jats_file, run_pdf_file, PdfEngine};
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Jats {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    Html {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        base_url: String,
        #[arg(long)]
        output: PathBuf,
    },
    Pdf {
        #[arg(long)]
        engine: CliPdfEngine,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value_t = 12)]
        page_limit: u32,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum CliPdfEngine {
    Unpdf,
    PdfOxide,
}

impl From<CliPdfEngine> for PdfEngine {
    fn from(value: CliPdfEngine) -> Self {
        match value {
            CliPdfEngine::Unpdf => Self::Unpdf,
            CliPdfEngine::PdfOxide => Self::PdfOxide,
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let started = Instant::now();

    let report = match cli.command {
        Command::Jats { input, output } => run_jats_file(&input, &output, started),
        Command::Html {
            input,
            base_url,
            output,
        } => run_html_file(&input, &base_url, &output, started),
        Command::Pdf {
            engine,
            input,
            output,
            page_limit,
        } => run_pdf_file(engine.into(), &input, &output, page_limit, started),
    };

    println!("{}", serde_json::to_string_pretty(&report?)?);
    Ok(())
}
