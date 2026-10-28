//! The typed contract between the Results page and an embedded chart.
//!
//! On Windows the Results chart is `ECharts` in a `WebView2` child window
//! (`webview_chart`). This module is the platform-independent part, so it is
//! tested everywhere: the host sends a [`Frame`] (one stored profile of one
//! version's cached results, tagged with the version, the results' source
//! fingerprint and a generation) and the chart answers with small JSON
//! messages. A selection is accepted only when it names the frame shown now;
//! anything else is refused and counted, never applied.
//!
//! The chart draws stored values only. It never receives a path, a file or
//! anything it could recalculate from.
use crate::state::{Action, Workspace};
use lab_results::data::Plot;
use serde::{Deserialize, Serialize};

/// The message protocol version, in every frame and every message.
pub const PROTOCOL: u8 = 1;
/// The longest message accepted from the chart, in bytes. The transport drops
/// longer ones before they reach [`Bridge::accept`].
pub const MAX_MESSAGE: usize = 1024;

/// A chart drawn outside egui, in a native child window over the Results
/// chart area. Unavailability is explicit; there is no renderer fallback.
pub trait EmbeddedChart {
    /// Whether the host can draw. No other renderer replaces an unavailable host.
    fn available(&self) -> bool;
    /// An explicit host failure, visible in the native shell.
    fn error(&self) -> Option<&str> {
        (!self.available()).then_some("ECharts / WebView2 unavailable")
    }
    /// Called once per frame: shows `content` (or nothing) placed over `area`
    /// (hidden when `None`), and returns the actions the chart's accepted
    /// messages ask for.
    fn sync(
        &mut self,
        ctx: &egui::Context,
        area: Option<egui::Rect>,
        content: Option<Content>,
    ) -> Vec<Action>;
}

/// Colours from the TALOS theme, as CSS colours, so the chart matches the
/// card it sits in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Style {
    pub background: String,
    pub text: String,
    pub muted: String,
    pub grid: String,
    pub axis: String,
    pub selected: String,
    pub reference: String,
    /// One per series, in series order.
    pub series: Vec<String>,
}

/// A CSS `rgba()` colour.
pub fn css(color: egui::Color32) -> String {
    let [r, g, b, a] = color.to_srgba_unmultiplied();
    format!("rgba({r},{g},{b},{:.3})", f32::from(a) / 255.0)
}

/// What the Results page charts this frame.
#[derive(Debug, PartialEq)]
pub struct Content {
    /// The version shown, as `branch/version`.
    pub version: String,
    /// The cached results' source fingerprint.
    pub source: String,
    /// The stored profile, cut to the story range.
    pub plot: Plot,
    pub selected: Option<String>,
    pub style: Style,
}

/// What the chart is sent.
#[derive(Debug, PartialEq, Serialize)]
pub struct Frame {
    pub protocol: u8,
    pub version: String,
    pub source: String,
    pub generation: u64,
    pub selected_story: Option<String>,
    pub plot: Plot,
    pub style: Style,
}

/// Why a message from the chart was not applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    Oversized,
    /// Not JSON, an unknown field, or fields that do not fit its type.
    Malformed,
    /// Another protocol, version, source or generation, or nothing charted.
    Stale,
    /// A story the frame shown does not have.
    UnknownStory,
    UnknownKind,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Message {
    protocol: u8,
    #[serde(rename = "type")]
    kind: String,
    version: Option<String>,
    source: Option<String>,
    generation: Option<u64>,
    story: Option<String>,
    key: Option<String>,
}

/// Whether `raw` is the chart page's one-time ready message.
pub fn is_ready(raw: &str) -> bool {
    raw.len() <= MAX_MESSAGE
        && serde_json::from_str::<Message>(raw).is_ok_and(|m| {
            m.protocol == PROTOCOL
                && m.kind == "ready"
                && m.version.is_none()
                && m.source.is_none()
                && m.generation.is_none()
                && m.story.is_none()
                && m.key.is_none()
        })
}

/// The host side of the contract: which frame the chart shows and whether it
/// has been sent.
#[derive(Debug, Default)]
pub struct Bridge {
    frame: Option<Frame>,
    /// The last generation issued; never reused, even after nothing was shown.
    generation: u64,
    /// The generation and selection last sent.
    sent: Option<(u64, Option<String>)>,
    refused: u64,
}

impl Bridge {
    /// Takes what the page charts this frame (`None`: nothing). New content
    /// (version, source, profile or style) starts a new generation, so
    /// messages about the old one are refused. A new selection alone keeps
    /// the generation.
    ///
    /// # Errors
    /// The generation counter is exhausted; nothing more can be sent.
    pub fn update(&mut self, content: Option<Content>) -> Result<(), String> {
        let Some(content) = content else {
            self.frame = None;
            return Ok(());
        };
        if let Some(frame) = &mut self.frame
            && frame.version == content.version
            && frame.source == content.source
            && frame.plot == content.plot
            && frame.style == content.style
        {
            frame.selected_story = content.selected;
            return Ok(());
        }
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or("Chart generation exhausted")?;
        self.frame = Some(Frame {
            protocol: PROTOCOL,
            version: content.version,
            source: content.source,
            generation: self.generation,
            selected_story: content.selected,
            plot: content.plot,
            style: content.style,
        });
        Ok(())
    }

    /// The frame shown now, if any.
    pub fn frame(&self) -> Option<&Frame> {
        self.frame.as_ref()
    }

    /// How many messages have been refused.
    pub fn refused(&self) -> u64 {
        self.refused
    }

    /// The frame as JSON when the chart has not been sent this generation and
    /// selection yet; marks it sent.
    ///
    /// # Errors
    /// The frame could not be serialized; it stays unsent.
    pub fn outgoing(&mut self) -> Option<Result<String, String>> {
        let frame = self.frame.as_ref()?;
        let key = (frame.generation, frame.selected_story.clone());
        if self.sent.as_ref() == Some(&key) {
            return None;
        }
        Some(match serde_json::to_string(frame) {
            Ok(json) => {
                self.sent = Some(key);
                Ok(json)
            }
            Err(error) => Err(format!("Serialize chart frame: {error}")),
        })
    }

    /// Delivery failed: send the current frame again next time.
    pub fn resend(&mut self) {
        self.sent = None;
    }

    /// A message from the chart, as the action it asks for.
    ///
    /// # Errors
    /// Why it was refused; refusals are counted.
    pub fn accept(&mut self, raw: &str) -> Result<Action, Refused> {
        let result = self.check(raw);
        if result.is_err() {
            self.refused = self.refused.saturating_add(1);
        }
        result
    }

    fn check_identity(&self, message: &Message) -> Result<&Frame, Refused> {
        let frame = self.frame.as_ref().ok_or(Refused::Stale)?;
        if message.version.as_deref() != Some(frame.version.as_str())
            || message.source.as_deref() != Some(frame.source.as_str())
            || message.generation != Some(frame.generation)
        {
            return Err(Refused::Stale);
        }
        Ok(frame)
    }

    fn check(&self, raw: &str) -> Result<Action, Refused> {
        if raw.len() > MAX_MESSAGE {
            return Err(Refused::Oversized);
        }
        let message: Message =
            serde_json::from_str(raw).map_err(|_malformed| Refused::Malformed)?;
        if message.protocol != PROTOCOL {
            return Err(Refused::Stale);
        }
        match message.kind.as_str() {
            "shortcut" => {
                self.check_identity(&message)?;
                shortcut(&message)
            }
            "select_story" | "clear_selection" => {
                let frame = self.check_identity(&message)?;
                if message.key.is_some() {
                    return Err(Refused::Malformed);
                }
                match (message.kind.as_str(), message.story) {
                    ("select_story", Some(story)) if frame.plot.stories.contains(&story) => {
                        Ok(Action::SelectResultStory(Some(story)))
                    }
                    ("select_story", Some(_)) => Err(Refused::UnknownStory),
                    ("clear_selection", None) => Ok(Action::SelectResultStory(None)),
                    _ => Err(Refused::Malformed),
                }
            }
            _ => Err(Refused::UnknownKind),
        }
    }
}

/// The workspace shortcuts, forwarded while the chart has keyboard focus
/// (the child window receives the keys, not egui). Nothing else is forwarded.
fn shortcut(message: &Message) -> Result<Action, Refused> {
    if message.story.is_some() {
        return Err(Refused::Malformed);
    }
    let digit = |n: usize| Ok(Action::SelectWorkspace(Workspace::ALL[n]));
    match message.key.as_deref() {
        Some("Digit1") => digit(0),
        Some("Digit2") => digit(1),
        Some("Digit3") => digit(2),
        Some("Digit4") => digit(3),
        Some("Digit5") => digit(4),
        Some("PageDown") => Ok(Action::NextWorkspace),
        Some("PageUp") => Ok(Action::PreviousWorkspace),
        _ => Err(Refused::UnknownKind),
    }
}

#[cfg(test)]
#[path = "chart_bridge_tests.rs"]
mod tests;
