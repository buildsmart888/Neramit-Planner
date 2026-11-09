# Dated project controls (Phase 02: native-file foundation)

This branch introduces **structured BOQ items**, **dated site physical-progress measurements**, and a **posted actual-cost ledger**. It does not claim that all Project Controls features are complete or that the branch passes Windows CI.

## Basic workflow through the existing local MCP server

1. Use `open_project` to open a project, then `list_tasks` to obtain stable task UIDs.
2. Import a BOQ with `import_boq`; each imported leaf activity retains a native BOQ record with its quantity, unit, amount and task UID. Inspect with `list_boq`.
3. Record physical progress using `record_progress`:
   - `{"uid":23,"as_of":"2026-11-09","cumulative_quantity":125}` computes 31.25% when that task has **exactly one** BOQ line of quantity 400.
   - `{"uid":23,"as_of":"2026-11-09","percent_complete":31.25,"note":"Site survey"}` works with no BOQ.
   - `dry_run:true` previews the result without keeping an entry. Corrections that reduce the physical percentage must include a note.
4. Use `list_progress` with optional `through` cutoff to inspect history. Every entry carries a unique id, task UID and actual observation date. Existing entries are never rewritten by the MCP record command.
5. Post actual costs separately using `record_actual_cost`, for example `{"uid":23,"incurred_on":"2026-11-09","amount":12000,"category":"labor","reference":"INV-001"}`; inspect with `list_actual_costs`.
6. **Save a native `.neramit` file.** The new data records are not represented in standard MPP/XML. Lossy MPP/XML saves are blocked while any native control records exist.

## Data semantics

- `Task.percent_complete`: existing schedule-duration progress. `record_progress` does **not** change it.
- `Task.physical_percent`: latest physical observation rounded to an integer. Dates and exact percentages remain in ProgressEntry records.
- `Task.cost` / `Task.actual_cost`: existing scheduling engine's cost estimates.
- `ActualCostEntry`: posted actual cost, independent from the scheduling engine cost estimate.
- `BoqLine.contract_amount`: fixed imported contract amount, not a cost-forecast ledger.
- S-Curve with recorded progress: uses physical observations at each period end (step function); unrecorded activities contribute zero. The old legacy reconstructed estimate remains available for old projects **without** observations.
- EVM with observations: EV uses baseline value × recorded physical completion as of the cutoff; AC uses posted actual costs when available. Without a cost ledger, AC/CPI/CV/EAC are unavailable, not silently substituted. Tasks with incomplete site measurement coverage can understate EV.
- Existing `.neramit` files without a controls object still open with empty defaults. New native files store version 2.

## Not yet implemented

- Desktop forms to input progress, costs and BOQ changes (currently through MCP).
- Import synchronization/reconciliation on re-import, BOQ unit-rate breakdown and variations.
- Approval workflow, authenticated site measurements, cost credits, retention release.
- Time-phased evidence/document attachments, audit trail with user identity.
- Full migration/export to XML/MPP (intentionally blocked to avoid data loss), IFC/4D integration.

## Verification before shipping

- Run `cargo test --locked` and `cargo build --locked` on Windows.
- Test ordinary and corrupt MPP12/MPP14 fixtures and compare outputs in actual Microsoft Project.
- Run `--qa-script` on 1k/5k/10k task projects and check memory/performance.
- Verify native save → reload → dates/progress/BOQ/cost equality and that XML/MPP protection works.
- Re-check Turkish/Thai locale, reports and installer on Windows.

Do **not merge or release** merely because the source is present. CI and independent benchmarks must pass first.
