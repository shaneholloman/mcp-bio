use super::StudyCommand;
use crate::cli::{ChartType, CommandOutcome};

pub(crate) async fn handle_command(
    cmd: StudyCommand,
    json: bool,
) -> anyhow::Result<CommandOutcome> {
    let text = match cmd {
        StudyCommand::List => {
            let studies = crate::entities::study::list_studies().await?;
            if json {
                crate::render::json::to_pretty(&studies)?
            } else {
                crate::render::markdown::study_list_markdown(&studies)
            }
        }
        StudyCommand::Download { list, study_id } => {
            if list {
                let result = crate::entities::study::list_downloadable_studies().await?;
                if json {
                    crate::render::json::to_pretty(&result)?
                } else {
                    crate::render::markdown::study_download_catalog_markdown(&result)
                }
            } else {
                let study_id = study_id.expect("clap should require study_id");
                let result = crate::entities::study::download_study(&study_id).await?;
                if json {
                    crate::render::json::to_pretty(&result)?
                } else {
                    crate::render::markdown::study_download_markdown(&result)
                }
            }
        }
        StudyCommand::Query {
            study,
            gene,
            query_type,
            chart,
        } => {
            let query_type = crate::entities::study::StudyQueryType::from_flag(&query_type)?;
            super::super::chart_json_conflict(&chart, json)?;
            if let Some(chart_type) = chart.chart {
                crate::render::chart::validate_query_chart_type(query_type, chart_type)?;
                let options = crate::render::chart::ChartRenderOptions::from(&chart);
                match query_type {
                    crate::entities::study::StudyQueryType::Mutations => match chart_type {
                        ChartType::Waterfall => {
                            let sample_counts =
                                crate::entities::study::mutation_counts_by_sample(&study, &gene)
                                    .await?;
                            crate::render::chart::render_mutation_waterfall_chart(
                                &study,
                                &gene,
                                &sample_counts,
                                &options,
                            )?
                        }
                        ChartType::Bar | ChartType::Pie => {
                            let result =
                                crate::entities::study::query_study(&study, &gene, query_type)
                                    .await?;
                            let crate::entities::study::StudyQueryResult::MutationFrequency(result) =
                                result
                            else {
                                unreachable!("mutation query should return mutation result");
                            };
                            crate::render::chart::render_mutation_frequency_chart(
                                &result, chart_type, &options,
                            )?
                        }
                        other => {
                            return Err(crate::error::BioMcpError::InvalidArgument(format!(
                                "Invalid chart type: {other}"
                            ))
                            .into());
                        }
                    },
                    crate::entities::study::StudyQueryType::Cna => {
                        let result =
                            crate::entities::study::query_study(&study, &gene, query_type).await?;
                        let crate::entities::study::StudyQueryResult::CnaDistribution(result) =
                            result
                        else {
                            unreachable!("cna query should return cna result");
                        };
                        crate::render::chart::render_cna_chart(&result, chart_type, &options)?
                    }
                    crate::entities::study::StudyQueryType::Expression => match chart_type {
                        ChartType::Histogram => {
                            let values =
                                crate::entities::study::expression_values(&study, &gene).await?;
                            crate::render::chart::render_expression_histogram_chart(
                                &study, &gene, &values, &options,
                            )?
                        }
                        ChartType::Density => {
                            let values =
                                crate::entities::study::expression_values(&study, &gene).await?;
                            crate::render::chart::render_expression_density_chart(
                                &study, &gene, &values, &options,
                            )?
                        }
                        other => {
                            return Err(crate::error::BioMcpError::InvalidArgument(format!(
                                "Invalid chart type: {other}"
                            ))
                            .into());
                        }
                    },
                }
            } else {
                let result = crate::entities::study::query_study(&study, &gene, query_type).await?;
                if json {
                    crate::render::json::to_pretty(&result)?
                } else {
                    crate::render::markdown::study_query_markdown(&result)
                }
            }
        }
        StudyCommand::TopMutated { study, limit } => {
            let result = crate::entities::study::top_mutated_genes(&study, limit).await?;
            if json {
                crate::render::json::to_pretty(&result)?
            } else {
                crate::render::markdown::study_top_mutated_markdown(&result)
            }
        }
        StudyCommand::Filter {
            study,
            mutated,
            amplified,
            deleted,
            expression_above,
            expression_below,
            cancer_type,
        } => {
            let mut criteria = Vec::new();
            for gene in mutated {
                criteria.push(crate::entities::study::FilterCriterion::Mutated(gene));
            }
            for gene in amplified {
                criteria.push(crate::entities::study::FilterCriterion::Amplified(gene));
            }
            for gene in deleted {
                criteria.push(crate::entities::study::FilterCriterion::Deleted(gene));
            }
            for value in expression_above {
                criteria.push(super::super::parse_expression_filter(
                    &value,
                    "--expression-above",
                    crate::entities::study::FilterCriterion::ExpressionAbove,
                )?);
            }
            for value in expression_below {
                criteria.push(super::super::parse_expression_filter(
                    &value,
                    "--expression-below",
                    crate::entities::study::FilterCriterion::ExpressionBelow,
                )?);
            }
            for value in cancer_type {
                criteria.push(crate::entities::study::FilterCriterion::CancerType(value));
            }
            if criteria.is_empty() {
                return Err(crate::error::BioMcpError::InvalidArgument(
                    crate::entities::study::filter_required_message().to_string(),
                )
                .into());
            }
            let result = crate::entities::study::filter(&study, criteria).await?;
            if json {
                crate::render::json::to_pretty(&result)?
            } else {
                crate::render::markdown::study_filter_markdown(&result)
            }
        }
        StudyCommand::Cohort { study, gene } => {
            let result = crate::entities::study::cohort(&study, &gene).await?;
            if json {
                crate::render::json::to_pretty(&result)?
            } else {
                crate::render::markdown::study_cohort_markdown(&result)
            }
        }
        StudyCommand::Survival {
            study,
            gene,
            endpoint,
            chart,
        } => {
            let endpoint = crate::entities::study::SurvivalEndpoint::from_flag(&endpoint)?;
            super::super::chart_json_conflict(&chart, json)?;
            if let Some(chart_type) = chart.chart {
                crate::render::chart::validate_standalone_chart_type(
                    "study survival",
                    chart_type,
                    &[ChartType::Bar, ChartType::Survival],
                )?;
                let result = crate::entities::study::survival(&study, &gene, endpoint).await?;
                let options = crate::render::chart::ChartRenderOptions::from(&chart);
                crate::render::chart::render_survival_chart(&result, chart_type, &options)?
            } else {
                let result = crate::entities::study::survival(&study, &gene, endpoint).await?;
                if json {
                    crate::render::json::to_pretty(&result)?
                } else {
                    crate::render::markdown::study_survival_markdown(&result)
                }
            }
        }
        StudyCommand::Compare {
            study,
            gene,
            compare_type,
            target,
            chart,
        } => {
            super::super::chart_json_conflict(&chart, json)?;
            match compare_type.trim().to_ascii_lowercase().as_str() {
                "expression" | "expr" => {
                    if let Some(chart_type) = chart.chart {
                        crate::render::chart::validate_compare_chart_type(
                            "expression",
                            chart_type,
                        )?;
                        let options = crate::render::chart::ChartRenderOptions::from(&chart);
                        match chart_type {
                            ChartType::Scatter => {
                                let points = crate::entities::study::expression_pairs_by_sample(
                                    &study, &gene, &target,
                                )
                                .await?;
                                crate::render::chart::render_expression_scatter_chart(
                                    &study, &gene, &target, &points, &options,
                                )?
                            }
                            ChartType::Box | ChartType::Violin | ChartType::Ridgeline => {
                                let groups = crate::entities::study::compare_expression_values(
                                    &study, &gene, &target,
                                )
                                .await?;
                                crate::render::chart::render_expression_compare_chart(
                                    &study, &gene, &target, &groups, chart_type, &options,
                                )?
                            }
                            other => {
                                return Err(crate::error::BioMcpError::InvalidArgument(format!(
                                    "Invalid chart type: {other}"
                                ))
                                .into());
                            }
                        }
                    } else {
                        let result =
                            crate::entities::study::compare_expression(&study, &gene, &target)
                                .await?;
                        if json {
                            crate::render::json::to_pretty(&result)?
                        } else {
                            crate::render::markdown::study_compare_expression_markdown(&result)
                        }
                    }
                }
                "mutations" | "mutation" => {
                    if let Some(chart_type) = chart.chart {
                        crate::render::chart::validate_compare_chart_type("mutations", chart_type)?;
                        let result =
                            crate::entities::study::compare_mutations(&study, &gene, &target)
                                .await?;
                        let options = crate::render::chart::ChartRenderOptions::from(&chart);
                        crate::render::chart::render_mutation_compare_chart(
                            &result, chart_type, &options,
                        )?
                    } else {
                        let result =
                            crate::entities::study::compare_mutations(&study, &gene, &target)
                                .await?;
                        if json {
                            crate::render::json::to_pretty(&result)?
                        } else {
                            crate::render::markdown::study_compare_mutations_markdown(&result)
                        }
                    }
                }
                other => {
                    return Err(crate::error::BioMcpError::InvalidArgument(format!(
                        "Unknown comparison type '{other}'. Expected: expression, mutations."
                    ))
                    .into());
                }
            }
        }
        StudyCommand::CoOccurrence {
            study,
            genes,
            chart,
        } => {
            super::super::chart_json_conflict(&chart, json)?;
            let genes = genes
                .split(',')
                .map(str::trim)
                .filter(|gene| !gene.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>();
            if genes.len() < 2 || genes.len() > 10 {
                return Err(crate::error::BioMcpError::InvalidArgument(
                    "--genes must contain 2 to 10 comma-separated symbols".into(),
                )
                .into());
            }
            if let Some(chart_type) = chart.chart {
                crate::render::chart::validate_standalone_chart_type(
                    "study co-occurrence",
                    chart_type,
                    &[ChartType::Bar, ChartType::Pie, ChartType::Heatmap],
                )?;
                let result = crate::entities::study::co_occurrence(&study, &genes).await?;
                let options = crate::render::chart::ChartRenderOptions::from(&chart);
                crate::render::chart::render_co_occurrence_chart(&result, chart_type, &options)?
            } else {
                let result = crate::entities::study::co_occurrence(&study, &genes).await?;
                if json {
                    crate::render::json::to_pretty(&result)?
                } else {
                    crate::render::markdown::study_co_occurrence_markdown(&result)
                }
            }
        }
    };

    Ok(CommandOutcome::stdout(text))
}
