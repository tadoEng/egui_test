//! Authored dashboard fixture and protocol. No engineering calculation/backend authority.
use serde::{Deserialize, Serialize};

pub(crate) const DATASET: &str = "LAB-DASH-01";
pub(crate) const REVISION: u32 = 1;
pub(crate) const STORIES: u16 = 69;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Case {
    A,
    B,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Direction {
    X,
    Y,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Units {
    MillimetresKilonewtons,
    MetresNewtons,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Config {
    pub first: u16,
    pub last: u16,
    pub case: Case,
    pub direction: Direction,
    pub units: Units,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            first: 1,
            last: STORIES,
            case: Case::A,
            direction: Direction::X,
            units: Units::MillimetresKilonewtons,
        }
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct State {
    config: Config,
    generation: u64,
    selected: Option<u16>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            config: Config::default(),
            generation: 1,
            selected: Some(1),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct Row {
    pub story: u16,
    pub drift: f64,
    pub shear: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct Packet {
    pub version: u8,
    pub dataset: &'static str,
    pub revision: u32,
    pub generation: u64,
    pub first: u16,
    pub last: u16,
    pub case: &'static str,
    pub direction: &'static str,
    pub drift_unit: &'static str,
    pub shear_unit: &'static str,
    pub drift_reference: f64,
    pub shear_reference: f64,
    pub selected_story: Option<u16>,
    pub rows: Vec<Row>,
}

impl State {
    pub fn config(&self) -> Config {
        self.config
    }
    pub fn selected(&self) -> Option<u16> {
        self.selected
    }
    pub fn reconfigure(&mut self, config: Config) -> Result<bool, &'static str> {
        if config.first == 0 || config.last > STORIES || config.first > config.last {
            return Err("Invalid synthetic story range");
        }
        if config == self.config {
            return Ok(false);
        }
        let next = self
            .generation
            .checked_add(1)
            .ok_or("Synthetic generation exhausted")?;
        self.config = config;
        self.generation = next;
        if self
            .selected
            .is_some_and(|story| story < config.first || story > config.last)
        {
            self.selected = Some(config.first);
        }
        Ok(true)
    }
    pub fn select(&mut self, story: Option<u16>) -> Result<bool, &'static str> {
        if story.is_some_and(|s| s < self.config.first || s > self.config.last) {
            return Err("Story is not in the current visible fixture range");
        }
        let changed = self.selected != story;
        self.selected = story;
        Ok(changed)
    }
    pub fn packet(&self) -> Packet {
        let case = match self.config.case {
            Case::A => 1.0,
            Case::B => 2.0,
        };
        let direction = match self.config.direction {
            Direction::X => 1.0,
            Direction::Y => 0.5,
        };
        let (drift_scale, shear_scale, drift_unit, shear_unit) = match self.config.units {
            Units::MillimetresKilonewtons => (1000.0, 0.001, "mm", "kN"),
            Units::MetresNewtons => (1.0, 1.0, "m", "N"),
        };
        // Authored signal formula, NOT analysis results or an ext-calc replacement.
        let rows = (self.config.first..=self.config.last)
            .map(|story| Row {
                story,
                drift: f64::from(story) * 0.0001 * case * direction * drift_scale,
                shear: f64::from(70 - story) * 10000.0 * case * direction * shear_scale,
            })
            .collect();
        Packet {
            version: 2,
            dataset: DATASET,
            revision: REVISION,
            generation: self.generation,
            first: self.config.first,
            last: self.config.last,
            case: match self.config.case {
                Case::A => "Fixture A",
                Case::B => "Fixture B",
            },
            direction: match self.config.direction {
                Direction::X => "X",
                Direction::Y => "Y",
            },
            drift_unit,
            shear_unit,
            drift_reference: 0.003 * drift_scale,
            shear_reference: 500000.0 * shear_scale,
            selected_story: self.selected,
            rows,
        }
    }
    pub fn accept(&mut self, text: &str) -> Result<bool, &'static str> {
        if text.len() > 1024 {
            return Err("Oversized dashboard message");
        }
        let event: Selection = serde_json::from_str(text)
            .map_err(|_untrusted_error| "Malformed dashboard selection")?;
        if event.version != 2
            || event.dataset != DATASET
            || event.revision != REVISION
            || event.generation != self.generation
        {
            return Err("Retired or unknown dashboard context");
        }
        match event.kind {
            Event::SelectStory => self.select(Some(event.story.ok_or("Missing story")?)),
            Event::ClearSelection if event.story.is_none() => self.select(None),
            Event::ClearSelection => Err("Clear selection cannot carry a story"),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Event {
    SelectStory,
    ClearSelection,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Plot {
    Drift,
    Shear,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    version: u8,
    #[serde(rename = "type")]
    kind: Event,
    dataset: String,
    revision: u32,
    generation: u64,
    #[serde(rename = "plot")]
    _plot: Plot,
    story: Option<u16>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Ready {
    version: u8,
    #[serde(rename = "type")]
    kind: String,
}
pub(crate) fn is_ready(text: &str) -> bool {
    text.len() <= 128
        && serde_json::from_str::<Ready>(text)
            .is_ok_and(|r| r.version == 2 && r.kind == "dashboard_ready")
}

pub(crate) fn packet_script(packet: &Packet) -> Result<String, serde_json::Error> {
    serde_json::to_string(packet).map(|json| format!("window.dashboardBridge.apply({json});"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn click(generation: u64, story: u16) -> String {
        format!(
            r#"{{"version":2,"type":"select_story","dataset":"LAB-DASH-01","revision":1,"generation":{generation},"plot":"drift","story":{story}}}"#
        )
    }
    #[test]
    fn authored_values_ranges_and_display_units_are_consistent() {
        let mut s = State::default();
        let p = s.packet();
        assert_eq!(p.rows.len(), 69);
        assert_eq!(
            p.rows[0],
            Row {
                story: 1,
                drift: 0.1,
                shear: 690.0
            }
        );
        assert!((p.rows[68].drift - 6.9).abs() < 1.0e-12);
        assert_eq!(p.rows[68].shear, 10.0);
        s.reconfigure(Config {
            first: 10,
            last: 12,
            case: Case::B,
            direction: Direction::Y,
            units: Units::MetresNewtons,
        })
        .expect("valid config");
        let q = s.packet();
        assert_eq!(q.rows.len(), 3);
        assert_eq!(
            q.rows[0],
            Row {
                story: 10,
                drift: 0.001,
                shear: 600000.0
            }
        );
        assert_eq!(q.selected_story, Some(10));
        assert_eq!((q.drift_unit, q.shear_unit), ("m", "N"));
        assert_eq!((q.drift_reference, q.shear_reference), (0.003, 500000.0));
    }
    #[test]
    fn switches_a_b_a_reject_old_events_and_repeated_selection_is_idempotent() {
        let mut s = State::default();
        let original = s.config();
        assert_eq!(s.accept(&click(1, 3)), Ok(true));
        assert_eq!(s.accept(&click(1, 3)), Ok(false));
        s.reconfigure(Config {
            case: Case::B,
            ..original
        })
        .expect("B");
        s.reconfigure(original).expect("A");
        assert!(s.accept(&click(1, 4)).is_err());
        assert_eq!(s.accept(&click(3, 4)), Ok(true));
        assert_eq!(s.packet().generation, 3);
    }
    #[test]
    fn invalid_context_story_shape_and_plot_are_refused() {
        let mut s = State::default();
        for bad in [
            click(1, 0),
            click(1, 70),
            click(1, 2).replace("LAB-DASH-01", "unknown"),
            click(1, 2).replace("\"revision\":1", "\"revision\":2"),
            click(1, 2).replace("drift", "arbitrary"),
            click(1, 2).replace("\"story\":2", "\"story\":2,\"injected\":true"),
            "x".repeat(1025),
        ] {
            assert!(s.accept(&bad).is_err(), "refuse {bad}");
        }
        assert_eq!(s.selected(), Some(1));
    }
    #[test]
    fn invalid_range_preserves_last_good_state_and_filtered_events_are_rejected() {
        let mut s = State::default();
        let good = s.packet();
        assert!(
            s.reconfigure(Config {
                first: 20,
                last: 10,
                ..s.config()
            })
            .is_err()
        );
        assert_eq!(s.packet(), good);
        s.reconfigure(Config {
            first: 10,
            last: 12,
            ..s.config()
        })
        .expect("valid range");
        assert!(s.accept(&click(2, 13)).is_err());
        assert_eq!(s.selected(), Some(10));
    }
    #[test]
    fn ready_and_serialized_packet_have_versioned_strict_shapes() {
        assert!(is_ready(r#"{"version":2,"type":"dashboard_ready"}"#));
        assert!(!is_ready(
            r#"{"version":2,"type":"dashboard_ready","extra":1}"#
        ));
        assert!(!is_ready(r#"{"version":1,"type":"dashboard_ready"}"#));
        let packet = State::default().packet();
        let json = serde_json::to_value(&packet).expect("serialize fixture");
        assert_eq!(json["rows"].as_array().expect("rows").len(), 69);
        assert_eq!(json["dataset"], DATASET);
        assert_eq!(json["generation"], 1);
        let script = packet_script(&packet).expect("serialize script");
        assert!(script.starts_with("window.dashboardBridge.apply({"));
        assert!(script.ends_with(");"));
    }
    #[test]
    fn clear_selection_does_not_create_an_echo_loop() {
        let mut s = State::default();
        let text = r#"{"version":2,"type":"clear_selection","dataset":"LAB-DASH-01","revision":1,"generation":1,"plot":"shear","story":null}"#;
        assert_eq!(s.accept(text), Ok(true));
        assert_eq!(s.accept(text), Ok(false));
        assert_eq!(s.packet().selected_story, None);
    }
}
