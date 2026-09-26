# Fix Auto-Sync Workspace Registration Issue

**Issue:** [#2288](https://github.com/Ankush-Jha/peach-ice-tea/issues/2288)

**Problem:** The zsh plugin automatically syncs workspaces in the background on every directory change, causing unintended parent directories to be registered as workspaces. When users run `peach workspace list`, ancestor directories appear as "Current" instead of the actual working directory.

**Root Cause:** `_peach_start_background_sync()` in `shell-plugin/lib/helpers.zsh:114-131` unconditionally runs `peach workspace sync` without checking if the workspace or its ancestors are already registered.

---

## Objective

Prevent auto-sync from creating unintended workspace registrations while maintaining the convenience of automatic syncing for already-indexed workspaces.

---

## Implementation Plan

### Phase 1: Add Porcelain Mode to Workspace Info Command

- [ ] Add `--porcelain` flag to `workspace info` command in `crates/peach_main/src/cli.rs:262-266`
- [ ] Update `on_workspace_info()` in `crates/peach_main/src/ui.rs:3328-3389` to handle porcelain mode
- [ ] In porcelain mode, return exit code 0 if workspace exists (Some), exit code 1 if None
- [ ] In porcelain mode, output should be silent (no text output, only exit code)
- [ ] Existing behavior (display mode) remains unchanged
- [ ] This leverages the existing `get_workspace_info()` call which already checks for workspace or ancestor

### Phase 2: Update Zsh Background Sync Logic

- [ ] Modify `_peach_start_background_sync()` in `shell-plugin/lib/helpers.zsh:114-131`
- [ ] Before running sync, call `$_PEACH_BIN workspace info "$workspace_path" --porcelain 2>/dev/null`
- [ ] Check exit code: if non-zero (not indexed), skip the sync and return early
- [ ] Only proceed with background sync if exit code is 0 (workspace or ancestor is already indexed)
- [ ] Add debug logging (when `PEACH_DEBUG=true`) to indicate when sync is skipped
- [ ] Ensure the check runs silently (stderr redirect already in place)

### Phase 3: Handle Initial Workspace Registration

- [ ] Ensure manual `peach workspace sync` still works for first-time registration
- [ ] Update shell-plugin documentation to clarify the auto-sync behavior (only syncs already-registered workspaces)
- [ ] Consider adding a one-time message when users navigate to unindexed directories suggesting `peach workspace sync`

### Phase 4: Update Workspace List Display (Optional Enhancement)

- [ ] Enhance `on_list_workspaces()` in `crates/peach_main/src/ui.rs:3266-3311` to distinguish between exact match and ancestor match
- [ ] When current workspace is an ancestor match, display as `[Current via ancestor]` or similar
- [ ] Show the actual current directory path when it differs from the workspace path
- [ ] This provides clarity without changing core behavior

---

## Verification Criteria

### Before Fix
- [ ] Run `cd /Users/username` (parent directory)
- [ ] Run `peach workspace sync` once
- [ ] Run `cd /Users/username/Documents/Projects/peach` (subdirectory)
- [ ] Trigger zsh accept-line (press Enter on any command)
- [ ] Run `peach workspace list`
- [ ] Verify: `/Users/username/Documents/Projects/peach` is incorrectly registered as a workspace

### After Fix
- [ ] Clean database and repeat above steps
- [ ] Run `cd /Users/username/Documents/Projects/peach`
- [ ] Trigger zsh accept-line (press Enter)
- [ ] Run `peach workspace list`
- [ ] Verify: Only `/Users/username` appears (no auto-sync of subdirectory)
- [ ] Verify: List shows parent as Current (expected due to ancestor matching)
- [ ] Run explicit `peach workspace sync` in subdirectory
- [ ] Verify: Now subdirectory is registered and will auto-sync on future visits

### Edge Cases
- [ ] Test behavior when no workspace exists at all (should not auto-sync)
- [ ] Test `peach workspace info` with `--porcelain` flag returns correct exit codes
- [ ] Test behavior in deeply nested directories (should find closest ancestor)
- [ ] Test behavior when `PEACH_SYNC_ENABLED=false` (should still respect flag)
- [ ] Test manual sync in new directory (should still work)

---

## Potential Risks and Mitigations

### Risk 1: Breaking Existing User Workflows
**Mitigation:** Users who rely on automatic workspace creation will need to run `peach workspace sync` once per workspace. This is a one-time migration cost for better UX long-term.

### Risk 2: Performance Impact
**Mitigation:** The `workspace info --porcelain` check reuses existing `get_workspace_info()` which is already fast. Negligible overhead.

### Risk 3: Race Conditions
**Mitigation:** The check and sync are not atomic, but this is acceptable - worst case, a sync happens when it shouldn't have. No data corruption risk.

---

## Alternative Approaches

### Alternative 1: Create New `is-indexed` Subcommand
Create a dedicated `peach workspace is-indexed` command instead of extending `info`.

**Pros:** Clearer intent, dedicated purpose  
**Cons:** More API surface, duplicates existing functionality

### Alternative 2: Display-Only Fix
Just improve the workspace list display to show `[Current via ancestor]` without changing auto-sync behavior.

**Pros:** Simpler implementation, no behavior changes  
**Cons:** Doesn't solve the root problem of unintended workspace registrations

### Alternative 3: Auto-Sync with Depth Limit
Only auto-sync directories within N levels of an existing workspace.

**Pros:** Prevents deep directory pollution  
**Cons:** Arbitrary limit, still creates unintended workspaces

---

## Dependencies

- No external dependencies
- Reuses existing `get_workspace_info()` method which already handles ancestor matching
- Zsh plugin already has access to `$_PEACH_BIN` for CLI calls

---

## Testing Strategy

1. **Unit Tests:** Add tests for `workspace info --porcelain` exit codes
2. **Integration Tests:** Test zsh function behavior with mocked CLI commands
3. **Manual Testing:** Follow verification criteria above
4. **Regression Testing:** Ensure existing workspace info functionality unchanged

---

## Documentation Updates

- [ ] Update `shell-plugin/README.md` to explain auto-sync behavior
- [ ] Add note about one-time `workspace sync` requirement for new workspaces
- [ ] Document `PEACH_SYNC_ENABLED` environment variable
- [ ] Update workspace info command help text to mention `--porcelain` flag
