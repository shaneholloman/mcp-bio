use super::*;

mod construction;
mod parsing;

fn topic_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<nlmSearchResult>
  <list num="1" start="0" per="1">
    <document rank="0" url="https://medlineplus.gov/chestpain.html">
      <content name="title">Chest Pain</content>
      <content name="FullSummary">Summary</content>
    </document>
  </list>
</nlmSearchResult>"#
}

fn marked_up_topic_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<nlmSearchResult>
  <list num="1" start="0" per="1">
    <document rank="0" url="https://medlineplus.gov/chestpain.html">
      <content name="title">&lt;span class="qt0"&gt;Chest&lt;/span&gt; Pain</content>
      <content name="FullSummary">&lt;p&gt;Chest pain summary.&lt;/p&gt;</content>
    </document>
  </list>
</nlmSearchResult>"#
}
