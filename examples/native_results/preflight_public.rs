//! Owned public CLI fixture; never accepts a user source/report directory.
use std::io::Write as _;
pub(crate) fn run() -> Result<(), String> {
    let (owned, _) =
        crate::fixture::fixture().map_err(|_private_error| "Cannot create owned public fixture")?;
    let root = owned.path();
    std::fs::write(
        root.join("authored.e2k"),
        include_str!("../../assets/fixtures/real01.e2k"),
    )
    .map_err(|_private_error| "Cannot write owned public E2K fixture")?;
    let observed = serde_json::json!({"schemaVersion":2,"isAnalyzed":false,"units":"kip-ft-F","collectedAt":"2025-12-31T00:00:00Z","stories":[{"name":"Roof","elevation":10.0},{"name":"L1","elevation":0.0}]});
    std::fs::write(root.join("model-metadata.json"), observed.to_string())
        .map_err(|_private_error| "Cannot write owned public metadata fixture")?;
    let report = crate::preflight::inspect(&crate::preflight::Options {
        root: root.to_owned(),
        e2k: None,
        allow_external: false,
        expected_sha256: None,
    });
    let ready = report.can_show_uncertified_preview();
    let path = root.to_owned();
    drop(owned);
    let input_removed = !path.exists();
    let value = serde_json::json!({"assessment":if ready && input_removed {"PASS"} else {"FAIL"},"fixture":"LAB-REAL01-01-r1","report":report,"owned_input_removed":input_removed,"source_current_certified":false});
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, &value)
        .map_err(|_private_error| "Cannot write public preflight receipt")?;
    writeln!(stdout)
        .and_then(|()| stdout.flush())
        .map_err(|_private_error| "Cannot write public preflight receipt")?;
    if ready && input_removed {
        Ok(())
    } else {
        Err("Public preflight fixture failed".into())
    }
}
