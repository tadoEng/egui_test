//! A loaded, immutable source document and its identity.
//!
//! Geometry, source attribution and fidelity come only from `lab-e2k-mesh`
//! (which projects through the pinned `ext-e2k`); nothing here parses E2K.
use crate::project::{Source, VersionEntry, VersionId};
use lab_e2k_mesh::pick::PickIndex;
use lab_e2k_mesh::{ForceUnit, LengthUnit, Mesh, Revision, SourceUnits, TempUnit, Vertex};

/// Which version a document was loaded for, and the exact bytes and geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DocumentId<'a> {
    pub version: &'a VersionId,
    pub revision: Revision,
}

pub struct Document {
    version: VersionId,
    mesh: Mesh,
    /// Built with the mesh on the loader thread, never on the UI thread.
    pick: PickIndex,
}
impl Document {
    /// # Errors
    /// The adapter's refusal (units, version, geometry or budget), as text.
    pub fn load(entry: &VersionEntry) -> Result<Self, String> {
        let Source::Embedded(bytes) = entry.source;
        let mesh = lab_e2k_mesh::build(bytes, lab_e2k_mesh::Config::default())
            .map_err(|error| error.to_string())?;
        let pick = PickIndex::new(mesh.vertices(), mesh.indices(), mesh.entities());
        Ok(Self {
            version: entry.id.clone(),
            mesh,
            pick,
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
    pub fn pick_index(&self) -> &PickIndex {
        &self.pick
    }
    /// Local display bounds (metres, origin-recentred), as the renderer uses them.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "bounds of already validated f32 display vertices"
    )]
    pub fn display_bounds(&self) -> lab_viewport::Bounds {
        let bounds = self.mesh.local_bounds_m();
        lab_viewport::Bounds {
            min: bounds.min.map(|v| v as f32),
            max: bounds.max.map(|v| v as f32),
        }
    }
}

/// The renderer reads the document's own buffers; nothing is copied. The GPU
/// key is the geometry digest, so byte-identical geometry reuses the resident
/// buffers while selection scope still changes with the load generation.
impl lab_viewport::GpuGeometry for Document {
    fn key(&self) -> lab_viewport::MeshKey {
        lab_viewport::MeshKey::SourceGeometry(self.mesh.revision().geometry_sha256)
    }
    fn vertices(&self) -> &[Vertex] {
        self.mesh.vertices()
    }
    fn indices(&self) -> &[u32] {
        self.mesh.indices()
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
