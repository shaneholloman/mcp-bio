# Trial Action Summary Surface

Rare-disease trial action summaries are explicit because they fetch full records
and render practical triage detail. Ordinary `search trial` stays compact; users
opt in when they need listed-site contacts, eligibility, caveats, and site
ranking from ClinicalTrials.gov data.

## Opt-in rare-disease action summaries fetch full listed-site detail

The action summary should run the rare-disease trial search plan, dedupe NCT IDs,
hydrate the selected candidates with full CTGov detail, and explain practical
next-step caveats without inventing unlisted sites.

```bash
../../tools/biomcp-ci search trial -c "Phelan-McDermid Syndrome" --action-summary --facility "University of Michigan" --limit 5 | mustmatch like '# Trial Action Summaries
NCT41700001
Open-label extension
Antecedent study required
Central Coordinator
central-action@example.test
Sex: All
Eligible Ages: 2 Years to 18 Years
Rare Disease Center
Ann Arbor
No listed CTGov site matched: University of Michigan'
```

## Action-summary JSON exposes structured fields for agents

Agents need machine-readable caveats and site matching, not only prose. JSON
mode should expose the same actionability as stable fields on each result.

```bash
../../tools/biomcp-ci --json search trial -c "Phelan-McDermid Syndrome" --action-summary --facility "University of Michigan" --limit 5 \
  | jq -r '.results[0].trial_type, .results[0].access_caveats[0].kind, .results[0].ranked_sites[0].match_status, .results[0].contacts[0].email, .results[0].eligibility.sex' \
  | mustmatch like 'open_label_extension
antecedent_study_required
no_listed_facility_match
central-action@example.test
All'
```

## Help, list, and user docs teach the opt-in action-summary mode

The command, list page, and user guide should teach the same opt-in surface and
its listed-sites-only limit so users do not mistake a summary for patient
matching or inferred site availability.

```bash
../../tools/biomcp-ci search trial --help | mustmatch like '--action-summary
listed CTGov sites
trial_type
access_caveats
ranked_sites'
```

```bash
../../tools/biomcp-ci list trial | mustmatch like '--action-summary
listed CTGov sites
trial_type
access_caveats
ranked_sites'
```

```bash
grep -h "action-summary\|listed CTGov sites\|trial_type\|access_caveats\|ranked_sites" ../../docs/user-guide/trial.md | mustmatch like '--action-summary
listed CTGov sites
trial_type
access_caveats
ranked_sites'
```
