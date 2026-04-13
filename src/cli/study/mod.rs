//! Study CLI subcommands.

use clap::Subcommand;

use crate::cli::ChartArgs;

#[derive(Subcommand, Debug)]
pub enum StudyCommand {
    #[command(after_help = "\
EXAMPLES:
  biomcp study list

See also: biomcp list study")]
    List,
    #[command(after_help = "\
EXAMPLES:
  biomcp study download --list
  biomcp study download msk_impact_2017
  biomcp study download brca_tcga_pan_can_atlas_2018

See also: biomcp list study")]
    Download {
        #[arg(long, conflicts_with = "study_id")]
        list: bool,
        #[arg(value_name = "STUDY_ID", required_unless_present = "list")]
        study_id: Option<String>,
    },
    #[command(after_help = "\
EXAMPLES:
  biomcp study query --study msk_impact_2017 --gene TP53 --type mutations
  biomcp study query --study brca_tcga_pan_can_atlas_2018 --gene ERBB2 --type cna
  biomcp study query --study paad_qcmg_uq_2016 --gene KRAS --type expression

See also: biomcp list study")]
    Query {
        #[arg(short, long)]
        study: String,
        #[arg(short, long)]
        gene: String,
        #[arg(short = 't', long = "type")]
        query_type: String,
        #[command(flatten)]
        chart: ChartArgs,
    },
    #[command(after_help = "\
EXAMPLES:
  biomcp study top-mutated --study msk_impact_2017
  biomcp study top-mutated --study cll_broad_2022 --limit 10

See also: biomcp list study")]
    TopMutated {
        #[arg(short, long)]
        study: String,
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    #[command(after_help = "\
EXAMPLES:
  biomcp study filter --study msk_impact_2017 --mutated TP53
  biomcp study filter --study brca_tcga_pan_can_atlas_2018 --mutated TP53 --amplified ERBB2
  biomcp study filter --study brca_tcga_pan_can_atlas_2018 --mutated TP53 --expression-above ERBB2:1.5 --cancer-type \"Breast Cancer\"

See also: biomcp list study")]
    Filter {
        #[arg(short, long)]
        study: String,
        #[arg(long)]
        mutated: Vec<String>,
        #[arg(long)]
        amplified: Vec<String>,
        #[arg(long)]
        deleted: Vec<String>,
        #[arg(long = "expression-above")]
        expression_above: Vec<String>,
        #[arg(long = "expression-below")]
        expression_below: Vec<String>,
        #[arg(long = "cancer-type")]
        cancer_type: Vec<String>,
    },
    #[command(after_help = "\
EXAMPLES:
  biomcp study cohort --study brca_tcga_pan_can_atlas_2018 --gene TP53

See also: biomcp list study")]
    Cohort {
        #[arg(short, long)]
        study: String,
        #[arg(short, long)]
        gene: String,
    },
    #[command(after_help = "\
EXAMPLES:
  biomcp study survival --study brca_tcga_pan_can_atlas_2018 --gene TP53
  biomcp study survival --study brca_tcga_pan_can_atlas_2018 --gene TP53 --endpoint DFS

See also: biomcp list study")]
    Survival {
        #[arg(short, long)]
        study: String,
        #[arg(short, long)]
        gene: String,
        #[arg(short, long, default_value = "os")]
        endpoint: String,
        #[command(flatten)]
        chart: ChartArgs,
    },
    #[command(after_help = "\
EXAMPLES:
  biomcp study compare --study brca_tcga_pan_can_atlas_2018 --gene TP53 --type expression --target ERBB2
  biomcp study compare --study brca_tcga_pan_can_atlas_2018 --gene TP53 --type mutations --target PIK3CA

See also: biomcp list study")]
    Compare {
        #[arg(short, long)]
        study: String,
        #[arg(short, long)]
        gene: String,
        #[arg(short = 't', long = "type")]
        compare_type: String,
        #[arg(long)]
        target: String,
        #[command(flatten)]
        chart: ChartArgs,
    },
    #[command(after_help = "\
EXAMPLES:
  biomcp study co-occurrence --study msk_impact_2017 --genes TP53,KRAS
  biomcp study co-occurrence --study brca_tcga_pan_can_atlas_2018 --genes TP53,PIK3CA,GATA3

See also: biomcp list study")]
    CoOccurrence {
        #[arg(short, long)]
        study: String,
        #[arg(short, long)]
        genes: String,
        #[command(flatten)]
        chart: ChartArgs,
    },
}

#[cfg(test)]
mod tests;
