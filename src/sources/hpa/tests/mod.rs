mod construction;
mod parsing;

const HPA_XML: &str = r#"
<entry>
  <name>BRAF</name>
  <tissueExpression source="HPA" technology="IHC" assayType="tissue">
    <summary type="tissue">Ubiquitous cytoplasmic expression.</summary>
    <verification type="reliability">supported</verification>
    <data>
      <tissue>Adipose tissue</tissue>
      <level type="expression">low</level>
    </data>
    <data>
      <tissue>Liver</tissue>
      <level type="expression">high</level>
    </data>
  </tissueExpression>
  <cellExpression source="HPA" technology="ICC/IF">
    <summary>Mainly localized to vesicles and cytosol.</summary>
    <verification type="reliability">approved</verification>
    <data>
      <location status="additional">plasma membrane</location>
      <location status="main">cytosol</location>
      <location status="main">vesicles</location>
      <location status="additional">plasma membrane</location>
    </data>
  </cellExpression>
  <rnaExpression source="HPA" technology="RNAseq" assayType="consensusTissue">
    <rnaSpecificity specificity="Low tissue specificity" />
    <rnaDistribution>Detected in all</rnaDistribution>
  </rnaExpression>
  <antibody>
    <tissueExpression source="HPA" technology="IHC" assayType="tissue">
      <data>
        <tissue>Artifact tissue</tissue>
        <level type="expression">medium</level>
      </data>
    </tissueExpression>
  </antibody>
</entry>
"#;
