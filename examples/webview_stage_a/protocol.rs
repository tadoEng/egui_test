use serde::{Deserialize, Serialize};
use std::fmt;

pub const PROTOCOL_VERSION: u8 = 1;
pub const STORY_COUNT: u8 = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Story(u8);

impl Story {
    pub const DEFAULT: Self = Self(3);

    pub fn get(self) -> u8 {
        self.0
    }

    pub fn all() -> impl Iterator<Item = Self> {
        (1..=STORY_COUNT).map(Self)
    }
}

impl TryFrom<u8> for Story {
    type Error = ProtocolError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if (1..=STORY_COUNT).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ProtocolError::StoryOutOfRange(value))
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChartSelection {
    version: u8,
    #[serde(rename = "type")]
    message_type: ChartMessageType,
    story: u8,
}

#[derive(Debug, Deserialize)]
enum ChartMessageType {
    #[serde(rename = "select_story")]
    SelectStory,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChartReady {
    version: u8,
    #[serde(rename = "type")]
    message_type: ReadyMessageType,
}

#[derive(Debug, Deserialize)]
enum ReadyMessageType {
    #[serde(rename = "chart_ready")]
    ChartReady,
}

pub fn is_chart_ready(message: &str) -> bool {
    message.len() <= 1024
        && serde_json::from_str::<ChartReady>(message).is_ok_and(|ready| {
            ready.version == PROTOCOL_VERSION
                && matches!(ready.message_type, ReadyMessageType::ChartReady)
        })
}

#[derive(Debug, Serialize)]
struct HostSelection {
    version: u8,
    #[serde(rename = "type")]
    message_type: &'static str,
    story: u8,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ProtocolError {
    InvalidJson(String),
    UnsupportedVersion(u8),
    StoryOutOfRange(u8),
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(error) => write!(formatter, "invalid chart message: {error}"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported chart protocol version {version}")
            }
            Self::StoryOutOfRange(story) => {
                write!(formatter, "story {story} is outside 1..={STORY_COUNT}")
            }
        }
    }
}

impl std::error::Error for ProtocolError {}

pub fn parse_chart_selection(message: &str) -> Result<Story, ProtocolError> {
    if message.len() > 1024 {
        return Err(ProtocolError::InvalidJson(
            "message exceeds 1024 bytes".into(),
        ));
    }
    let selection: ChartSelection = serde_json::from_str(message)
        .map_err(|error| ProtocolError::InvalidJson(error.to_string()))?;
    if selection.version != PROTOCOL_VERSION {
        return Err(ProtocolError::UnsupportedVersion(selection.version));
    }
    match selection.message_type {
        ChartMessageType::SelectStory => Story::try_from(selection.story),
    }
}

pub fn host_selection_script(story: Story) -> Result<String, serde_json::Error> {
    let message = HostSelection {
        version: PROTOCOL_VERSION,
        message_type: "select_story",
        story: story.get(),
    };
    let json = serde_json::to_string(&message)?;
    Ok(format!("window.hostBridge.receive({json});"))
}

#[derive(Debug)]
pub struct SelectionState {
    selected: Story,
}

impl Default for SelectionState {
    fn default() -> Self {
        Self {
            selected: Story::DEFAULT,
        }
    }
}

impl SelectionState {
    pub fn selected(&self) -> Story {
        self.selected
    }

    pub fn select(&mut self, story: Story) -> bool {
        if self.selected == story {
            false
        } else {
            self.selected = story;
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_chart_selection_is_accepted() {
        assert_eq!(
            parse_chart_selection(r#"{"version":1,"type":"select_story","story":6}"#),
            Ok(Story(6))
        );
    }

    #[test]
    fn ready_message_requires_known_version_type_and_exact_fields() {
        assert!(is_chart_ready(r#"{"version":1,"type":"chart_ready"}"#));
        assert!(!is_chart_ready(r#"{"version":2,"type":"chart_ready"}"#));
        assert!(!is_chart_ready(
            r#"{"version":1,"type":"chart_ready","story":3}"#
        ));
        assert!(!is_chart_ready("invalid"));
    }

    #[test]
    fn malformed_unknown_and_out_of_range_messages_are_rejected() {
        let malformed = parse_chart_selection("not json");
        let unknown = parse_chart_selection(
            r#"{"version":1,"type":"select_story","story":3,"project":"untrusted"}"#,
        );
        let wrong_type = parse_chart_selection(r#"{"version":1,"type":"open_url","story":3}"#);
        let out_of_range =
            parse_chart_selection(r#"{"version":1,"type":"select_story","story":7}"#);

        assert!(matches!(malformed, Err(ProtocolError::InvalidJson(_))));
        assert!(matches!(unknown, Err(ProtocolError::InvalidJson(_))));
        assert!(matches!(wrong_type, Err(ProtocolError::InvalidJson(_))));
        assert_eq!(out_of_range, Err(ProtocolError::StoryOutOfRange(7)));
    }

    #[test]
    fn unsupported_protocol_version_is_rejected() {
        assert_eq!(
            parse_chart_selection(r#"{"version":2,"type":"select_story","story":3}"#),
            Err(ProtocolError::UnsupportedVersion(2))
        );
    }

    #[test]
    fn oversized_chart_message_is_rejected_before_json_parsing() {
        assert!(matches!(
            parse_chart_selection(&" ".repeat(1025)),
            Err(ProtocolError::InvalidJson(_))
        ));
    }

    #[test]
    fn repeated_selection_does_not_create_a_state_transition() {
        let mut state = SelectionState::default();
        assert!(!state.select(Story::DEFAULT));
        assert!(state.select(Story(4)));
        assert!(!state.select(Story(4)));
    }

    #[test]
    fn host_message_is_json_serialized_and_allow_listed() {
        assert_eq!(
            host_selection_script(Story(2)).expect("fixed message serializes"),
            r#"window.hostBridge.receive({"version":1,"type":"select_story","story":2});"#
        );
    }
}
