/// Table names for Bor custom tables
pub const BOR_SPANS_TABLE: &str = "BorSpans";
pub const BOR_SNAPSHOTS_TABLE: &str = "BorSnapshots";
pub const BOR_RECEIPTS_TABLE: &str = "BorReceipts";
pub const BOR_TX_LOOKUP_TABLE: &str = "BorTxLookup";
pub const BOR_META_TABLE: &str = "BorMeta";

/// All Bor custom table names
pub const BOR_TABLES: &[&str] = &[
    BOR_SPANS_TABLE,
    BOR_SNAPSHOTS_TABLE,
    BOR_RECEIPTS_TABLE,
    BOR_TX_LOOKUP_TABLE,
    BOR_META_TABLE,
];

/// Key types for each table
/// BorSpans: u64 (span_id) -> SpanCompact (serialized Span)
/// BorSnapshots: B256 (block_hash) -> BorSnapshotCompact
/// BorReceipts: B256 (receipt_key) -> Vec<u8> (RLP bytes)
/// BorTxLookup: B256 (tx_hash) -> (u64, u64) (block_number, tx_index)
/// BorMeta: u64 (meta_key) -> u64 (value)
///
/// Meta keys
pub const META_LAST_SPAN_ID: u64 = 0;
pub const META_LAST_SNAPSHOT_BLOCK: u64 = 1;
pub const META_LAST_BOR_RECEIPT_BLOCK: u64 = 2;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_names() {
        assert_eq!(BOR_SPANS_TABLE, "BorSpans");
        assert_eq!(BOR_SNAPSHOTS_TABLE, "BorSnapshots");
        assert_eq!(BOR_RECEIPTS_TABLE, "BorReceipts");
        assert_eq!(BOR_TX_LOOKUP_TABLE, "BorTxLookup");
        assert_eq!(BOR_META_TABLE, "BorMeta");
    }

    #[test]
    fn test_all_tables_count() {
        assert_eq!(BOR_TABLES.len(), 5);
    }

    #[test]
    fn test_meta_keys() {
        assert_eq!(META_LAST_SPAN_ID, 0);
        assert_eq!(META_LAST_SNAPSHOT_BLOCK, 1);
        assert_eq!(META_LAST_BOR_RECEIPT_BLOCK, 2);
    }
}
