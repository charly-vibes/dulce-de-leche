// Purpose: Regression fixtures for SEC-06 / DDL-bas — publish only tested
//          releases with narrowly scoped credentials.
// Responsibilities: Parse `.github/workflows/release.yml` and
//   `.github/workflows/ci.yml` as text and assert the security invariants:
//   (1) every third-party action is pinned to a full 40-hex commit SHA,
//   (2) only the publish job holds contents:write and publication
//       credentials, (3) publish depends on test + security jobs for the
//       exact tagged commit, (4) the bd bootstrap archive is checksum
//       verified before extraction.
// Rationale: These invariants are the verifiable value claim of DDL-bas;
//   keeping them as executable fixtures (not prose) means any workflow edit
//   that re-widens credentials or unpins actions fails `just ci`.

use std::path::Path;

fn read(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

/// Split a workflow document into (job_name, body) sections keyed on the
/// two-space-indented job keys under `jobs:`. Returns them in file order.
fn job_sections(doc: &str) -> Vec<(String, Vec<&str>)> {
    let mut jobs: Vec<(String, Vec<&str>)> = Vec::new();
    let mut current: Option<(String, Vec<&str>)> = None;
    let mut in_jobs = false;
    for line in doc.lines() {
        if line == "jobs:" {
            in_jobs = true;
            continue;
        }
        if !in_jobs {
            continue;
        }
        if !line.starts_with("  ") && !line.is_empty() {
            // left the jobs: block (next top-level key or end)
            if let Some(j) = current.take() {
                jobs.push(j);
            }
            in_jobs = false;
            continue;
        }
        let is_job_key = line.starts_with("  ")
            && !line.starts_with("   ")
            && line.contains(':')
            && !line.trim_start().starts_with('-');
        if is_job_key {
            if let Some(j) = current.take() {
                jobs.push(j);
            }
            let name = line.trim_end().trim_end_matches(':').trim().to_string();
            current = Some((name, Vec::new()));
        } else if let Some((_, body)) = current.as_mut() {
            body.push(line);
        }
    }
    if let Some(j) = current.take() {
        jobs.push(j);
    }
    jobs
}

#[test]
fn every_third_party_action_is_pinned_to_full_commit_sha() {
    for wf in ["release.yml", "ci.yml"] {
        let doc = read(&format!(".github/workflows/{wf}"));
        let mut pinned = 0;
        for line in doc.lines() {
            let Some(idx) = line.find("uses:") else {
                continue;
            };
            let spec = line[idx + 5..].trim().trim_matches('"');
            assert!(
                !spec.starts_with("./"),
                "{wf}: local action should not need pinning: {spec}"
            );
            let Some((_, version)) = spec.split_once('@') else {
                panic!("{wf}: action without pin: {spec}");
            };
            assert!(
                version.len() == 40 && version.chars().all(|c| c.is_ascii_hexdigit()),
                "{wf}: action `{spec}` must use a full 40-hex commit SHA, got `{version}`"
            );
            pinned += 1;
        }
        assert!(pinned > 0, "{wf}: fixture found no actions to check");
    }
}

#[test]
fn only_publish_job_has_contents_write_or_publication_credentials() {
    let doc = read(".github/workflows/release.yml");
    let jobs = job_sections(&doc);
    assert!(
        jobs.iter().any(|(n, _)| n == "publish"),
        "release.yml must define a publish job"
    );
    for (name, body) in &jobs {
        let text = body.join("\n");
        if name != "publish" {
            assert!(
                !text.contains("contents: write"),
                "{name}: non-publish job must not hold contents:write"
            );
            for cred in ["CARGO_REGISTRY_TOKEN", "TAP_GITHUB_TOKEN"] {
                assert!(
                    !text.contains(cred),
                    "{name}: publication credential {cred} must be scoped to the publish job only"
                );
            }
        } else {
            assert!(
                text.contains("contents: write"),
                "publish job must explicitly hold contents:write"
            );
            assert!(
                text.contains("CARGO_REGISTRY_TOKEN"),
                "publish job must hold CARGO_REGISTRY_TOKEN"
            );
        }
    }
    // Workflow-level default must be read-only so build jobs inherit least
    // privilege even if they gain new steps.
    assert!(
        doc.contains("permissions:\n  contents: read"),
        "release.yml must default the workflow to contents: read"
    );
}

#[test]
fn publish_depends_on_test_and_security_jobs_for_the_tagged_commit() {
    let doc = read(".github/workflows/release.yml");
    let jobs = job_sections(&doc);
    let publish = &jobs
        .iter()
        .find(|(n, _)| n == "publish")
        .expect("publish job")
        .1;
    let needs = publish
        .iter()
        .find(|l| l.trim_start().starts_with("needs:"))
        .expect("publish job must declare needs");
    for dep in [
        "build-linux",
        "build-darwin",
        "build-windows",
        "test",
        "security",
    ] {
        assert!(
            needs.contains(dep),
            "publish must need `{dep}`; failed/skipped/cancelled prerequisites must prevent publication"
        );
    }
    for (name, required) in [("test", "cargo test --locked"), ("security", "cargo audit")] {
        let job = jobs
            .iter()
            .find(|(n, _)| *n == name)
            .unwrap_or_else(|| panic!("release.yml must define a `{name}` job"));
        assert!(
            job.1.iter().any(|l| l.contains(required)),
            "`{name}` job must run `{required}` on the exact tagged commit"
        );
    }
    // The security job must also run a secret scan.
    let security = &jobs
        .iter()
        .find(|(n, _)| n == "security")
        .expect("security")
        .1;
    assert!(
        security.iter().any(|l| l.contains("just secret-scan")),
        "security job must run `just secret-scan`"
    );
}

#[test]
fn bd_bootstrap_archive_is_checksum_verified_before_extraction() {
    let doc = read(".github/workflows/ci.yml");
    let verify_idx = doc
        .find("sha256sum -c")
        .expect("ci.yml must verify the bd archive checksum");
    let extract_idx = doc
        .find("tar -xzf")
        .expect("ci.yml must extract the bd archive");
    assert!(
        verify_idx < extract_idx,
        "bd archive must be checksum-verified BEFORE extraction/execution"
    );
    assert!(
        doc.contains("beads_") && doc.contains(".tar.gz"),
        "ci.yml fixture lost the bd archive download context"
    );
}
