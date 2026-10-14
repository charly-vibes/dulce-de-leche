//! Schema conformance tests for ddl's JSON envelopes (DDL-6zn.4).
//!
//! Pins the machine contract so it cannot drift again:
//! - `envelope_version` is always "0.1"
//! - `ddl doctor --json` data = { summary: {pass, warn, fail}, diagnostics:
//!   [{tool, check, level: pass|warn|fail, detail, fix?}] } — summary is the
//!   roll-up of the items, never contradicts them
//! - `ddl status --json` items carry no nulls and the section summary agrees
//!   with the item levels
//! - no null values anywhere inside `data`

mod common;
use assert_cmd::Command;
use serde_json::Value;
use std::time::Duration;

const CMD_TIMEOUT: Duration = Duration::from_secs(20);

fn ddl_json(args: &[&str]) -> Value {
    let temp = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("ddl").unwrap();
    common::clean_git_env(&mut cmd);
    cmd.current_dir(temp.path());
    cmd.args(args).timeout(CMD_TIMEOUT);
    let out = cmd.assert().success().get_output().stdout.clone();
    serde_json::from_slice(&out).expect("valid JSON envelope on stdout")
}

/// Recursively assert there are no null values inside `value`.
fn assert_no_nulls(value: &Value, path: &str) {
    match value {
        Value::Null => panic!("unexpected null at {path}"),
        Value::Object(map) => {
            for (k, v) in map {
                assert_no_nulls(v, &format!("{path}/{k}"));
            }
        }
        Value::Array(items) => {
            for (i, v) in items.iter().enumerate() {
                assert_no_nulls(v, &format!("{path}[{i}]"));
            }
        }
        _ => {}
    }
}

#[test]
fn doctor_envelope_matches_pinned_contract() {
    let envelope = ddl_json(&["doctor", "--json"]);
    assert_eq!(envelope["ok"], serde_json::json!(true));
    assert_eq!(envelope["envelope_version"], serde_json::json!("0.1"));
    assert_eq!(envelope["envelope_kind"], serde_json::json!("doctor"));
    assert_no_nulls(&envelope["data"], "doctor.data");

    let data = &envelope["data"];
    let mut keys: Vec<&str> = data
        .as_object()
        .unwrap()
        .keys()
        .map(|s| s.as_str())
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec!["diagnostics", "summary"],
        "exactly these data keys"
    );

    // diagnostics: structured items with the exact key set
    let diagnostics = data["diagnostics"].as_array().expect("diagnostics array");
    assert!(!diagnostics.is_empty());
    for (i, item) in diagnostics.iter().enumerate() {
        let obj = item.as_object().expect("diagnostic item is an object");
        for key in ["tool", "check", "level", "detail"] {
            assert!(obj.contains_key(key), "item {i} missing '{key}': {item}");
            assert!(
                obj[key].is_string(),
                "item {i} '{key}' must be a string, got {item}"
            );
        }
        assert!(
            obj["level"] == "pass" || obj["level"] == "warn" || obj["level"] == "fail",
            "item {i} bad level: {item}"
        );
        for key in obj.keys() {
            assert!(
                ["tool", "check", "level", "detail", "fix"].contains(&key.as_str()),
                "item {i} has unexpected key '{key}'"
            );
        }
        if let Some(fix) = obj.get("fix") {
            assert!(
                fix.is_string(),
                "item {i} fix must be a string when present"
            );
        }
    }

    // summary is the roll-up of the items — single source, never contradictory
    let summary = &data["summary"];
    for key in ["pass", "warn", "fail"] {
        assert!(summary[key].is_u64(), "summary.{key} must be a number");
    }
    let count = |level: &str| diagnostics.iter().filter(|i| i["level"] == level).count() as u64;
    assert_eq!(summary["pass"], serde_json::json!(count("pass")));
    assert_eq!(summary["warn"], serde_json::json!(count("warn")));
    assert_eq!(summary["fail"], serde_json::json!(count("fail")));

    // gh#46 inversion guard: a healthy check is pass, never warn. Any
    // "no issues found" detail (genesis's empty-result pass message) must
    // carry level "pass" — warn means an actual problem.
    for item in diagnostics.iter() {
        if item["detail"] == serde_json::json!("no issues found") {
            assert_eq!(
                item["level"],
                serde_json::json!("pass"),
                "healthy check rendered as warn (gh#46 inversion): {item}"
            );
        }
    }
    // The platform check is structurally pass on any supported platform —
    // it must never contribute a warn/fail to healthy-machine output.
    if let Some(platform) = diagnostics
        .iter()
        .find(|i| i["check"] == serde_json::json!("ddl.platform"))
    {
        assert_eq!(
            platform["level"],
            serde_json::json!("pass"),
            "platform check on supported platform must be pass: {platform}"
        );
    }
}

#[test]
fn status_envelope_has_no_nulls_and_consistent_summary() {
    let envelope = ddl_json(&["status", "--json"]);
    assert_eq!(envelope["envelope_version"], serde_json::json!("0.1"));
    assert_no_nulls(&envelope["data"], "status.data");

    let sections = envelope["data"]["sections"]
        .as_array()
        .expect("sections array");
    assert!(!sections.is_empty());
    for section in sections {
        let items = section["items"].as_array().expect("items array");
        for item in items {
            for key in ["label", "value", "level"] {
                assert!(
                    item[key].is_string(),
                    "status item '{}' field '{key}' must be a string (no nulls): {item}",
                    item.get("label").cloned().unwrap_or_default()
                );
            }
        }
        // section summary must agree with the item levels
        let summary = section["summary"].as_str().expect("summary string");
        let count = |needle: &str| {
            summary
                .split(',')
                .find_map(|part| {
                    let part = part.trim();
                    part.strip_suffix(needle)
                        .and_then(|n| n.trim().parse::<u64>().ok())
                })
                .unwrap_or_else(|| panic!("unparseable summary: {summary}"))
        };
        let healthy = count(" healthy");
        let warnings = count(" warnings");
        let errors = count(" errors");
        let items_count = |level: &str| items.iter().filter(|i| i["level"] == level).count() as u64;
        assert_eq!(
            healthy,
            items_count("healthy"),
            "summary '{summary}' vs items"
        );
        assert_eq!(
            warnings,
            items_count("warning"),
            "summary '{summary}' vs items"
        );
        assert_eq!(errors, items_count("error"), "summary '{summary}' vs items");
    }
}
