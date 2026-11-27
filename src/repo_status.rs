//! Repo-status single source of truth (DDL-76lm, openspec
//! `ddl-status-single-source-of-truth`): the `.ddl/status.json` record, the
//! per-tool feed obtainers, the content-state staleness engine, and the
//! `ddl status --refresh` write ordering.
//!
//! Responsibilities:
//!
//! - the versioned, tolerant `repo-status.v1` record (D7): mirroring the
//!   brief envelope's shape, unknown producer fields preserved under
//!   `payload`, a feed that fails to parse is recorded `unavailable` with
//!   the raw error — never dropped;
//! - the [`FeedObtainer`] trait: subprocess protocol, per-tool timeout
//!   ([`crate::installer::exec::child_deadline`]), error capture — ddl stays
//!   orchestrator-clean, no tool library linkage (D3);
//! - the staleness engine (D2): verdicts come from content state —
//!   committed artifact vs fresh re-obtainment, volatile envelope fields
//!   normalized out of both sides — never from file timestamps;
//! - the `--refresh` write ordering (D4): obtain feeds → `podras brief
//!   --write` → `bd export` → atomic record rewrite, under the dot-ddl
//!   manifest lock, per-step reported, failures actionable.
//!
//! Rationale: one owned answer to "what is this repo's status" — freshness
//! is data computed by ddl, and the ordering a failed step prescribes is
//! the same sentence the emitted brief-drift gate prints
//! ([`STATUS_ORDERING_MESSAGE`]), so the gate and `--refresh` cannot drift.

use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::error::{DdlError, Result};
use crate::installer::{child_deadline, run_child_captured};
use crate::manifest::Manifest;

/// The write-ordering sentence. Shared between the `--refresh` failure
/// output and the emitted brief-drift gate's failure message — one source
/// of truth for the ordering (D4), so the gate message and `--refresh`
/// cannot drift.
pub const STATUS_ORDERING_MESSAGE: &str = "Run `ddl status --refresh` — it regenerates the brief render (podras brief --write), updates the tracked export (.beads/issues.jsonl), and rewrites the status record; then re-commit.";

/// Contract-precedence statement embedded in the refresh report and in
/// the `.ddl/status.json` record itself (task 1.6, spec scenario
/// "Contract precedence documented in-band"): it names ddl as the
/// producer, the `generated_at` timestamp slot as the record's write
/// stamp, and the live record (`.ddl/status.json`) as the single source
/// of truth for repo status — hand-written status docs are not.
pub const CONTRACT_PRECEDENCE_HEADER: &str = "Repo status record — produced by ddl (ddl status --refresh); the generated_at timestamp is the write stamp of this record, and this file (.ddl/status.json) is the single source of truth for repo status. Hand-written status documents are not authoritative.";

/// Schema version of the status record (D7: versioned and tolerant).
pub const RECORD_SCHEMA_VERSION: &str = "repo-status.v1";

/// Envelope protocol version carried by the record — dulce's pinned
/// envelope version (see `output.rs`).
pub const RECORD_ENVELOPE_VERSION: &str = "0.1";

/// File name of the status record inside `.ddl/`.
pub const RECORD_FILE_NAME: &str = "status.json";

/// Volatile envelope fields normalized out of BOTH sides of every
/// staleness comparison (D2): they differ on every producer invocation,
/// so an unnormalized diff is always non-empty — the exact failure mode
/// that mislabeled fresh content as stale.
pub const VOLATILE_FIELDS: &[&str] = &[
    "generated_at",
    "as_of",
    "cli_version",
    "observed_at",
    "staleness",
];

// ---------------------------------------------------------------------------
// Record types (D7 — versioned, tolerant)
// ---------------------------------------------------------------------------

/// The `.ddl/status.json` record: mirrors the brief envelope's shape
/// (`ok`/`data`, per-feed entries, `schema_version`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatusRecord {
    pub ok: bool,
    pub envelope_version: String,
    pub schema_version: String,
    /// RFC3339 stamp of when the record was (re)written.
    pub generated_at: String,
    /// In-band contract-precedence provenance
    /// ([`CONTRACT_PRECEDENCE_HEADER`]): ddl is the producer and this
    /// record is the source of truth for repo status. Tolerant parse
    /// (D7): defaulted when absent in an older record.
    #[serde(default = "default_record_header")]
    pub header: String,
    pub data: StatusData,
}

/// Default `header` value for tolerant parse of older records that lack
/// the field.
fn default_record_header() -> String {
    CONTRACT_PRECEDENCE_HEADER.to_string()
}

/// Payload of the status record: one entry per feed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatusData {
    pub feeds: Vec<FeedEntry>,
}

/// One feed's entry in the record. Either available (`observed_at` +
/// `staleness` + `payload`) or `unavailable` (`status` + `detail`).
/// Tolerant by construction: no `deny_unknown_fields`, every field
/// defaulted, unknown producer fields ride inside `payload` untouched.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedEntry {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub staleness: Option<Staleness>,
    /// The producer's payload (whole envelope for podras, counts for bd,
    /// repo state for git). Unknown/extra producer fields are preserved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
    /// The literal `"unavailable"` for a feed whose producer could not
    /// answer; absent for available feeds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// The producer's error for an `unavailable` feed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Content-state staleness verdict (D2): `fresh` means the committed
/// artifact matches a fresh re-obtainment after volatile-field
/// normalization; `stale` means it does not. Never derived from file
/// timestamps — those appear only as display metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Staleness {
    Fresh,
    Stale,
}

impl FeedEntry {
    /// An available feed entry, stamped now and `fresh` until a staleness
    /// verdict says otherwise.
    pub fn available(name: &str, payload: serde_json::Value) -> Self {
        Self {
            name: name.to_string(),
            observed_at: Some(Utc::now().to_rfc3339()),
            staleness: Some(Staleness::Fresh),
            payload: Some(payload),
            status: None,
            detail: None,
        }
    }

    /// An `unavailable` entry carrying the producer's error verbatim (D7:
    /// a feed that fails is recorded, never dropped).
    pub fn unavailable(name: &str, detail: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            observed_at: None,
            staleness: None,
            payload: None,
            status: Some("unavailable".to_string()),
            detail: Some(detail.into()),
        }
    }

    /// Attach a staleness verdict.
    pub fn with_staleness(mut self, staleness: Staleness) -> Self {
        self.staleness = Some(staleness);
        self
    }

    /// Whether the producer answered (false for `unavailable` entries).
    pub fn is_available(&self) -> bool {
        self.status.is_none()
    }

    /// Display field (phase 2): when the producer was last observed.
    pub fn observed_at(&self) -> Option<&str> {
        self.observed_at.as_deref()
    }

    /// Display field (phase 2): the content-state verdict, `None` for
    /// `unavailable` entries.
    pub fn staleness(&self) -> Option<Staleness> {
        self.staleness
    }
}

impl StatusRecord {
    /// Build a record from freshly obtained feed entries.
    pub fn new(feeds: Vec<FeedEntry>) -> Self {
        Self {
            ok: true,
            envelope_version: RECORD_ENVELOPE_VERSION.to_string(),
            schema_version: RECORD_SCHEMA_VERSION.to_string(),
            generated_at: Utc::now().to_rfc3339(),
            header: CONTRACT_PRECEDENCE_HEADER.to_string(),
            data: StatusData { feeds },
        }
    }

    /// The envelope's `data` payload (the whole value when the envelope
    /// carries no `data` key) — the comparison unit for brief staleness.
    pub fn envelope_data(envelope: &serde_json::Value) -> serde_json::Value {
        envelope
            .get("data")
            .cloned()
            .unwrap_or_else(|| envelope.clone())
    }
}

/// Path of the status record inside the `.ddl/` directory.
pub fn record_path(ddl_dir: &Path) -> PathBuf {
    ddl_dir.join(RECORD_FILE_NAME)
}

/// Read the status record. Tolerant parse (D7): unknown fields are
/// ignored, known fields defaulted — only a structurally broken file is
/// an error.
pub fn load_record(path: &Path) -> Result<StatusRecord> {
    let raw = std::fs::read(path).map_err(DdlError::Io)?;
    serde_json::from_slice(&raw).map_err(DdlError::Serde)
}

/// Atomically rewrite the status record (same SEC-01 protocol as the
/// manifest write: temp sibling → fsync → rename → parent fsync — a
/// reader never sees a partial file, D4).
pub fn write_record_atomic(path: &Path, record: &StatusRecord) -> Result<()> {
    let contents = serde_json::to_vec_pretty(record).map_err(DdlError::Serde)?;
    crate::dot_ddl::persist::write_bytes_atomic(path, &contents)
}

// ---------------------------------------------------------------------------
// Staleness engine (D2 — content state, never timestamps)
// ---------------------------------------------------------------------------

/// Recursively remove the volatile fields (D2) from both comparison
/// sides. Per-source `observed_at` values are removed too: they are the
/// same key at any object depth.
pub fn normalize_volatile(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for key in VOLATILE_FIELDS {
                map.remove(*key);
            }
            for (_, v) in map.iter_mut() {
                normalize_volatile(v);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                normalize_volatile(item);
            }
        }
        _ => {}
    }
}

/// Brief-payload normalization: the volatile envelope fields plus the
/// HEAD-dependent and clone-local fields the existing dulce brief-drift
/// gate strips (`.data.recent`, `source_hash`, `head_sha`, per-source
/// warnings, curation mtime, orphan_commits signal churn). Real content
/// drift — tickets, specs, non-orphan signals — still compares non-empty.
pub fn normalize_brief_payload(value: &mut serde_json::Value) {
    normalize_volatile(value);
    strip_head_churn(value);
}

fn strip_head_churn(value: &mut serde_json::Value) {
    const CHURN_KEYS: &[&str] = &["recent", "source_hash", "head_sha", "warnings"];
    match value {
        serde_json::Value::Object(map) => {
            for key in CHURN_KEYS {
                map.remove(*key);
            }
            if let Some(curation) = map.get_mut("curation")
                && let Some(obj) = curation.as_object_mut()
            {
                obj.remove("updated_at");
            }
            if let Some(signals) = map.get_mut("signals").and_then(|s| s.as_array_mut()) {
                signals
                    .retain(|s| s.get("kind").and_then(|k| k.as_str()) != Some("orphan_commits"));
            }
            for (_, v) in map.iter_mut() {
                strip_head_churn(v);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                strip_head_churn(item);
            }
        }
        _ => {}
    }
}

/// Whether the committed brief envelope payload differs from the fresh
/// one after normalization. This is the volatile-fields lesson pinned by
/// the unit test below: two envelopes differing only in `generated_at` /
/// `observed_at` / `cli_version` compare EQUAL.
pub fn brief_payload_is_stale(
    committed_data: &serde_json::Value,
    fresh_data: &serde_json::Value,
) -> bool {
    let mut committed = committed_data.clone();
    let mut fresh = fresh_data.clone();
    normalize_brief_payload(&mut committed);
    normalize_brief_payload(&mut fresh);
    committed != fresh
}

/// Normalize a brief render (BRIEF.md) for comparison — the method the
/// existing dulce brief-drift gate uses: compare the narrative through
/// the health section, drop the `## Recent` listing (current-HEAD churn)
/// and the final `## Sources & freshness` section (clone-local mtimes and
/// stamps), drop orphan_commits lines, trim per-line and trailing
/// whitespace.
pub fn normalize_render_text(render: &str) -> String {
    let mut lines: Vec<&str> = Vec::new();
    let mut skip = false;
    for line in render.lines() {
        if line.starts_with("## Sources & freshness") {
            break;
        }
        if line.trim_end() == "## Recent" {
            skip = true;
            continue;
        }
        if line.starts_with("## ") {
            skip = false;
        }
        if skip {
            continue;
        }
        if line.contains("orphan_commits")
            || line.contains("orphan commits")
            || line.contains("orphan-commits")
        {
            continue;
        }
        lines.push(line.trim_end());
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// Whether the committed render differs from the fresh render after
/// normalization (the render-comparison method of the dulce gate).
pub fn render_is_stale(committed_render: &str, fresh_render: &str) -> bool {
    normalize_render_text(committed_render) != normalize_render_text(fresh_render)
}

/// Whether the tracked beads store differs from a fresh `bd export`
/// (whisper-class lesson pinned by the unit test below: byte-identical
/// content — the tracker store unchanged — is NEVER stale, no matter the
/// file timestamps).
pub fn beads_export_is_stale(tracked: &[u8], exported: &[u8]) -> bool {
    tracked != exported
}

// ---------------------------------------------------------------------------
// FeedObtainer trait + the four obtainers (D3)
// ---------------------------------------------------------------------------

/// Everything a feed obtainer may need about the current repository.
#[derive(Debug, Clone)]
pub struct FeedContext {
    pub repo_root: PathBuf,
    pub ddl_dir: PathBuf,
    pub podras_installed: bool,
    pub bd_installed: bool,
    pub openspec_installed: bool,
}

/// A per-tool status feed obtainer. The subprocess protocol (spawn with
/// null stdin under the per-tool child deadline, capture output), the
/// error capture, and the tolerant parse all live behind this trait so
/// ddl stays orchestrator-clean (D3: no tool library linkage).
///
/// Contract: `obtain` returns `Err(producer_error)` when the feed cannot
/// be produced — the orchestrator records it as an `unavailable` entry
/// with the error verbatim in `detail`, never dropping the feed.
pub trait FeedObtainer {
    /// Feed name as recorded in the status record.
    fn name(&self) -> &'static str;

    /// Obtain the feed, including its staleness verdict where a committed
    /// artifact exists to compare against (D2).
    fn obtain(&self, ctx: &FeedContext) -> std::result::Result<FeedEntry, String>;
}

/// Run a producer subprocess with captured output under the per-tool
/// child deadline (D3). On any failure the detail names the command and
/// carries the producer's error — it becomes the `unavailable` detail.
fn run_captured(bin: &str, args: &[&str], cwd: &Path) -> std::result::Result<String, String> {
    let invocation = format!("{bin} {}", args.join(" "));
    let mut cmd = std::process::Command::new(bin);
    cmd.args(args).current_dir(cwd);
    let output = run_child_captured(&mut cmd, child_deadline())
        .map_err(|e| format!("{invocation} failed: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr = stderr.trim();
        return Err(format!(
            "{invocation} exited with {code}: {stderr}",
            code = output.status.code().unwrap_or(-1),
            stderr = if stderr.is_empty() {
                "(no stderr)"
            } else {
                stderr
            }
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// podras feed: the typed brief envelope (`podras brief --json`). Only
/// produced when podras is installed AND the repository has a brief
/// (`BRIEF.md`/`BRIEF.json` or `.podras/`); otherwise `unavailable` with
/// the reason. Staleness compares the committed `BRIEF.json` data payload
/// — or, where a committed render exists, the committed `BRIEF.md` vs a
/// fresh render — per D2.
pub struct PodrasFeed;

impl PodrasFeed {
    /// Whether the repository has a brief for podras to render.
    pub fn has_brief(repo_root: &Path) -> bool {
        repo_root.join("BRIEF.md").exists()
            || repo_root.join("BRIEF.json").exists()
            || repo_root.join(".podras").exists()
    }

    /// Fresh render text via a temp-dir `podras brief --write --out-dir`
    /// (the brief-drift-gate method; never writes the working tree).
    fn fresh_render(&self, ctx: &FeedContext) -> Option<String> {
        let dir = std::env::temp_dir().join(format!(
            "ddl-podras-render-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).ok()?;
        let rendered = run_captured(
            "podras",
            &["brief", "--write", "--out-dir", &dir.to_string_lossy()],
            &ctx.repo_root,
        );
        let text = std::fs::read_to_string(dir.join("BRIEF.md")).ok();
        let _ = std::fs::remove_dir_all(&dir);
        rendered.ok()?;
        text
    }

    fn verdict(&self, ctx: &FeedContext, fresh_envelope: &serde_json::Value) -> Staleness {
        // WHY: where a committed render exists, render comparison is the
        // method the existing dulce brief-drift gate uses (D2).
        if let Ok(committed_md) = std::fs::read_to_string(ctx.repo_root.join("BRIEF.md"))
            && let Some(fresh_md) = self.fresh_render(ctx)
        {
            return if render_is_stale(&committed_md, &fresh_md) {
                Staleness::Stale
            } else {
                Staleness::Fresh
            };
        }
        // WHY: a fresh render could not be produced — fall through to the
        // committed-envelope comparison rather than guessing.
        if let Ok(raw) = std::fs::read(ctx.repo_root.join("BRIEF.json"))
            && let Ok(committed) = serde_json::from_slice::<serde_json::Value>(&raw)
            && brief_payload_is_stale(
                &StatusRecord::envelope_data(&committed),
                &StatusRecord::envelope_data(fresh_envelope),
            )
        {
            return Staleness::Stale;
        }
        // WHY: a corrupt committed envelope is not evidence of staleness
        // (whisper-class lesson): conservative verdict stays fresh.
        Staleness::Fresh
    }
}

impl FeedObtainer for PodrasFeed {
    fn name(&self) -> &'static str {
        "podras"
    }

    fn obtain(&self, ctx: &FeedContext) -> std::result::Result<FeedEntry, String> {
        if !ctx.podras_installed {
            return Err("podras is not installed (run `ddl install podras`)".to_string());
        }
        if !Self::has_brief(&ctx.repo_root) {
            return Err(
                "no brief in this repository (no BRIEF.md, BRIEF.json, or .podras/)".to_string(),
            );
        }
        let stdout = run_captured("podras", &["brief", "--json"], &ctx.repo_root)?;
        let envelope: serde_json::Value = serde_json::from_str(&stdout)
            .map_err(|e| format!("podras brief --json produced unparseable output: {e}"))?;
        let staleness = self.verdict(ctx, &envelope);
        Ok(FeedEntry::available(self.name(), envelope).with_staleness(staleness))
    }
}

/// bd (beads) feed: `bd list --json` counts. Staleness diffs the tracked
/// `.beads/issues.jsonl` against a fresh `bd export` — the tracked store
/// unchanged means the feed is NOT stale, whatever the timestamps say.
pub struct BdFeed;

impl BdFeed {
    fn verdict(&self, ctx: &FeedContext) -> Staleness {
        let tracked_path = ctx.repo_root.join(".beads").join("issues.jsonl");
        let Ok(tracked) = std::fs::read(&tracked_path) else {
            // Nothing committed to drift from: not stale.
            return Staleness::Fresh;
        };
        let Ok(exported) = run_captured("bd", &["export"], &ctx.repo_root) else {
            // Cannot produce the comparison evidence: conservative verdict.
            return Staleness::Fresh;
        };
        if beads_export_is_stale(&tracked, exported.as_bytes()) {
            Staleness::Stale
        } else {
            Staleness::Fresh
        }
    }
}

impl FeedObtainer for BdFeed {
    fn name(&self) -> &'static str {
        "bd"
    }

    fn obtain(&self, ctx: &FeedContext) -> std::result::Result<FeedEntry, String> {
        if !ctx.bd_installed {
            return Err("bd is not installed (run `ddl install bd`)".to_string());
        }
        let stdout = run_captured("bd", &["list", "--json"], &ctx.repo_root)?;
        let counts: serde_json::Value = serde_json::from_str(&stdout)
            .map_err(|e| format!("bd list --json produced unparseable output: {e}"))?;
        let staleness = self.verdict(ctx);
        Ok(FeedEntry::available(self.name(), counts).with_staleness(staleness))
    }
}

/// git feed: branch, ahead/behind vs upstream, dirty file count —
/// computed live from the genesis git helpers. Always `fresh` (cheap, no
/// committed artifact to compare against).
pub struct GitFeed;

impl FeedObtainer for GitFeed {
    fn name(&self) -> &'static str {
        "git"
    }

    fn obtain(&self, ctx: &FeedContext) -> std::result::Result<FeedEntry, String> {
        use genesis::git::{self, EnvPolicy};

        let root = &ctx.repo_root;
        let branch = git::current_branch(root)
            .map_err(|e| format!("git: {e}"))?
            .unwrap_or_else(|| "(detached HEAD)".to_string());
        let dirty_files = git::uncommitted_files_lossy(root).len();
        let (ahead, behind, upstream_detail) = match git::run(
            root,
            EnvPolicy::Inherit,
            &["rev-list", "--left-right", "--count", "@{upstream}...HEAD"],
        ) {
            Ok(out) => {
                let text = String::from_utf8_lossy(&out.stdout);
                let mut parts = text.split_whitespace();
                // `--left-right --count A...HEAD`: left counts commits only
                // in A (the upstream — how far behind), right only in HEAD
                // (how far ahead).
                let behind = parts.next().and_then(|p| p.parse::<u64>().ok());
                let ahead = parts.next().and_then(|p| p.parse::<u64>().ok());
                (ahead, behind, None)
            }
            Err(_) => (
                None,
                None,
                Some("no upstream configured for this branch".to_string()),
            ),
        };
        let payload = serde_json::json!({
            "branch": branch,
            "ahead": ahead,
            "behind": behind,
            "dirty_files": dirty_files,
            "upstream_detail": upstream_detail,
        });
        Ok(FeedEntry::available(self.name(), payload))
    }
}

/// openspec feed: `openspec list --json` active changes. `unavailable`
/// when the command produces no output, fails to parse, or reports no
/// active changes. Always `fresh` (live).
pub struct OpenspecFeed;

impl FeedObtainer for OpenspecFeed {
    fn name(&self) -> &'static str {
        "openspec"
    }

    fn obtain(&self, ctx: &FeedContext) -> std::result::Result<FeedEntry, String> {
        if !ctx.openspec_installed {
            return Err("openspec is not installed (run `ddl install openspec`)".to_string());
        }
        let stdout = run_captured("openspec", &["list", "--json"], &ctx.repo_root)?;
        if stdout.trim().is_empty() {
            return Err("openspec list --json produced no output".to_string());
        }
        let parsed: serde_json::Value = serde_json::from_str(&stdout)
            .map_err(|e| format!("openspec list --json produced unparseable output: {e}"))?;
        let changes = parsed
            .get("changes")
            .and_then(|c| c.as_array())
            .or_else(|| parsed.as_array());
        match changes {
            Some(list) if list.is_empty() => Err("no active changes".to_string()),
            Some(list) => {
                let payload = serde_json::json!({ "active_changes": list.len(), "changes": list });
                Ok(FeedEntry::available(self.name(), payload))
            }
            None => Ok(FeedEntry::available(self.name(), parsed)),
        }
    }
}

/// The four v1 feed obtainers, in record order (D3: repo-*status* feeds
/// are limited to podras/bd/git/openspec — the four that answer "what is
/// in flight and is that answer fresh"; wai pipeline state and dont
/// claims are named future feeds, not omissions).
pub fn all_obtainers() -> Vec<Box<dyn FeedObtainer>> {
    vec![
        Box::new(PodrasFeed),
        Box::new(BdFeed),
        Box::new(GitFeed),
        Box::new(OpenspecFeed),
    ]
}

/// Run every obtainer; a producer error becomes an `unavailable` entry
/// with the error verbatim (never dropped).
pub fn obtain_all(ctx: &FeedContext) -> Vec<FeedEntry> {
    all_obtainers()
        .into_iter()
        .map(|obtainer| {
            let name = obtainer.name();
            obtainer
                .obtain(ctx)
                .unwrap_or_else(|detail| FeedEntry::unavailable(name, detail))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// `ddl status --refresh` write ordering (D4)
// ---------------------------------------------------------------------------

/// Outcome of one ordering step, for per-step human reporting.
#[derive(Debug, Clone, PartialEq)]
pub struct StepReport {
    pub name: &'static str,
    pub ok: bool,
    pub detail: String,
}

/// The refresh outcome: per-step reports plus the record as written.
/// `header` is the in-band contract-precedence statement
/// ([`CONTRACT_PRECEDENCE_HEADER`]) for the human step-report rendering
/// — the refresh path prints it before the per-step lines so the report
/// carries its own provenance.
#[derive(Debug, Clone, PartialEq)]
pub struct RefreshReport {
    pub steps: Vec<StepReport>,
    pub header: String,
    pub record: StatusRecord,
}

/// The `--refresh` ordering under the dot-ddl manifest lock (D4):
/// obtain feeds (with staleness verdicts vs committed artifacts) → when
/// podras + brief: `podras brief --write` → `bd export` to the tracked
/// `.beads/issues.jsonl` (skipped when no beads or bd absent) → atomic
/// record rewrite. A failed step names the step, the producer's error,
/// and the exact remaining command ([`STATUS_ORDERING_MESSAGE`]), then
/// stops — no later steps with stale inputs.
pub fn run_refresh(repo_root: &Path, ddl_dir: &Path, manifest: &Manifest) -> Result<RefreshReport> {
    let manifest_path = ddl_dir.join("manifest.json");
    // Serialize ddl-side writers (D4); released when the guard drops.
    let _lock = crate::dot_ddl::persist::manifest_lock_guard(&manifest_path)?;

    let mut steps: Vec<StepReport> = Vec::new();

    // Step 1: obtain feeds (staleness verdicts computed in-obtainer).
    let ctx = FeedContext {
        repo_root: repo_root.to_path_buf(),
        ddl_dir: ddl_dir.to_path_buf(),
        podras_installed: manifest.is_installed("podras"),
        bd_installed: manifest.is_installed("bd"),
        openspec_installed: manifest.is_installed("openspec"),
    };
    let feeds = obtain_all(&ctx);
    let available = feeds.iter().filter(|f| f.is_available()).count();
    let stale = feeds
        .iter()
        .filter(|f| f.staleness() == Some(Staleness::Stale))
        .count();
    steps.push(StepReport {
        name: "obtain-feeds",
        ok: true,
        detail: format!(
            "{available}/{total} feeds obtained ({stale} stale)",
            total = feeds.len()
        ),
    });

    // WHY: step 2 — the brief render regeneration is skipped unless podras
    // + brief are both present; every other refresh step is unconditional.
    if ctx.podras_installed && PodrasFeed::has_brief(repo_root) {
        run_captured("podras", &["brief", "--write"], repo_root).map_err(|detail| {
            DdlError::Other(format!(
                "refresh step `brief render` failed: {detail}\n\n{STATUS_ORDERING_MESSAGE}"
            ))
        })?;
        steps.push(StepReport {
            name: "brief-render",
            ok: true,
            detail: "podras brief --write regenerated BRIEF.md / BRIEF.json".to_string(),
        });
    }

    // Step 3: update the tracked export (skip when no beads or bd absent).
    if ctx.bd_installed && repo_root.join(".beads").exists() {
        let exported = run_captured("bd", &["export"], repo_root).map_err(|detail| {
            DdlError::Other(format!(
                "refresh step `bd export` failed: {detail}\n\n{STATUS_ORDERING_MESSAGE}"
            ))
        })?;
        let tracked = repo_root.join(".beads").join("issues.jsonl");
        crate::dot_ddl::persist::write_bytes_atomic(&tracked, exported.as_bytes())?;
        steps.push(StepReport {
            name: "bd-export",
            ok: true,
            detail: format!("tracked export updated: {}", tracked.display()),
        });
    }

    // Step 4: rewrite the record atomically.
    let record = StatusRecord::new(feeds);
    let path = record_path(ddl_dir);
    write_record_atomic(&path, &record).map_err(|e| {
        DdlError::Other(format!(
            "refresh step `record write` failed: {e}\n\n{STATUS_ORDERING_MESSAGE}"
        ))
    })?;
    steps.push(StepReport {
        name: "record-write",
        ok: true,
        detail: format!("status record written: {}", path.display()),
    });

    Ok(RefreshReport {
        steps,
        header: CONTRACT_PRECEDENCE_HEADER.to_string(),
        record,
    })
}

// ---------------------------------------------------------------------------
// Unmanaged status artifacts (task 1.7 — spec "Session summary artifacts")
// ---------------------------------------------------------------------------

/// A root-level hand-written status artifact with no producer. Managed
/// outputs (BRIEF.md / BRIEF.json = podras render, everything under
/// `.ddl/`) are never reported — only files no tool writes, whose status
/// claims have no live producer behind them.
#[derive(Debug, Clone, PartialEq)]
pub struct UnmanagedArtifact {
    /// Repo-root-relative path of the artifact.
    pub path: PathBuf,
    /// RFC3339 last-commit date (`git log -1 --format=%cI`), `None` when
    /// the file is untracked (never committed).
    pub last_commit_date: Option<String>,
}

/// The explicit, conservative name list of recognized hand-written root
/// status artifacts (task 1.7): kept exact-match so a stray name never
/// sweeps up ordinary docs. Grounded in this repository's own
/// `AGENT-SUMMARY.md` — a free-form agent/session status narrative.
pub const UNMANAGED_STATUS_ARTIFACT_NAMES: &[&str] =
    &["AGENT-SUMMARY.md", "STATUS.md", "SESSION-SUMMARY.md"];

/// Detect the [`UNMANAGED_STATUS_ARTIFACT_NAMES`] files at the repo root
/// that exist, with their last-commit dates via the genesis git runner
/// (`None` when untracked or git is unavailable). Never reports the
/// managed outputs (BRIEF.md/BRIEF.json, `.ddl/*`).
pub fn detect_unmanaged_status_artifacts(repo_root: &Path) -> Vec<UnmanagedArtifact> {
    use genesis::git::{self, EnvPolicy};

    let mut found = Vec::new();
    for name in UNMANAGED_STATUS_ARTIFACT_NAMES {
        let path = PathBuf::from(name);
        if !repo_root.join(&path).is_file() {
            continue;
        }
        let last_commit_date = match git::run(
            repo_root,
            EnvPolicy::StripHookContext,
            &["log", "-1", "--format=%cI", "--", name],
        ) {
            Ok(output) => {
                let line = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if line.is_empty() { None } else { Some(line) }
            }
            Err(_) => None,
        };
        found.push(UnmanagedArtifact {
            path,
            last_commit_date,
        });
    }
    found
}

// ---------------------------------------------------------------------------
// Tests (task 1.4 pins: content diff, not timestamps)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// (a) volatile-fields: two fresh envelopes differing only in
    /// `generated_at` / per-source `observed_at` / `cli_version` compare
    /// EQUAL after normalization — the volatile stamp churn of every
    /// `podras brief --json` run must not read as content drift (D2).
    #[test]
    fn volatile_fields_normalize_to_equal() {
        let earlier = json!({
            "ok": true,
            "schema_version": "brief.v1",
            "generated_at": "2026-11-27T10:00:00Z",
            "cli_version": "0.14.2",
            "data": {
                "health": { "signals": [], "reason": "clean tree" },
                "sources": [
                    { "name": "bd", "observed_at": "2026-11-27T09:59:58Z", "count": 33 },
                    { "name": "git", "observed_at": "2026-11-27T09:59:59Z", "dirty": 0 }
                ]
            }
        });
        let later = json!({
            "ok": true,
            "schema_version": "brief.v1",
            "generated_at": "2026-11-27T17:30:01Z",
            "cli_version": "0.14.3",
            "data": {
                "health": { "signals": [], "reason": "clean tree" },
                "sources": [
                    { "name": "bd", "observed_at": "2026-11-27T17:29:59Z", "count": 33 },
                    { "name": "git", "observed_at": "2026-11-27T17:30:00Z", "dirty": 0 }
                ]
            }
        });
        assert!(!brief_payload_is_stale(
            &StatusRecord::envelope_data(&earlier),
            &StatusRecord::envelope_data(&later)
        ));
        // Symmetry: the same pair through the raw volatile normalization.
        let mut a = earlier.clone();
        let mut b = later.clone();
        normalize_volatile(&mut a);
        normalize_volatile(&mut b);
        assert_eq!(a, b);
        // And real content drift still reads as stale.
        let mut drifted = later.clone();
        drifted["data"]["sources"][0]["count"] = json!(34);
        assert!(brief_payload_is_stale(
            &StatusRecord::envelope_data(&earlier),
            &StatusRecord::envelope_data(&drifted)
        ));
    }

    /// (b) whisper-class: the tracked beads store byte-identical to the
    /// `bd export` output is NEVER stale — the verdict is a content diff,
    /// not a timestamp comparison (EDGE-001: the proxy mislabeled whisper
    /// because the export's mtime moved).
    #[test]
    fn unchanged_beads_store_is_never_stale() {
        let store = b"{\"id\":\"ddl-1\",\"title\":\"a\"}\n{\"id\":\"ddl-2\",\"title\":\"b\"}\n";
        assert!(!beads_export_is_stale(store, store));
        assert!(!beads_export_is_stale(
            store,
            b"{\"id\":\"ddl-1\",\"title\":\"a\"}\n{\"id\":\"ddl-2\",\"title\":\"b\"}\n"
        ));
        assert!(beads_export_is_stale(
            store,
            b"{\"id\":\"ddl-1\",\"title\":\"a\"}\n"
        ));
        assert!(beads_export_is_stale(b"{}", store));
    }

    /// Render normalization: the `## Recent` listing and the final
    /// `## Sources & freshness` section are clone-local churn and must
    /// not decide staleness; narrative drift must.
    #[test]
    fn render_normalization_ignores_head_churn() {
        let committed = "# Brief\n\n## Health\n\nall clear\n\n## Recent\n\nddl-76lm fix\n\n## Sources & freshness\n\ngenerated_at: 2026-11-27T10:00:00Z\n";
        let fresh = "# Brief\n\n## Health\n\nall clear\n\n## Recent\n\nddl-999zz new commits\n\n## Sources & freshness\n\ngenerated_at: 2026-11-27T17:30:00Z\n";
        assert!(!render_is_stale(committed, fresh));
        let drifted = "# Brief\n\n## Health\n\n2 blocked issues\n\n## Recent\n\nddl-76lm fix\n\n## Sources & freshness\n\ngenerated_at: 2026-11-27T17:30:00Z\n";
        assert!(render_is_stale(committed, drifted));
    }

    /// Record round-trip: an `unavailable` entry and an unknown payload
    /// survive a serialize/parse cycle untouched (D7 tolerance).
    #[test]
    fn record_round_trip_preserves_unavailable_and_payload() {
        let record = StatusRecord::new(vec![
            FeedEntry::available(
                "bd",
                json!({ "open": 3, "blocked": 1, "custom_field": [1, 2] }),
            ),
            FeedEntry::unavailable("openspec", "no active changes"),
        ]);
        let raw = serde_json::to_string_pretty(&record).unwrap();
        let parsed: StatusRecord = serde_json::from_str(&raw).unwrap();
        assert_eq!(record, parsed);
        assert_eq!(parsed.schema_version, "repo-status.v1");
        assert_eq!(
            parsed.data.feeds[0].payload.as_ref().unwrap()["custom_field"],
            json!([1, 2])
        );
        assert!(!parsed.data.feeds[1].is_available());
        assert_eq!(
            parsed.data.feeds[1].detail.as_deref(),
            Some("no active changes")
        );
        assert!(parsed.data.feeds[1].staleness().is_none());
    }

    /// Contract precedence in-band (task 1.6): a freshly built record
    /// carries the contract-precedence header, and a parsed record
    /// WITHOUT the field defaults to it (tolerant parse, D7).
    #[test]
    fn record_carries_contract_precedence_header() {
        let record = StatusRecord::new(vec![]);
        assert_eq!(record.header, CONTRACT_PRECEDENCE_HEADER);
        // The header names the producer, the timestamp slot, and the
        // source of truth.
        for needle in ["ddl status --refresh", "generated_at", ".ddl/status.json"] {
            assert!(
                CONTRACT_PRECEDENCE_HEADER.contains(needle),
                "header must mention {needle}"
            );
        }
        // Older/foreign record JSON lacking the field parses tolerantly
        // and defaults to the header.
        let legacy: StatusRecord = serde_json::from_value(json!({
            "ok": true,
            "envelope_version": RECORD_ENVELOPE_VERSION,
            "schema_version": RECORD_SCHEMA_VERSION,
            "generated_at": "2026-01-01T00:00:00Z",
            "data": { "feeds": [] }
        }))
        .unwrap();
        assert_eq!(legacy.header, CONTRACT_PRECEDENCE_HEADER);
    }

    /// Task 1.7 detection: a committed root AGENT-SUMMARY.md is detected
    /// with its last-commit date; an untracked summary file is detected
    /// with date `None`; managed outputs (BRIEF.md) are ignored.
    #[test]
    fn detect_unmanaged_status_artifacts_pins() {
        use genesis::git::{self, EnvPolicy};

        let root = tempfile::tempdir().unwrap();
        let repo = root.path();
        let git_args: &[&[&str]] = &[
            &["init", "-q"],
            &[
                "-c",
                "user.email=t@e.st",
                "-c",
                "user.name=t",
                "add",
                "AGENT-SUMMARY.md",
            ],
            &[
                "-c",
                "user.email=t@e.st",
                "-c",
                "user.name=t",
                "commit",
                "-q",
                "-m",
                "s",
            ],
        ];
        std::fs::write(repo.join("AGENT-SUMMARY.md"), "# session status\n").unwrap();
        for args in git_args {
            git::run(repo, EnvPolicy::StripHookContext, args).unwrap();
        }
        // Untracked summary + a managed output that must be ignored.
        std::fs::write(repo.join("SESSION-SUMMARY.md"), "draft\n").unwrap();
        std::fs::write(repo.join("BRIEF.md"), "# brief\n").unwrap();

        let found = detect_unmanaged_status_artifacts(repo);
        let committed = found
            .iter()
            .find(|a| a.path == PathBuf::from("AGENT-SUMMARY.md"))
            .expect("committed AGENT-SUMMARY.md detected");
        assert!(
            committed
                .last_commit_date
                .as_deref()
                .unwrap_or_default()
                .contains('T'),
            "last-commit date is an RFC3339 stamp: {:?}",
            committed.last_commit_date
        );
        let untracked = found
            .iter()
            .find(|a| a.path == PathBuf::from("SESSION-SUMMARY.md"))
            .expect("untracked SESSION-SUMMARY.md detected");
        assert_eq!(untracked.last_commit_date, None);
        assert!(
            !found.iter().any(|a| a.path == PathBuf::from("BRIEF.md")),
            "BRIEF.md is a managed podras output, never reported"
        );
        assert_eq!(found.len(), 2);
    }
}
