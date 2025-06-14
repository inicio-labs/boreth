use serde::Deserialize;

pub const FETCH_SPAN_FORMAT: &str = "bor/span/{}";

#[derive(Debug, Deserialize)]
pub struct Span {
    pub span_id: u64,
    pub start_block: u64,
    pub end_block: u64,
}

#[derive(Debug, Deserialize)]
pub struct HeimdallSpan {
    pub span: Span,
}

#[derive(Debug, Deserialize)]
pub struct SpanResponse {
    #[allow(dead_code)]
    pub height: String,
    pub result: HeimdallSpan,
}
