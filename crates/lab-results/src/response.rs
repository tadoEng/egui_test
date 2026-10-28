//! Story response profiles, projected from stored `CalcOutput` fields.
//!
//! Each profile is the values one stored field holds, story by story, in the
//! stored story order. Missing stories stay gaps (`None`), signs are kept as
//! written, and no envelope, maximum, unit conversion or axis convention is
//! inferred: a moment profile is labelled with the stored field it reads.
use crate::data::{Direction, Plot, Series, Snapshot, validate_membership, validate_order};
use ext_calc::output::{DisplacementOutput, DriftOutput, StoryForceCaseProfile};
use std::collections::BTreeSet;

/// A stored story response a version's cached results may hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Response {
    WindDrift,
    SeismicDrift,
    WindDisplacement,
    StoryShear,
    StoryMoment,
}
impl Response {
    pub const ALL: [Self; 5] = [
        Self::WindDrift,
        Self::SeismicDrift,
        Self::WindDisplacement,
        Self::StoryShear,
        Self::StoryMoment,
    ];
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::WindDrift => "Story drift · wind",
            Self::SeismicDrift => "Story drift · seismic",
            Self::WindDisplacement => "Story displacement · wind",
            Self::StoryShear => "Story shear",
            // The stored Mx and My components, side by side. Which component
            // is the overturning moment for a direction is a convention this
            // viewer has not verified, so it names fields, not physics.
            Self::StoryMoment => "Story moment · stored Mx, My",
        }
    }
    /// Envelope rows carry a group as well as a case; case profiles do not.
    #[must_use]
    pub fn grouped(self) -> bool {
        matches!(
            self,
            Self::WindDrift | Self::SeismicDrift | Self::WindDisplacement
        )
    }
}

/// The groups and cases one stored response actually has, in stored order
/// for cases of a profile, sorted for envelope rows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Choices {
    pub groups: Vec<String>,
    pub cases: Vec<String>,
}

enum Source<'a> {
    Drift(&'a DriftOutput),
    Displacement(&'a DisplacementOutput),
    Profiles(&'a [StoryForceCaseProfile], &'a [String]),
}

impl Snapshot {
    fn source(&self, response: Response, direction: Direction) -> Option<Source<'_>> {
        fn pick<T>(direction: Direction, x: T, y: T) -> T {
            match direction {
                Direction::X => x,
                Direction::Y => y,
            }
        }
        let calc = &self.calc;
        match response {
            Response::WindDrift => calc
                .drift_wind
                .as_ref()
                .map(|d| Source::Drift(pick(direction, &d.x, &d.y))),
            Response::SeismicDrift => calc
                .drift_seismic
                .as_ref()
                .map(|d| Source::Drift(pick(direction, &d.x, &d.y))),
            Response::WindDisplacement => calc
                .displacement_wind
                .as_ref()
                .map(|d| Source::Displacement(pick(direction, &d.x, &d.y))),
            Response::StoryShear | Response::StoryMoment => {
                calc.story_forces.as_ref().map(|forces| {
                    Source::Profiles(
                        pick(direction, &forces.x_profiles[..], &forces.y_profiles[..]),
                        &forces.story_order,
                    )
                })
            }
        }
    }

    /// The responses these cached results hold, in [`Response::ALL`] order.
    #[must_use]
    pub fn responses(&self) -> Vec<Response> {
        Response::ALL
            .into_iter()
            .filter(|&response| self.source(response, Direction::X).is_some())
            .collect()
    }

    /// The groups stored for one response and direction, and the cases that
    /// have rows for `group` (all cases when `group` is `None` or the
    /// response has no groups), so a group/case pair offered always plots.
    ///
    /// # Errors
    /// The response is not in these cached results.
    pub fn response_choices(
        &self,
        response: Response,
        direction: Direction,
        group: Option<&str>,
    ) -> Result<Choices, String> {
        let in_group = |name: &String| group.is_none_or(|g| name == g);
        let sorted = |items: Vec<&String>| {
            items
                .into_iter()
                .cloned()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
        };
        Ok(
            match self
                .source(response, direction)
                .ok_or_else(|| missing(response))?
            {
                Source::Drift(drift) => Choices {
                    groups: sorted(drift.rows.iter().map(|r| &r.group_name).collect()),
                    cases: sorted(
                        drift
                            .rows
                            .iter()
                            .filter(|r| in_group(&r.group_name))
                            .map(|r| &r.output_case)
                            .collect(),
                    ),
                },
                Source::Displacement(disp) => Choices {
                    groups: sorted(disp.rows.iter().map(|r| &r.group_name).collect()),
                    cases: sorted(
                        disp.rows
                            .iter()
                            .filter(|r| in_group(&r.group_name))
                            .map(|r| &r.output_case)
                            .collect(),
                    ),
                },
                Source::Profiles(profiles, _) => Choices {
                    groups: Vec::new(),
                    cases: profiles.iter().map(|p| p.output_case.clone()).collect(),
                },
            },
        )
    }

    /// One stored response profile for a group (envelope rows only) and case.
    ///
    /// # Errors
    /// A missing response, group or case; a missing, duplicate or unknown
    /// story; or a non-finite stored value. Nothing is filled in.
    pub fn response(
        &self,
        response: Response,
        direction: Direction,
        group: Option<&str>,
        case: &str,
    ) -> Result<Plot, String> {
        let axis = match direction {
            Direction::X => "X",
            Direction::Y => "Y",
        };
        let units = &self.calc.meta.units;
        match self
            .source(response, direction)
            .ok_or_else(|| missing(response))?
        {
            Source::Drift(drift) => {
                let group = group.ok_or("Choose a group")?;
                if !drift.allowable_ratio.is_finite() || drift.allowable_ratio < 0.0 {
                    return Err("Invalid cached drift reference".into());
                }
                let rows: Vec<_> = drift
                    .rows
                    .iter()
                    .filter(|r| r.group_name == group && r.output_case == case)
                    .collect();
                let series = match direction {
                    Direction::X => envelope(
                        &drift.story_order,
                        &rows,
                        |r| &r.story,
                        |r| r.max_drift_x_pos,
                        |r| r.max_drift_x_neg,
                    ),
                    Direction::Y => envelope(
                        &drift.story_order,
                        &rows,
                        |r| &r.story,
                        |r| r.max_drift_y_pos,
                        |r| r.max_drift_y_neg,
                    ),
                }?;
                Ok(Plot {
                    title: format!("{} {axis} · {group} · {case}", response.label()),
                    unit: "ratio".into(),
                    stories: drift.story_order.clone(),
                    series,
                    reference: Some(drift.allowable_ratio),
                })
            }
            Source::Displacement(disp) => {
                let group = group.ok_or("Choose a group")?;
                let rows: Vec<_> = disp
                    .rows
                    .iter()
                    .filter(|r| r.group_name == group && r.output_case == case)
                    .collect();
                let mut series = match direction {
                    Direction::X => envelope(
                        &disp.story_order,
                        &rows,
                        |r| &r.story,
                        |r| r.max_disp_x_pos_ft,
                        |r| r.max_disp_x_neg_ft,
                    ),
                    Direction::Y => envelope(
                        &disp.story_order,
                        &rows,
                        |r| &r.story,
                        |r| r.max_disp_y_pos_ft,
                        |r| r.max_disp_y_neg_ft,
                    ),
                }?;
                if !disp.story_limits.is_empty() {
                    validate_membership(
                        &disp.story_order,
                        disp.story_limits.iter().map(|r| &r.story),
                    )?;
                    let limits = column(
                        &disp.story_order,
                        &disp.story_limits,
                        |r| &r.story,
                        |r| r.limit_ft,
                    );
                    finite(&limits)?;
                    series.push(Series {
                        name: "Stored story limit".into(),
                        values: limits,
                    });
                }
                Ok(Plot {
                    title: format!("{} {axis} · {group} · {case}", response.label()),
                    unit: units.length.clone(),
                    stories: disp.story_order.clone(),
                    series,
                    reference: None,
                })
            }
            Source::Profiles(profiles, order) => {
                profile_plot(response, direction, profiles, order, case, units)
            }
        }
    }
}

type Row = ext_calc::output::StoryForceCaseRow;
/// A stored field of a story-force case row: its name and how to read it.
type Field = (&'static str, fn(&Row) -> f64);

/// Shear or moment profile from the X- or Y-direction case profiles.
fn profile_plot(
    response: Response,
    direction: Direction,
    profiles: &[StoryForceCaseProfile],
    order: &[String],
    case: &str,
    units: &ext_calc::output::UnitLabels,
) -> Result<Plot, String> {
    let axis = match direction {
        Direction::X => "X",
        Direction::Y => "Y",
    };
    validate_order(order)?;
    let profile = profiles
        .iter()
        .find(|p| p.output_case == case)
        .ok_or("No cached profile for this case")?;
    validate_membership(order, profile.rows.iter().map(|r| &r.story))?;
    // The direction picks the stored X or Y profile list; the
    // fields read are named, never re-labelled as a physical axis.
    let fields: &[Field] = match (response, direction) {
        (Response::StoryShear, Direction::X) => &[("Vx", |r: &Row| r.vx_kip)],
        (Response::StoryShear, Direction::Y) => &[("Vy", |r: &Row| r.vy_kip)],
        _ => &[("Mx", |r: &Row| r.mx_kip_ft), ("My", |r: &Row| r.my_kip_ft)],
    };
    let unit = if response == Response::StoryShear {
        &units.force
    } else {
        &units.moment
    };
    let mut series = Vec::new();
    for (field, read) in fields {
        let values = column(order, &profile.rows, |r| &r.story, read);
        finite(&values)?;
        series.push(Series {
            name: format!("{field} (stored)"),
            values,
        });
    }
    Ok(Plot {
        title: format!("{} · {axis}-direction profiles · {case}", response.label()),
        unit: unit.clone(),
        stories: order.to_vec(),
        series,
        reference: None,
    })
}

fn missing(response: Response) -> String {
    format!(
        "No cached {} output in these results",
        response.label().to_lowercase()
    )
}

/// Stored positive and negative envelope fields as two series, in story order.
fn envelope<R>(
    order: &[String],
    rows: &[&R],
    story: impl Fn(&R) -> &String,
    pos: impl Fn(&R) -> f64,
    neg: impl Fn(&R) -> f64,
) -> Result<Vec<Series>, String> {
    validate_order(order)?;
    if rows.is_empty() {
        return Err("No cached rows for this group and case".into());
    }
    validate_membership(order, rows.iter().map(|r| story(r)))?;
    let positive = column(order, rows, |r: &&R| story(r), |r: &&R| pos(r));
    let negative = column(order, rows, |r: &&R| story(r), |r: &&R| neg(r));
    finite(&positive)?;
    finite(&negative)?;
    Ok(vec![
        Series {
            name: "Stored positive envelope".into(),
            values: positive,
        },
        Series {
            name: "Stored negative envelope".into(),
            values: negative,
        },
    ])
}

/// One value per story in `order`; a story without a row is a gap.
fn column<R>(
    order: &[String],
    rows: &[R],
    story: impl Fn(&R) -> &String,
    value: impl Fn(&R) -> f64,
) -> Vec<Option<f64>> {
    order
        .iter()
        .map(|name| rows.iter().find(|r| story(r) == name).map(&value))
        .collect()
}

fn finite(values: &[Option<f64>]) -> Result<(), String> {
    crate::data::validate_values(values)
}

#[cfg(test)]
#[path = "response_tests.rs"]
mod tests;
