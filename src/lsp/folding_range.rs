use tower_lsp::lsp_types::*;

use super::document::{EnvDocEntry, EnvLineType};

pub fn compute_folding_ranges(entries: &[EnvDocEntry]) -> Vec<FoldingRange> {
    let mut ranges = Vec::new();
    let mut comment_start: Option<u32> = None;
    let mut comment_end: u32 = 0;
    let mut blank_start: Option<u32> = None;
    let mut blank_end: u32 = 0;

    for entry in entries {
        match entry.line_type {
            EnvLineType::Comment => {
                if comment_start.is_none() {
                    comment_start = Some(entry.line);
                }
                comment_end = entry.line;
                if let Some(start) = blank_start.take() {
                    if blank_end > start {
                        ranges.push(make_region_range(start, blank_end));
                    }
                }
            }
            EnvLineType::Blank => {
                if blank_start.is_none() {
                    blank_start = Some(entry.line);
                }
                blank_end = entry.line;
                if let Some(start) = comment_start.take() {
                    if comment_end > start {
                        ranges.push(make_comment_range(start, comment_end));
                    }
                }
            }
            EnvLineType::EnvVar | EnvLineType::Other => {
                if let Some(start) = comment_start.take() {
                    if comment_end > start {
                        ranges.push(make_comment_range(start, comment_end));
                    }
                }
                if let Some(start) = blank_start.take() {
                    if blank_end > start {
                        ranges.push(make_region_range(start, blank_end));
                    }
                }
            }
        }
    }

    if let Some(start) = comment_start {
        if comment_end > start {
            ranges.push(make_comment_range(start, comment_end));
        }
    }
    if let Some(start) = blank_start {
        if blank_end > start {
            ranges.push(make_region_range(start, blank_end));
        }
    }

    ranges
}

fn make_comment_range(start: u32, end: u32) -> FoldingRange {
    FoldingRange {
        start_line: start,
        start_character: Some(0),
        end_line: end,
        end_character: Some(0),
        kind: Some(FoldingRangeKind::Comment),
        collapsed_text: None,
    }
}

fn make_region_range(start: u32, end: u32) -> FoldingRange {
    FoldingRange {
        start_line: start,
        start_character: Some(0),
        end_line: end,
        end_character: Some(0),
        kind: Some(FoldingRangeKind::Region),
        collapsed_text: None,
    }
}
