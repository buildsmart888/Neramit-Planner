# CI release blocker – 2026-11-09

**Do not merge or release PR #1 / PR #2 until a real Rust build and test run succeeds.**

## Observations

- PR #1: https://github.com/buildsmart888/Neramit-Planner/actions/runs/42647589970
- PR #2: https://github.com/buildsmart888/Neramit-Planner/actions/runs/42651001717
- Both workflows finished with conclusion `failure`.
- Windows Rust test suite jobs returned an empty list of executed steps (`steps=[]`).
- Both check runs report a single annotation, but the available connector cannot fetch the annotation content; logs returned `BlobNotFound`.
- **We cannot tell whether this is a hosted runner/setup problem or a code failure.** Do not mark `cargo test` or `cargo build` as either passed or failed from these check outcomes alone.

## To unblock

1. Open each Actions run in the GitHub browser and read the job-level diagnostic/annotation.
2. Check repository Actions policy, hosted runner availability and workflow permissions.
3. Re-run the Windows job once the cause is resolved.
4. If hosted Actions cannot run, build with a local Windows Rust toolchain:
   ```powershell
   cargo test --locked
   cargo build --locked
   ```
5. Record exact compiler/test output, then fix code issues if any.
6. Perform MPP12/MPP14 independent fixtures + native .neramit round-trip and UI smoke tests before merge/release.

The GitHub Issues feature is disabled for this repository (HTTP 410), so this blocker is tracked in this file and in the draft PR descriptions.
