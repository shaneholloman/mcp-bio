# Source Versioning Matrix

This matrix tracks which upstream API endpoints are version-pinned and where unversioned endpoints remain intentional.

| Source | Base URL | Version status | Rationale | Last reviewed |
|---|---|---|---|---|
| AlphaGenome | `https://gdmscience.googleapis.com:443` | Unversioned | Google endpoint is service-versioned server-side; no stable path segment exposed | 2026-02-15 |
| cBioPortal | `https://www.cbioportal.org/api` | Unversioned | Public API path is stable without explicit version segment | 2026-02-15 |
| CDC WONDER VAERS | `https://wonder.cdc.gov/controller/datarequest/D8` | Unversioned | CDC WONDER exposes the VAERS D8 database through a stable dataset ID and XML POST contract; BioMCP freezes the request/response shape in fixtures instead of relying on a versioned path segment | 2026-04-18 |
| ChEMBL | `https://www.ebi.ac.uk/chembl/api/data` | Unversioned | ChEMBL data API is stable at `/api/data`; no URL version convention | 2026-02-15 |
| ClinicalTrials.gov | `https://clinicaltrials.gov/api/v2` | Versioned (`v2`) | Endpoint already pinned to public v2 API | 2026-02-15 |
| Enrichr | `https://maayanlab.cloud/Enrichr` | Unversioned | Service does not publish versioned path variant for current API | 2026-02-15 |
| Europe PMC | `https://www.ebi.ac.uk/europepmc/webservices/rest` | Unversioned | REST root is stable and not versioned in URL | 2026-02-15 |
| gnomAD GraphQL | `https://gnomad.broadinstitute.org/api` | Unversioned | Versioning is dataset-level (`gnomad_r4`, `gnomad_r3`, `gnomad_r2_1`) in query payload | 2026-02-15 |
| g:Profiler | `https://biit.cs.ut.ee/gprofiler/api` | Unversioned | Public endpoint does not expose version path segment | 2026-02-15 |
| HPO JAX API | `https://ontology.jax.org/api/hp` | Unversioned | API path is canonical and currently unversioned | 2026-02-15 |
| InterPro | `https://www.ebi.ac.uk/interpro/api` | Unversioned | Public endpoint has no URL versioning model | 2026-02-15 |
| LitSense2 | `https://www.ncbi.nlm.nih.gov/research/litsense2-api/api` | Versioned-by-product (`litsense2-api`) | Version identity is in the product namespace rather than the path suffix | 2026-04-10 |
| MyChem.info | `https://mychem.info/v1` | Versioned (`v1`) | Endpoint already pinned | 2026-02-15 |
| MyDisease.info | `https://mydisease.info/v1` | Versioned (`v1`) | Endpoint already pinned | 2026-02-15 |
| MyGene.info | `https://mygene.info/v3` | Versioned (`v3`) | Endpoint already pinned | 2026-02-15 |
| MyVariant.info | `https://myvariant.info/v1` | Versioned (`v1`) | Endpoint already pinned | 2026-02-15 |
| NCBI Genetic Testing Registry | `https://ftp.ncbi.nlm.nih.gov/pub/GTR/data` | Unversioned bulk export | GTR bulk files are published at stable filenames and refreshed in place rather than by versioned path | 2026-04-17 |
| NCBI ID Converter | `https://pmc.ncbi.nlm.nih.gov/tools/idconv/api/v1/articles` | Versioned (`v1`) | Endpoint already pinned | 2026-02-15 |
| NCI CTS | `https://clinicaltrialsapi.cancer.gov/api/v2` | Versioned (`v2`) | Endpoint already pinned | 2026-02-15 |
| NIH Reporter | `https://api.reporter.nih.gov/v2` | Versioned (`v2`) | Endpoint already pinned to the public NIH Reporter v2 project search API | 2026-04-11 |
| OncoKB (prod/demo) | `https://www.oncokb.org/api/v1` / `https://demo.oncokb.org/api/v1` | Versioned (`v1`) | Endpoint already pinned | 2026-02-15 |
| OpenFDA | `https://api.fda.gov` | Unversioned | Public OpenFDA API is path-stable without version segment | 2026-02-15 |
| OpenTargets | `https://api.platform.opentargets.org/api/v4/graphql` | Versioned (`v4`) | Endpoint already pinned | 2026-02-15 |
| PMC OA | `https://www.ncbi.nlm.nih.gov/pmc/utils/oa/oa.fcgi` | Unversioned | Legacy utility endpoint; no version path available | 2026-02-15 |
| PubMed | `https://eutils.ncbi.nlm.nih.gov/entrez/eutils` | Unversioned | PubMed search still uses legacy E-utilities endpoints without explicit path versioning | 2026-04-10 |
| PubTator3 | `https://www.ncbi.nlm.nih.gov/research/pubtator3-api` | Versioned-by-product (`pubtator3`) | Version identity is in product namespace | 2026-02-15 |
| QuickGO | `https://www.ebi.ac.uk/QuickGO/services` | Unversioned | Service endpoint is canonical and not path-versioned | 2026-02-15 |
| Reactome Content Service | `https://reactome.org/ContentService` | Unversioned | No explicit major version path in public endpoint | 2026-02-15 |
| SEER Explorer | `https://seer.cancer.gov/statistics-network/explorer/source/content_writers` | Unversioned | Undocumented PHP endpoints have no stable version segment; BioMCP validates requested site codes and decoded payload structure | 2026-04-10 |
| Semantic Scholar | `https://api.semanticscholar.org` | Unversioned | Public API base is stable without a version segment; endpoint versions live below the base path | 2026-03-15 |
| STRING | `https://string-db.org/api` | Unversioned | API route uses format path segment; no stable version URL segment | 2026-02-15 |
| UniProt REST | `https://rest.uniprot.org` | Unversioned | REST base is canonical and not versioned in URL | 2026-02-15 |

## Notes

- If a provider introduces a stable version path, update the corresponding `src/sources/*.rs` base constant and this table in the same change.
- gnomAD versioning is handled by dataset selection in GraphQL variables and is verified by dataset fallback tests.
