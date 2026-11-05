//! Beads no-db stamping and tracked-export policy decision (DDL.repo.beads-export).

/// Beads no-db stamp outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeadsChange {
    Created,
    FlippedToNoDb,
    Unchanged,
}

/// Stamp `no-db: true` into .beads/config.yaml `content` (None = missing).
/// - missing → created with no-db: true
/// - present with `no-db: false` → flipped to true
/// - present with `no-db: true` → unchanged
pub fn ensure_beads_nodb(content: Option<&str>) -> (String, BeadsChange) {
    match content {
        None => (
            "# Beads Configuration File\n\
             # Stamped by `ddl init --gates`. no-db keeps the issue store in\n\
             # .beads/issues.jsonl; on bd 1.x Dolt remains the storage backend\n\
             # and agents must never run dolt commands.\nno-db: true\n"
                .to_string(),
            BeadsChange::Created,
        ),
        Some(text) => {
            let key_present = text.lines().any(|l| l.trim_start().starts_with("no-db:"));
            if !key_present {
                let mut out = text.to_string();
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str("# stamped by `ddl init --gates`\nno-db: true\n");
                return (out, BeadsChange::FlippedToNoDb);
            }
            let mut out = String::new();
            let mut flipped = false;
            for line in text.lines() {
                if line.trim_start().starts_with("no-db:") {
                    let value = line.split(':').nth(1).unwrap_or("").trim();
                    if value == "true" {
                        out.push_str(line);
                    } else {
                        out.push_str("no-db: true");
                        flipped = true;
                    }
                } else {
                    out.push_str(line);
                }
                out.push('\n');
            }
            (
                out,
                if flipped {
                    BeadsChange::FlippedToNoDb
                } else {
                    BeadsChange::Unchanged
                },
            )
        }
    }
}

/// Snapshot of the beads export state `apply_gates_wiring` needs to decide
/// whether a pre-flip export is required (DDLBeads: the funkalization trap —
/// stamping `no-db: true` over an embedded-Dolt repo with no export makes
/// every existing issue invisible to bd).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BeadsExportInput {
    /// `.beads/embeddeddolt/` exists (a Dolt database holds issues).
    pub embeddeddolt_exists: bool,
    /// `.beads/issues.jsonl` exists.
    pub issues_jsonl_exists: bool,
}

/// True when the repo has beads data in a Dolt database but no JSONL export:
/// the export must happen BEFORE `no-db: true` is stamped, otherwise bd
/// (reading only the JSONL in no-db mode) reports an empty tracker.
pub fn beads_export_needs_export(input: &BeadsExportInput) -> bool {
    input.embeddeddolt_exists && !input.issues_jsonl_exists
}
