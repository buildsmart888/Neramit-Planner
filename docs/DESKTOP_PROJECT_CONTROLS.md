# Desktop Project Controls – Site progress & actual costs

**Scope:** incrementally adds an editable report page to the existing Neramit Planner 1.0.1 desktop shell. This branch is stacked on [Phase 02 PR #2](https://github.com/buildsmart888/Neramit-Planner/pull/2), which in turn depends on [PR #1](https://github.com/buildsmart888/Neramit-Planner/pull/1).

## Where to open

- Ribbon **Project → Project Controls** (direct shortcut).
- **Report → In Progress → Project Controls**, or **Report → Costs → Project Controls**.

## Physical Progress tab

1. Select a local leaf activity. The row number may change when tasks move; the editor saves its permanent **task UID**.
2. Enter the actual survey date `YYYY-MM-DD`.
3. Enter physical percentage **or** cumulative quantity when exactly one BOQ item with a valid positive planned quantity is linked to that activity. If both are supplied, they must agree within 0.05 percentage points.
4. Add supporting evidence/reference text; a downward progress correction requires a note.
5. Click **Record site progress**. The new item is appended, visible in the dated history table. It changes only **Physical %** and does not overwrite scheduled **% Complete**.
6. Use **Undo** if the last change needs to be reversed.

The page also shows BOQ code, description, planned quantity, unit and contract amount for the selected task. All history is available via MCP `list_progress`; the UI lists at most the 100 latest rows.

## Actual Cost tab

1. Select activity, incurred date, amount, category and optional invoice/receipt reference.
2. Click **Post actual cost** to append the dated ledger item. It does not overwrite resource-based cost estimates.
3. Review posted amount as of the project status date and recent ledger rows.

The UI reports failures without mutating the project or creating an Undo step.

## Protection and compatibility

- File format: **Save As → Neramit Planner (.neramit)**. Imported structured BOQ, actual-cost and progress-history records are native-only.
- MPP/XML save is blocked while native controls are nonempty, so they cannot be lost silently. CSV/XLSX/PDF report exports are not native backups.
- **Deleting any task (including a summary and descendants) associated with these records is blocked** in desktop and MCP. Until a versioned audit/archive workflow exists, keep those tasks in the model or restore a native backup.
- Existing old `.neramit` files still deserialize using empty defaults; new files use the native version 2 wrapper.

## Quality gates

Automated tests were authored for parsing input dates/numbers, BOQ and progress round trips, cost ledger cutoffs, undo/dry-run via MCP, deletion guard, and native file save/read. They require `cargo test --locked` on Windows. **Do not treat them as passed merely because the code is checked in.**

The Windows GitHub-hosted Actions runs for earlier PRs have failed before steps started; see `docs/CI_RELEASE_BLOCKER.md`. The cause must be diagnosed from GitHub Actions UI job annotations or by a local Windows Rust build before merging any stacked PR.

## Planned follow-ons

- BOQ reimport/reconciliation with explicit, audited change orders.
- Unit-price and material/labor breakdown, evidence attachments, approval roles and corrections.
- Native backup/recovery for orphaned controls.
- Certified EVM snapshots and verification against independent data.
