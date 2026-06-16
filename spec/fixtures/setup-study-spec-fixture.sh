#!/usr/bin/env bash
set -euo pipefail

repo_root="${1:-$PWD}"
cache_dir="$repo_root/.cache"
study_root="$cache_dir/spec-study-datasets"

rm -rf "$study_root"
mkdir -p \
  "$study_root/msk_impact_2017" \
  "$study_root/brca_tcga_pan_can_atlas_2018" \
  "$study_root/paad_qcmg_uq_2016" \
  "$cache_dir"

cat >"$study_root/msk_impact_2017/meta_study.txt" <<'EOF'
cancer_study_identifier: msk_impact_2017
name: MSK-IMPACT 2017
type_of_cancer: mixed
citation: Demo citation
EOF

cat >"$study_root/msk_impact_2017/data_mutations.txt" <<'EOF'
Hugo_Symbol	Tumor_Sample_Barcode	Variant_Classification	HGVSp_Short
TP53	S1	Missense_Mutation	p.R175H
TP53	S2	Missense_Mutation	p.R248Q
KRAS	S1	Missense_Mutation	p.G12D
KRAS	S3	Missense_Mutation	p.G12V
EOF

cat >"$study_root/msk_impact_2017/data_sv.txt" <<'EOF'
Sample_Id	SV_Status	Site1_Hugo_Symbol	Site1_Chromosome	Site1_Position	Site2_Hugo_Symbol	Site2_Chromosome	Site2_Position	Site2_Effect_On_Frame	Tumor_Split_Read_Count	Normal_Split_Read_Count	Event_Info
S1	SOMATIC	KIF5B	10	32315678	RET	10	43612032	in-frame	12	0	KIF5B-RET Fusion
S2	SOMATIC	ETV6	12	11869969	NTRK3	15	88420184	in-frame	8	0	ETV6-NTRK3 Fusion
EOF

cat >"$study_root/msk_impact_2017/data_clinical_sample.txt" <<'EOF'
# comment
PATIENT_ID	SAMPLE_ID	CANCER_TYPE	CANCER_TYPE_DETAILED	ONCOTREE_CODE
P1	S1	Lung Cancer	Lung Adenocarcinoma	LUAD
P2	S2	Lung Cancer	Lung Adenocarcinoma	LUAD
P3	S3	Lung Cancer	Lung Adenocarcinoma	LUAD
EOF

cat >"$study_root/brca_tcga_pan_can_atlas_2018/meta_study.txt" <<'EOF'
cancer_study_identifier: brca_tcga_pan_can_atlas_2018
name: BRCA TCGA PanCan Atlas 2018
type_of_cancer: brca
citation: Demo citation
EOF

cat >"$study_root/brca_tcga_pan_can_atlas_2018/data_mutations.txt" <<'EOF'
Hugo_Symbol	Tumor_Sample_Barcode	Variant_Classification	HGVSp_Short
TP53	S1	Missense_Mutation	p.R175H
PIK3CA	S2	Missense_Mutation	p.H1047R
EOF

cat >"$study_root/brca_tcga_pan_can_atlas_2018/data_clinical_sample.txt" <<'EOF'
# comment
PATIENT_ID	SAMPLE_ID	CANCER_TYPE	CANCER_TYPE_DETAILED	ONCOTREE_CODE
P1	S1	Breast Cancer	Breast Invasive Carcinoma	BRCA
P1	S3	Breast Cancer	Breast Invasive Carcinoma	BRCA
P2	S2	Breast Cancer	Breast Invasive Carcinoma	BRCA
EOF

cat >"$study_root/brca_tcga_pan_can_atlas_2018/data_clinical_patient.txt" <<'EOF'
# comment
PATIENT_ID	OS_STATUS	OS_MONTHS	DFS_STATUS	DFS_MONTHS	PFS_STATUS	PFS_MONTHS	DSS_STATUS	DSS_MONTHS
P1	1:DECEASED	12	1:Recurred	8	1:Progressed	7	1:Died of disease	12
P2	0:LIVING	24	0:DiseaseFree	20	0:No progression	18	0:Alive	24
EOF

cat >"$study_root/brca_tcga_pan_can_atlas_2018/data_cna.txt" <<'EOF'
Hugo_Symbol	Entrez_Gene_Id	S1	S2	S3
ERBB2	2064	2	0	1
EOF

cat >"$study_root/brca_tcga_pan_can_atlas_2018/data_mrna_seq_v2_rsem_zscores_ref_all_samples.txt" <<'EOF'
Hugo_Symbol	Entrez_Gene_Id	S1	S2	S3
TP53	7157	3.0	1.0	2.0
ERBB2	2064	2.0	1.0	4.0
EOF

cat >"$study_root/paad_qcmg_uq_2016/meta_study.txt" <<'EOF'
cancer_study_identifier: paad_qcmg_uq_2016
name: PAAD QCMG UQ 2016
type_of_cancer: paad
citation: Demo citation
EOF

cat >"$study_root/paad_qcmg_uq_2016/data_mutations.txt" <<'EOF'
Hugo_Symbol	Tumor_Sample_Barcode	Variant_Classification	HGVSp_Short
KRAS	S1	Missense_Mutation	p.G12D
EOF

cat >"$study_root/paad_qcmg_uq_2016/data_clinical_sample.txt" <<'EOF'
# comment
PATIENT_ID	SAMPLE_ID	CANCER_TYPE	CANCER_TYPE_DETAILED	ONCOTREE_CODE
P1	S1	Pancreatic Cancer	Pancreatic Adenocarcinoma	PAAD
EOF

cat >"$study_root/paad_qcmg_uq_2016/data_clinical_patient.txt" <<'EOF'
# comment
PATIENT_ID	DFS_STATUS	DFS_MONTHS
P1	1:Recurred	10
EOF

cat >"$study_root/paad_qcmg_uq_2016/data_mrna_seq_v2_rsem_zscores_ref_all_samples.txt" <<'EOF'
Hugo_Symbol	Entrez_Gene_Id	S1
KRAS	3845	1.5
EOF

printf 'export BIOMCP_STUDY_DIR=%q\n' "$study_root" > "$cache_dir/spec-study-env"
