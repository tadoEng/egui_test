//! A loaded, immutable source document and its identity.
//!
//! Geometry, source attribution and fidelity come only from `lab-e2k-mesh`
//! (which projects through the pinned `ext-e2k`); nothing here parses E2K.
use crate::project::{Source, VersionEntry, VersionId};
use lab_e2k_mesh::{ForceUnit, LengthUnit, Mesh, Revision, SourceUnits, TempUnit};

/// Which version a document was loaded for, and the exact bytes and geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DocumentId<'a> {
    pub version: &'a VersionId,
    pub revision: Revision,
}

pub struct Document {
    version: VersionId,
    mesh: Mesh,
}
impl Document {
    /// # Errors
    /// The adapter's refusal (units, version, geometry or budget), as text.
    pub fn load(entry: &VersionEntry) -> Result<Self, String> {
        let Source::Embedded(bytes) = entry.source;
        let mesh = lab_e2k_mesh::build(bytes, lab_e2k_mesh::Config::default())
            .map_err(|error| error.to_string())?;
        Ok(Self {
            version: entry.id.clone(),
            mesh,
        })
    }
    pub fn id(&self) -> DocumentId<'_> {
        DocumentId {
            version: &self.version,
            revision: self.mesh.revision(),
        }
    }
    pub fn version(&self) -> &VersionId {
        &self.version
    }
    pub fn mesh(&self) -> &Mesh {
        &self.mesh
    }
    pub fn story_names(&self) -> impl Iterator<Item = &str> {
        self.mesh.stories().iter().map(|story| story.name.as_str())
    }
}

/// Source unit names, as declared in the E2K `$ CONTROLS` section.
pub fn unit_names(units: SourceUnits) -> [&'static str; 3] {
    [
        match units.force {
            ForceUnit::Kip => "kip",
            ForceUnit::Lb => "lb",
            ForceUnit::Kn => "kN",
            ForceUnit::N => "N",
        },
        length_name(units.length),
        match units.temperature {
            TempUnit::F => "°F",
            TempUnit::C => "°C",
        },
    ]
}
fn length_name(length: LengthUnit) -> &'static str {
    match length {
        LengthUnit::Ft => "ft",
        LengthUnit::In => "in",
        LengthUnit::M => "m",
        LengthUnit::Cm => "cm",
        LengthUnit::Mm => "mm",
    }
}
/// A length the adapter converted to metres, with the declared source value it
/// came from, e.g. `0.1524 m (0.5 ft)`.
pub fn length(metres: f64, units: SourceUnits) -> String {
    let source = metres / units.metres_per_length_unit();
    if units.length == LengthUnit::M {
        format!("{metres:.4} m")
    } else {
        format!(
            "{metres:.4} m ({} {})",
            trim(source),
            length_name(units.length)
        )
    }
}
fn trim(value: f64) -> String {
    let text = format!("{value:.6}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}
