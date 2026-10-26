//! Cached geometry and deterministic stress fixtures, independent of the UI.

pub(crate) const STORY_COUNT: usize = 6;
pub(crate) const STORY_HEIGHT_M: f32 = 4.0;

/// Synthetic sizes available in the lab.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum SyntheticPreset {
    #[default]
    Demo,
    FiftyThousand,
    HundredThousand,
}

impl SyntheticPreset {
    pub fn count(self) -> usize {
        match self {
            Self::Demo => 48,
            Self::FiftyThousand => 50_000,
            Self::HundredThousand => 100_000,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Demo => "48 members (demo)",
            Self::FiftyThousand => "50,000 members",
            Self::HundredThousand => "100,000 members",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MemberKind {
    Column,
    Beam,
    OtherFrame,
    Shell,
}

impl MemberKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Column => "Column",
            Self::Beam => "Beam",
            Self::OtherFrame => "Other frame",
            Self::Shell => "Shell outline",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Member {
    pub key: String,
    pub story: usize,
    pub kind: MemberKind,
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub ring: Vec<[f32; 3]>,
}

impl Member {
    pub fn segments(&self) -> impl Iterator<Item = [[f32; 3]; 2]> + '_ {
        std::iter::once([self.start, self.end])
            .filter(|_| self.ring.is_empty())
            .chain(
                self.ring
                    .iter()
                    .copied()
                    .zip(self.ring.iter().copied().cycle().skip(1))
                    .take(self.ring.len())
                    .map(Into::into),
            )
    }
}

#[derive(Debug)]
pub(crate) struct SceneModel {
    pub members: Vec<Member>,
    pub stories: Vec<String>,
    pub center: [f32; 3],
    pub radius: f32,
    pub source: &'static str,
    pub frames: usize,
    pub shells: usize,
    pub joints: usize,
    pub skipped: usize,
    pub warnings: usize,
    pub build_ms: f64,
}

impl SceneModel {
    pub fn from_members(members: Vec<Member>, stories: Vec<String>, source: &'static str) -> Self {
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for member in &members {
            for segment in member.segments() {
                for point in segment {
                    for axis in 0..3 {
                        min[axis] = min[axis].min(point[axis]);
                        max[axis] = max[axis].max(point[axis]);
                    }
                }
            }
        }
        let center = std::array::from_fn(|axis| min[axis].midpoint(max[axis]));
        let radius =
            ((max[0] - min[0]).powi(2) + (max[1] - min[1]).powi(2) + (max[2] - min[2]).powi(2))
                .sqrt()
                * 0.5;
        let shells = members
            .iter()
            .filter(|m| m.kind == MemberKind::Shell)
            .count();
        Self {
            frames: members.len() - shells,
            shells,
            members,
            stories,
            center,
            radius: radius.max(0.01),
            source,
            joints: 0,
            skipped: 0,
            warnings: 0,
            build_ms: 0.0,
        }
    }

    pub fn synthetic(preset: SyntheticPreset) -> Self {
        let started = web_time::Instant::now();
        let (members, stories) = if preset == SyntheticPreset::Demo {
            (sample_members(), STORY_COUNT)
        } else {
            let bays = if preset == SyntheticPreset::FiftyThousand {
                18
            } else {
                25
            };
            let mut members = Vec::with_capacity(preset.count());
            let mut story = 0;
            while members.len() < preset.count() {
                story += 1;
                let lower = (story - 1) as f32 * STORY_HEIGHT_M;
                let upper = story as f32 * STORY_HEIGHT_M;
                for x in 0..=bays {
                    for y in 0..=bays {
                        let px = x as f32 * 6.0;
                        let py = y as f32 * 6.0;
                        let candidates = [
                            (MemberKind::Column, [px, py, lower], [px, py, upper], true),
                            (
                                MemberKind::Beam,
                                [px, py, upper],
                                [px + 6.0, py, upper],
                                x < bays,
                            ),
                            (
                                MemberKind::Beam,
                                [px, py, upper],
                                [px, py + 6.0, upper],
                                y < bays,
                            ),
                        ];
                        for (direction, (kind, start, end, include)) in
                            candidates.into_iter().enumerate()
                        {
                            if include && members.len() < preset.count() {
                                members.push(Member {
                                    key: format!("{x}:{y}:{direction}@S{story}"),
                                    story,
                                    kind,
                                    start,
                                    end,
                                    ring: Vec::new(),
                                });
                            }
                        }
                    }
                }
            }
            (members, story)
        };
        let mut scene = Self::from_members(
            members,
            (1..=stories).map(|s| format!("Story {s}")).collect(),
            "Synthetic",
        );
        scene.build_ms = started.elapsed().as_secs_f64() * 1000.0;
        scene
    }
}

impl Default for SceneModel {
    fn default() -> Self {
        Self::synthetic(SyntheticPreset::Demo)
    }
}

pub(crate) fn sample_members() -> Vec<Member> {
    let plan = [[-3.0, -2.5], [3.0, -2.5], [3.0, 2.5], [-3.0, 2.5]];
    let mut members = Vec::with_capacity(STORY_COUNT * 8);
    for story in 1..=STORY_COUNT {
        let lower = (story - 1) as f32 * STORY_HEIGHT_M;
        let upper = story as f32 * STORY_HEIGHT_M;
        for (index, [x, y]) in plan.iter().copied().enumerate() {
            members.push(Member {
                key: format!("C{}@S{story}", index + 1),
                story,
                kind: MemberKind::Column,
                start: [x, y, lower],
                end: [x, y, upper],
                ring: Vec::new(),
            });
        }
        for index in 0..plan.len() {
            let a = plan[index];
            let b = plan[(index + 1) % plan.len()];
            members.push(Member {
                key: format!("B{}@S{story}", index + 1),
                story,
                kind: MemberKind::Beam,
                start: [a[0], a[1], upper],
                end: [b[0], b[1], upper],
                ring: Vec::new(),
            });
        }
    }
    members
}

pub(crate) struct Projection {
    center: [f32; 3],
    origin: egui::Pos2,
    yaw: (f32, f32),
    pitch: (f32, f32),
    scale: f32,
}

impl Projection {
    pub fn new(center: [f32; 3], origin: egui::Pos2, yaw: f32, pitch: f32, scale: f32) -> Self {
        Self {
            center,
            origin,
            yaw: yaw.sin_cos(),
            pitch: pitch.sin_cos(),
            scale,
        }
    }

    pub fn project(&self, point: [f32; 3]) -> egui::Pos2 {
        let [x, y, z] = std::array::from_fn(|axis| point[axis] - self.center[axis]);
        let horizontal_x = x * self.yaw.1 - y * self.yaw.0;
        let horizontal_y = x * self.yaw.0 + y * self.yaw.1;
        self.origin
            + egui::vec2(horizontal_x, horizontal_y * self.pitch.0 - z * self.pitch.1) * self.scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn stress_presets_have_exact_counts_unique_keys_and_finite_geometry() {
        for preset in [
            SyntheticPreset::FiftyThousand,
            SyntheticPreset::HundredThousand,
        ] {
            let scene = SceneModel::synthetic(preset);
            assert_eq!(scene.members.len(), preset.count());
            assert_eq!(
                scene
                    .members
                    .iter()
                    .map(|m| &m.key)
                    .collect::<HashSet<_>>()
                    .len(),
                preset.count()
            );
            assert!(
                scene
                    .members
                    .iter()
                    .all(|m| m.start.iter().chain(&m.end).all(|v| v.is_finite()))
            );
            assert!(scene.radius.is_finite() && scene.radius > 0.0);
            assert!(
                scene
                    .members
                    .iter()
                    .all(|m| (1..=scene.stories.len()).contains(&m.story))
            );
        }
    }
}
