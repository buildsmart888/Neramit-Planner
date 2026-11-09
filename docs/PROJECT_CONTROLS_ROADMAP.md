# Project Controls foundation: incremental delivery

The existing 1.0.1 desktop and MCP workflows remain the baseline. Do not replace them with a new application.

## Phase 1: verified correctness

- [x] Add a transactional `schedule::level_checked` API returning `LevelingReport`.
- [x] Propagate schedule errors and unresolved resource conflicts through the UI and MCP.
- [x] Avoid drawing fabricated historical EV/AC values; clearly label S-Curve estimates.
- [x] Add regression tests for leveling conflicts and rollback.
- [x] Add Windows GitHub Actions build/test workflow.
- [ ] Run Windows CI and fix failures. **Not verified by code review alone.**
- [ ] Validate MPP12/MPP14 with independent actual Microsoft Project fixtures.
- [ ] Benchmark 1k/5k/10k task projects using `--qa-script`.
- [ ] Validate installer/portable signature and update manifests end-to-end.

## Phase 2: physical progress and period history

Planned stable data entities:
- `WorkPackage { id, wbs, title }`
- `BoqItem { id, code, description, quantity, unit, material_amount, labor_amount, linked_task_uid }`
- `ProgressEntry { id, task_uid, recorded_at, period_end, quantity_this_period, quantity_cumulative, percent_complete, source, evidence, approved_by }`
- `ActualCostEntry { id, task_uid, incurred_on, amount, category, source_reference }`
- `ScheduleSnapshot { id, status_date, baseline_revision, task_results }`

Design invariants:
1. **Snapshot histories must never be inferred from the latest % Complete.** Old periods contain only dated records that actually exist.
2. Model quantity and unit explicitly; do not depend on free-form task Notes.
3. Never silently overwrite actual quantities; corrections create an audit record.
4. Use task stable UID and separate BOQ ID. Restructuring the outline must not lose mappings.
5. Progress cannot exceed authorized contract quantity unless it is flagged as a variation.
6. EVM historical PV, EV and AC must be calculated at each cut-off from baseline, accepted physical progress, and posted actual costs.
7. Values derived from a heuristic must be labeled **estimate**.
8. Keep `.neramit` backwards compatible by using versioned serde defaults and migration tests.
9. **MPP/XML round-trip limitation:** Do not treat new native-only entities as persisted to .mpp/.xml unless adapters and round-trip tests explicitly prove it. Warn or block lossy export.
10. MCP changes require dry-run, rollback, stable-id validation and explicit save to disk.

## Phase 3: BOQ and cost controls

- Import mappings and reconciliation report: source row, original quantities, parsed quantities, computed totals.
- Approved variation orders and change histories.
- Separate planned budget, committed cost, posted actual costs, and billing/receipts.
- Payment certificates, partial payments, tax, advances and retention release.

## Release gate

- All existing Rust tests pass on Windows.
- New and existing schedules are semantically identical unless a specific fix is documented.
- MPP file compatibility verified using independently created real fixtures; do not rely only on generated test files.
- No crash on invalid/corrupt .mpp or .xlsx.
- Manual UI smoke tests in Thai and English, portable and installer modes.
- No release or merge while any P0 defect remains open.

## Known limitations of this branch

This branch establishes safer leveling and more truthful reports; it does **not** yet implement progress-history storage, independent cost ledger, full quantity controls or 4D BIM. The workflow only confirms tests when it actually runs; the presence of a YAML file is not proof of a successful build.
