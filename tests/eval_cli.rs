use std::process::Command;

use tempfile::tempdir;

#[test]
fn eval_recommendation_offline_writes_report_files() {
    let output_dir = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_issue-finder"))
        .args([
            "eval",
            "recommendation",
            "--offline",
            "--output",
            output_dir.path().to_str().unwrap(),
        ])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(output_dir.path().join("metrics.json").exists());
    assert!(output_dir.path().join("report.md").exists());
    assert!(output_dir.path().join("visible.jsonl").exists());

    let metrics = std::fs::read_to_string(output_dir.path().join("metrics.json")).unwrap();
    assert!(metrics.contains("\"samples\""));
    let visible = std::fs::read_to_string(output_dir.path().join("visible.jsonl")).unwrap();
    assert!(visible.contains("\"sampleId\""));
}
