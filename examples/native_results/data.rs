//! Presentation of existing authoritative DTOs, never engineering recomputation.
use ext_calc::{output::CalcOutput, source_units::SourceUnitMetadata};
use ext_core::version::VersionManifest;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::{collections::BTreeSet, fs, io::Read as _, path::Path};

pub(crate) struct Snapshot {
    pub(crate) calc: CalcOutput,
    pub(crate) manifest: VersionManifest,
    pub(crate) fingerprint: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Direction {
    X,
    Y,
}
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Config {
    pub(crate) direction: Direction,
    pub(crate) drift_group: String,
    pub(crate) drift_case: String,
    pub(crate) shear_case: String,
}
pub(crate) struct Choices {
    pub(crate) groups: Vec<String>,
    pub(crate) cases: Vec<String>,
    pub(crate) forces: Vec<String>,
}
#[derive(Serialize)]
pub(crate) struct Series {
    pub(crate) name: String,
    pub(crate) values: Vec<Option<f64>>,
}
#[derive(Serialize)]
pub(crate) struct Plot {
    pub(crate) title: String,
    pub(crate) unit: String,
    pub(crate) stories: Vec<String>,
    pub(crate) series: Vec<Series>,
    pub(crate) reference: Option<f64>,
}
#[derive(Serialize)]
pub(crate) struct Packet {
    pub(crate) version: u8,
    pub(crate) source: String,
    pub(crate) generation: u64,
    pub(crate) selected_story: Option<String>,
    pub(crate) drift: Plot,
    pub(crate) shear: Plot,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Select {
    pub(crate) version: u8,
    #[serde(rename = "type")]
    pub(crate) kind: String,
    pub(crate) source: String,
    pub(crate) generation: u64,
    pub(crate) plot: String,
    pub(crate) story: Option<String>,
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, String> {
    let meta = fs::metadata(path).map_err(|e| format!("Read artifact metadata: {e}"))?;
    if !meta.is_file() || meta.len() > 64 * 1024 * 1024 {
        return Err("Missing or oversized cached artifact".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .and_then(|file| file.take(64 * 1024 * 1024 + 1).read_to_end(&mut bytes))
        .map_err(|e| format!("Read cached artifact: {e}"))?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("Oversized cached artifact".into());
    }
    Ok(bytes)
}
impl Snapshot {
    pub(crate) fn load(root: &Path) -> Result<Self, String> {
        let root = root
            .canonicalize()
            .map_err(|e| format!("Resolve version directory: {e}"))?;
        if root.join(".partial").exists() {
            return Err("Refusing an incomplete version (.partial)".into());
        }
        let paths = [
            "manifest.json",
            "results/calc_output.json",
            "results/source-units.json",
        ];
        for relative in paths {
            let resolved = root
                .join(relative)
                .canonicalize()
                .map_err(|e| format!("Resolve artifact: {e}"))?;
            if !resolved.starts_with(&root) {
                return Err("Cached artifact resolves outside the selected directory".into());
            }
        }
        let bytes = paths
            .iter()
            .map(|p| read_bounded(&root.join(p)))
            .collect::<Result<Vec<_>, _>>()?;
        let manifest: VersionManifest =
            serde_json::from_slice(&bytes[0]).map_err(|e| format!("Version manifest: {e}"))?;
        let calc: CalcOutput = serde_json::from_slice(&bytes[1])
            .map_err(|e| format!("Authoritative CalcOutput: {e}"))?;
        let source: SourceUnitMetadata =
            serde_json::from_slice(&bytes[2]).map_err(|e| format!("Source-unit metadata: {e}"))?;
        source
            .context()
            .map_err(|e| format!("Source-unit validation: {e}"))?;
        if calc.meta.version_id != manifest.id || calc.meta.branch != manifest.branch {
            return Err("Cached result branch/version does not match manifest".into());
        }
        let provenance = calc
            .meta
            .unit_provenance
            .as_ref()
            .ok_or("Cached results lack unit provenance")?;
        SourceUnitMetadata::from_actual(
            &provenance.source_preset,
            &source.force,
            &source.length,
            &source.temperature,
        )
        .map_err(|e| format!("Cached preset provenance: {e}"))?;
        if provenance.source.force != source.force
            || provenance.source.global_length != source.length
            || provenance.canonical.force != calc.meta.units.force
            || provenance.canonical.global_length != calc.meta.units.length
            || provenance.canonical.moment != calc.meta.units.moment
        {
            return Err("Conflicting cached/source/canonical unit labels".into());
        }
        if calc.meta.units.force != "kip"
            || calc.meta.units.length != "ft"
            || calc.meta.units.moment != "kip·ft"
        {
            return Err("This viewer supports confirmed canonical kip/ft fields only; no unit conversion is inferred".into());
        }
        let mut hasher = Sha256::new();
        for (path, original) in paths.iter().zip(&bytes) {
            if read_bounded(&root.join(path))? != *original {
                return Err("Source changed while loading; retry".into());
            }
            hasher.update(path.as_bytes());
            hasher.update((original.len() as u64).to_le_bytes());
            hasher.update(original);
        }
        if root.join(".partial").exists() {
            return Err("Version became incomplete while loading".into());
        }
        Ok(Self {
            calc,
            manifest,
            fingerprint: format!("{:x}", hasher.finalize()),
        })
    }
    fn drift(&self, direction: Direction) -> Result<&ext_calc::output::DriftOutput, String> {
        let wind = self
            .calc
            .drift_wind
            .as_ref()
            .ok_or("No cached wind drift output")?;
        Ok(match direction {
            Direction::X => &wind.x,
            Direction::Y => &wind.y,
        })
    }
    fn profiles(
        &self,
        direction: Direction,
    ) -> Result<&[ext_calc::output::StoryForceCaseProfile], String> {
        let forces = self
            .calc
            .story_forces
            .as_ref()
            .ok_or("No cached story-force output")?;
        Ok(match direction {
            Direction::X => &forces.x_profiles,
            Direction::Y => &forces.y_profiles,
        })
    }
    pub(crate) fn choices(&self, direction: Direction) -> Result<Choices, String> {
        let drift = self.drift(direction)?;
        let groups = drift
            .rows
            .iter()
            .map(|r| r.group_name.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let cases = drift
            .rows
            .iter()
            .map(|r| r.output_case.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let force_cases = self
            .profiles(direction)?
            .iter()
            .map(|p| p.output_case.clone())
            .collect();
        Ok(Choices {
            groups,
            cases,
            forces: force_cases,
        })
    }
    pub(crate) fn initial_config(&self, direction: Direction) -> Result<Config, String> {
        let drift = self.drift(direction)?;
        let row = drift.rows.first().ok_or("No cached wind drift rows")?;
        let force = self
            .profiles(direction)?
            .first()
            .ok_or("No cached force case profiles")?;
        Ok(Config {
            direction,
            drift_group: row.group_name.clone(),
            drift_case: row.output_case.clone(),
            shear_case: force.output_case.clone(),
        })
    }
    pub(crate) fn packet(
        &self,
        config: &Config,
        generation: u64,
        selected: Option<String>,
    ) -> Result<Packet, String> {
        let drift = self.drift(config.direction)?;
        let force = self
            .calc
            .story_forces
            .as_ref()
            .ok_or("No cached story-force output")?;
        let profile = self
            .profiles(config.direction)?
            .iter()
            .find(|p| p.output_case == config.shear_case)
            .ok_or("Unknown shear case")?;
        validate_order(&drift.story_order)?;
        validate_order(&force.story_order)?;
        if !drift.allowable_ratio.is_finite() || drift.allowable_ratio < 0.0 {
            return Err("Invalid cached drift reference".into());
        }
        let matching = drift
            .rows
            .iter()
            .filter(|r| r.group_name == config.drift_group && r.output_case == config.drift_case)
            .collect::<Vec<_>>();
        if matching.is_empty() {
            return Err("No cached rows for this drift group/case".into());
        }
        validate_membership(&drift.story_order, matching.iter().map(|row| &row.story))?;
        validate_membership(
            &force.story_order,
            profile.rows.iter().map(|row| &row.story),
        )?;
        let mut positive = Vec::new();
        let mut negative = Vec::new();
        for story in &drift.story_order {
            let row = matching.iter().find(|r| &r.story == story);
            let pos = row.map(|r| match config.direction {
                Direction::X => r.max_drift_x_pos,
                Direction::Y => r.max_drift_y_pos,
            });
            let neg = row.map(|r| match config.direction {
                Direction::X => r.max_drift_x_neg,
                Direction::Y => r.max_drift_y_neg,
            });
            validate_values(&[pos, neg])?;
            positive.push(pos);
            negative.push(neg);
        }
        let shear =
            force
                .story_order
                .iter()
                .map(|story| {
                    profile.rows.iter().find(|r| &r.story == story).map(|r| {
                        match config.direction {
                            Direction::X => r.vx_kip,
                            Direction::Y => r.vy_kip,
                        }
                    })
                })
                .collect::<Vec<_>>();
        validate_values(&shear)?;
        Ok(Packet {
            version: 1,
            source: self.fingerprint.clone(),
            generation,
            selected_story: selected,
            drift: Plot {
                title: format!(
                    "Cached wind drift {:?} · {} · {}",
                    config.direction, config.drift_group, config.drift_case
                ),
                unit: "ratio".into(),
                stories: drift.story_order.clone(),
                series: vec![
                    Series {
                        name: "Stored positive envelope".into(),
                        values: positive,
                    },
                    Series {
                        name: "Stored negative envelope".into(),
                        values: negative,
                    },
                ],
                reference: Some(drift.allowable_ratio),
            },
            shear: Plot {
                title: format!(
                    "Cached signed shear {:?} · {}",
                    config.direction, config.shear_case
                ),
                unit: self.calc.meta.units.force.clone(),
                stories: force.story_order.clone(),
                series: vec![Series {
                    name: config.shear_case.clone(),
                    values: shear,
                }],
                reference: None,
            },
        })
    }
}
fn validate_order(order: &[String]) -> Result<(), String> {
    if order.is_empty()
        || order.len() > 10000
        || order.iter().any(|s| s.is_empty() || s.len() > 512)
        || order.iter().collect::<BTreeSet<_>>().len() != order.len()
    {
        return Err("Missing/duplicate authoritative story order".into());
    }
    Ok(())
}
fn validate_values(values: &[Option<f64>]) -> Result<(), String> {
    if values.iter().flatten().any(|v| !v.is_finite()) {
        return Err("Non-finite cached plot value".into());
    }
    Ok(())
}
fn validate_membership<'a>(
    order: &[String],
    stories: impl Iterator<Item = &'a String>,
) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for story in stories {
        if !order.contains(story) || !seen.insert(story) {
            return Err("Unknown/duplicate cached story; no aggregation is inferred".into());
        }
    }
    Ok(())
}
impl Packet {
    pub(crate) fn accept(&self, raw: &str) -> Result<Option<String>, String> {
        let msg: Select = serde_json::from_str(raw)
            .map_err(|error| format!("Invalid result selection: {error}"))?;
        if msg.version != 1 || msg.source != self.source || msg.generation != self.generation {
            return Err("Stale result selection".into());
        }
        let plot = match msg.plot.as_str() {
            "drift" => &self.drift,
            "shear" => &self.shear,
            _ => return Err("Unknown plot".into()),
        };
        match (msg.kind.as_str(), msg.story) {
            ("select_story", Some(story)) if plot.stories.contains(&story) => Ok(Some(story)),
            ("clear_selection", None) => Ok(None),
            _ => Err("Unknown cached story/action".into()),
        }
    }
}

#[cfg(test)]
#[path = "data_tests.rs"]
mod tests;
