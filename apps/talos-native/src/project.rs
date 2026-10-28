//! Project and version identity, and the public synthetic demo project.
//!
//! The demo is the independently authored public synthetic tower: two E2K
//! snapshots that differ in two source-backed ways (the L12 slab's insertion
//! point and the L1-L2 perimeter beam depth). It is a public demonstration of
//! version routing, never an `EtabExtension` commit: real project, branch and
//! committed-version state comes from `ext-api` (APP-01D).
//!
//! Two other sources exist (APP-01C): version folders the user names on the
//! command line, read in place and never written; and the public authored
//! LAB-REAL01 results fixture, the only public cached-results sample.
use std::fmt;
use std::path::PathBuf;

/// Where the project and version list come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Authority {
    /// Embedded public fixtures; no repository, no commit history, no analysis.
    PublicSyntheticDemo,
    /// The authored public cached-results fixture: stored values, not analysis.
    PublicResultsFixture,
    /// Version folders named on the command line, read in place.
    LocalVersions,
}
impl Authority {
    pub fn label(self) -> &'static str {
        match self {
            Self::PublicSyntheticDemo => "Synthetic demo",
            Self::PublicResultsFixture => "Authored fixture",
            Self::LocalVersions => "Local · read-only",
        }
    }
    pub fn detail(self) -> &'static str {
        match self {
            Self::PublicSyntheticDemo => {
                "Public synthetic snapshots embedded in this build. Not an EtabExtension \
                 repository: versions are not committed, and nothing has been analyzed."
            }
            Self::PublicResultsFixture => {
                "The public LAB-REAL01 fixture: an authored two-story E2K and authored cached \
                 result values. Not an analysis and not an EtabExtension repository."
            }
            Self::LocalVersions => {
                "Version folders you named on the command line, read in place. Nothing is \
                 written, analyzed or sent anywhere. Version identity comes from each \
                 folder's manifest; this is not ext-api repository state."
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

/// Where one version's source (and any cached results) are read from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// An embedded E2K; no cached results.
    Embedded(&'static [u8]),
    /// The public authored results fixture, built in memory.
    PublicResultsFixture,
    /// A version folder: `manifest.json`, `model.e2k` and optionally
    /// `results/calc_output.json` with `results/source-units.json`.
    Folder(PathBuf),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VersionEntry {
    pub id: VersionId,
    /// An authored description, or the folder manifest's own message.
    pub summary: String,
    pub source: Source,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    pub name: String,
    pub authority: Authority,
    pub versions: Vec<VersionEntry>,
}
impl Project {
    /// `main/v1` and `main/v2`: the public synthetic tower
    /// (`docs/native/fixtures/tower`, sha256 in `fixtures.sha256`).
    pub fn public_demo() -> Self {
        Self {
            name: "Public demo tower".to_owned(),
            authority: Authority::PublicSyntheticDemo,
            versions: vec![
                VersionEntry {
                    id: VersionId::new("main", "v1"),
                    summary: "Roof slab MIDDLE · podium beams 2.5 ft".to_owned(),
                    source: Source::Embedded(include_bytes!(
                        "../../../docs/native/fixtures/tower/tower-v1.e2k"
                    )),
                },
                VersionEntry {
                    id: VersionId::new("main", "v2"),
                    summary: "Roof slab TOP · podium beams 3 ft".to_owned(),
                    source: Source::Embedded(include_bytes!(
                        "../../../docs/native/fixtures/tower/tower-v2.e2k"
                    )),
                },
            ],
        }
    }
    /// The public LAB-REAL01 results fixture as a one-version project.
    pub fn public_results_fixture() -> Self {
        let (manifest, _, _) = lab_results::fixture::values();
        let text = |key: &str| manifest[key].as_str().unwrap_or_default().to_owned();
        Self {
            name: "LAB-REAL01 results fixture".to_owned(),
            authority: Authority::PublicResultsFixture,
            versions: vec![VersionEntry {
                id: VersionId::new(&text("branch"), &text("id")),
                summary: "Authored two-story fixture · cached values, not analysis".to_owned(),
                source: Source::PublicResultsFixture,
            }],
        }
    }
    /// One version per folder, in the order given; identity comes from each
    /// folder's `manifest.json`. Nothing in a folder is written.
    ///
    /// # Errors
    /// An unreadable or invalid manifest, or two folders claiming one version.
    pub fn from_folders(folders: &[PathBuf]) -> Result<Self, String> {
        let mut versions: Vec<VersionEntry> = Vec::new();
        for folder in folders {
            let manifest = lab_results::folder::read_manifest(folder)?;
            let id = VersionId::new(&manifest.branch, &manifest.id);
            if versions.iter().any(|entry| entry.id == id) {
                return Err(format!("Two folders both claim version {id}"));
            }
            versions.push(VersionEntry {
                id,
                summary: manifest.message.clone(),
                source: Source::Folder(folder.clone()),
            });
        }
        if versions.is_empty() {
            return Err("No version folders were given".to_owned());
        }
        Ok(Self {
            name: "Local versions".to_owned(),
            authority: Authority::LocalVersions,
            versions,
        })
    }
    pub fn version(&self, id: &VersionId) -> Option<&VersionEntry> {
        self.versions.iter().find(|entry| &entry.id == id)
    }
    pub fn latest(&self) -> Option<&VersionEntry> {
        self.versions.last()
    }
}
