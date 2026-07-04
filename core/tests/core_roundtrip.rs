use paimon_ftindex_core::io::{PosWriter, SliceReader};
use paimon_ftindex_core::{
    FullTextIndexConfig, FullTextIndexReader, FullTextIndexWriter, FullTextQuery, MatchOperator,
};

fn build_index() -> anyhow::Result<Vec<u8>> {
    let mut writer = FullTextIndexWriter::new(FullTextIndexConfig::new())?;
    writer.add_document(10, "Apache Paimon supports full text search")?;
    writer.add_document(11, "Tantivy is a Rust search engine")?;
    writer.add_document(12, "Paimon tables can use indexes")?;

    let mut bytes = Vec::new();
    {
        let mut output = PosWriter::new(&mut bytes);
        writer.write(&mut output)?;
    }
    Ok(bytes)
}

#[test]
fn match_query_round_trip() -> anyhow::Result<()> {
    let bytes = build_index()?;
    let mut reader = FullTextIndexReader::open(SliceReader::new(bytes))?;
    let result = reader.search(FullTextQuery::match_query("paimon", "text"), 10)?;

    assert_eq!(reader.metadata().document_count, 3);
    assert_eq!(result.row_ids.len(), 2);
    assert!(result.row_ids.contains(&10));
    assert!(result.row_ids.contains(&12));
    assert_eq!(result.scores.len(), 2);
    Ok(())
}

#[test]
fn match_query_and_operator_filters_terms() -> anyhow::Result<()> {
    let bytes = build_index()?;
    let mut reader = FullTextIndexReader::open(SliceReader::new(bytes))?;
    let query = FullTextQuery::Match {
        column: "text".to_string(),
        terms: "paimon indexes".to_string(),
        operator: MatchOperator::And,
        boost: 1.0,
    };
    let result = reader.search(query, 10)?;

    assert_eq!(result.row_ids, vec![12]);
    Ok(())
}

#[test]
fn phrase_query_uses_positions() -> anyhow::Result<()> {
    let bytes = build_index()?;
    let mut reader = FullTextIndexReader::open(SliceReader::new(bytes))?;
    let result = reader.search(FullTextQuery::phrase("full text", "text"), 10)?;

    assert_eq!(result.row_ids, vec![10]);
    Ok(())
}

#[test]
fn boost_query_requires_positive_match() -> anyhow::Result<()> {
    let mut writer = FullTextIndexWriter::new(FullTextIndexConfig::new())?;
    writer.add_document(1, "apache paimon")?;
    writer.add_document(2, "tantivy only")?;

    let mut bytes = Vec::new();
    writer.write(&mut PosWriter::new(&mut bytes))?;

    let mut reader = FullTextIndexReader::open(SliceReader::new(bytes))?;
    let query = FullTextQuery::Boost {
        positive: Box::new(FullTextQuery::match_query("paimon", "text")),
        negative: Box::new(FullTextQuery::match_query("tantivy", "text")),
        negative_boost: 0.5,
    };
    let result = reader.search(query, 10)?;

    assert_eq!(result.row_ids, vec![1]);
    Ok(())
}

#[test]
fn query_json_round_trip() -> anyhow::Result<()> {
    let query = FullTextQuery::match_query("apache paimon", "text").operator_and();
    let json = query.to_json()?;
    let parsed = FullTextQuery::from_json(&json)?;

    assert_eq!(parsed, query);
    Ok(())
}
