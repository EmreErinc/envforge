//! Concatenated cross-tool scanner. Catches injection split across
//! adjacent tools' descriptions.

use crate::ops::mcp_poison::description_scan::DescriptionScanner;
use crate::ops::mcp_poison::error::ScannerError;
use crate::ops::mcp_poison::finding::PoisonFinding;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: String,
}

pub struct BlobScanner;

impl BlobScanner {
    pub const MAX_BLOB_BYTES: usize = 1024 * 1024;

    pub fn scan(tools: &[ToolDescriptor]) -> Result<Vec<PoisonFinding>, ScannerError> {
        let mut tools_sorted = tools.to_vec();
        tools_sorted.sort_by(|a, b| a.name.cmp(&b.name));

        let mut blob = String::new();
        for t in &tools_sorted {
            blob.push_str("\n# tool: ");
            blob.push_str(&t.name);
            blob.push('\n');
            blob.push_str(&t.description);
            blob.push('\n');
            if blob.len() >= Self::MAX_BLOB_BYTES {
                blob.truncate(Self::MAX_BLOB_BYTES);
                break;
            }
        }
        DescriptionScanner::scan("<blob>", &blob)
    }
}
