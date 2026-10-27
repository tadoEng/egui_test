use serde_json::{Value, json};
use std::fs;

pub(crate) fn fixture() -> std::io::Result<(tempfile::TempDir, Value)> {
    let root = tempfile::tempdir()?;
    fs::create_dir(root.path().join("results"))?;
    let units = json!({"force":"kip","globalLength":"ft","displacement":"ft","sectionArea":"in²","floorArea":"ft²","stress":"ksi","moment":"kip·ft","volume":"ft³","unitWeight":"kip/ft³"});
    let drift_row = |story: &str, p: f64, n: f64| json!({"story":story,"groupName":"G","outputCase":"W","maxDispXPosFt":0.0,"maxDispXNegFt":0.0,"maxDispYPosFt":0.0,"maxDispYNegFt":0.0,"maxDriftXPos":p,"maxDriftXNeg":n,"maxDriftYPos":p,"maxDriftYNeg":n});
    let drift = json!({"allowableRatio":0.01,"rows":[drift_row("L1",0.001,-0.0015),drift_row("Roof",0.002,-0.0025)],"storyOrder":["Roof","L1"],"governing":{"story":"Roof","groupName":"G","outputCase":"W","direction":"X","sense":"+","driftRatio":0.002,"dcr":0.2,"pass":true},"pass":true,"roofDispX":null,"roofDispY":null,"dispLimit":null,"dispPass":null});
    let force_row = |story: &str, v: f64| json!({"story":story,"elevationFt":0.0,"vxKip":v,"vyKip":v,"mxKipFt":0.0,"myKipFt":0.0});
    let profile = json!({"outputCase":"FX","rows":[force_row("L1",-2.0),force_row("Roof",7.0)]});
    let calc = json!({"meta":{"versionId":"v1","branch":"main","code":"LAB","generatedAt":"2026-01-01T00:00:00Z","units":{"force":"kip","length":"ft","stress":"ksi","moment":"kip·ft"},"unitProvenance":{"sourcePreset":"kip-ft-F","source":units,"canonical":units}},"summary":{"overallStatus":"loaded","checkCount":0,"passCount":0,"failCount":0,"lines":[]},"storyForces":{"rows":[],"storyOrder":["Roof","L1"],"xProfiles":[profile],"yProfiles":[profile]},"driftWind":{"x":drift,"y":drift}});
    let manifest = json!({"id":"v1","branch":"main","message":"Synthetic","author":"LAB","timestamp":"2026-01-01T00:00:00Z","parent":null,"edbSizeBytes":0,"e2kSizeBytes":null,"isAnalyzed":false,"e2kGenerated":false,"materialsExtracted":false,"metadataExtracted":false,"metadataSizeBytes":null,"gitCommitHash":null});
    fs::write(root.path().join("manifest.json"), manifest.to_string())?;
    fs::write(root.path().join("results/source-units.json"),json!({"schemaVersion":1,"preset":"kip-ft-F","force":"kip","length":"ft","temperature":"F"}).to_string())?;
    fs::write(
        root.path().join("results/calc_output.json"),
        calc.to_string(),
    )?;
    Ok((root, calc))
}
