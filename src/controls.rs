//! Native-only project controls: quantity-backed BOQ, dated physical progress and posted costs.
//! Do not project these records into MPP/XML without an explicit, tested extension format.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::model::Project;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BoqLine {
    pub id: u64,
    pub task_uid: u32,
    pub code: String,
    pub description: String,
    pub quantity: Option<f64>,
    pub unit: String,
    pub contract_amount: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProgressEntry {
    pub id: u64,
    pub task_uid: u32,
    /// Date this cumulative physical progress was actually measured (not created retrospectively).
    pub as_of: NaiveDate,
    /// Cumulative measured quantity, where a BOQ measurement exists.
    pub cumulative_quantity: Option<f64>,
    /// Physical percentage from the observation, not the task's current schedule percentage.
    pub percent_complete: f64,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActualCostEntry {
    pub id: u64,
    pub task_uid: u32,
    pub incurred_on: NaiveDate,
    pub amount: f64,
    pub category: String,
    pub reference: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectControls {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub boq: Vec<BoqLine>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub progress: Vec<ProgressEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actual_costs: Vec<ActualCostEntry>,
    #[serde(default)]
    pub next_id: u64,
}

impl ProjectControls {
    pub fn is_empty(&self) -> bool {
        self.boq.is_empty() && self.progress.is_empty() && self.actual_costs.is_empty()
    }

    fn allocate_id(&mut self) -> u64 {
        let highest = self.boq.iter().map(|e| e.id)
            .chain(self.progress.iter().map(|e| e.id))
            .chain(self.actual_costs.iter().map(|e| e.id))
            .max()
            .unwrap_or(0);
        let id = self.next_id.max(highest.saturating_add(1)).max(1);
        self.next_id = id.saturating_add(1);
        id
    }

    /// Last recorded observation at or before an explicit cut-off. Never use the latest
    /// task % Complete to invent historical progress for dates lacking a record.
    pub fn progress_as_of(&self, task_uid: u32, cutoff: NaiveDate) -> Option<&ProgressEntry> {
        self.progress.iter()
            .filter(|entry| entry.task_uid == task_uid && entry.as_of <= cutoff)
            .max_by_key(|entry| (entry.as_of, entry.id))
    }

    pub fn posted_cost_as_of(&self, cutoff: NaiveDate) -> f64 {
        self.actual_costs.iter().filter(|entry| entry.incurred_on <= cutoff)
            .map(|entry| entry.amount).sum()
    }
}

pub fn add_boq_line(p: &mut Project, task_uid: u32, code: String,
                    description: String, quantity: Option<f64>, unit: String,
                    contract_amount: f64) -> Result<BoqLine, String> {
    if !p.tasks.iter().any(|t| t.uid == task_uid) {
        return Err(format!("Unknown BOQ task UID {task_uid}"));
    }
    if !contract_amount.is_finite() || contract_amount < 0.0 ||
       quantity.is_some_and(|q| !q.is_finite() || q < 0.0) {
        return Err("BOQ quantity and contract amount must be finite and nonnegative".into());
    }
    let line = BoqLine {
        id: p.controls.allocate_id(), task_uid, code, description,
        quantity, unit, contract_amount,
    };
    p.controls.boq.push(line.clone());
    Ok(line)
}

/// Record a physical survey. A correction is appended as a new entry with a note rather
/// than silently replacing the old entry. The scheduled % Complete remains independent.
pub fn record_progress(p: &mut Project, task_uid: u32, as_of: NaiveDate,
                       percent: Option<f64>, cumulative_quantity: Option<f64>,
                       note: String) -> Result<ProgressEntry, String> {
    let Some(task_index) = p.tasks.iter().position(|t| t.uid == task_uid) else {
        return Err(format!("Unknown task UID {task_uid}"));
    };
    if p.tasks[task_index].is_summary || p.tasks[task_index].external.is_some() {
        return Err("Only a local leaf task can receive a physical-progress entry".into());
    }
    if cumulative_quantity.is_some_and(|q| !q.is_finite() || q < 0.0) {
        return Err("Cumulative quantity must be finite and nonnegative".into());
    }
    let linked: Vec<&BoqLine> = p.controls.boq.iter().filter(|x| x.task_uid == task_uid).collect();
    let derived = if let Some(q) = cumulative_quantity {
        if linked.len() == 1 {
            let planned = linked[0].quantity.ok_or("Linked BOQ item has no planned quantity")?;
            if planned <= 0.0 {
                return Err("Linked BOQ quantity must be positive".into());
            }
            if q > planned + planned.abs() * 1e-9 {
                return Err("Measured quantity exceeds the approved BOQ quantity; create a variation before recording it".into());
            }
            Some(q / planned * 100.0)
        } else {
            None
        }
    } else {
        None
    };
    let pct = percent.or(derived).ok_or("Give percent_complete, or a measured quantity for a task with exactly one BOQ item")?;
    if !pct.is_finite() || !(0.0..=100.0).contains(&pct) {
        return Err("Physical progress must be from 0 to 100 percent".into());
    }
    if let (Some(given), Some(from_quantity)) = (percent, derived) {
        if (given - from_quantity).abs() > 0.05 {
            return Err("Physical percent and measured BOQ quantity disagree by more than 0.05 percentage points".into());
        }
    }
    let previous = p.controls.progress.iter()
        .filter(|entry| entry.task_uid == task_uid)
        .max_by_key(|entry| (entry.as_of, entry.id));
    if previous.is_some_and(|old| as_of >= old.as_of && pct + 1e-6 < old.percent_complete) && note.trim().is_empty() {
        return Err("A progress reduction is a correction: add a note explaining it".into());
    }
    let should_update = previous.is_none_or(|old| as_of >= old.as_of);
    let entry = ProgressEntry {
        id: p.controls.allocate_id(), task_uid, as_of,
        cumulative_quantity, percent_complete: pct, note,
    };
    p.controls.progress.push(entry.clone());
    if should_update {
        p.tasks[task_index].physical_percent = pct.round() as u8;
    }
    Ok(entry)
}

/// Posted cost entries never change the engine's modeled task cost. EVM can use the
/// independently posted ledger, while resource-rate estimates remain separately labeled.
pub fn record_actual_cost(p: &mut Project, task_uid: u32, incurred_on: NaiveDate,
                          amount: f64, category: String, reference: String) -> Result<ActualCostEntry, String> {
    if !p.tasks.iter().any(|t| t.uid == task_uid && t.external.is_none()) {
        return Err(format!("Unknown or external task UID {task_uid}"));
    }
    if !amount.is_finite() || amount < 0.0 {
        return Err("Posted actual cost must be finite and nonnegative; credits require a separate approved adjustment".into());
    }
    if category.trim().is_empty() {
        return Err("Actual cost category is required".into());
    }
    let entry = ActualCostEntry {
        id: p.controls.allocate_id(), task_uid, incurred_on,
        amount, category, reference,
    };
    p.controls.actual_costs.push(entry.clone());
    Ok(entry)
}

/// Sum earned baseline cost using ONLY dated physical measurements.
pub fn earned_value_as_of(p: &Project, cutoff: NaiveDate) -> f64 {
    p.tasks.iter().filter(|t| !t.is_summary && t.active && t.external.is_none())
        .map(|t| {
            let base_cost = t.baseline(0).and_then(|b| b.cost).unwrap_or(0.0).max(0.0);
            let recorded = p.controls.progress_as_of(t.uid, cutoff)
                .map(|x| x.percent_complete / 100.0).unwrap_or(0.0);
            base_cost * recorded
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Task;

    fn project() -> Project {
        let mut p = Project::new();
        p.tasks.clear();
        p.tasks.push(Task::new(1, "Foundation", p.start));
        p
    }

    #[test]
    fn old_projects_deserialize_with_empty_controls() {
        let p = project();
        let json = serde_json::to_value(&p).unwrap();
        assert!(json.get("controls").is_none());
        let reopened: Project = serde_json::from_value(json).unwrap();
        assert!(reopened.controls.is_empty());
    }

    #[test]
    fn boq_and_physical_observations_survive_native_json() {
        let mut p = project();
        let start = p.start;
        add_boq_line(&mut p, 1, "1.1".into(), "Concrete".into(),
                     Some(400.0), "m3".into(), 800000.0).unwrap();
        let observed = record_progress(&mut p, 1, start, None, Some(125.0), "".into()).unwrap();
        assert!((observed.percent_complete - 31.25).abs() < 1e-9);
        assert_eq!(p.tasks[0].physical_percent, 31);
        let reopened: Project = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(reopened.controls.progress_as_of(1, start), Some(&observed));
        assert_eq!(reopened.controls.boq.len(), 1);
    }

    #[test]
    fn historic_cutoff_does_not_retroactively_use_current_progress() {
        let mut p = project();
        let start = p.start;
        record_progress(&mut p, 1, start, Some(10.0), None, "".into()).unwrap();
        record_progress(&mut p, 1, start + chrono::Duration::days(7), Some(65.0), None, "".into()).unwrap();
        assert_eq!(p.controls.progress_as_of(1, start).unwrap().percent_complete, 10.0);
        assert!(p.controls.progress_as_of(1, start - chrono::Duration::days(1)).is_none());
        assert_eq!(p.tasks[0].physical_percent, 65);
        // A correction becomes a new append-only record with its own id.
        assert!(record_progress(&mut p, 1, start + chrono::Duration::days(8), Some(60.0), None, "".into()).is_err());
        record_progress(&mut p, 1, start + chrono::Duration::days(8), Some(60.0), None, "Re-measured".into()).unwrap();
        assert_eq!(p.controls.progress.len(), 3);
    }

    #[test]
    fn xml_export_fails_before_writing_when_native_controls_exist() {
        let mut p = project();
        let start = p.start;
        record_progress(&mut p, 1, start, Some(40.0), None, "".into()).unwrap();
        let error = crate::fileio::save(std::path::Path::new("should-not-be-created.xml"), &p).unwrap_err();
        assert!(error.contains(".neramit"), "{error}");
    }

    #[test]
    fn actual_cost_ledger_is_time_phased_and_independent() {
        let mut p = project();
        let start = p.start;
        record_actual_cost(&mut p, 1, start, 2200.0, "labor".into(), "INV-1".into()).unwrap();
        record_actual_cost(&mut p, 1, start + chrono::Duration::days(1), 300.0, "material".into(), "INV-2".into()).unwrap();
        assert_eq!(p.controls.posted_cost_as_of(start), 2200.0);
        assert_eq!(p.controls.posted_cost_as_of(start + chrono::Duration::days(1)), 2500.0);
        assert_eq!(p.tasks[0].actual_cost, 0.0);
    }
}
