//! Gates wiring tests.

#[cfg(test)]
use super::*;

/// Default gate commands — the shipped rendering (`PRE_COMMIT_BLOCK`).
fn default_gates() -> GateCommands {
    GateCommands::default()
}

const COMMANDS_HOOK: &str = "\
pre-commit:
  parallel: true
  commands:
bd:
  run: bd hooks run pre-commit
fmt-check:
  glob: \"*.rs\"
  run: just fmt-check

pre-push:
  commands:
bd:
  run: bd hooks run pre-push
";

#[test]
fn missing_lefthook_yml_is_created_with_both_managed_blocks() {
    let (out, change) = ensure_lefthook_gates(None, &default_gates()).unwrap();
    assert_eq!(change, Change::Created);
    assert!(out.contains("pre-commit:"));
    assert!(out.contains("pre-push:"));
    assert!(out.contains(PRE_COMMIT_BLOCK));
    assert!(out.contains(PRE_PUSH_BLOCK));
    // DDL-fw7: the created file must already chain the beads hooks —
    // `ddl init -y --gates` then `ddl doctor` must be clean for
    // ddl.repo.bd-hooks without a second manual pass.
    assert!(out.contains("bd hooks run pre-commit"));
    assert!(out.contains("bd hooks run pre-push"));
    // managed entries must live INSIDE commands: (indent > 2)
    for line in out.lines() {
        if line.contains("ddl-ah-check") || line.contains("ddl-pretender-gate") {
            assert!(
                line.starts_with("    "),
                "managed entry not indented under commands:: {line:?}"
            );
        }
    }
}

#[test]
fn block_is_inserted_inside_commands_preserving_existing_commands() {
    let (out, change) = ensure_lefthook_gates(Some(COMMANDS_HOOK), &default_gates()).unwrap();
    assert_eq!(change, Change::Updated);
    // existing commands preserved
    assert!(out.contains("bd hooks run pre-commit"));
    assert!(out.contains("just fmt-check"));
    // managed block after the pre-commit commands: line, before existing entries
    let cmds_pos = out.find("  commands:").unwrap();
    let managed_pos = out.find(MARKER_START_PREFIX).unwrap();
    let bd_pos = out.find("bd hooks run pre-commit").unwrap();
    assert!(cmds_pos < managed_pos && managed_pos < bd_pos);
    assert!(out.contains(PRE_COMMIT_BLOCK));
    assert!(out.contains(PRE_PUSH_BLOCK));
}

#[test]
fn rerun_is_idempotent_no_duplicates() {
    let (once, _) = ensure_lefthook_gates(Some(COMMANDS_HOOK), &default_gates()).unwrap();
    let (twice, change) = ensure_lefthook_gates(Some(&once), &default_gates()).unwrap();
    assert_eq!(change, Change::Unchanged);
    assert_eq!(once, twice);
    assert_eq!(twice.matches(MARKER_START_PREFIX).count(), 2); // one per hook
}

#[test]
fn mangled_block_outside_commands_is_repaired() {
    // espectacular mangle class: ah writer glued the managed block
    // directly under `pre-commit:` instead of inside `commands:`
    let mangled = "\
pre-commit:
# ddl:managed:start (pre-commit)
  ah-check:
    run: ah check
# ddl:managed:end (pre-commit)

  parallel: true
  commands:
spec-gates:
  run: spk lint openspec
";
    let (out, change) = ensure_lefthook_gates(Some(mangled), &default_gates()).unwrap();
    assert_eq!(change, Change::Updated);
    // stray block removed (no markers glued at top of section)
    assert!(!out.contains("\n# ddl:managed:start (pre-commit)\n"));
    // repaired inside commands:, existing commands kept
    let cmds_pos = out.find("  commands:").unwrap();
    let managed_pos = out.find(MARKER_START_PREFIX).unwrap();
    assert!(cmds_pos < managed_pos);
    assert!(out.contains("run: pretender check --diff-only --mode gate"));
    assert!(out.contains("run: spk lint openspec/specs || test $? -eq 2"));
    assert!(out.contains(PRE_COMMIT_BLOCK));
    // exactly one pre-commit marker pair
    assert_eq!(out.matches("(pre-commit)").count(), 2); // start + end
}

#[test]
fn hook_without_commands_gets_commands_added() {
    let bare = "pre-commit:\n  parallel: true\n";
    let (out, change) = ensure_lefthook_gates(Some(bare), &default_gates()).unwrap();
    assert_eq!(change, Change::Updated);
    let cmds_pos = out.find("  commands:").unwrap();
    let managed_pos = out.find(MARKER_START_PREFIX).unwrap();
    assert!(cmds_pos < managed_pos);
    assert!(out.contains(PRE_COMMIT_BLOCK));
}

#[test]
fn gitignore_entries_added_idempotently() {
    let (out, added) = ensure_gitignore_tool_dirs(None);
    assert_eq!(
        added,
        GITIGNORE_TOOL_DIR_ENTRIES
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
    );
    assert!(out.contains(".testaruda/"));

    let existing = "target/\n.ddl/**/*.db\n";
    let (out2, added2) = ensure_gitignore_tool_dirs(Some(existing));
    assert_eq!(added2.len(), 2);
    assert!(out2.starts_with("target/")); // existing content preserved

    let (out3, added3) = ensure_gitignore_tool_dirs(Some(&out));
    assert!(added3.is_empty(), "re-run must not duplicate: {added3:?}");
    assert_eq!(out3, out);
}

#[test]
fn wiring_report_reports_export_and_staging_fields() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".beads/embeddeddolt")).unwrap();
    std::fs::write(
        dir.path().join(".beads/config.yaml"),
        "# Beads Configuration File\nno-db: false\n",
    )
    .unwrap();
    // A JSONL pre-seeded so no bd subprocess result leaks into the assert.
    std::fs::write(dir.path().join(".beads/issues.jsonl"), "{}").unwrap();
    // No git repo here → staging is a no-op (best-effort).
    let report = apply_gates_wiring(dir.path()).unwrap();
    assert_eq!(report.beads, BeadsChange::FlippedToNoDb);
    assert!(!report.beads_exported, "jsonl already present → no export");
    assert!(
        report.beads_staged.is_empty(),
        "no git repo → nothing staged"
    );
    let config = std::fs::read_to_string(dir.path().join(".beads/config.yaml")).unwrap();
    assert!(config.contains("no-db: true"));
}

#[test]
fn beads_nodb_created_flipped_and_stable() {
    let (out, change) = ensure_beads_nodb(None);
    assert_eq!(change, BeadsChange::Created);
    assert!(out.contains("no-db: true"));

    let (out2, change2) = ensure_beads_nodb(Some("# Beads Configuration File\nno-db: false\n"));
    assert_eq!(change2, BeadsChange::FlippedToNoDb);
    assert!(out2.contains("no-db: true"));
    assert!(!out2.contains("no-db: false"));

    let (out3, change3) = ensure_beads_nodb(Some("# Beads Configuration File\nno-db: true\n"));
    assert_eq!(change3, BeadsChange::Unchanged);
    assert_eq!(out3, "# Beads Configuration File\nno-db: true\n");
}

#[test]
fn detect_beads_export_problems_covers_invariant() {
    // No .beads at all → healthy (nothing to check).
    let base = BeadsExportProblemsInput {
        beads_dir_exists: false,
        ..Default::default()
    };
    assert!(detect_beads_export_problems(&base).is_empty());

    // Healthy no-db repo: tracked, fresh export.
    let healthy = BeadsExportProblemsInput {
        beads_dir_exists: true,
        config_exists: true,
        config_no_db: true,
        embeddeddolt_exists: false,
        issues_jsonl_exists: true,
        issues_jsonl_tracked: Some(true),
        issues_jsonl_lines: Some(41),
        bd_count: Some(41),
    };
    assert!(detect_beads_export_problems(&healthy).is_empty());

    // db-mode config without the flip.
    let db_mode = BeadsExportProblemsInput {
        beads_dir_exists: true,
        config_exists: true,
        config_no_db: false,
        ..healthy
    };
    let p = detect_beads_export_problems(&db_mode);
    assert!(p.iter().any(|m| m.contains("no-db mode")), "{p:?}");

    // DB with data but no export (funkalization trap).
    let no_export = BeadsExportProblemsInput {
        beads_dir_exists: true,
        embeddeddolt_exists: true,
        issues_jsonl_exists: false,
        issues_jsonl_tracked: None,
        config_exists: true,
        config_no_db: true,
        ..Default::default()
    };
    let p = detect_beads_export_problems(&no_export);
    assert!(
        p.iter().any(|m| m.contains("issues.jsonl is missing")),
        "{p:?}"
    );

    // Export present but untracked (gently trap).
    let untracked = BeadsExportProblemsInput {
        beads_dir_exists: true,
        config_exists: true,
        config_no_db: true,
        issues_jsonl_exists: true,
        issues_jsonl_tracked: Some(false),
        ..healthy
    };
    let p = detect_beads_export_problems(&untracked);
    assert!(p.iter().any(|m| m.contains("not tracked")), "{p:?}");

    // Stale export (uarup/titi/XAct trap).
    let stale = BeadsExportProblemsInput {
        beads_dir_exists: true,
        embeddeddolt_exists: false,
        config_exists: true,
        config_no_db: true,
        issues_jsonl_exists: true,
        issues_jsonl_tracked: Some(true),
        issues_jsonl_lines: Some(3),
        bd_count: Some(7),
    };
    let p = detect_beads_export_problems(&stale);
    assert!(p.iter().any(|m| m.contains("stale")), "{p:?}");

    // bd absent → staleness skipped, other checks still run.
    let no_bd = BeadsExportProblemsInput {
        beads_dir_exists: true,
        embeddeddolt_exists: false,
        config_exists: true,
        config_no_db: true,
        issues_jsonl_exists: true,
        issues_jsonl_tracked: Some(true),
        issues_jsonl_lines: Some(3),
        bd_count: None,
    };
    assert!(detect_beads_export_problems(&no_bd).is_empty());
}

#[test]
fn beads_export_needs_export_only_when_db_exists_and_jsonl_missing() {
    let input = |db: bool, jsonl: bool| BeadsExportInput {
        embeddeddolt_exists: db,
        issues_jsonl_exists: jsonl,
    };
    // DB with no export → the funkalization trap: flipping no-db would
    // hide every issue. Export must happen first.
    assert!(beads_export_needs_export(&input(true, false)));
    // No DB (pure no-db repo or pre-init) → nothing to export.
    assert!(!beads_export_needs_export(&input(false, false)));
    assert!(!beads_export_needs_export(&input(false, true)));
    // Export already present → no-op.
    assert!(!beads_export_needs_export(&input(true, true)));
}

#[test]
fn detect_flags_missing_file_and_clean_wiring() {
    assert_eq!(
        detect_lefthook_gates_problems(None, &default_gates()),
        vec!["lefthook.yml not found — run `ddl init --gates`"]
    );
    // The fixture fixture has the managed blocks wired by hand? No — the
    // plain COMMANDS_HOOK fixture has no managed blocks at all.
    let problems = detect_lefthook_gates_problems(Some(COMMANDS_HOOK), &default_gates());
    assert_eq!(problems.len(), 2, "both hooks missing: {problems:?}");
    assert!(problems[0].contains("pre-commit managed block missing"));
    assert!(problems[1].contains("pre-push managed block missing"));

    // Fully wired (idempotent output of ensure) → no problems.
    let (wired, _) = ensure_lefthook_gates(Some(COMMANDS_HOOK), &default_gates()).unwrap();
    assert!(detect_lefthook_gates_problems(Some(&wired), &default_gates()).is_empty());

    // A file that merely exists but has no managed blocks still counts
    // as unwired — existence alone is not "gates wired".
    let bare_header = "# lefthook.yml — ddl-managed gates (created by `ddl init --gates`)\n";
    assert_eq!(
        detect_lefthook_gates_problems(Some(bare_header), &default_gates()).len(),
        2,
        "both hooks flagged"
    );
}

#[test]
fn detect_flags_mangled_duplicate_and_mismatched_blocks() {
    // Block glued outside commands: (espectacular mangle class)
    let mangled = "pre-commit:\n# ddl:managed:start (pre-commit)\n  ah-check:\n    run: ah check\n# ddl:managed:end (pre-commit)\n\n  commands:\n    spec-gates:\n      run: spk lint openspec\n";
    let problems = detect_lefthook_gates_problems(Some(mangled), &default_gates());
    assert!(
        problems.iter().any(|p| p.contains("outside commands:")),
        "mangled block flagged: {problems:?}"
    );
    assert!(
        problems
            .iter()
            .any(|p| p.contains("pre-push managed block missing"))
    );

    // Duplicate blocks
    let (once, _) = ensure_lefthook_gates(Some(COMMANDS_HOOK), &default_gates()).unwrap();
    let duplicated = format!("{once}\n{}\n", PRE_COMMIT_BLOCK);
    let problems = detect_lefthook_gates_problems(Some(&duplicated), &default_gates());
    assert!(
        problems
            .iter()
            .any(|p| p.contains("pre-commit managed block duplicated")),
        "duplicate flagged: {problems:?}"
    );

    // Mismatched markers: start without end
    let mismatched = format!("{once}\n# ddl:managed:start (pre-commit)\n");
    let problems = detect_lefthook_gates_problems(Some(&mismatched), &default_gates());
    assert!(
        problems
            .iter()
            .any(|p| p.contains("pre-commit managed block has mismatched start/end")),
        "mismatch flagged: {problems:?}"
    );
}

#[test]
fn default_render_matches_the_canonical_pre_commit_block() {
    assert_eq!(render_pre_commit_block(&default_gates()), PRE_COMMIT_BLOCK);
}

#[test]
fn default_render_matches_the_canonical_pre_push_block() {
    assert_eq!(render_pre_push_block(&default_gates()), PRE_PUSH_BLOCK);
}

#[test]
fn ah_check_override_changes_both_rendered_spans() {
    let commands = GateCommands {
        ah_check: "python3 scripts/ah_check_ratchet.py".to_string(),
        ..default_gates()
    };
    assert_eq!(
        render_pre_push_block(&commands),
        "    # ddl:managed:start (pre-push)\n    ddl-ah-check:\n      run: python3 scripts/ah_check_ratchet.py\n    # ddl:managed:end (pre-push)"
    );

    let (wired, _) = ensure_lefthook_gates(Some(COMMANDS_HOOK), &default_gates()).unwrap();
    let (out, change) = ensure_lefthook_gates(Some(&wired), &commands).unwrap();
    assert_eq!(change, Change::Updated);
    assert_eq!(
        out.matches("run: python3 scripts/ah_check_ratchet.py")
            .count(),
        2,
        "pre-commit and pre-push both carry the ratchet: {out}"
    );
    assert!(!out.contains("run: ah check\n"), "{out}");
    assert!(!out.contains(PRE_PUSH_BLOCK), "{out}");
}

#[test]
fn empty_ah_check_omits_the_entry_from_both_spans() {
    let commands = GateCommands {
        ah_check: String::new(),
        ..default_gates()
    };
    assert_eq!(
        render_pre_push_block(&commands),
        "    # ddl:managed:start (pre-push)\n    # ddl:managed:end (pre-push)"
    );
    assert!(!render_pre_commit_block(&commands).contains("ddl-ah-check"));
}

#[test]
fn stale_pre_push_span_is_replaced_with_the_configured_command() {
    let (wired, _) = ensure_lefthook_gates(Some(COMMANDS_HOOK), &default_gates()).unwrap();
    assert!(wired.contains(PRE_PUSH_BLOCK));

    let commands = GateCommands {
        ah_check: "python3 scripts/ah_check_ratchet.py".to_string(),
        ..default_gates()
    };
    let (out, change) = ensure_lefthook_gates(Some(&wired), &commands).unwrap();
    assert_eq!(
        change,
        Change::Updated,
        "stale pre-push span is not a no-op"
    );
    assert!(out.contains(&render_pre_push_block(&commands)), "{out}");
    assert!(!out.contains(PRE_PUSH_BLOCK), "{out}");
    // Replacement, not duplication: one marker pair per hook.
    assert_eq!(out.matches("(pre-push)").count(), 2);
    assert_eq!(out.matches("(pre-commit)").count(), 2);
}

#[test]
fn detect_reports_drift_for_hand_mutated_pre_push_command() {
    let configured = GateCommands {
        ah_check: "ah check --all".to_string(),
        ..default_gates()
    };
    let (wired, _) = ensure_lefthook_gates(Some(COMMANDS_HOOK), &configured).unwrap();
    // Hand-revert only the pre-push span to the old bare `ah check`.
    let mutated = wired.replace(&render_pre_push_block(&configured), PRE_PUSH_BLOCK);
    assert_ne!(mutated, wired);

    let problems = detect_lefthook_gates_problems(Some(&mutated), &configured);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].contains("pre-push managed block has drifted"),
        "{problems:?}"
    );
    assert!(problems[0].contains("ddl init --gates"), "{problems:?}");

    // A span matching the configured rendering is clean.
    assert!(detect_lefthook_gates_problems(Some(&wired), &configured).is_empty());
}

#[test]
fn configured_command_overrides_change_the_rendered_line() {
    let commands = GateCommands {
        ah_check: "ah check --strict".to_string(),
        ..default_gates()
    };
    let block = render_pre_commit_block(&commands);
    assert!(block.contains("      run: ah check --strict"), "{block}");
    assert!(!block.contains("      run: ah check\n"), "{block}");
    // Untouched overrides keep their defaults.
    assert!(block.contains("      run: pretender check --diff-only --mode gate"));
    assert!(block.contains("      run: spk lint openspec/specs || test $? -eq 2"));
}

#[test]
fn empty_spk_lint_omits_the_managed_entry() {
    let commands = GateCommands {
        spk_lint: String::new(),
        ..default_gates()
    };
    let block = render_pre_commit_block(&commands);
    assert!(!block.contains("ddl-spk-lint"), "{block}");
    assert!(!block.contains("spk lint"), "{block}");
    assert!(block.contains("ddl-ah-check"));
    assert!(block.contains("ddl-pretender-gate"));
    assert!(block.contains("# ddl:managed:end (pre-commit)"));
}

#[test]
fn stale_span_is_replaced_with_the_configured_commands() {
    let (wired, change) = ensure_lefthook_gates(Some(COMMANDS_HOOK), &default_gates()).unwrap();
    assert_eq!(change, Change::Updated);

    let commands = GateCommands {
        ah_check: "ah check --strict".to_string(),
        ..default_gates()
    };
    let (out, change) = ensure_lefthook_gates(Some(&wired), &commands).unwrap();
    assert_eq!(change, Change::Updated, "stale span is not a no-op");
    assert!(out.contains(&render_pre_commit_block(&commands)), "{out}");
    assert!(
        !out.contains(PRE_COMMIT_BLOCK),
        "stale canonical span replaced: {out}"
    );
    // Exactly one marker pair per hook — replacement, not duplication.
    assert_eq!(out.matches("(pre-commit)").count(), 2);
    assert_eq!(out.matches("(pre-push)").count(), 2);
    // The pre-push span follows the same ah_check override.
    assert!(out.contains(&render_pre_push_block(&commands)), "{out}");
    assert!(!out.contains(PRE_PUSH_BLOCK), "{out}");
}

#[test]
fn detect_reports_drift_for_hand_mutated_command_inside_correct_span() {
    let (wired, _) = ensure_lefthook_gates(Some(COMMANDS_HOOK), &default_gates()).unwrap();
    let mutated_block = PRE_COMMIT_BLOCK.replace("run: ah check", "run: ah check --all");
    let mutated = wired.replace(PRE_COMMIT_BLOCK, &mutated_block);
    assert_ne!(mutated, wired);

    let problems = detect_lefthook_gates_problems(Some(&mutated), &default_gates());
    assert!(
        problems.iter().any(|p| p.contains("drifted")),
        "hand-mutated command flagged as drift: {problems:?}"
    );
    // Placement is fine — the only reported defect is the drift.
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].contains("ddl init --gates"),
        "fix command named: {problems:?}"
    );

    // A wiring produced from the configured rendering is clean.
    let configured = GateCommands {
        ah_check: "ah check --all".to_string(),
        ..default_gates()
    };
    let (wired_configured, _) = ensure_lefthook_gates(Some(COMMANDS_HOOK), &configured).unwrap();
    assert!(detect_lefthook_gates_problems(Some(&wired_configured), &configured).is_empty());
}

#[test]
fn fresh_canonical_template_contains_both_managed_blocks() {
    let (out, change) = ensure_lefthook_gates(None, &default_gates()).unwrap();
    assert_eq!(change, Change::Created);
    assert!(out.contains(PRE_COMMIT_BLOCK));
    assert!(out.contains(PRE_PUSH_BLOCK));
    assert!(detect_lefthook_gates_problems(Some(&out), &default_gates()).is_empty());
}

#[test]
fn init_wiring_renders_spans_from_ddl_config() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".ddl")).unwrap();
    std::fs::write(
        dir.path().join(".ddl/config.toml"),
        "[gates]\nah_check = \"ah check --strict\"\nspk_lint = \"\"\n",
    )
    .unwrap();
    // Existing canonical wiring is stale against this config.
    let (wired, _) = ensure_lefthook_gates(None, &default_gates()).unwrap();
    std::fs::write(dir.path().join("lefthook.yml"), &wired).unwrap();

    let report = apply_gates_wiring(dir.path()).unwrap();
    assert_eq!(report.lefthook, Change::Updated, "stale span rewritten");
    let out = std::fs::read_to_string(dir.path().join("lefthook.yml")).unwrap();
    assert_eq!(
        out.matches("      run: ah check --strict").count(),
        2,
        "both spans rendered from config: {out}"
    );
    assert!(
        !out.contains("ddl-spk-lint"),
        "empty spk_lint omitted: {out}"
    );
}

#[test]
fn openspec_inits_before_ah() {
    assert_eq!(tool_init_order(&["ah", "openspec"]), vec!["openspec", "ah"]);
    assert_eq!(tool_init_order(&["ah"]), vec!["ah"]);
    assert_eq!(
        tool_init_order(&["bd", "ah", "openspec", "wai"]),
        vec!["bd", "openspec", "ah", "wai"]
    );
    assert!(tool_init_order(&[]).is_empty());
}
