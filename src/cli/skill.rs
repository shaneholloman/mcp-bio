use std::collections::HashSet;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

use clap::Subcommand;

use crate::error::BioMcpError;

#[derive(Subcommand, Debug)]
pub enum SkillCommand {
    /// List embedded worked examples
    List,
    /// Render the canonical agent-facing prompt
    Render,
    /// Show a specific use-case by number or name
    #[command(external_subcommand)]
    Show(Vec<String>),
    /// Install BioMCP skill guidance to an agent directory
    Install {
        /// Agent root or skills directory (e.g. ~/.claude, ~/.claude/skills, ~/.claude/skills/biomcp)
        dir: Option<String>,
        /// Replace existing installation
        #[arg(long)]
        force: bool,
    },
}

#[derive(Debug, Clone)]
struct UseCaseMeta {
    number: String,
    slug: String,
    title: String,
    description: Option<String>,
    embedded_path: String,
}

#[derive(Debug, Clone)]
pub(crate) struct UseCaseRef {
    pub slug: String,
    pub title: String,
}

fn embedded_text(path: &str) -> Result<String, BioMcpError> {
    match crate::skill_assets::text(path) {
        Ok(text) => Ok(text),
        Err(BioMcpError::NotFound { .. }) => Err(BioMcpError::NotFound {
            entity: "skill".into(),
            id: path.to_string(),
            suggestion: "Try: biomcp skill".into(),
        }),
        Err(_) => Err(BioMcpError::InvalidArgument(
            "Embedded skill file is not valid UTF-8".into(),
        )),
    }
}

fn canonical_prompt_body() -> Result<String, BioMcpError> {
    let mut body = embedded_text("SKILL.md")?;
    while body.ends_with('\n') {
        body.pop();
    }
    Ok(body)
}

/// Renders the canonical agent-facing BioMCP prompt.
///
/// # Errors
///
/// Returns an error if the embedded prompt cannot be loaded.
pub fn render_system_prompt() -> Result<String, BioMcpError> {
    canonical_prompt_body()
}

fn canonical_prompt_file_bytes() -> Result<Vec<u8>, BioMcpError> {
    let mut body = canonical_prompt_body()?;
    body.push('\n');
    Ok(body.into_bytes())
}

fn parse_title_and_description(markdown: &str) -> (String, Option<String>) {
    let mut title: Option<String> = None;
    let mut description: Option<String> = None;

    for line in markdown.lines() {
        let line = line.trim_end();
        if title.is_none() && line.starts_with("# ") {
            title = Some(line.trim_start_matches("# ").trim().to_string());
            continue;
        }
        if title.is_some() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            // First non-empty line after the title.
            description = Some(trimmed.to_string());
            break;
        }
    }

    (title.unwrap_or_else(|| "Untitled".into()), description)
}

fn use_case_index() -> Result<Vec<UseCaseMeta>, BioMcpError> {
    let mut out: Vec<UseCaseMeta> = Vec::new();

    for file in crate::skill_assets::iter() {
        let path = file.as_ref();
        if !path.starts_with("use-cases/") || !path.ends_with(".md") {
            continue;
        }

        let file_name = path
            .rsplit('/')
            .next()
            .unwrap_or(path)
            .trim_end_matches(".md");

        let (number, slug) = match file_name.split_once('-') {
            Some((n, rest)) if n.len() == 2 && n.chars().all(|c| c.is_ascii_digit()) => {
                (n.to_string(), rest.to_string())
            }
            _ => continue,
        };

        let content = embedded_text(path)?;
        let (title, description) = parse_title_and_description(&content);

        out.push(UseCaseMeta {
            number,
            slug,
            title,
            description,
            embedded_path: path.to_string(),
        });
    }

    out.sort_by_key(|m| m.number.parse::<u32>().unwrap_or(999));
    Ok(out)
}

/// Returns the embedded BioMCP skill overview document.
///
/// # Errors
///
/// Returns an error if the embedded overview document cannot be loaded.
pub fn show_overview() -> Result<String, BioMcpError> {
    canonical_prompt_body()
}

/// Lists available embedded skill use-cases.
///
/// # Errors
///
/// Returns an error if embedded skill metadata cannot be loaded.
pub fn list_use_cases() -> Result<String, BioMcpError> {
    let cases = use_case_index()?;
    if cases.is_empty() {
        return Ok("No skills found".into());
    }

    let mut out = String::new();
    out.push_str("# BioMCP Worked Examples\n\n");
    out.push_str(
        "Worked examples are short, executable investigation patterns. Run `biomcp skill <name>` to open one.\n\n",
    );
    for c in cases {
        out.push_str(&format!("{} {} - {}\n", c.number, c.slug, c.title));
        if let Some(desc) = c.description {
            out.push_str(&format!("  {desc}\n"));
        }
        out.push('\n');
    }
    Ok(out)
}

pub(crate) fn list_use_case_refs() -> Result<Vec<UseCaseRef>, BioMcpError> {
    Ok(use_case_index()?
        .into_iter()
        .map(|c| UseCaseRef {
            slug: c.slug,
            title: c.title,
        })
        .collect())
}

fn normalize_use_case_key(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Accept "01", "1", "01-treatment-lookup", or "treatment-lookup"
    if trimmed.chars().all(|c| c.is_ascii_digit())
        && let Ok(n) = trimmed.parse::<u32>()
    {
        return format!("{n:02}");
    }

    let lowered = trimmed.to_ascii_lowercase();
    if lowered.len() >= 3
        && lowered.as_bytes()[0].is_ascii_digit()
        && lowered.as_bytes()[1].is_ascii_digit()
        && lowered.as_bytes()[2] == b'-'
    {
        return lowered[3..].to_string();
    }

    lowered
}

/// Shows one skill use-case by number or slug.
///
/// # Errors
///
/// Returns an error if the requested skill does not exist or cannot be loaded.
pub fn show_use_case(name: &str) -> Result<String, BioMcpError> {
    let key = normalize_use_case_key(name);
    if key.is_empty() {
        return show_overview();
    }

    let cases = use_case_index()?;
    let found = cases.into_iter().find(|c| c.number == key || c.slug == key);
    let Some(found) = found else {
        return Err(BioMcpError::NotFound {
            entity: "skill".into(),
            id: name.to_string(),
            suggestion: "Try: biomcp skill list".into(),
        });
    };

    embedded_text(&found.embedded_path)
}

fn expand_tilde(path: &str) -> Result<PathBuf, BioMcpError> {
    if path == "~" {
        let home = std::env::var("HOME")
            .map_err(|_| BioMcpError::InvalidArgument("HOME is not set".into()))?;
        return Ok(PathBuf::from(home));
    }
    if let Some(rest) = path.strip_prefix("~/") {
        let home = std::env::var("HOME")
            .map_err(|_| BioMcpError::InvalidArgument("HOME is not set".into()))?;
        return Ok(PathBuf::from(home).join(rest));
    }
    Ok(PathBuf::from(path))
}

fn resolve_install_dir(input: PathBuf) -> PathBuf {
    let ends_with = |path: &Path, a: &str, b: &str| -> bool {
        let mut comps = path.components().rev();
        let Some(last) = comps.next().and_then(|c| c.as_os_str().to_str()) else {
            return false;
        };
        let Some(prev) = comps.next().and_then(|c| c.as_os_str().to_str()) else {
            return false;
        };
        prev == a && last == b
    };

    if ends_with(&input, "skills", "biomcp") {
        return input;
    }

    if input.file_name().and_then(|v| v.to_str()) == Some("skills") {
        return input.join("biomcp");
    }

    input.join("skills").join("biomcp")
}

#[derive(Debug, Clone)]
struct CandidateEntry {
    key: &'static str,
    agent_root: PathBuf,
    skills_dir: PathBuf,
    biomcp_dir: PathBuf,
    skill_md: PathBuf,
}

fn candidate_entry(key: &'static str, agent_root: PathBuf, skills_rel: &[&str]) -> CandidateEntry {
    let skills_dir = skills_rel
        .iter()
        .fold(agent_root.clone(), |path, component| path.join(component));
    let biomcp_dir = skills_dir.join("biomcp");
    let skill_md = biomcp_dir.join("SKILL.md");

    CandidateEntry {
        key,
        agent_root,
        skills_dir,
        biomcp_dir,
        skill_md,
    }
}

fn candidate_entries(home: &Path, cwd: &Path) -> Vec<CandidateEntry> {
    vec![
        candidate_entry("home-agents", home.join(".agents"), &["skills"]),
        candidate_entry("home-claude", home.join(".claude"), &["skills"]),
        candidate_entry("home-codex", home.join(".codex"), &["skills"]),
        candidate_entry(
            "home-opencode",
            home.join(".config").join("opencode"),
            &["skills"],
        ),
        candidate_entry("home-pi", home.join(".pi"), &["agent", "skills"]),
        candidate_entry("home-gemini", home.join(".gemini"), &["skills"]),
        candidate_entry("cwd-agents", cwd.join(".agents"), &["skills"]),
        candidate_entry("cwd-claude", cwd.join(".claude"), &["skills"]),
    ]
}

fn find_existing_install(candidates: &[CandidateEntry]) -> Option<(PathBuf, Vec<PathBuf>)> {
    let mut primary: Option<PathBuf> = None;
    let mut also_found: Vec<PathBuf> = Vec::new();

    for candidate in candidates {
        if !candidate.skill_md.is_file() {
            continue;
        }
        if primary.is_none() {
            primary = Some(candidate.biomcp_dir.clone());
        } else {
            also_found.push(candidate.biomcp_dir.clone());
        }
    }

    primary.map(|path| (path, also_found))
}

fn skills_dir_has_other_skills(skills_dir: &Path) -> bool {
    if !skills_dir.exists() {
        return false;
    }

    let Ok(entries) = fs::read_dir(skills_dir) else {
        return false;
    };

    entries.flatten().any(|entry| {
        if entry.file_name() == "biomcp" {
            return false;
        }

        entry.file_type().is_ok_and(|kind| kind.is_dir())
    })
}

fn find_best_target(candidates: &[CandidateEntry]) -> Result<(PathBuf, &'static str), BioMcpError> {
    let mut seen_skills_dirs: HashSet<PathBuf> = HashSet::new();
    let mut populated_entries: Vec<&CandidateEntry> = Vec::new();

    for candidate in candidates {
        if !seen_skills_dirs.insert(candidate.skills_dir.clone()) {
            continue;
        }
        if skills_dir_has_other_skills(&candidate.skills_dir) {
            populated_entries.push(candidate);
        }
    }

    if let Some(home_agents) = populated_entries
        .iter()
        .find(|candidate| candidate.key == "home-agents")
    {
        return Ok((
            home_agents.biomcp_dir.clone(),
            "existing skills directory detected",
        ));
    }

    if let Some(first_populated) = populated_entries.first() {
        return Ok((
            first_populated.biomcp_dir.clone(),
            "existing skills directory detected",
        ));
    }

    if let Some(home_agents) = candidates
        .iter()
        .find(|candidate| candidate.key == "home-agents")
        && home_agents.agent_root.exists()
    {
        return Ok((
            home_agents.biomcp_dir.clone(),
            "existing agent root detected",
        ));
    }

    if let Some(home_claude) = candidates
        .iter()
        .find(|candidate| candidate.key == "home-claude")
        && home_claude.agent_root.exists()
    {
        return Ok((
            home_claude.biomcp_dir.clone(),
            "existing agent root detected",
        ));
    }

    if let Some(first_existing_root) = candidates
        .iter()
        .find(|candidate| candidate.agent_root.exists())
    {
        return Ok((
            first_existing_root.biomcp_dir.clone(),
            "existing agent root detected",
        ));
    }

    let home_agents = candidates
        .iter()
        .find(|candidate| candidate.key == "home-agents")
        .ok_or_else(|| {
            BioMcpError::InvalidArgument("Missing home-agents install candidate".into())
        })?;

    Ok((
        home_agents.biomcp_dir.clone(),
        "no existing agent directories found; using cross-tool default",
    ))
}

fn prompt_confirm(path: &Path) -> Result<bool, BioMcpError> {
    let mut stderr = io::stderr();
    write!(
        &mut stderr,
        "Install BioMCP skills to {}? [y/N]: ",
        path.display()
    )
    .map_err(BioMcpError::Io)?;
    stderr.flush().map_err(BioMcpError::Io)?;

    let mut line = String::new();
    io::stdin().read_line(&mut line).map_err(BioMcpError::Io)?;
    let ans = line.trim().to_ascii_lowercase();
    Ok(ans == "y" || ans == "yes")
}

fn write_stderr_line(line: &str) -> Result<(), BioMcpError> {
    let mut stderr = io::stderr();
    writeln!(&mut stderr, "{line}").map_err(BioMcpError::Io)
}

fn install_to_dir(dir: &Path, force: bool) -> Result<String, BioMcpError> {
    let target = dir.to_path_buf();
    let installed_marker = target.join("SKILL.md");
    if installed_marker.exists() && !force {
        return Ok(format!(
            "Skills already installed at {} (use --force to replace)",
            target.display()
        ));
    }

    // Write into a sibling temp directory, then swap into place.
    // This avoids the remove_dir_all + create_dir_all race (EEXIST on
    // macOS) and ensures stale files from older releases are cleaned up.
    let parent = target.parent().ok_or_else(|| {
        BioMcpError::InvalidArgument("Install path has no parent directory".into())
    })?;
    fs::create_dir_all(parent)?;
    let staging = parent.join(".biomcp-install-tmp");
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    fs::create_dir(&staging)?;

    for file in crate::skill_assets::iter() {
        let rel = file.as_ref();
        let Ok(asset) = crate::skill_assets::bytes(rel) else {
            continue;
        };

        let out_path = staging.join(rel);
        if let Some(p) = out_path.parent() {
            fs::create_dir_all(p)?;
        }
        let bytes = if rel == "SKILL.md" {
            canonical_prompt_file_bytes()?
        } else {
            asset.into_owned()
        };
        fs::write(&out_path, bytes)?;
    }

    // Swap: remove old target (if any), rename staging into place.
    if target.exists() {
        fs::remove_dir_all(&target)?;
    }
    fs::rename(&staging, &target)
        .map_err(BioMcpError::Io)
        .or_else(|_| {
            // rename fails across filesystems; fall back to copy + remove.
            copy_dir_all(&staging, &target)?;
            fs::remove_dir_all(&staging).map_err(BioMcpError::Io)
        })?;

    Ok(format!("Installed BioMCP skills to {}", target.display()))
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), BioMcpError> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src).map_err(BioMcpError::Io)? {
        let entry = entry.map_err(BioMcpError::Io)?;
        let dest = dst.join(entry.file_name());
        if entry.file_type().map_err(BioMcpError::Io)?.is_dir() {
            copy_dir_all(&entry.path(), &dest)?;
        } else {
            fs::write(&dest, fs::read(entry.path()).map_err(BioMcpError::Io)?)?;
        }
    }
    Ok(())
}

/// Installs embedded skills into a supported agent directory.
///
/// # Errors
///
/// Returns an error when the destination path is invalid, not writable, or no
/// supported installation directory can be determined.
pub fn install_skills(dir: Option<&str>, force: bool) -> Result<String, BioMcpError> {
    if let Some(dir) = dir {
        let base = expand_tilde(dir)?;
        let target = resolve_install_dir(base);
        return install_to_dir(&target, force);
    }

    let home = expand_tilde("~")?;
    let cwd = std::env::current_dir().map_err(BioMcpError::Io)?;
    let candidates = candidate_entries(&home, &cwd);

    let (target, reason, also_found) =
        if let Some((target, also_found)) = find_existing_install(&candidates) {
            (target, "existing BioMCP skill found", also_found)
        } else {
            let (target, reason) = find_best_target(&candidates)?;
            (target, reason, Vec::new())
        };

    if !also_found.is_empty() {
        let extra = also_found
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        write_stderr_line(&format!("Note: BioMCP skill also found at: {extra}"))?;
    }

    write_stderr_line(&format!("Auto-detected: {} ({reason})", target.display()))?;

    if std::io::stdin().is_terminal() && !prompt_confirm(&target)? {
        return Ok("No installation selected".into());
    }

    install_to_dir(&target, force)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDirGuard;
    use serde_json::Value;

    struct TestPaths {
        _guard: TempDirGuard,
        home: PathBuf,
        cwd: PathBuf,
    }

    impl TestPaths {
        fn new(name: &str) -> Self {
            let guard = TempDirGuard::new(&format!("skill-{name}"));
            let root = guard.path();
            let home = root.join("home");
            let cwd = root.join("cwd");

            fs::create_dir_all(&home).expect("create test home dir");
            fs::create_dir_all(&cwd).expect("create test cwd dir");

            Self {
                _guard: guard,
                home,
                cwd,
            }
        }

        fn create_file(&self, path: &Path) {
            let parent = path.parent().expect("path has parent");
            fs::create_dir_all(parent).expect("create parent dirs");
            fs::write(path, "# test").expect("write test file");
        }
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn read_json_fixture(path: &Path) -> Value {
        let contents = fs::read_to_string(path).expect("read JSON fixture");
        serde_json::from_str(&contents).expect("parse JSON fixture")
    }

    #[test]
    fn embedded_skill_overview_is_routing_first_and_points_to_worked_examples()
    -> Result<(), BioMcpError> {
        let overview = show_overview()?;

        assert!(overview.contains("biomcp suggest \"<question>\""));
        assert!(overview.contains("## Routing rules"));
        assert!(overview.contains("## Section reference"));
        assert!(overview.contains("## Cross-entity pivot rules"));
        assert!(overview.contains("## How-to reference"));
        assert!(overview.contains("## Anti-patterns"));
        assert!(overview.contains("## Output and evidence rules"));
        assert!(overview.contains("## Answer commitment"));
        assert!(overview.contains("biomcp search drug --indication \"<disease>\""));
        assert!(overview.contains("biomcp ema sync"));
        assert!(overview.contains("biomcp who sync"));
        assert!(overview.contains("biomcp cvx sync"));
        assert!(overview.contains("biomcp discover \"<free text>\""));
        assert!(overview.contains("biomcp search article -k \"<query>\" --type review --limit 5"));
        assert!(!overview.contains("../docs/"));
        assert!(!overview.contains(".md)"));
        assert!(overview.contains("Never do more than 3 article searches for one question."));
        assert!(overview.contains("pass `--json --session <token>`"));
        assert!(
            overview.contains("session loop-breaker suggestions with `command` and `reason` only")
        );
        assert!(overview.contains("ClinicalTrials.gov usually does not index nicknames"));
        assert!(overview.contains("add `--drug <name>` to `search article`"));
        assert!(
            overview
                .contains("`biomcp article batch <pmid1> <pmid2> ...` uses spaces between PMIDs.")
        );
        assert!(
            overview.contains(
                "If one command already answers the question, stop searching and answer."
            )
        );
        assert!(
            overview.find("## Cross-entity pivot rules") < overview.find("## How-to reference")
        );
        assert!(overview.find("biomcp suggest \"<question>\"") < overview.find("## Routing rules"));
        assert!(overview.find("biomcp ema sync") < overview.find("## Section reference"));
        assert!(overview.find("biomcp who sync") < overview.find("## Section reference"));
        assert!(overview.find("biomcp cvx sync") < overview.find("## Section reference"));
        assert!(overview.find("## How-to reference") < overview.find("## Anti-patterns"));
        assert!(overview.find("## Anti-patterns") < overview.find("## Output and evidence rules"));
        assert!(
            overview.find("## Output and evidence rules") < overview.find("## Answer commitment")
        );
        assert!(
            overview.find("## Answer commitment")
                < overview.find("Run `biomcp skill list` for worked examples")
        );
        assert!(overview.contains("Run `biomcp skill list` for worked examples"));

        Ok(())
    }

    #[test]
    fn canonical_prompt_body_matches_overview_and_normalizes_newlines() -> Result<(), BioMcpError> {
        let body = canonical_prompt_body()?;
        let rendered = render_system_prompt()?;
        let file_bytes = canonical_prompt_file_bytes()?;

        assert_eq!(show_overview()?, body);
        assert_eq!(rendered, body);
        assert!(!body.ends_with('\n'));
        assert_eq!(file_bytes, format!("{body}\n").into_bytes());
        assert!(file_bytes.ends_with(b"\n"));
        assert!(!file_bytes.ends_with(b"\n\n"));

        Ok(())
    }

    #[test]
    fn install_to_dir_writes_canonical_skill_md_and_assets() -> Result<(), BioMcpError> {
        let paths = TestPaths::new("install-canonical-skill");
        let target = paths.cwd.join("skills/biomcp");

        install_to_dir(&target, true)?;

        assert_eq!(
            fs::read(target.join("SKILL.md"))?,
            canonical_prompt_file_bytes()?
        );
        assert!(target.join("use-cases").is_dir());
        assert!(target.join("jq-examples.md").is_file());
        assert!(target.join("examples").is_dir());
        assert!(target.join("schemas").is_dir());

        Ok(())
    }

    #[test]
    fn validate_skills_target_uses_uv_dev_environment() {
        let makefile = fs::read_to_string(repo_root().join("Makefile")).expect("read Makefile");
        let pyproject =
            fs::read_to_string(repo_root().join("pyproject.toml")).expect("read pyproject");

        assert!(makefile.contains("validate-skills:"));
        assert!(makefile.contains("uv run --extra dev sh -c"));
        assert!(makefile.contains("./scripts/validate-skills.sh"));
        assert!(makefile.contains("PATH=\"$(CURDIR)/target/release:$$PATH\""));
        assert!(pyproject.contains("\"jsonschema>="));
    }

    #[test]
    fn refreshed_search_examples_are_non_empty() {
        let article_path = repo_root().join("skills/examples/search-article.json");
        let article_payload = read_json_fixture(&article_path);
        let article_count = article_payload
            .get("count")
            .and_then(Value::as_u64)
            .expect("article count should be present");
        let article_returned = article_payload
            .pointer("/pagination/returned")
            .and_then(Value::as_u64)
            .expect("article pagination.returned should be present");
        let article_results = article_payload
            .get("results")
            .and_then(Value::as_array)
            .expect("article results should be an array");

        assert!(
            article_count > 0,
            "article example should keep at least one row"
        );
        assert!(
            article_returned > 0,
            "article example should report returned rows"
        );
        assert!(
            !article_results.is_empty(),
            "article example should keep non-empty results"
        );

        let drug_path = repo_root().join("skills/examples/search-drug.json");
        let drug_payload = read_json_fixture(&drug_path);
        let drug_count = drug_payload
            .pointer("/regions/us/count")
            .and_then(Value::as_u64)
            .expect("drug regions.us.count should be present");
        let drug_returned = drug_payload
            .pointer("/regions/us/pagination/returned")
            .and_then(Value::as_u64)
            .expect("drug regions.us.pagination.returned should be present");
        let drug_results = drug_payload
            .pointer("/regions/us/results")
            .and_then(Value::as_array)
            .expect("drug regions.us.results should be an array");

        assert_eq!(
            drug_payload.get("region"),
            Some(&Value::String("us".to_string()))
        );
        assert!(drug_count > 0, "drug example should keep at least one row");
        assert!(
            drug_returned > 0,
            "drug example should report returned rows"
        );
        assert!(
            !drug_results.is_empty(),
            "drug example should keep non-empty results"
        );
    }

    #[test]
    fn embedded_use_case_catalog_lists_expected_worked_examples() -> Result<(), BioMcpError> {
        let refs = list_use_case_refs()?;
        let slugs = refs
            .iter()
            .map(|case| case.slug.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            slugs,
            vec![
                "treatment-lookup",
                "symptom-phenotype",
                "gene-disease-orientation",
                "article-follow-up",
                "variant-pathogenicity",
                "drug-regulatory",
                "gene-function-localization",
                "mechanism-pathway",
                "trial-recruitment",
                "pharmacogene-cumulative",
                "disease-locus-mapping",
                "cellular-process-regulation",
                "mutation-catalog",
                "syndrome-disambiguation",
                "negative-evidence",
            ]
        );

        let listing = list_use_cases()?;
        assert!(listing.contains("# BioMCP Worked Examples"));
        assert!(listing.contains("05 variant-pathogenicity"));
        assert!(listing.contains(
            "15 negative-evidence - Pattern: Negative evidence and no-association checks"
        ));

        let numbered = show_use_case("05")?;
        assert!(numbered.contains("# Pattern: Variant pathogenicity evidence"));

        let mutation = show_use_case("13")?;
        assert!(mutation.contains("# Pattern: Mutation catalog for one gene and disease"));

        Ok(())
    }

    #[test]
    fn embedded_use_case_anchor_commands_parse() -> Result<(), BioMcpError> {
        let cases = use_case_index()?
            .into_iter()
            .filter(|case| {
                case.number
                    .parse::<u32>()
                    .is_ok_and(|number| (5..=15).contains(&number))
            })
            .collect::<Vec<_>>();
        assert_eq!(cases.len(), 11);

        for case in cases {
            let content = embedded_text(&case.embedded_path)?;
            let mut blocks = Vec::new();
            let mut current = String::new();
            let mut in_bash_block = false;

            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed == "```bash" {
                    assert!(
                        !in_bash_block,
                        "{} should not nest fenced bash blocks",
                        case.slug
                    );
                    in_bash_block = true;
                    current.clear();
                    continue;
                }
                if trimmed == "```" && in_bash_block {
                    blocks.push(current.trim_end().to_string());
                    in_bash_block = false;
                    continue;
                }
                if in_bash_block {
                    current.push_str(line);
                    current.push('\n');
                }
            }

            assert!(
                !in_bash_block,
                "{} has an unterminated fenced bash block",
                case.slug
            );
            assert_eq!(blocks.len(), 1, "{} should have one bash block", case.slug);

            let commands = blocks[0]
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>();
            assert!(
                (3..=4).contains(&commands.len()),
                "{} should have 3-4 anchor commands",
                case.slug
            );

            for command in commands {
                assert!(
                    command.starts_with("biomcp "),
                    "{} command should start with biomcp: {command}",
                    case.slug
                );
                for forbidden in [
                    "|",
                    ">",
                    "<",
                    "2>&1",
                    "grep",
                    "cat ",
                    "jq ",
                    "/home/ian/workspace/research/",
                ] {
                    assert!(
                        !command.contains(forbidden),
                        "{} command contains forbidden token {forbidden}: {command}",
                        case.slug
                    );
                }

                let argv = shlex::split(command)
                    .unwrap_or_else(|| panic!("shlex failed for {}: {command}", case.slug));
                crate::cli::try_parse_cli(argv).unwrap_or_else(|err| {
                    panic!("{} command did not parse: {command}: {err}", case.slug)
                });
            }
        }

        Ok(())
    }

    #[test]
    fn missing_skill_suggests_skill_catalog() {
        let err = show_use_case("99").expect_err("missing skill should error");
        let msg = err.to_string();

        assert!(msg.contains("skill '99' not found"));
        assert!(msg.contains("Try: biomcp skill list"));
    }

    #[test]
    fn find_existing_install_detects_claude() {
        let paths = TestPaths::new("existing-claude");
        let skill_md = paths.home.join(".claude/skills/biomcp/SKILL.md");
        paths.create_file(&skill_md);

        let candidates = candidate_entries(&paths.home, &paths.cwd);
        let (target, also_found) =
            find_existing_install(&candidates).expect("expected existing install");

        assert_eq!(target, paths.home.join(".claude/skills/biomcp"));
        assert!(also_found.is_empty());
    }

    #[test]
    fn find_existing_install_prefers_agents_and_reports_others() {
        let paths = TestPaths::new("existing-prefer-agents");
        paths.create_file(&paths.home.join(".agents/skills/biomcp/SKILL.md"));
        paths.create_file(&paths.home.join(".claude/skills/biomcp/SKILL.md"));

        let candidates = candidate_entries(&paths.home, &paths.cwd);
        let (target, also_found) =
            find_existing_install(&candidates).expect("expected existing installs");

        assert_eq!(target, paths.home.join(".agents/skills/biomcp"));
        assert_eq!(also_found, vec![paths.home.join(".claude/skills/biomcp")]);
    }

    #[test]
    fn find_existing_install_ignores_skill_md_directory() -> Result<(), BioMcpError> {
        let paths = TestPaths::new("existing-ignore-directory");
        fs::create_dir_all(paths.home.join(".claude/skills/biomcp/SKILL.md"))?;

        let candidates = candidate_entries(&paths.home, &paths.cwd);
        let existing = find_existing_install(&candidates);

        assert!(existing.is_none());
        Ok(())
    }

    #[test]
    fn find_best_target_prefers_agents_populated_skills_dir() -> Result<(), BioMcpError> {
        let paths = TestPaths::new("best-populated-prefer-agents");
        paths.create_file(&paths.home.join(".agents/skills/example/SKILL.md"));
        paths.create_file(&paths.home.join(".claude/skills/other/SKILL.md"));

        let candidates = candidate_entries(&paths.home, &paths.cwd);
        let (target, reason) = find_best_target(&candidates)?;

        assert_eq!(target, paths.home.join(".agents/skills/biomcp"));
        assert_eq!(reason, "existing skills directory detected");
        Ok(())
    }

    #[test]
    fn find_best_target_ignores_non_skill_files_in_skills_dir() -> Result<(), BioMcpError> {
        let paths = TestPaths::new("best-ignore-non-skill-files");
        paths.create_file(&paths.home.join(".claude/skills/.DS_Store"));
        paths.create_file(&paths.home.join(".codex/skills/example/SKILL.md"));

        let candidates = candidate_entries(&paths.home, &paths.cwd);
        let (target, reason) = find_best_target(&candidates)?;

        assert_eq!(target, paths.home.join(".codex/skills/biomcp"));
        assert_eq!(reason, "existing skills directory detected");
        Ok(())
    }

    #[test]
    fn find_best_target_falls_back_to_agents_root_then_claude_root() -> Result<(), BioMcpError> {
        let agents = TestPaths::new("best-root-agents");
        fs::create_dir_all(agents.home.join(".agents"))?;
        let (agents_target, agents_reason) =
            find_best_target(&candidate_entries(&agents.home, &agents.cwd))?;
        assert_eq!(agents_target, agents.home.join(".agents/skills/biomcp"));
        assert_eq!(agents_reason, "existing agent root detected");

        let claude = TestPaths::new("best-root-claude");
        fs::create_dir_all(claude.home.join(".claude"))?;
        let (claude_target, claude_reason) =
            find_best_target(&candidate_entries(&claude.home, &claude.cwd))?;
        assert_eq!(claude_target, claude.home.join(".claude/skills/biomcp"));
        assert_eq!(claude_reason, "existing agent root detected");

        Ok(())
    }

    #[test]
    fn find_best_target_preserves_pi_agent_skills_path() -> Result<(), BioMcpError> {
        let paths = TestPaths::new("best-pi");
        fs::create_dir_all(paths.home.join(".pi"))?;

        let candidates = candidate_entries(&paths.home, &paths.cwd);
        let (target, reason) = find_best_target(&candidates)?;

        assert_eq!(target, paths.home.join(".pi/agent/skills/biomcp"));
        assert_eq!(reason, "existing agent root detected");
        Ok(())
    }

    #[test]
    fn find_best_target_defaults_to_home_agents_when_nothing_exists() -> Result<(), BioMcpError> {
        let paths = TestPaths::new("best-default");

        let candidates = candidate_entries(&paths.home, &paths.cwd);
        let (target, reason) = find_best_target(&candidates)?;

        assert_eq!(target, paths.home.join(".agents/skills/biomcp"));
        assert_eq!(
            reason,
            "no existing agent directories found; using cross-tool default"
        );
        Ok(())
    }
}
