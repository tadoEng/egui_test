use super::*;
use lab_results::data::Series;

fn style() -> Style {
    Style {
        background: css(egui::Color32::WHITE),
        text: css(egui::Color32::BLACK),
        muted: css(egui::Color32::GRAY),
        grid: css(egui::Color32::LIGHT_GRAY),
        axis: css(egui::Color32::GRAY),
        selected: css(egui::Color32::from_rgba_unmultiplied(0, 128, 128, 64)),
        reference: css(egui::Color32::RED),
        series: vec![css(egui::Color32::BLUE)],
    }
}

fn plot(title: &str) -> Plot {
    Plot {
        title: title.into(),
        unit: "kN".into(),
        stories: vec!["Roof".into(), "L2".into()],
        series: vec![Series {
            name: "Vx".into(),
            values: vec![Some(-12.5), None],
        }],
        reference: None,
    }
}

fn content(title: &str, selected: Option<&str>) -> Content {
    Content {
        version: "main/v2".into(),
        source: "abc123".into(),
        plot: plot(title),
        selected: selected.map(Into::into),
        style: style(),
    }
}

fn select(generation: u64, story: &str) -> String {
    serde_json::json!({
        "protocol": 1, "type": "select_story", "version": "main/v2",
        "source": "abc123", "generation": generation, "story": story,
    })
    .to_string()
}

fn generation(bridge: &Bridge) -> u64 {
    bridge.frame().map_or(0, |frame| frame.generation)
}

#[test]
fn a_frame_carries_identity_and_stored_values_with_gaps() {
    let mut bridge = Bridge::default();
    bridge
        .update(Some(content("Story shear", Some("L2"))))
        .expect("valid JSON");
    let json: serde_json::Value =
        serde_json::from_str(&bridge.outgoing().expect("a frame").expect("serialized"))
            .expect("valid JSON");
    assert_eq!(json["protocol"], 1);
    assert_eq!(json["version"], "main/v2");
    assert_eq!(json["source"], "abc123");
    assert_eq!(json["generation"], 1);
    assert_eq!(json["selected_story"], "L2");
    assert_eq!(json["plot"]["stories"], serde_json::json!(["Roof", "L2"]));
    assert_eq!(
        json["plot"]["series"][0]["values"],
        serde_json::json!([-12.5, null])
    );
    assert!(
        json["style"]["series"][0]
            .as_str()
            .expect("valid JSON")
            .starts_with("rgba(")
    );
    // Sent once; nothing new until the frame or the selection changes.
    assert!(bridge.outgoing().is_none());
}

#[test]
fn a_new_selection_resends_without_a_new_generation_and_new_content_starts_one() {
    let mut bridge = Bridge::default();
    bridge
        .update(Some(content("Story shear", None)))
        .expect("update");
    bridge.outgoing();
    bridge
        .update(Some(content("Story shear", Some("Roof"))))
        .expect("valid JSON");
    assert_eq!(generation(&bridge), 1);
    assert!(bridge.outgoing().is_some());
    bridge
        .update(Some(content("Story moment", Some("Roof"))))
        .expect("valid JSON");
    assert_eq!(generation(&bridge), 2);
    assert!(bridge.outgoing().is_some());
    // Nothing charted, then the same content again: still a new generation,
    // so nothing aimed at the earlier one can land.
    bridge.update(None).expect("update");
    assert!(bridge.outgoing().is_none());
    bridge
        .update(Some(content("Story moment", Some("Roof"))))
        .expect("valid JSON");
    assert_eq!(generation(&bridge), 3);
}

#[test]
fn a_failed_delivery_is_sent_again() {
    let mut bridge = Bridge::default();
    bridge
        .update(Some(content("Story shear", None)))
        .expect("update");
    assert!(bridge.outgoing().is_some());
    bridge.resend();
    assert!(bridge.outgoing().is_some());
}

#[test]
fn selections_are_accepted_only_for_the_frame_shown() {
    let mut bridge = Bridge::default();
    // Nothing charted yet: refused.
    assert_eq!(bridge.accept(&select(1, "L2")).err(), Some(Refused::Stale));
    bridge
        .update(Some(content("Story shear", None)))
        .expect("update");
    assert!(matches!(
        bridge.accept(&select(1, "L2")),
        Ok(Action::SelectResultStory(Some(story))) if story == "L2"
    ));
    let clear = serde_json::json!({
        "protocol": 1, "type": "clear_selection", "version": "main/v2",
        "source": "abc123", "generation": 1, "story": null,
    })
    .to_string();
    assert!(matches!(
        bridge.accept(&clear),
        Ok(Action::SelectResultStory(None))
    ));
    // The query changes: a click on the previous chart arrives late.
    bridge
        .update(Some(content("Story moment", None)))
        .expect("update");
    assert_eq!(bridge.accept(&select(1, "L2")).err(), Some(Refused::Stale));
    assert!(bridge.accept(&select(2, "L2")).is_ok());
    // Another version or source with the right generation is still refused.
    for (field, value) in [("version", "main/v1"), ("source", "other")] {
        let mut message: serde_json::Value =
            serde_json::from_str(&select(2, "L2")).expect("valid JSON");
        message[field] = value.into();
        assert_eq!(
            bridge.accept(&message.to_string()).err(),
            Some(Refused::Stale),
            "{field}"
        );
    }
    assert_eq!(
        bridge.accept(&select(2, "L9")).err(),
        Some(Refused::UnknownStory)
    );
    assert_eq!(bridge.refused(), 5);
}

#[test]
fn malformed_oversized_and_unknown_messages_are_refused() {
    let mut bridge = Bridge::default();
    bridge
        .update(Some(content("Story shear", None)))
        .expect("update");
    let mut extra: serde_json::Value = serde_json::from_str(&select(1, "L2")).expect("valid JSON");
    extra["script"] = "alert(1)".into();
    let mut protocol: serde_json::Value =
        serde_json::from_str(&select(1, "L2")).expect("valid JSON");
    protocol["protocol"] = 2.into();
    let clear_with_story = select(1, "L2").replace("select_story", "clear_selection");
    let long = select(1, &"L".repeat(MAX_MESSAGE));
    for (raw, why) in [
        ("not json", Refused::Malformed),
        (extra.to_string().as_str(), Refused::Malformed),
        (protocol.to_string().as_str(), Refused::Stale),
        (clear_with_story.as_str(), Refused::Malformed),
        (r#"{"protocol":1,"type":"navigate"}"#, Refused::UnknownKind),
        (long.as_str(), Refused::Oversized),
    ] {
        assert_eq!(bridge.accept(raw).err(), Some(why), "{raw:.60}");
    }
}

#[test]
fn only_workspace_shortcuts_are_forwarded() {
    let mut bridge = Bridge::default();
    bridge
        .update(Some(content("Story shear", None)))
        .expect("frame");
    let key = |key: &str| {
        serde_json::json!({"protocol":1,"type":"shortcut","key":key,"version":"main/v2","source":"abc123","generation":1}).to_string()
    };
    assert!(matches!(
        bridge.accept(&key("Digit1")),
        Ok(Action::SelectWorkspace(Workspace::Model))
    ));
    assert!(matches!(
        bridge.accept(&key("Digit5")),
        Ok(Action::SelectWorkspace(Workspace::Reports))
    ));
    assert!(matches!(
        bridge.accept(&key("PageDown")),
        Ok(Action::NextWorkspace)
    ));
    assert!(matches!(
        bridge.accept(&key("PageUp")),
        Ok(Action::PreviousWorkspace)
    ));
    for refused in [key("KeyQ"), key("Digit6")] {
        assert_eq!(bridge.accept(&refused).err(), Some(Refused::UnknownKind));
    }
    let mut with_story: serde_json::Value = serde_json::from_str(&key("Digit1")).expect("message");
    with_story["story"] = "L2".into();
    assert_eq!(
        bridge.accept(&with_story.to_string()).err(),
        Some(Refused::Malformed)
    );
    let old = key("Digit1");
    bridge.update(None).expect("hidden Model");
    assert_eq!(bridge.accept(&old).err(), Some(Refused::Stale));
    bridge
        .update(Some(content("Story shear", None)))
        .expect("return");
    assert_eq!(bridge.accept(&old).err(), Some(Refused::Stale));
}

#[test]
fn the_ready_message_is_exact() {
    assert!(is_ready(r#"{"protocol":1,"type":"ready"}"#));
    for raw in [
        r#"{"protocol":2,"type":"ready"}"#,
        r#"{"protocol":1,"type":"ready","generation":1}"#,
        r#"{"protocol":1,"type":"ready","extra":true}"#,
        r#"{"protocol":1,"type":"select_story"}"#,
        "ready",
    ] {
        assert!(!is_ready(raw), "{raw}");
    }
}

/// The messages the real chart page posted in headless Chromium
/// (`docs/native/evidence/app-01c-echarts/chart_page_test.js`), for the same
/// frames as `content` builds: the page and this bridge agree on the contract.
#[test]
fn the_chart_pages_own_messages_are_accepted_and_go_stale() {
    let posted: Vec<String> = serde_json::from_str(include_str!(
        "../../../docs/native/evidence/app-01c-echarts/messages.json"
    ))
    .expect("valid JSON");
    assert!(is_ready(&posted[0]), "the page's ready message");
    let mut bridge = Bridge::default();
    bridge
        .update(Some(content("Story shear", None)))
        .expect("update");
    let (first, second): (Vec<_>, Vec<_>) = posted[1..]
        .iter()
        .partition(|raw| !raw.contains(r#""generation":2"#));
    for raw in &first {
        // Preserve the historical browser receipt. Its old unscoped shortcuts
        // are now refused; do not edit old evidence to imply a new browser run.
        if raw.contains("shortcut") {
            assert_eq!(bridge.accept(raw).err(), Some(Refused::Stale));
        } else {
            assert!(bridge.accept(raw).is_ok(), "{raw}");
        }
    }
    // The page was then sent generation 2 ("Story moment").
    bridge
        .update(Some(content("Story moment", None)))
        .expect("update");
    for raw in &second {
        assert!(
            matches!(bridge.accept(raw), Ok(Action::SelectResultStory(Some(story))) if story == "Roof"),
            "{raw}"
        );
    }
    for raw in first.iter().filter(|raw| raw.contains("generation")) {
        assert_eq!(bridge.accept(raw).err(), Some(Refused::Stale), "{raw}");
    }
}
