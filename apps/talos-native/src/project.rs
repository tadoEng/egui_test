//! Project and version identity, and the public synthetic demo project.
//!
//! The demo is two independently authored E2K snapshots that differ in one
//! source-backed way (the roof slab's insertion point). It is a public
//! demonstration of version routing, never an `EtabExtension` commit: real
//! project, branch and committed-version state comes from `ext-api` (APP-01D).
use std::fmt;

/// Where the project and version list come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Authority {
    /// Embedded public fixtures; no repository, no commit history, no analysis.
    PublicSyntheticDemo,
}
impl Authority {
    pub fn label(self) -> &'static str {
        match self {
            Self::PublicSyntheticDemo => "Synthetic demo",
        }
    }
    pub fn detail(self) -> &'static str {
        match self {
            Self::PublicSyntheticDemo => {
                "Public synthetic snapshots embedded in this build. Not an EtabExtension \
                 repository: versions are not committed, and nothing has been analyzed."
            }
        }
    }
}

/// One immutable version on one branch.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VersionId {
    pub branch: String,
    pub version: String,
}
impl VersionId {
    pub fn new(branch: &str, version: &str) -> Self {
        Self {
            branch: branch.to_owned(),
            version: version.to_owned(),
        }
    }
}
impl fmt::Display for VersionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.branch, self.version)
    }
}

/// Immutable source bytes for one version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Embedded(&'static [u8]),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VersionEntry {
    pub id: VersionId,
    /// An authored one-line description, not a commit message.
    pub summary: &'static str,
    pub source: Source,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    pub name: String,
    pub authority: Authority,
    pub versions: Vec<VersionEntry>,
}
impl Project {
    /// `main/v1` and `main/v2`: the committed REAL-03 `middle` and `top`
    /// fixtures (see `docs/native/evidence/real03-shells/fixtures.sha256`).
    pub fn public_demo() -> Self {
        Self {
            name: "Public demo project".to_owned(),
            authority: Authority::PublicSyntheticDemo,
            versions: vec![
                VersionEntry {
                    id: VersionId::new("main", "v1"),
                    summary: "Roof slab S inserted at MIDDLE",
                    source: Source::Embedded(include_bytes!(
                        "../../../docs/native/evidence/real03-shells/fixtures/middle.e2k"
                    )),
                },
                VersionEntry {
                    id: VersionId::new("main", "v2"),
                    summary: "Roof slab S inserted at TOP",
                    source: Source::Embedded(include_bytes!(
                        "../../../docs/native/evidence/real03-shells/fixtures/top.e2k"
                    )),
                },
            ],
        }
    }
    pub fn version(&self, id: &VersionId) -> Option<&VersionEntry> {
        self.versions.iter().find(|entry| &entry.id == id)
    }
    pub fn latest(&self) -> Option<&VersionEntry> {
        self.versions.last()
    }
}
