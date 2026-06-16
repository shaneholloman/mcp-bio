use std::path::Path;

mod construction;
mod parsing;

fn write_fixture_bundle(root: &Path) {
    std::fs::write(
        root.join("cvx.txt"),
        "03|MMR|measles, mumps and rubella virus vaccine, live||Active|False|2020/06/02\n94|MMRV|measles, mumps, rubella, and varicella vaccine, live||Active|False|2020/06/02\n62|HPV, quadrivalent|human papilloma virus vaccine, quadrivalent||Active|False|2020/06/02\n165|HPV9|Human Papillomavirus 9-valent vaccine||Active|False|2014/12/11\n133|Pneumococcal conjugate PCV 13|pneumococcal conjugate vaccine, 13 valent||Active|False|2010/05/28\n140|Influenza, split virus, trivalent, PF|Influenza, split virus, trivalent, injectable, preservative free||Active|False|2024/05/02\n141|Influenza, split virus, trivalent, preservative|Influenza, split virus, trivalent, injectable, contains preservative||Active|False|2024/05/02\n208|COVID-19, mRNA, LNP-S, PF, 30 mcg/0.3 mL dose|SARS-COV-2 (COVID-19) vaccine, mRNA, spike protein, LNP, preservative free, 30 mcg/0.3mL dose||Inactive|False|2023/11/14\n217|COVID-19, mRNA, LNP-S, PF, 30 mcg/0.3 mL dose, tris-sucrose|SARS-COV-2 (COVID-19) vaccine, mRNA, spike protein, LNP, preservative free, 30 mcg/0.3mL dose, tris-sucrose formulation||Inactive|False|2023/11/02\n27|botulinum antitoxin|botulinum antitoxin||Active|True|2020/09/04\n",
    )
    .expect("write cvx fixture");
    std::fs::write(
        root.join("TRADENAME.txt"),
        "M-M-R II|MMR|03|Merck and Co., Inc.|MSD|Active|Active|2020/06/02|\nProQuad|MMRV|94|Merck and Co., Inc.|MSD|Active|Active|2020/06/02|\nGARDASIL|HPV, quadrivalent|62|Merck and Co., Inc.|MSD|Active|Inactive|2010/05/28|\nGardasil 9|HPV9|165|Merck and Co., Inc.|MSD|Active|Active|2014/12/11|\nCOMIRNATY|COVID-19, mRNA, LNP-S, PF, 30 mcg/0.3 mL dose|208|Pfizer, Inc|PFR|Active|Active|2023/09/06|\nCOMIRNATY|COVID-19, mRNA, LNP-S, PF, 30 mcg/0.3 mL dose, tris-sucrose|217|Pfizer, Inc|PFR|Active|Active|2023/09/06|\nPREVNAR 13|Pneumococcal conjugate PCV 13|133|Pfizer, Inc|PFR|Active|Active|2010/05/28|\nPREVNAR 13|Pneumococcal conjugate PCV 13|133|Wyeth|WAL|Active|Inactive|2010/05/28|\nFluzone trivalent, preservative free|Influenza, split virus, trivalent, PF|140|Sanofi Pasteur|PMC|Active|Active|2024/05/17|\nFluzone trivalent, with preservative|Influenza, split virus, trivalent, preservative|141|Sanofi Pasteur|PMC|Active|Active|2024/05/14|\nNEVERMATCH|botulinum antitoxin|27|Nobody|ZZZ|Active|Active|2020/09/04|\n",
    )
    .expect("write tradename fixture");
    std::fs::write(
        root.join("mvx.txt"),
        "MSD|Merck and Co., Inc.||Active|2012/10/18\nPMC|Sanofi Pasteur||Active|2026/04/14\nWAL|Wyeth|acquired by Pfizer 10/15/2009|Active|2010/05/28\nPFR|Pfizer, Inc|COVID-19 vaccine in co-development with BioNTech|Active|2020/10/30\n",
    )
    .expect("write mvx fixture");
}
