//! Desktop project controls editor: site progress, posted costs, and linked BOQ.
//! All edits stage a Project clone before checkpointing, preventing partial mutations.

use crate::{app::App, controls, theme};
use crate::i18n::tr;
use chrono::{Local, NaiveDate};
use eframe::egui::{self, RichText, Ui};
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Serialize, Deserialize)]
struct Form {
    mode: u8,
    task_uid: u32,
    date: String,
    percent: String,
    cumulative: String,
    note: String,
    amount: String,
    category: String,
    reference: String,
    message: String,
    error: bool,
}

fn date_input(text: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(text.trim(), "%Y-%m-%d")
        .map_err(|_| "Date must be YYYY-MM-DD, e.g. 2026-11-10".into())
}

fn number_input(text: &str, label: &str) -> Result<Option<f64>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let n: f64 = text.parse().map_err(|_| format!("{label}: enter a number without commas"))?;
    if !n.is_finite() || n < 0.0 {
        return Err(format!("{label} must be a finite nonnegative number"));
    }
    Ok(Some(n))
}

fn text_field(ui: &mut Ui, value: &mut String, hint: &str, width: f32) {
    ui.add(egui::TextEdit::singleline(value).desired_width(width).hint_text(hint));
}

fn intro(ui: &mut Ui, title: &str, description: &str) {
    ui.label(RichText::new(tr(title)).size(17.0).strong().color(theme::GREEN));
    ui.label(RichText::new(tr(description)).color(theme::TEXT_DIM));
    ui.add_space(5.0);
}

fn commit(app: &mut App, staged: crate::model::Project, status: String) {
    app.checkpoint();
    app.doc = staged;
    app.status = status;
}

/// View > Project Controls: native .neramit-only site measurement & cost entry.
pub fn controls_page(app: &mut App, ui: &mut Ui) {
    let key = app.path.as_ref().map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| app.doc.title.clone());
    let id = ui.make_persistent_id(("project-controls-editor", key));
    let mut form: Form = ui.data_mut(|d| d.get_persisted_mut_or(id, Form::default()).clone());
    if form.date.is_empty() {
        form.date = app.status_instant().date().to_string();
    }
    if form.category.is_empty() {
        form.category = "material".into();
    }

    ui.horizontal(|ui| {
        ui.selectable_value(&mut form.mode, 0, tr("Physical Progress"));
        ui.selectable_value(&mut form.mode, 1, tr("Actual Cost"));
    });
    ui.separator();

    // Native task UIDs are stable even if tasks are reordered.
    let tasks: Vec<(u32, String)> = app.doc.tasks.iter().enumerate()
        .filter(|(_, t)| !t.is_summary && t.external.is_none())
        .map(|(i, t)| (t.uid, format!("{}  {}", i + 1, t.name))).collect();
    if tasks.is_empty() {
        ui.label(tr("No local leaf tasks. Create tasks or import a BOQ first."));
        ui.data_mut(|d| d.insert_persisted(id, form));
        return;
    }
    if !tasks.iter().any(|(uid, _)| *uid == form.task_uid) {
        form.task_uid = app.current_task().and_then(|i| app.doc.tasks.get(i))
            .map(|t| t.uid).filter(|uid| tasks.iter().any(|(i, _)| i == uid))
            .unwrap_or(tasks[0].0);
    }
    let selected_name = tasks.iter().find(|(uid, _)| *uid == form.task_uid)
        .map(|(_, name)| name.clone()).unwrap_or_default();

    if form.mode == 0 {
        intro(ui, "Record physical progress",
            "Dated site measurements. Scheduled % Complete remains independent.");
    } else {
        intro(ui, "Post actual costs",
            "Records paid or incurred amounts, not automatically estimated resource costs.");
    }
    egui::Grid::new("project-controls-form").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
        ui.label(tr("Task"));
        egui::ComboBox::from_id_salt("project-controls-task").width(380.0)
            .selected_text(crate::thai::shape(&selected_name))
            .show_ui(ui, |ui| {
                for (uid, name) in &tasks {
                    ui.selectable_value(&mut form.task_uid, *uid, crate::thai::shape(name));
                }
            });
        ui.end_row();

        ui.label(tr(if form.mode == 0 { "Survey date" } else { "Incurred date" }));
        text_field(ui, &mut form.date, "YYYY-MM-DD", 150.0);
        ui.end_row();

        if form.mode == 0 {
            ui.label(tr("Physical progress (%)"));
            text_field(ui, &mut form.percent, "e.g. 31.25 (optional)", 215.0);
            ui.end_row();

            ui.label(tr("Cumulative BOQ quantity"));
            text_field(ui, &mut form.cumulative, "e.g. 125 (optional)", 215.0);
            ui.end_row();

            ui.label(tr("Evidence / correction reason"));
            text_field(ui, &mut form.note, "Survey reference or reason", 380.0);
            ui.end_row();
        } else {
            ui.label(tr("Amount"));
            text_field(ui, &mut form.amount, "e.g. 12000.00", 215.0);
            ui.end_row();

            ui.label(tr("Category"));
            egui::ComboBox::from_id_salt("project-cost-category")
                .selected_text(&form.category).show_ui(ui, |ui| {
                    for category in ["material", "labor", "equipment", "subcontract", "other"] {
                        ui.selectable_value(&mut form.category, category.to_string(), category);
                    }
                });
            ui.end_row();

            ui.label(tr("Invoice / receipt"));
            text_field(ui, &mut form.reference, "INV-001", 380.0);
            ui.end_row();
        }
    });

    let save_label = if form.mode == 0 { "Record site progress" } else { "Post actual cost" };
    if ui.button(tr(save_label)).clicked() {
        // The Project clone is changed only if validation fully succeeds.
        let result = (|| -> Result<(crate::model::Project, String), String> {
            let date = date_input(&form.date)?;
            let mut staged = app.doc.clone();
            let message = if form.mode == 0 {
                let pct = number_input(&form.percent, "Physical %")?;
                let qty = number_input(&form.cumulative, "Cumulative quantity")?;
                let e = controls::record_progress(
                    &mut staged, form.task_uid, date, pct, qty, form.note.clone())?;
                format!("Site survey saved: UID {} / {} / {:.2}%", e.task_uid, e.as_of, e.percent_complete)
            } else {
                let amount = number_input(&form.amount, "Actual cost")?.ok_or("Amount is required")?;
                let e = controls::record_actual_cost(
                    &mut staged, form.task_uid, date, amount,
                    form.category.clone(), form.reference.clone())?;
                format!("Actual cost posted: UID {} / {} / {:.2}", e.task_uid, e.incurred_on, e.amount)
            };
            Ok((staged, message))
        })();
        match result {
            Ok((staged, message)) => {
                commit(app, staged, message.clone());
                form.message = message;
                form.error = false;
                form.percent.clear();
                form.cumulative.clear();
                form.note.clear();
                form.amount.clear();
                form.reference.clear();
            }
            Err(error) => {
                form.message = error;
                form.error = true;
            }
        }
    }
    if !form.message.is_empty() {
        ui.colored_label(if form.error { egui::Color32::DARK_RED } else { theme::GREEN },
                         crate::thai::shape(&form.message));
    }
    ui.label(RichText::new(tr("Native progress and cost records are saved to .neramit only. MPP/XML output is blocked to prevent losing these fields."))
        .color(theme::TEXT_DIM));

    ui.add_space(12.0);
    if form.mode == 0 {
        ui.label(RichText::new(tr("BOQ linked to selected activity")).strong().size(16.0));
        let lines: Vec<_> = app.doc.controls.boq.iter().filter(|x| x.task_uid == form.task_uid).collect();
        if lines.is_empty() {
            ui.label(tr("No BOQ for this activity. Use physical percentage instead."));
        } else {
            egui::Grid::new("project-control-boq").striped(true).spacing([14.0, 5.0]).show(ui, |ui| {
                for label in ["Code", "Description", "Planned quantity", "Unit", "Amount"] {
                    ui.strong(tr(label));
                }
                ui.end_row();
                for line in lines {
                    ui.label(&line.code);
                    ui.label(crate::thai::shape(&line.description));
                    ui.label(line.quantity.map(|v| format!("{v:.3}")).unwrap_or_else(|| "-".into()));
                    ui.label(crate::thai::shape(&line.unit));
                    ui.label(format!("{:.2}", line.contract_amount));
                    ui.end_row();
                }
            });
        }
        ui.add_space(10.0);
        ui.label(RichText::new(tr("Physical progress history")).strong().size(16.0));
        let mut entries: Vec<_> = app.doc.controls.progress.iter()
            .filter(|e| e.task_uid == form.task_uid).collect();
        entries.sort_by_key(|e| std::cmp::Reverse((e.as_of, e.id)));
        if entries.is_empty() {
            ui.label(tr("No site surveys recorded yet."));
        } else {
            egui::Grid::new("project-control-progress").striped(true).spacing([14.0, 5.0]).show(ui, |ui| {
                for label in ["Date", "Physical %", "Measured qty", "Note", "Record ID"] {
                    ui.strong(tr(label));
                }
                ui.end_row();
                for e in entries.iter().take(100) {
                    ui.label(e.as_of.to_string());
                    ui.label(format!("{:.2}%", e.percent_complete));
                    ui.label(e.cumulative_quantity.map(|v| format!("{v:.3}")).unwrap_or_else(|| "-".into()));
                    ui.label(crate::thai::shape(&e.note));
                    ui.label(e.id.to_string());
                    ui.end_row();
                }
            });
            if entries.len() > 100 {
                ui.label("Showing the latest 100 records. Use MCP list_progress for all records.");
            }
        }
    } else {
        ui.label(RichText::new(tr("Posted cost ledger")).strong().size(16.0));
        let recorded = app.doc.controls.posted_cost_as_of(app.status_instant().date());
        ui.label(format!("Posted cost through status date: {:.2}", recorded));
        let mut entries: Vec<_> = app.doc.controls.actual_costs.iter().collect();
        entries.sort_by_key(|e| std::cmp::Reverse((e.incurred_on, e.id)));
        if entries.is_empty() {
            ui.label(tr("No posted actual costs yet."));
        } else {
            egui::Grid::new("project-control-costs").striped(true).spacing([14.0, 5.0]).show(ui, |ui| {
                for label in ["Date", "Task UID", "Amount", "Category", "Reference", "Record ID"] {
                    ui.strong(tr(label));
                }
                ui.end_row();
                for e in entries.iter().take(100) {
                    ui.label(e.incurred_on.to_string());
                    ui.label(e.task_uid.to_string());
                    ui.label(format!("{:.2}", e.amount));
                    ui.label(&e.category);
                    ui.label(crate::thai::shape(&e.reference));
                    ui.label(e.id.to_string());
                    ui.end_row();
                }
            });
            if entries.len() > 100 {
                ui.label("Showing the latest 100 records. Use MCP list_actual_costs for all records.");
            }
        }
    }
    ui.data_mut(|d| d.insert_persisted(id, form));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn site_form_validates_dates_and_numbers() {
        assert!(date_input("2026-11-10").is_ok());
        assert!(date_input("10/11/2026").is_err());
        assert_eq!(number_input("", "qty").unwrap(), None);
        assert_eq!(number_input("125.25", "qty").unwrap(), Some(125.25));
        assert!(number_input("-0.5", "qty").is_err());
        assert!(number_input("NaN", "qty").is_err());
        assert!(number_input("1,250", "qty").is_err());
    }
}
