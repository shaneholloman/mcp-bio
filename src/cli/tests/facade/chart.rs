use super::*;

#[test]
fn chart_args_default_to_no_chart() {
    let args = ChartArgs {
        chart: None,
        terminal: false,
        output: None,
        title: None,
        theme: None,
        palette: None,
        cols: None,
        rows: None,
        width: None,
        height: None,
        scale: None,
        mcp_inline: false,
    };
    assert_eq!(args.chart, None);
    assert!(!args.terminal);
    assert!(!args.mcp_inline);
    assert_eq!(args.cols, None);
    assert_eq!(args.rows, None);
    assert_eq!(args.width, None);
    assert_eq!(args.height, None);
    assert_eq!(args.scale, None);
}

#[test]
fn chart_dimension_flags_validate_positive_values() {
    let cols_err = Cli::try_parse_from([
        "biomcp",
        "study",
        "query",
        "--study",
        "msk_impact_2017",
        "--gene",
        "TP53",
        "--type",
        "mutations",
        "--chart",
        "bar",
        "--cols",
        "0",
    ])
    .expect_err("zero columns should fail");
    assert!(cols_err.to_string().contains("--cols must be >= 1"));

    let scale_err = Cli::try_parse_from([
        "biomcp",
        "study",
        "query",
        "--study",
        "msk_impact_2017",
        "--gene",
        "TP53",
        "--type",
        "mutations",
        "--chart",
        "bar",
        "--scale",
        "0",
    ])
    .expect_err("zero scale should fail");
    assert!(scale_err.to_string().contains("--scale must be > 0"));

    let nan_err = Cli::try_parse_from([
        "biomcp",
        "study",
        "query",
        "--study",
        "msk_impact_2017",
        "--gene",
        "TP53",
        "--type",
        "mutations",
        "--chart",
        "bar",
        "--scale",
        "NaN",
        "-o",
        "chart.png",
    ])
    .expect_err("non-finite scale should fail");
    assert!(
        nan_err
            .to_string()
            .contains("--scale must be a finite number > 0")
    );
}

#[test]
fn rewrite_mcp_chart_args_preserves_svg_sizing_flags() {
    let args = vec![
        "biomcp".to_string(),
        "study".to_string(),
        "query".to_string(),
        "--study".to_string(),
        "demo".to_string(),
        "--gene".to_string(),
        "TP53".to_string(),
        "--type".to_string(),
        "mutations".to_string(),
        "--chart".to_string(),
        "bar".to_string(),
        "--width".to_string(),
        "1200".to_string(),
        "--height".to_string(),
        "600".to_string(),
        "--title".to_string(),
        "Example".to_string(),
    ];

    let text = rewrite_mcp_chart_args(&args, McpChartPass::Text).expect("text rewrite");
    assert!(!text.iter().any(|value| value == "--chart"));
    assert!(!text.iter().any(|value| value == "--width"));
    assert!(!text.iter().any(|value| value == "--height"));

    let svg = rewrite_mcp_chart_args(&args, McpChartPass::Svg).expect("svg rewrite");
    assert!(svg.iter().any(|value| value == "--chart"));
    assert!(svg.iter().any(|value| value == "--width"));
    assert!(svg.iter().any(|value| value == "--height"));
    assert!(svg.iter().any(|value| value == "--mcp-inline"));
}

#[test]
fn rewrite_mcp_chart_args_rejects_terminal_and_png_only_flags() {
    let cols_err = rewrite_mcp_chart_args(
        &[
            "biomcp".to_string(),
            "study".to_string(),
            "query".to_string(),
            "--study".to_string(),
            "demo".to_string(),
            "--gene".to_string(),
            "TP53".to_string(),
            "--type".to_string(),
            "mutations".to_string(),
            "--chart".to_string(),
            "bar".to_string(),
            "--cols".to_string(),
            "80".to_string(),
        ],
        McpChartPass::Svg,
    )
    .expect_err("mcp svg rewrite should reject terminal sizing");
    assert!(
        cols_err
            .to_string()
            .contains("--cols/--rows require terminal chart output"),
        "{cols_err}"
    );

    let scale_err = rewrite_mcp_chart_args(
        &[
            "biomcp".to_string(),
            "study".to_string(),
            "query".to_string(),
            "--study".to_string(),
            "demo".to_string(),
            "--gene".to_string(),
            "TP53".to_string(),
            "--type".to_string(),
            "mutations".to_string(),
            "--chart".to_string(),
            "bar".to_string(),
            "--scale".to_string(),
            "2.0".to_string(),
        ],
        McpChartPass::Svg,
    )
    .expect_err("mcp svg rewrite should reject png scale");
    assert!(
        scale_err
            .to_string()
            .contains("--scale requires PNG chart output"),
        "{scale_err}"
    );
}
