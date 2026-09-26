# Config Read at Application Init — Surface Errors, Remove Silent Defaults

## Objective

Currently, `PeachConfig` is read lazily from disk inside `PeachEnvironmentInfra` and any parse/deserialization errors are silently swallowed — returning `PeachConfig::default()` (all-zero values) with only a tracing log that the user never sees. This causes silent breakage of all tool limits and agent parameters when the user's config file is corrupt or invalid.

The goal is to:
1. Read `PeachConfig` **once at application startup** in `main.rs`, surfacing any parse error directly to the user before the app proceeds.
2. Pass the pre-read config through the construction chain to every consumer.
3. **Remove `get_config()` from the `EnvironmentInfra` trait** entirely — it is no longer needed since config is injected at construction time.
4. Preserve the `update_environment` write path so that `peach config set` commands continue to work correctly.

---

## Architecture Overview

### Current Flow (Lazy, Silent-Error)

```
main.rs
  └─ PeachAPI::init(cwd)
       └─ PeachInfra::new(cwd)
            └─ PeachEnvironmentInfra::new(cwd)      ← cache = None
                 └─ (first get_config() call)
                      └─ PeachConfig::read()         ← disk I/O
                           └─ Err(_) → Default::default()  ← SILENT!
```

### Target Flow (Eager, Error-Surfaced)

```
main.rs
  └─ PeachConfig::read()?                            ← fails loudly here
  └─ PeachAPI::init(cwd, config)
       └─ PeachInfra::new(cwd, config)
            └─ PeachEnvironmentInfra::new(cwd, config)  ← cache = Some(config)
            └─ PeachHttpInfra::new(config, ...)
            └─ PeachDirectoryReaderService::new(config.max_parallel_file_reads)
            └─ PeachGrpcClient::new(config.services_url.parse()?)
```

### What `update_environment` Does (Preserved)

`PeachEnvironmentInfra::update_environment` performs a **read-from-disk → mutate → write-to-disk → cache-invalidate** cycle whenever the user changes provider/model/reasoning settings. After invalidation, the next `get_config()` call re-reads from disk. Since `get_config()` is being removed from the trait, the internal cache mechanism in `PeachEnvironmentInfra` must be adapted so the fresh post-write value is propagated back to all consumers that hold a stored config.

The cleanest resolution: `update_environment` returns the updated `PeachConfig` in its result, and callers store the new value. However, since the trait is used in many places, the simplest compatible approach is to keep the cache internal to `PeachEnvironmentInfra` but make `update_environment` callable from the services layer to update its stored config — or to have the application re-read config after an update via a narrower dedicated trait method (not `get_config()` on the general infra trait).

---

## Key Invariants to Preserve

1. **`update_environment` must still work** — `peach config set model ...` must update the TOML and the in-memory state visible to subsequent calls.
2. **`/new` conversation re-creation** — `on_new()` calls `(self.new_api)()`, rebuilding `PeachAPI`. The captured `PeachConfig` in the closure must be the **latest** config (post any `update_environment` calls), not the startup snapshot.
3. **`get_config()` removal scope** — removing it from `EnvironmentInfra` does not mean removing it from every consumer; it means the consumers receive the value as a constructor argument or method parameter rather than calling `infra.get_config()` at runtime.
4. **`services_url` panic elimination** — with config surfaced in `main.rs`, the `.expect()` on `services_url.parse()` can be converted to `?`-propagation, making `PeachInfra::new()` fallible.

---

## Implementation Plan

### Phase 1 — Surface Config Errors in `PeachConfig::read()`

- [~] Task 1.1. **Fix silent error in `ConfigReader::read_global()`** (`crates/peach_config/src/reader.rs`): The `.required(false)` flag on `config::File::from(path)` silently swallows parse errors for malformed TOML files. Change this so that if the file *exists* but is invalid (e.g., malformed TOML, wrong types), an error is returned. Only missing files should be silently skipped. This may require checking file existence before adding the `config` source, or using a custom file reader that returns `Err` on parse failure but `Ok` on file-not-found.

- [ ] Task 1.2. **Fix silent skip in `ConfigReader::read_legacy()`** (`crates/peach_config/src/reader.rs`): Currently uses `if let Ok(content) = content { ... } else { self }` — silently ignores errors. Change to at minimum emit a `warn!` log message, or propagate the error. Since legacy JSON is a migration concern, a `warn!` is appropriate rather than a hard error.

- [ ] Task 1.3. **Verify `PeachConfig::read()` return type** — it already returns `anyhow::Result<PeachConfig>`. No signature change needed here. The fix is upstream in the reader chain ensuring errors actually reach the `Result::Err` variant.

### Phase 2 — Read Config Once in `main.rs`

- [ ] Task 2.1. **Call `PeachConfig::read()` at the top of `main()` in `crates/peach_main/src/main.rs`** before the `UI::init` call. Use `?` propagation so any error is printed to stderr and exits with a non-zero code. The error message from `anyhow` will include the cause (e.g., "invalid TOML at line 12: expected string, found integer"), which is exactly what the user needs to see.

- [ ] Task 2.2. **Thread the `config: PeachConfig` into the `UI::init` factory closure** in `main.rs`. The closure currently captures `cwd: PathBuf`; it must now also capture `config: PeachConfig`. Since `PeachConfig` derives `Clone`, the closure can clone it on each invocation (once at startup, once per `/new` command).

  > **Note on `/new` and config freshness**: The closure will capture the **startup config**. After a `peach config set` command, `update_environment` writes to disk and invalidates the `PeachEnvironmentInfra` internal cache. However, since `get_config()` is being removed from the trait, the stale startup config captured in the closure would be used on the next `/new`. This must be addressed in Phase 4.

### Phase 3 — Propagate Config Through the Construction Chain

- [ ] Task 3.1. **Change `PeachAPI::init` signature** (`crates/peach_api/src/peach_api.rs`): Add `config: PeachConfig` as a parameter. Forward it to `PeachInfra::new(cwd, config)`. This is the only `impl PeachAPI<...>` concrete method that constructs the infra stack.

- [ ] Task 3.2. **Change `PeachInfra::new` signature** (`crates/peach_infra/src/peach_infra.rs`): Add `config: PeachConfig` as a parameter. Remove the `config_infra.get_config()` call that currently triggers the lazy disk read. Pass `config` directly to:
  - `PeachEnvironmentInfra::new(cwd, config)` — seeds the internal cache
  - `PeachHttpInfra::new(config.clone(), ...)` — already accepts `PeachConfig`
  - `PeachDirectoryReaderService::new(config.max_parallel_file_reads)` — already accepts the field
  - `PeachGrpcClient::new(...)` — use `?` propagation instead of `.expect()` (see Task 3.3)

  Make `PeachInfra::new` return `anyhow::Result<Self>` to allow `?`-propagation from within.

- [ ] Task 3.3. **Replace `.expect()` with `?` for `services_url` parsing** (`crates/peach_infra/src/peach_infra.rs:73-78`): Now that `PeachInfra::new` is fallible, convert `config.services_url.parse().expect(...)` to `config.services_url.parse().context("services_url must be a valid URL")?`. This turns a panic into a clean error message at startup.

- [ ] Task 3.4. **Change `PeachEnvironmentInfra::new` signature** (`crates/peach_infra/src/env.rs`): Add `config: PeachConfig` parameter. Initialize `cache` as `Arc::new(Mutex::new(Some(config)))` instead of `None`. The `cached_config()` method (`env.rs:125-134`) already handles the `Some` case by returning the cached value — it will simply never need to perform a disk read for the initial config. The read-from-disk path in `read_from_disk()` becomes dead code after this change and can be removed.

- [ ] Task 3.5. **Update `PeachAPI::init` call site in `main.rs`** to handle the new `Result<Self>` return from `PeachInfra::new` (if propagated up through `PeachAPI::init`). The closure passed to `UI::init` currently produces `A` (not `Result<A>`); if `PeachAPI::init` becomes fallible, either the closure's return type changes to `Result<A>` and `on_new` handles the error, or the URL validation is done eagerly before the closure is constructed in `main.rs`.

  > **Recommended resolution**: Validate `services_url` in `main.rs` by parsing it there using `config.services_url.parse::<Url>().context(...)?` before creating the closure. Pass the parsed `Url` into `PeachInfra::new` instead of the raw string. This keeps the factory closure infallible (consistent with current `UI<A, F>` design where `F: Fn() -> A`).

### Phase 4 — Handle Config Freshness After `update_environment`

The `/new` closure captures `PeachConfig` at startup. After `update_environment` writes a new config and invalidates the `PeachEnvironmentInfra` cache, the next `/new` would reconstruct `PeachAPI` with the stale startup config. This must be addressed.

- [ ] Task 4.1. **Expose a config-accessor on the existing `API` trait** that the `UI` can use to retrieve the latest config when constructing a new API instance on `/new`. Since `PeachEnvironmentInfra` still holds the authoritative in-memory cache (updated after `update_environment`), calling `api.get_config()` after an `update_environment` will return the fresh value. The `UI` can call `self.api.get_config()` inside `on_new` to get the latest config, then pass it to the new `PeachAPI` factory.

  Concretely: Change the factory closure stored in `new_api` from `Fn() -> A` to `Fn(PeachConfig) -> A`. The `UI::on_new` method calls `(self.new_api)(self.api.get_config())` to forward the live config into the new API instance.

  Adjust `UI<A, F>` struct and `UI::init` accordingly:
  - Change the `F` bound from `Fn() -> A` to `Fn(PeachConfig) -> A`
  - Update `main.rs` closure from `move || PeachAPI::init(cwd.clone())` to `move |config| PeachAPI::init(cwd.clone(), config)`
  - Update `on_new` to call `(self.new_api)(self.api.get_config())` 

  > This preserves `get_config()` on the `API` trait (not `EnvironmentInfra`) for this specific use case. The `API` trait's `get_config()` can delegate to the stored infra's cache (same as today), but the `EnvironmentInfra` trait no longer exposes it.

### Phase 5 — Remove `get_config()` from `EnvironmentInfra` Trait

- [ ] Task 5.1. **Remove `get_config()` from the `EnvironmentInfra` trait** in `crates/peach_app/src/infra.rs`. This is the core structural change. All code that calls `infra.get_config()` through the trait must be updated to receive `PeachConfig` through another mechanism (constructor parameter, method parameter).

- [ ] Task 5.2. **Remove `get_config()` from `PeachInfra`** (`crates/peach_infra/src/peach_infra.rs`): The delegation to `config_infra.get_config()` is no longer needed.

- [ ] Task 5.3. **Remove `get_config()` from `PeachRepo`** (`crates/peach_repo/src/peach_repo.rs`): Same delegation removal.

- [ ] Task 5.4. **Remove `get_config()` from `PeachServices`** (`crates/peach_services/src/peach_services.rs`): Same delegation removal. The `AppConfigService` methods currently call `self.infra.get_config()` to read existing config fields before mutating — these should instead use the config passed through method parameters or read from the updated state returned by `update_environment`.

- [ ] Task 5.5. **Audit all call sites of `get_config()` through the `EnvironmentInfra` trait** — specifically inside `crates/peach_app/src/app.rs`, `crates/peach_app/src/agent.rs`, `crates/peach_app/src/tool_executor.rs`, `crates/peach_app/src/operation.rs`, and `crates/peach_services/src/app_config.rs`. For each call site:
  - If the caller is a service method, consider passing `PeachConfig` as a method parameter from the call site one level up.
  - If the caller is a long-lived struct that currently reads config on every operation, consider storing `PeachConfig` as a field injected at construction time.

- [ ] Task 5.6. **Update `PeachAppConfigService::get_default_provider`, `get_provider_model`, etc.** (`crates/peach_services/src/app_config.rs`): These currently call `self.infra.get_config()` to read the current provider/model. After removing `get_config()` from `EnvironmentInfra`, the service needs another way to get the current config. Options:
  - Store `PeachConfig` in `PeachAppConfigService` at construction time (simplest, but may go stale after `update_environment`).
  - Have `update_environment` return the updated `PeachConfig` value, letting callers store the fresh value.
  - Keep a **separate, narrow trait** like `ConfigReader` with only `get_config()` on it, distinct from `EnvironmentInfra`, used only where runtime config re-reads are genuinely needed (i.e., after `update_environment` writes).

  > **Recommended**: The cleanest approach given the existing architecture is to have `update_environment` return `PeachConfig` (the new config after applying ops). Services that call `update_environment` can then update their stored config reference. This requires changing `update_environment`'s return type from `anyhow::Result<()>` to `anyhow::Result<PeachConfig>` in the `EnvironmentInfra` trait.

### Phase 6 — Remove `read_from_disk()` Dead Code

- [ ] Task 6.1. **Remove `read_from_disk()` from `PeachEnvironmentInfra`** (`crates/peach_infra/src/env.rs`): With the cache always pre-seeded from the constructor, the `read_from_disk()` method and its `error!("Failed to read config file. Using default config.")` fallback are dead code. Remove both the method and the `// NOTE: This should never-happen` comment that has been lying about the real risk.

- [ ] Task 6.2. **Simplify `cached_config()`** (`crates/peach_infra/src/env.rs`): The `Mutex<Option<PeachConfig>>` can be simplified. If the cache is always initialized via the constructor and only set to `None` by `update_environment` (which then repopulates from disk), the `Option` wrapper is still needed for the update cycle. Retain the structure but remove the `read_from_disk()` call in the `None` branch — instead call `ConfigReader::default().read_defaults().read_global().build()?` directly and propagate the error rather than swallowing it.

  > **Note**: `cached_config()` currently returns `PeachConfig` (not `Result<PeachConfig>`). If `update_environment` invalidates the cache and the subsequent re-read can fail, `cached_config()` must return `Result<PeachConfig>`. Propagate this change upward through `get_config()` on `PeachEnvironmentInfra` (which may remain internal/non-trait after trait removal).

### Phase 7 — Update Tests and Snapshots

- [ ] Task 7.1. **Update mock implementations of `EnvironmentInfra`** in test code: Any `#[cfg(test)]` or mock structs that implement `EnvironmentInfra` will need `get_config()` removed from their impl blocks after Phase 5. Search for all `impl EnvironmentInfra` in the codebase and remove the `get_config()` method bodies.

- [ ] Task 7.2. **Update tests that exercise `PeachInfra::new` or `PeachAPI::init`**: These now require a `PeachConfig` argument. Construct test configs using `PeachConfig::default()` (which provides Rust-defaulted zeroes) or `PeachConfig::read()` where a realistic config is needed.

- [ ] Task 7.3. **Run `cargo insta test --accept`** to update any snapshot tests affected by structural changes.

- [ ] Task 7.4. **Run `cargo check`** across the workspace to surface any remaining compilation errors from the trait removal cascade.

---

## Verification Criteria

- Starting peach with a corrupt `~/.peach.toml` file (e.g., `peach = {invalid`) must print a clear, human-readable error message to stderr and exit with a non-zero code — not silently start with default config.
- Starting peach with a `PEACH_SERVICES_URL=not-a-url` environment variable must print a clear error and exit rather than panicking.
- `peach config set model anthropic claude-3-opus` must still work correctly — the model is updated in `~/.peach.toml` and subsequent operations use the new model.
- Starting a new conversation with `/new` after `peach config set ...` must reflect the updated config (not the startup snapshot).
- `peach env` must still display the current config.
- All existing tests pass under `cargo insta test --accept`.
- `cargo check` produces no errors across the workspace.

---

## Potential Risks and Mitigations

1. **`/new` gets stale config after `update_environment`**
   Mitigation: Change the factory closure type from `Fn() -> A` to `Fn(PeachConfig) -> A` (Task 4.1). `on_new` reads the latest config from the live API before constructing the new one.

2. **`EnvironmentInfra` implementors in external or test code break on trait change**
   Mitigation: Search all `impl EnvironmentInfra` blocks across the workspace (Task 7.1) and update them. Since the trait is internal (not `pub` across crate boundaries to user code), the scope is bounded to this repository.

3. **`update_environment` re-read from disk can fail post-startup**
   Mitigation: Change `update_environment` return type to `anyhow::Result<PeachConfig>` and propagate the error up through the service layer to the UI, which displays it as a user-visible error message rather than silently ignoring it.

4. **`ConfigReader::read_global()` change breaks behavior for missing config file**
   Mitigation: The fix in Task 1.1 must distinguish between "file does not exist" (skip silently, as today) and "file exists but is malformed" (return error). Use `path.exists()` check before deciding whether to add the source as required or not.

5. **`PeachInfra::new` becoming `Result`-returning cascades through `PeachAPI::init` and the closure**
   Mitigation: Pre-validate `services_url` in `main.rs` before the closure (Task 3.5, recommended approach). This keeps the factory closure return type as `A` (not `Result<A>`), preserving the `UI<A, F>` generic constraint without requiring a `UI` redesign.

6. **`AppConfigService::get_default_provider` and similar read-config methods need config access**
   Mitigation: Implement the `update_environment` → returns `PeachConfig` approach (Task 5.6). Alternatively, store `PeachConfig` at service construction time and update it in-place after each `update_environment` call returns. The former is cleaner.

---

## Alternative Approaches

1. **Keep `get_config()` on `EnvironmentInfra` but add startup validation**: Instead of full removal, keep the lazy read but call `PeachConfig::read()?` in `main.rs` purely for validation (discard the result), then let the infra re-read it lazily. Simpler change, but duplicates disk reads and doesn't achieve the "config piped through everywhere" goal. Does not eliminate the dead error path in `read_from_disk()`.

2. **Wrap `PeachConfig` in `Arc<RwLock<PeachConfig>>`**: Rather than threading config through constructors, store a shared `Arc<RwLock<PeachConfig>>` that all consumers read from. `update_environment` acquires the write lock and updates in place. This is a valid reactive pattern but adds lock complexity and requires write-lock acquisition on every config read, even in hot paths like `tool_executor`.

3. **Narrow the `EnvironmentInfra` trait instead of removing `get_config()`**: Split into `EnvironmentInfra` (env vars, environment) and a separate `ConfigInfra` trait (just `get_config()` and `update_environment()`). Services that need only config use `ConfigInfra`; services that need only env use `EnvironmentInfra`. This reduces the trait surface each service depends on and is a valid design improvement, but is a larger refactor scope than strictly required.
