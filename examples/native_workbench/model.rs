//! Presentation state only; parsing and engineering DTOs stay in pinned crates.
use crate::{
    data::{Choices, Config, Direction, Packet, Snapshot},
    preflight::{self, Options, Report},
};
use lab_e2k_mesh::{DisplayMode, Mesh, Story};
use std::path::{Path, PathBuf};

pub(crate) struct Session {
    pub(crate) snapshot: Snapshot,
    pub(crate) report: Report,
    pub(crate) config: Config,
    pub(crate) choices: [Choices; 2],
    pub(crate) packet: Packet,
    pub(crate) stories: Vec<Story>,
    pub(crate) mesh: Option<Mesh>,
    pub(crate) entity_count: usize,
    pub(crate) withheld: usize,
    pub(crate) projection_notes: usize,
    pub(crate) mesh_bytes: usize,
    pub(crate) linked: bool,
    pub(crate) refusal: Option<&'static str>,
    root: PathBuf,
    scope: String,
}
fn options(root: &Path) -> Options {
    Options {
        root: root.to_owned(),
        e2k: None,
        allow_external: false,
        expected_sha256: None,
    }
}
impl Session {
    pub(crate) fn load(root: &Path) -> Result<Self, &'static str> {
        let (mut checked, report) = preflight::load_checked_preview(&options(root))?;
        if checked.snapshot.manifest.id.is_empty() || checked.snapshot.manifest.branch.is_empty() {
            return Err("missing_version_identity");
        }
        let mesh = lab_e2k_mesh::build(checked.e2k_bytes(), Default::default())
            .map_err(|_private_error| "geometry_conversion_refused")?;
        let config = checked
            .snapshot
            .initial_config(Direction::X)
            .map_err(|_private_error| "supported_chart_context_missing")?;
        let choices =
            [Direction::X, Direction::Y].map(|direction| checked.snapshot.choices(direction));
        let [x, y] = choices;
        let choices = [
            x.map_err(|_private_error| "chart_choices_refused")?,
            y.map_err(|_private_error| "chart_choices_refused")?,
        ];
        let scope = checked.scoped_fingerprint();
        let mut packet = checked
            .snapshot
            .packet(&config, 1, None)
            .map_err(|_private_error| "chart_packet_refused")?;
        packet.source.clone_from(&scope);
        // Keep read handles through DTO projection and mesh construction, then
        // repeat byte/path/mtime checks before publishing either part together.
        checked.verify_inputs()?;
        let root = root.to_owned();
        let stories = mesh.stories().to_vec();
        let entity_count = mesh.entities().len();
        let withheld = mesh
            .entities()
            .iter()
            .filter(|e| e.mode == DisplayMode::Withheld)
            .count();
        let projection_notes = mesh.projection_skips().len();
        let mesh_bytes = mesh.buffer_bytes();
        Ok(Self {
            snapshot: checked.snapshot,
            report,
            config,
            choices,
            packet,
            stories,
            mesh: Some(mesh),
            entity_count,
            withheld,
            projection_notes,
            mesh_bytes,
            linked: true,
            refusal: None,
            root,
            scope,
        })
    }
    pub(crate) fn common_story(&self, story: &str) -> bool {
        self.stories.iter().any(|s| s.name == story)
            && self.packet.drift.stories.iter().any(|s| s == story)
            && self.packet.shear.stories.iter().any(|s| s == story)
    }
    pub(crate) fn select(&mut self, story: Option<String>) -> Result<(), &'static str> {
        if !self.linked {
            return Err("version_unlinked");
        }
        if story.as_deref().is_some_and(|s| !self.common_story(s)) {
            return Err("story_outside_shared_scope");
        }
        self.packet.selected_story = story;
        Ok(())
    }
    pub(crate) fn accept(&mut self, raw: &str) -> Result<(), &'static str> {
        if raw.len() > 1024 {
            return Err("oversized_selection");
        }
        let selected = self
            .packet
            .accept(raw)
            .map_err(|_private_error| "selection_context_refused")?;
        self.select(selected)
    }
    pub(crate) fn configure(&mut self, config: Config) -> Result<(), &'static str> {
        if !self.linked {
            return Err("version_unlinked");
        }
        if config == self.config {
            return Ok(());
        }
        let next = self
            .packet
            .generation
            .checked_add(1)
            .ok_or("generation_exhausted")?;
        let mut packet = self
            .snapshot
            .packet(&config, next, None)
            .map_err(|_private_error| "chart_context_refused")?;
        packet.source.clone_from(&self.scope);
        self.config = config;
        self.packet = packet;
        Ok(())
    }
    // An immutable selected version is never silently replaced. A changed or
    // unreadable input retains the last valid display but expires all selection.
    // Reopening is explicit; A->B->A on disk cannot revive the old generation.
    pub(crate) fn recheck(&mut self) -> Result<(), &'static str> {
        let checked = preflight::load_checked_preview(&options(&self.root)).and_then(
            |(mut checked, _report)| {
                checked.verify_inputs()?;
                if checked.scoped_fingerprint() != self.scope {
                    return Err("selected_version_changed");
                }
                Ok(())
            },
        );
        if let Err(code) = checked {
            self.linked = false;
            self.refusal = Some(code);
            self.packet.selected_story = None;
            if let Some(next) = self.packet.generation.checked_add(1) {
                self.packet.generation = next;
            }
            return Err(code);
        }
        if self.linked {
            Ok(())
        } else {
            Err("reopen_required_after_source_change")
        }
    }
}

// This function accepts no user path and creates independently authored data.
// Only this owned fixture is eligible for public smoke receipts.
pub(crate) fn public_fixture() -> Result<tempfile::TempDir, &'static str> {
    let (input, _) =
        crate::fixture::fixture().map_err(|_private_error| "public_fixture_create_failed")?;
    let e2k = include_str!("../../assets/fixtures/real01.e2k").replace(
        "AREAASSIGN \"S\" \"Roof\"",
        "AREAASSIGN \"S\" \"Roof\" OPENING \"NO\"",
    );
    std::fs::write(input.path().join("authored.e2k"), e2k)
        .map_err(|_private_error| "public_fixture_write_failed")?;
    Ok(input)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
