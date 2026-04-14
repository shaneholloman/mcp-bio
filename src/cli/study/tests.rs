use clap::{CommandFactory, Parser};

use super::handle_command;
use crate::cli::study::StudyCommand;
use crate::cli::{ChartType, Cli, Commands, execute};

mod charts;
mod parsing;
mod validation;
