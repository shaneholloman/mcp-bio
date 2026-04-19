# Explore: HPO Phenotype Enrichment for Clinical Symptoms

## Spike Question

What source architecture should BioMCP use to enrich disease phenotype coverage
for clinical symptoms beyond the existing HPO-backed disease phenotype section?

Success means identifying sources, mapping strategy, integration shape, cross
entity linkages, sizing, and a small-scale measurement on uterine fibroids plus
at least two more common diseases where current HPO coverage is sparse.

## Prior Art Summary

Current BioMCP behavior:
- `get disease <id> phenotypes` resolves the disease through MyDisease, maps
  MyDisease `hpo.phenotype_related_to_disease` rows into `DiseasePhenotype`,
  augments with Monarch disease-phenotype associations, then resolves HPO labels
  and frequency labels through the JAX HPO API.
- The public row model is HPO-first: `DiseasePhenotype` requires `hpo_id` and
  carries optional name, evidence, frequency, onset, sex, stage, qualifiers, and
  source.
- Deduplication is normalized HPO ID based.
- The markdown renderer already warns that phenotype output is source-backed
  and may be incomplete, then suggests review literature.
- `search phenotype` is already the reverse phenotype-to-disease workflow:
  HPO IDs or symptom phrases resolve to HPO terms and then Monarch semantic
  similarity search ranks diseases.
- OpenTargets already has GraphQL parsing for disease HPO phenotype evidence,
  but the current client routes it into prevalence context rather than disease
  phenotype rows.

Prior-art decision:
- Reuse `get disease <id> phenotypes` as the entry point.
- Reuse HPO IDs as the primary normalized dedupe key where available.
- Reuse the optional enrichment pattern and incomplete-coverage warning.
- Change the data model before adding broad sources: source-native clinical
  symptom text cannot honestly fit a required `hpo_id: String` row without
  extraction and confidence-scored HPO mapping.

## Approaches Tried

Small-scale diseases:
- Uterine fibroid / uterine leiomyoma (`ICD10CM:D25` in the ticket; resolved by
  BioMCP as `MONDO:0007886`).
- Endometriosis.
- Chronic venous insufficiency / venous leg ulceration.

Result files:
- `results/current_biomcp_hpo_baseline.json`
- `results/curated_source_landscape.json`
- `results/wikidata_p780_probe.json`
- `results/clinical_summary_medlineplus_probe.json`

Scripts:
- `scripts/baseline_biomcp_hpo.py`
- `scripts/curated_source_landscape.py`
- `scripts/wikidata_p780_probe.py`
- `scripts/clinical_summary_medlineplus_probe.py`
- `scripts/run_all_probes.py`

### Approach 1: Current HPO/Monarch Baseline

Method:
- Ran `biomcp --json get disease <query> phenotypes` for the three diseases.
- Counted phenotype rows and lexical overlap against a manually scoped clinical
  symptom set for each disease.

Measurements:
- Total phenotype rows: 2 across 3 diseases.
- Uterine fibroid returned `Uterine leiomyoma` and `Typified by somatic
  mosaicism`.
- Endometriosis returned 0 phenotype rows.
- Chronic venous insufficiency returned 0 phenotype rows.
- Expected clinical symptom recall: 0/23 = 0.000.

Interpretation:
- This reproduces the ticket's core failure mode. Current HPO/Monarch rows are
  curated and normalized but too sparse for common clinical symptom questions.

### Approach 2: Curated Source Landscape

Method:
- Probed OpenTargets HPO phenotype GraphQL, MedGen E-utilities, Orphanet/ORDO
  via OLS, OMIM API availability, Disease Ontology via OLS, and SNOMED CT
  browser search.
- Measured small-scale disease resolution and whether a source exposed direct
  symptom/phenotype rows.

Measurements:
- OpenTargets HPO had phenotype rows for 1/3 diseases, only uterine fibroid.
- MedGen resolved 3/3 diseases but exposed concept/xref summaries, not a symptom
  table.
- SNOMED CT search resolved 3/3 diseases, but useful relations are terminology
  relations such as finding site/morphology, not disease symptom lists.
- Disease Ontology resolved 1/3 and did not expose symptom annotations.
- Orphanet/ORDO resolved 0/3 common-disease sample entries.
- OMIM was not measurable without `OMIM_API_KEY`; clinical synopsis is likely
  valuable for Mendelian diseases but has credential/licensing friction.

Interpretation:
- Curated sources are useful for identity, xrefs, rare disease, or frequency
  evidence, but they do not solve common-disease clinical symptom coverage at
  this scale.

### Approach 3: Wikidata P780

Method:
- Queried Wikidata SPARQL for exact English disease labels and `symptoms and
  signs` (`P780`) values.
- Also recorded identifiers/xrefs where present.

Measurements:
- Disease entities resolved for uterine fibroid, endometriosis, and chronic
  venous insufficiency related labels.
- `P780` symptom assertions: 0 across all 3 diseases.
- Expected clinical symptom recall: 0/23 = 0.000.

Interpretation:
- Wikidata has useful xrefs for these diseases but `P780` is not populated
  enough to drive enrichment for this sample. Do not make it an early source.

### Approach 4: Source-Native Clinical Summary

Method:
- Queried MedlinePlus health topic search XML for the disease query variants.
- Measured lexical symptom recall against the same expected symptom sets.
- This intentionally measured source-native plain-language text, not normalized
  HPO rows.

Measurements:
- Topics found for 3/3 diseases.
- Total expected symptom recall: 14/23 = 0.609.
- Uterine fibroid recall: 0.625.
- Endometriosis recall: 0.857.
- Chronic venous insufficiency recall: 0.375.
- Noise observed: related topics such as hysterectomy, ectopic pregnancy, and
  deep vein thrombosis appeared alongside direct disease topics.

Interpretation:
- Source-native clinical summaries are the only measured approach that filled a
  meaningful part of the symptom gap. They need disease-page selection,
  section-aware extraction, and HPO mapping before inclusion in structured
  output.

## Source Comparison

| Source | Small-scale coverage | Quality tier | API availability | Refresh cadence | Recommendation |
|---|---:|---|---|---|---|
| Current HPO/Monarch | 2 rows total; 0/23 symptom recall | Curated normalized HPO | Public BioMCP-supported APIs | Upstream HPO/Monarch releases | Keep as tier 1 normalized baseline |
| OpenTargets HPO phenotypes | Rows for 1/3 diseases | Curated/aggregated HPO evidence | Public GraphQL | Versioned Open Targets releases | Reuse later for HPO frequency/evidence, not primary symptom fill |
| MedGen | Disease resolution 3/3 | Curated NCBI concept aggregation | Public E-utilities | NCBI Entrez updates | Use for xrefs/disambiguation only |
| Orphanet/ORDO | 0/3 common diseases via OLS | Curated rare-disease data | OLS plus Orphadata downloads | Dated Orphadata releases | Defer to rare-disease enrichment ticket |
| OMIM Clinical Synopsis | Not measured without key | Curated clinical synopsis | API key and OMIM terms required | OMIM updated daily | Defer until licensing/key decision; high value for Mendelian diseases |
| Disease Ontology | Resolution 1/3 | Curated disease classification | Public OLS | Monthly DO releases; OBO daily from GitHub | Use identity/xrefs, not symptoms |
| Wikidata P780 | Disease xrefs resolved; 0 P780 rows | Community assertions | Public SPARQL | Continuous community edits | Do not integrate early |
| SNOMED CT | Search resolution 3/3 | Licensed clinical terminology | Browser API exists; production licensing required | International Edition monthly releases | Use later for terminology mapping, not direct symptom lists |
| MedlinePlus summaries | 3/3 topics; 14/23 symptom recall | NLM plain-language clinical summaries | Public XML search API | XML updated Tuesday-Saturday | Best measured coverage; promote as source-native clinical feature input |

## Recommended Approach

Pivot from "add another disease-phenotype table source" to a two-tier phenotype
architecture:

1. Keep the existing HPO/Monarch section as `curated_hpo_phenotypes`.
2. Add an enriched `clinical_features` section under `get disease <id>
   phenotypes` for source-native symptom phrases.
3. Feed `clinical_features` first from MedlinePlus health topic pages because it
   was the only source that improved clinical symptom recall in the small-scale
   probe.
4. Map extracted features to HPO where possible, but keep source-native terms
   when mapping confidence is low.
5. Treat OpenTargets/MedGen/DO/SNOMED as supporting identity, xref, frequency,
   and mapping sources rather than first-order symptom sources.
6. Defer OMIM and Orphanet until licensing/rare-disease scope is explicit.
7. Do not use Wikidata P780 as an early source for common-disease symptom
   enrichment.

## Draft Output Schema

Recommended JSON shape:

```json
{
  "phenotypes": [
    {
      "hpo_id": "HP:0000131",
      "name": "Uterine leiomyoma",
      "evidence": "PCS",
      "frequency": null,
      "source": "infores:omim",
      "evidence_tier": "curated_hpo"
    }
  ],
  "clinical_features": [
    {
      "label": "Heavy or painful periods",
      "feature_type": "symptom",
      "normalized_hpo_id": null,
      "normalized_hpo_label": null,
      "mapping_confidence": 0.0,
      "source": "MedlinePlus",
      "source_url": "https://medlineplus.gov/uterinefibroids.html",
      "source_native_id": "uterinefibroids",
      "evidence_tier": "clinical_summary",
      "evidence_text": "Symptoms may include: Heavy or painful periods...",
      "frequency": null,
      "body_system": "reproductive",
      "rank": 1
    }
  ],
  "phenotype_coverage": {
    "curated_hpo_count": 2,
    "clinical_feature_count": 8,
    "mapped_feature_count": 5,
    "unmapped_feature_count": 3,
    "coverage_note": "Clinical features include source-native terms and may not all have high-confidence HPO mappings."
  }
}
```

Markdown shape:
- Keep the current HPO table.
- Add a `Clinical Features` table with columns: Feature, HPO mapping, Source,
  Tier, Evidence.
- Keep the incompleteness note, but update it to distinguish curated HPO rows
  from source-native clinical features.

Ranking and dedupe:
- Deduplicate mapped rows by HPO ID.
- Deduplicate unmapped rows by normalized label plus disease ID plus source.
- Prefer direct disease pages over related topic pages.
- Rank curated HPO first for ontology workflows; rank clinical features first
  for symptom-question summaries.
- Preserve source-native evidence text for auditability.

## Cross-Entity Linkage Map

| Linkage | Current state | Enriched design |
|---|---|---|
| Disease -> phenotype | Existing HPO/Monarch section | Keep; add clinical feature rows and coverage metadata |
| Phenotype -> disease | Existing `search phenotype` HPO/Monarch semsim | Add optional reverse lookup over mapped clinical features only after mappings are validated |
| Disease -> article | Existing fallback suggestion | Use articles/reviews as exploit validation, not first spike source |
| Disease -> MedlinePlus topic | Exists indirectly through discover source code | Promote direct disease-page selection for symptom extraction |
| Gene -> phenotype | Via gene-disease-phenotype chain | Use only mapped HPO features for gene phenotype pivots |
| Drug -> symptoms/side effects | Not same as disease phenotypes | Keep separate evidence tier and label drug AE rows distinctly to avoid symptom/side-effect conflation |
| SNOMED/MedGen/DO xrefs | Identity support | Use for disambiguation and mapping provenance, not displayed symptom rows |

## Decision

Winner: source-native clinical summary extraction with HPO mapping, using the
existing HPO/Monarch section as the curated baseline.

Why:
- Current HPO/Monarch measured 0.000 symptom recall on the small common-disease
  sample.
- Wikidata P780 also measured 0.000 recall and zero symptom assertions.
- Curated ontology/terminology sources mostly resolved disease identity but did
  not expose direct symptom lists for the sample.
- MedlinePlus source-native summaries measured 0.609 recall and covered all 3
  diseases, but require page selection, extraction, and normalization to avoid
  noisy related-topic symptoms.

## Outcome

`pivot`

Do not exploit a direct Wikidata/MedGen/DO/SNOMED-to-HPO enrichment. Pivot to a
clinical feature enrichment design that keeps curated HPO rows separate from
source-native clinical symptoms and maps the latter to HPO opportunistically.

## Sizing Estimate

Multi-ticket effort.

Suggested build slices:
- Ticket 1: Add `clinical_features` schema, renderer section, and tests with
  fixture-only MedlinePlus extraction.
- Ticket 2: Implement disease-page selection and section-aware symptom
  extraction from MedlinePlus, including provenance and noise controls.
- Ticket 3: Add HPO mapping for extracted source-native features using the
  existing JAX HPO search plus confidence thresholds and manual allow/deny
  fixtures for common symptoms.
- Ticket 4: Add reverse lookup or `search disease --clinical-feature` only after
  enough mapped rows are validated.
- Optional ticket: OMIM/Orphanet licensing and rare-disease curated synopsis
  enrichment.

## Risks for Exploit

- Source-native summaries are text, not curated phenotype rows. Extraction must
  be section-aware and auditable.
- MedlinePlus search returns related topics; exploit must select direct disease
  pages before extracting symptoms.
- HPO mapping can be lossy. The output must keep source-native terms when
  confidence is low instead of forcing a false HPO ID.
- Common symptoms overlap with drug adverse events. Drug side effects and
  disease symptoms need separate source tiers.
- OMIM and SNOMED licensing can affect production eligibility.
- Evaluation needs a gold symptom set larger than the 3-disease spike sample.

