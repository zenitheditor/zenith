use std::sync::Arc;

use zenith_core::ast::AssetKind;
use zenith_zpx::ZpxDoc;

use crate::error::ProduceError;

pub trait AssetProducer {
    fn produce(&self, req: ProduceRequest) -> Result<ProducedAsset, ProduceError>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProduceRequest {
    FileImport {
        kind: AssetKind,
        bytes: Arc<[u8]>,
        provenance: FileImportProvenance,
    },
    ZpxBake {
        doc: ZpxDoc,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProducedAsset {
    pub kind: AssetKind,
    pub bytes: Arc<[u8]>,
    pub sha256: String,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provenance {
    FileImport(FileImportProvenance),
    ZpxBake(ZpxBakeProvenance),
}

impl Provenance {
    /// The producer tag recorded as `AssetDecl.producer_kind` /
    /// `Op::AddAsset.producer_kind` (e.g. `"file-import"`, `"zpx-bake"`).
    pub fn kind_str(&self) -> &'static str {
        match self {
            Provenance::FileImport(_) => "file-import",
            Provenance::ZpxBake(_) => "zpx-bake",
        }
    }

    /// The producer-specific source reference recorded as
    /// `AssetDecl.producer_source` / `Op::AddAsset.producer_source`: the
    /// imported file's path for `file-import`, or the source `.zpx`
    /// manifest's content hash for `zpx-bake`.
    pub fn source_str(&self) -> &str {
        match self {
            Provenance::FileImport(p) => &p.source,
            Provenance::ZpxBake(p) => &p.source_sha256,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileImportProvenance {
    pub source: String,
}

impl FileImportProvenance {
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZpxBakeProvenance {
    pub source_sha256: String,
}
