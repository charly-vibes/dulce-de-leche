//! DDL-dqp regression guard: `docs/src/SUMMARY.md` must not carry absolute
//! ecosystem URLs. mdBook 0.5 materializes absolute-URL SUMMARY entries as
//! `docs/src/https:/...` junk directories on local builds; the ecosystem
//! entry belongs in the landing page's prose instead (see specodelic-j0m).

use std::fs;

const SUMMARY: &str = "docs/src/SUMMARY.md";

#[test]
fn summary_has_no_absolute_urls() {
    let summary =
        fs::read_to_string(SUMMARY).unwrap_or_else(|e| panic!("failed to read {SUMMARY}: {e}"));
    let offenders: Vec<&str> = summary
        .lines()
        .filter(|line| line.contains("http://") || line.contains("https://"))
        .collect();
    assert!(
        offenders.is_empty(),
        "absolute-URL entries in {SUMMARY} materialize junk dirs on local mdBook builds; \
         move them into the landing page's prose instead. Offending lines: {offenders:#?}"
    );
}

#[test]
fn ecosystem_entry_lives_in_landing_page_prose() {
    // The ecosystem link may be removed from SUMMARY only if the landing
    // page (first SUMMARY entry, typically index.md/introduction.md) carries
    // it as prose.
    let summary = fs::read_to_string(SUMMARY).expect("read SUMMARY.md");
    if !summary.contains("dulce-de-leche/ecosystem-map.html") {
        return; // not present anywhere: nothing to migrate
    }
    // If SUMMARY still links the ecosystem relatively, ensure the landing
    // page exists and mentions the ecosystem map too.
    let landing = summary
        .lines()
        .find_map(|l| {
            l.trim()
                .strip_prefix('[')
                .and_then(|rest| rest.split_once("]("))
                .map(|(_, target)| target.trim_end_matches(')').trim().to_string())
        })
        .expect("SUMMARY must have at least one entry");
    let landing_path = format!("docs/src/{landing}");
    let landing = fs::read_to_string(&landing_path)
        .unwrap_or_else(|e| panic!("failed to read landing page {landing_path}: {e}"));
    assert!(
        landing.contains("dulce-de-leche/ecosystem-map.html"),
        "ecosystem entry stays in SUMMARY but landing page {landing_path} \
         does not mention the ecosystem map"
    );
}
