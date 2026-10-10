//! Hook-env hygiene for dulce tests (testaruda-c64 pattern, generalized).
//!
//! When tests run under a git hook from a linked WORKTREE, git exports
//! GIT_DIR (pointing at the outer repo's worktree gitdir); fixtures that
//! `git init` a temp dir then race on the outer repo's config lock
//! ("could not lock config file … File exists"). The strip runs at binary
//! init (.init_array), before any test thread starts, so it does not race
//! parallel test threads.

#[cfg(unix)]
#[used]
#[unsafe(link_section = ".init_array")]
static STRIP_HOOK_GIT_ENV: extern "C" fn() = strip_hook_git_env;

#[cfg(unix)]
extern "C" fn strip_hook_git_env() {
    for var in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_PREFIX",
        "GIT_CONFIG_PARAMETERS",
        "GIT_QUARANTINE_PATH",
    ] {
        // Safe: called once at process init, before test threads spawn.
        unsafe { std::env::remove_var(var) };
    }
}

/// Strip hook-exported git env from a spawned tool command (assert_cmd) —
/// children of `ddl` (git init, bd init) must not target the OUTER repo.
pub fn clean_git_env(cmd: &mut assert_cmd::Command) {
    for var in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_PREFIX",
        "GIT_CONFIG_PARAMETERS",
        "GIT_QUARANTINE_PATH",
    ] {
        cmd.env_remove(var);
    }
}
