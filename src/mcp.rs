//! `NeramitPlanner --mcp`: a Model Context Protocol server on stdin / stdout (one JSON-RPC message
//! per line), so AI assistants such as Claude Code or Codex can open, read, change and save
//! projects. Changes go through the program's own code (cell parsing, link checks, scheduling,
//! Undo) on a project held in memory: nothing is written until `save` or `export`, and the .mpp a
//! project was opened from is never overwritten.

use crate::app::{App, FilterSpec, TCol, TaskFilter};
use crate::model::{Holiday, Resource, ResourceType, Task};
use chrono::{NaiveDate, NaiveDateTime};
use eframe::egui;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Protocol versions this server speaks, newest first.
const PROTOCOLS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

/// Rows returned by `list_tasks` when no limit is given.
const DEFAULT_LIMIT: usize = 200;

/// Tasks listed by name in a change report (the count covers the rest).
const DIFF_ROWS: usize = 30;

const INSTRUCTIONS: &str = "Neramit Planner schedules (Gantt). Open a file with open_project (or start one with new_project) and pass the returned `doc` to the other tools (it may be left out while only one project is open). \
Tasks are referred to by `id` (the row number shown in the program, which changes when rows are added or deleted) or `uid` (the unique ID, which never changes). \
Changes stay in memory until `save`; every change returns what moved (tasks whose dates changed and the project finish), and `dry_run: true` shows that without keeping the change. \
Durations are typed as in the program: 5, 5d, 0.5d, 4h, 2w, 3ed (elapsed days), \"5 วัน\"; a duration of 0 makes a milestone. Dates are YYYY-MM-DD. \
Predecessors are typed as in the program's Predecessors column: \"3\", \"3FS+2d\", \"4SS, 7FF-1d\". \
Typing a start or finish on an auto-scheduled task sets a Start / Finish No Earlier Than constraint, as in the program: prefer links.";

struct Doc {
    app: App,
    /// (kept alive with the app)
    _ctx: egui::Context,
    /// modified time of the file when it was opened or last saved here
    disk_time: Option<SystemTime>,
}

pub struct Server {
    docs: Vec<(String, Doc)>,
    next: u32,
}

/// `NeramitPlanner --mcp`: serve until stdin closes.
pub fn run() -> i32 {
    // settings, recent files and favourites stay as the user left them
    crate::app::NO_PERSIST.store(true, std::sync::atomic::Ordering::Relaxed);
    crate::i18n::set_lang(crate::i18n::Lang::English);
    let mut server = Server { docs: Vec::new(), next: 1 };
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        if let Some(reply) = server.message(&line) {
            if writeln!(out, "{reply}").and_then(|_| out.flush()).is_err() {
                break;
            }
        }
    }
    0
}

fn rpc_error(id: Value, code: i64, msg: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": msg } })
}

impl Server {
    /// One JSON-RPC message in; the reply (None for a notification or a response).
    pub fn message(&mut self, line: &str) -> Option<Value> {
        let msg: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => return Some(rpc_error(Value::Null, -32700, &format!("Parse error: {e}"))),
        };
        let Some(method) = msg.get("method").and_then(Value::as_str) else {
            // a response to something we never asked, or a batch (not used by MCP any more)
            return msg.is_array().then(|| rpc_error(Value::Null, -32600, "Batches are not supported"));
        };
        let id = msg.get("id").cloned()?;
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        let result = match method {
            "initialize" => {
                let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or(PROTOCOLS[0]);
                let version = PROTOCOLS.iter().find(|v| **v == asked).copied().unwrap_or(PROTOCOLS[0]);
                Ok(json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": { "name": "neramit-planner", "title": crate::app::product(), "version": env!("CARGO_PKG_VERSION") },
                    "instructions": INSTRUCTIONS,
                }))
            }
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tools() })),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or("").to_string();
                let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
                Ok(self.call_tool(&name, &args))
            }
            _ => Err((-32601, format!("Method not found: {method}"))),
        };
        Some(match result {
            Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
            Err((code, m)) => rpc_error(id, code, &m),
        })
    }

    fn call_tool(&mut self, name: &str, args: &Value) -> Value {
        let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.tool(name, args)));
        let (value, is_error) = match run {
            Ok(Ok(v)) => (v, false),
            Ok(Err(e)) => (json!({ "error": e }), true),
            Err(_) => (json!({ "error": format!("{name} failed inside Neramit Planner. The project may be in an unexpected state: check it, or close it without saving and open it again.") }), true),
        };
        let text = serde_json::to_string_pretty(&value).unwrap_or_default();
        json!({ "content": [{ "type": "text", "text": text }], "structuredContent": value, "isError": is_error })
    }

    fn tool(&mut self, name: &str, a: &Value) -> Result<Value, String> {
        match name {
            "open_project" => self.open_project(a),
            "new_project" => self.new_project(a),
            "list_open_projects" => Ok(self.list_open()),
            "close_project" => self.close_project(a),
            "project_summary" => {
                let d = self.doc(a)?;
                Ok(summary(&d.app))
            }
            "list_tasks" => list_tasks(&self.doc(a)?.app, a),
            "get_task" => {
                let app = &self.doc(a)?.app;
                let i = task_index(app, a)?;
                Ok(task_detail(app, i))
            }
            "critical_path" => Ok(critical_path(&self.doc(a)?.app)),
            "list_resources" => Ok(list_resources(&self.doc(a)?.app)),
            "progress_report" => progress_report(&self.doc(a)?.app, a),
            "compare_projects" => compare_projects(&self.doc(a)?.app, a),
            "add_tasks" => self.change(a, add_tasks),
            "update_tasks" => self.change(a, update_tasks),
            "delete_tasks" => self.change(a, delete_tasks),
            "link_tasks" => self.change(a, link_tasks),
            "unlink_tasks" => self.change(a, unlink_tasks),
            "add_resources" => self.change(a, add_resources),
            "update_project" => self.change(a, update_project),
            "set_baseline" => self.change(a, |app, a| {
                let n = opt_u64(a, "number")?.unwrap_or(0);
                if n > 10 {
                    return Err("number is 0 (Baseline) to 10 (Baseline 10)".into());
                }
                app.set_baseline(n as u8, false, false, false);
                Ok(json!({ "baseline": n }))
            }),
            "level_resources" => self.change(a, |app, _| {
                let report = crate::schedule::level_checked(&mut app.doc, None, None)?;
                crate::custom::apply_formulas(&mut app.doc);
                Ok(json!({
                    "delayed_tasks": report.delayed_tasks,
                    "unresolved_resources": report.unresolved_resources,
                    "fully_leveled": report.unresolved_resources == 0
                }))
            }),
            "import_boq" => self.change(a, import_boq),
            "list_boq" => Ok(json!({ "items": self.doc(a)?.app.doc.controls.boq })),
            "list_progress" => {
                let app = &self.doc(a)?.app;
                let target = if a.get("id").is_some() || a.get("uid").is_some() { Some(app.doc.tasks[task_index(app, a)?].uid) } else { None };
                let through = opt_date(a, "through")?;
                Ok(json!({ "entries": app.doc.controls.progress.iter()
                    .filter(|e| target.is_none_or(|uid| e.task_uid == uid) && through.is_none_or(|d| e.as_of <= d))
                    .collect::<Vec<_>>() }))
            }
            "record_progress" => self.change(a, |app, a| {
                let uid = app.doc.tasks[task_index(app, a)?].uid;
                let as_of = opt_date(a, "as_of")?.ok_or("as_of is required")?;
                let percent = opt_f64(a, "percent_complete")?;
                let quantity = opt_f64(a, "cumulative_quantity")?;
                let note = opt_str(a, "note")?.unwrap_or("").to_string();
                let entry = crate::controls::record_progress(&mut app.doc, uid, as_of, percent, quantity, note)?;
                Ok(json!({ "recorded": entry, "schedule_percent_unchanged": true }))
            }),
            "list_actual_costs" => Ok(json!({ "entries": self.doc(a)?.app.doc.controls.actual_costs })),
            "record_actual_cost" => self.change(a, |app, a| {
                let uid = app.doc.tasks[task_index(app, a)?].uid;
                let incurred_on = opt_date(a, "incurred_on")?.ok_or("incurred_on is required")?;
                let amount = opt_f64(a, "amount")?.ok_or("amount is required")?;
                let category = opt_str(a, "category")?.unwrap_or("").to_string();
                let reference = opt_str(a, "reference")?.unwrap_or("").to_string();
                let entry = crate::controls::record_actual_cost(&mut app.doc, uid, incurred_on, amount, category, reference)?;
                Ok(json!({ "recorded": entry, "estimated_task_cost_unchanged": true }))
            }),
            "payment_schedule" => Ok(payment_schedule(&self.doc(a)?.app)),
            "set_payment_schedule" => self.change(a, set_payment_schedule),
            "undo" => self.undo(a),
            "save" => self.save(a),
            "export" => self.export(a),
            _ => Err(format!("Unknown tool: {name}")),
        }
    }

    // ------------------------------------------------------------------ documents

    fn doc(&mut self, a: &Value) -> Result<&mut Doc, String> {
        let key = a.get("doc").and_then(Value::as_str);
        let k = match key {
            Some(k) => self.docs.iter().position(|(id, _)| id == k).ok_or_else(|| format!("No open project has doc \"{k}\". Open projects: {}", self.open_names()))?,
            None if self.docs.len() == 1 => 0,
            None if self.docs.is_empty() => return Err("No project is open. Use open_project or new_project first.".into()),
            None => return Err(format!("Several projects are open: say which one with `doc` ({})", self.open_names())),
        };
        self.docs[k].1.app.doc.apply_currency();
        Ok(&mut self.docs[k].1)
    }

    fn open_names(&self) -> String {
        self.docs.iter().map(|(id, d)| format!("{id} = {}", d.app.doc_name())).collect::<Vec<_>>().join(", ")
    }

    fn add_doc(&mut self, app: App, ctx: egui::Context, disk_time: Option<SystemTime>) -> String {
        let id = format!("d{}", self.next);
        self.next += 1;
        self.docs.push((id.clone(), Doc { app, _ctx: ctx, disk_time }));
        id
    }

    fn list_open(&self) -> Value {
        json!({ "projects": self.docs.iter().map(|(id, d)| json!({
            "doc": id,
            "name": d.app.doc_name(),
            "path": d.app.path.as_ref().map(|p| p.display().to_string()),
            "unsaved_changes": d.app.dirty,
            "tasks": d.app.doc.tasks.len(),
        })).collect::<Vec<_>>() })
    }

    fn open_project(&mut self, a: &Value) -> Result<Value, String> {
        let path = PathBuf::from(req_str(a, "path")?);
        if !path.is_file() {
            return Err(format!("{} does not exist", path.display()));
        }
        let canon = path.canonicalize().ok();
        if let Some((id, d)) = self.docs.iter().find(|(_, d)| d.app.path.as_ref().and_then(|p| p.canonicalize().ok()) == canon && canon.is_some()) {
            let mut v = summary(&d.app);
            v["doc"] = json!(id);
            v["note"] = json!("This file was already open: the project in memory is returned (with any unsaved changes).");
            return Ok(v);
        }
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(&ctx, None);
        app.open_path(path.clone());
        if let Some((_, e)) = app.message.take() {
            if app.path.is_none() {
                return Err(e);
            }
        }
        let disk_time = modified(&path);
        let mut v = summary(&app);
        let id = self.add_doc(app, ctx, disk_time);
        v["doc"] = json!(id);
        Ok(v)
    }

    fn new_project(&mut self, a: &Value) -> Result<Value, String> {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(&ctx, None);
        app.doc.localize_new();
        if let Some(t) = opt_str(a, "title")? {
            app.doc.title = t.to_string();
        }
        if let Some(s) = opt_date(a, "start")? {
            app.doc.start = s;
            app.doc.start_dt = None;
        }
        if let Some(p) = opt_str(a, "path")? {
            app.path = Some(PathBuf::from(p));
        }
        app.recalc();
        let mut v = summary(&app);
        let id = self.add_doc(app, ctx, None);
        v["doc"] = json!(id);
        Ok(v)
    }

    fn close_project(&mut self, a: &Value) -> Result<Value, String> {
        let discard = opt_bool(a, "discard_changes")?.unwrap_or(false);
        let d = self.doc(a)?;
        if d.app.dirty && !discard {
            return Err("The project has unsaved changes: save it first, or close with discard_changes: true.".into());
        }
        let name = d.app.doc_name();
        let key = a.get("doc").and_then(Value::as_str).map(str::to_string);
        self.docs.retain(|(id, _)| key.as_deref().map(|k| k != id).unwrap_or(false));
        Ok(json!({ "closed": name }))
    }

    /// A change: one Undo step, the schedule recalculated, and a report of what moved. With
    /// `dry_run` the project is put back as it was.
    fn change(&mut self, a: &Value, f: impl FnOnce(&mut App, &Value) -> Result<Value, String>) -> Result<Value, String> {
        let dry_run = opt_bool(a, "dry_run")?.unwrap_or(false);
        let app = &mut self.doc(a)?.app;
        let before = app.doc.clone();
        let dirty = app.dirty;
        // one Undo step for the whole call (the program keeps one per frame)
        app.frame += 1;
        app.checkpoint();
        app.message = None;
        let res = f(app, a);
        let res = res.and_then(|v| {
            app.recalc();
            match app.message.take() {
                Some((_, e)) => Err(e),
                None => Ok(v),
            }
        });
        match res {
            Ok(mut v) => {
                let diff = schedule_diff(&before, app);
                if dry_run {
                    app.doc = before;
                    app.dirty = dirty;
                    app.forget_checkpoint();
                    app.recalc();
                    v["dry_run"] = json!(true);
                    v["note"] = json!("Nothing was changed: this is what the change would do.");
                }
                v["changes"] = diff;
                if !dry_run {
                    v["unsaved_changes"] = json!(true);
                }
                Ok(v)
            }
            Err(e) => {
                app.doc = before;
                app.dirty = dirty;
                app.forget_checkpoint();
                app.recalc();
                Err(format!("{e} (nothing was changed)"))
            }
        }
    }

    fn undo(&mut self, a: &Value) -> Result<Value, String> {
        let steps = opt_u64(a, "steps")?.unwrap_or(1).max(1);
        let app = &mut self.doc(a)?.app;
        let before = app.doc.clone();
        let mut done = 0;
        for _ in 0..steps {
            if !app.can_undo() {
                break;
            }
            app.undo();
            done += 1;
        }
        if done == 0 {
            return Err("There is nothing to undo.".into());
        }
        Ok(json!({ "undone_steps": done, "changes": schedule_diff(&before, app) }))
    }

    fn save(&mut self, a: &Value) -> Result<Value, String> {
        let overwrite = opt_bool(a, "overwrite")?.unwrap_or(false);
        let given = opt_str(a, "path")?.map(PathBuf::from);
        let d = self.doc(a)?;
        let target = match given.clone().or_else(|| d.app.path.clone()) {
            Some(p) => p,
            None => return Err("This project has no file yet: give a path ending in .neramit (or .xml / .mpp).".into()),
        };
        let ext = ext_of(&target);
        if !matches!(ext.as_str(), "neramit" | "xml" | "mpp") {
            return Err("save writes .neramit, .xml or .mpp files; use export for .pdf, .xlsx and .csv.".into());
        }
        let same_as_open = d.app.path.as_ref().is_some_and(|p| same_file(p, &target));
        if ext == "mpp" && same_as_open {
            return Err("The .mpp the project was opened from is never overwritten: give a new file name (or save as .neramit).".into());
        }
        if target.exists() {
            if same_as_open {
                if !overwrite && d.disk_time.is_some() && modified(&target) != d.disk_time {
                    return Err(format!("{} was changed on disk after it was opened here (perhaps saved in Neramit Planner). Pass overwrite: true to replace it, or close and open it again.", target.display()));
                }
            } else if !overwrite {
                return Err(format!("{} already exists: pass overwrite: true to replace it.", target.display()));
            }
        }
        d.app.message = None;
        if !d.app.save_to(target.clone()) {
            return Err(d.app.message.take().map(|m| m.1).unwrap_or_else(|| "The file could not be saved.".into()));
        }
        d.disk_time = modified(&target);
        Ok(json!({ "saved": target.display().to_string(), "tasks": d.app.doc.tasks.len() }))
    }

    fn export(&mut self, a: &Value) -> Result<Value, String> {
        let path = PathBuf::from(req_str(a, "path")?);
        let overwrite = opt_bool(a, "overwrite")?.unwrap_or(false);
        let ext = ext_of(&path);
        if !matches!(ext.as_str(), "pdf" | "xlsx" | "csv" | "xml") {
            return Err("export writes .pdf, .xlsx, .csv or .xml".into());
        }
        if path.exists() && !overwrite {
            return Err(format!("{} already exists: pass overwrite: true to replace it.", path.display()));
        }
        let d = self.doc(a)?;
        d.app.message = None;
        d.app.export_quiet(&path);
        if let Some((_, e)) = d.app.message.take() {
            return Err(e);
        }
        Ok(json!({ "exported": path.display().to_string() }))
    }
}

// ---------------------------------------------------------------------- reading

fn modified(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

fn ext_of(p: &Path) -> String {
    p.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default()
}

fn iso(t: NaiveDateTime) -> String {
    t.format("%Y-%m-%dT%H:%M").to_string()
}

fn start_of(t: &Task) -> String {
    t.start_dt.map(iso).unwrap_or_else(|| t.start.to_string())
}

fn finish_of(t: &Task) -> String {
    t.finish_dt.map(iso).unwrap_or_else(|| t.finish.to_string())
}

fn days(app: &App, minutes: i64) -> f64 {
    let d = minutes as f64 / app.doc.calendar.minutes_per_day() as f64;
    (d * 100.0).round() / 100.0
}

fn duration_text(app: &App, t: &Task) -> String {
    let mpd = app.doc.calendar.minutes_per_day();
    let s = crate::util::fmt_dur(t.dur_minutes(mpd), t.dur_unit, t.dur_elapsed, app.dur_units(), &app.doc.options, false);
    if t.estimated { format!("{s}?") } else { s }
}

fn wbs_codes(app: &App) -> Vec<String> {
    let numbers = app.doc.outline_numbers();
    app.doc.tasks.iter().zip(numbers).map(|(t, n)| if t.wbs.is_empty() { n } else { t.wbs.clone() }).collect()
}

fn task_row(app: &App, i: usize, wbs: &[String]) -> Value {
    let t = &app.doc.tasks[i];
    json!({
        "id": i + 1,
        "uid": t.uid,
        "wbs": wbs[i],
        "name": t.name,
        "outline_level": t.outline_level,
        "summary": t.is_summary,
        "milestone": t.is_milestone(),
        "mode": if t.manual { "manual" } else { "auto" },
        "active": t.active,
        "duration": duration_text(app, t),
        "duration_days": days(app, t.dur_minutes(app.doc.calendar.minutes_per_day())),
        "start": start_of(t),
        "finish": finish_of(t),
        "percent_complete": t.percent_complete,
        "predecessors": app.cell_text(i, TCol::Predecessors),
        "resources": app.doc.resource_names(t),
        "critical": t.critical,
        "total_slack_days": days(app, t.slack_min),
        "status": app.status_text(i),
    })
}

fn task_detail(app: &App, i: usize) -> Value {
    let wbs = wbs_codes(app);
    let mut v = task_row(app, i, &wbs);
    let t = &app.doc.tasks[i];
    let o = v.as_object_mut().unwrap();
    o.insert("successors".into(), json!(app.cell_text(i, TCol::Successors)));
    o.insert("notes".into(), json!(t.notes));
    o.insert("constraint_type".into(), json!(t.constraint_type.label()));
    o.insert("constraint_date".into(), json!(t.constraint_date.filter(|_| t.constraint_type.needs_date()).map(|d| d.to_string())));
    o.insert("deadline".into(), json!(t.deadline.map(|d| d.to_string())));
    o.insert("priority".into(), json!(t.priority));
    o.insert("estimated".into(), json!(t.estimated));
    o.insert("task_type".into(), json!(t.task_type.label()));
    o.insert("free_slack_days".into(), json!(days(app, t.free_slack_min)));
    o.insert("actual_start".into(), json!(t.actual_start.map(iso)));
    o.insert("actual_finish".into(), json!(t.actual_finish.map(iso)));
    o.insert("work_hours".into(), json!((t.work_min as f64 / 60.0 * 100.0).round() / 100.0));
    o.insert("cost".into(), json!((t.cost * 100.0).round() / 100.0));
    o.insert(
        "baseline".into(),
        match t.baseline(0) {
            Some(b) => json!({
                "start": b.start.map(iso),
                "finish": b.finish.map(iso),
                "duration_days": b.dur_min.map(|m| days(app, m)),
                "finish_variance_days": match (b.finish, t.finish_dt) {
                    (Some(bf), Some(f)) => Some(days(app, if f >= bf { app.doc.calendar.work_minutes(bf, f) } else { -app.doc.calendar.work_minutes(f, bf) })),
                    _ => None,
                },
            }),
            None => Value::Null,
        },
    );
    if t.is_summary {
        o.insert("subtasks".into(), json!(app.doc.children_end(i) - i - 1));
    }
    if let Some(e) = &t.external {
        o.insert("external_project".into(), json!(e.path));
    }
    v
}

fn summary(app: &App) -> Value {
    let p = &app.doc;
    let leaves: Vec<&Task> = p.tasks.iter().filter(|t| !t.is_summary && t.active && t.external.is_none()).collect();
    let start = p.tasks.iter().filter_map(|t| t.start_dt).min();
    let finish = p.tasks.iter().filter_map(|t| t.finish_dt).max();
    let mpd = p.calendar.minutes_per_day();
    let total: i64 = leaves.iter().map(|t| t.dur_minutes(mpd)).sum();
    let done: i64 = leaves.iter().map(|t| t.actual_minutes(mpd)).sum();
    let wd = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    json!({
        "name": app.doc_name(),
        "path": app.path.as_ref().map(|p| p.display().to_string()),
        "title": p.title,
        "manager": p.manager,
        "company": p.company,
        "project_start": p.start.to_string(),
        "start": start.map(iso),
        "finish": finish.map(iso),
        "status_date": p.status_date.map(|d| d.to_string()),
        "tasks": p.tasks.len(),
        "summary_tasks": p.tasks.iter().filter(|t| t.is_summary).count(),
        "milestones": p.tasks.iter().filter(|t| t.is_milestone()).count(),
        "critical_tasks": leaves.iter().filter(|t| t.critical).count(),
        "complete_tasks": leaves.iter().filter(|t| t.percent_complete == 100).count(),
        "percent_complete": if total > 0 { (done as f64 * 100.0 / total as f64).round() } else { 0.0 },
        "resources": p.resources.len(),
        "overallocated_resources": p.resources.iter().filter(|r| r.overallocated).count(),
        "cost": (p.tasks.iter().filter(|t| t.outline_level == 1).map(|t| t.cost).sum::<f64>() * 100.0).round() / 100.0,
        "calendar": {
            "working_days": wd.iter().zip(p.calendar.working_days).filter(|(_, w)| *w).map(|(d, _)| *d).collect::<Vec<_>>(),
            "hours_per_day": p.calendar.hours_per_day,
            "holidays": p.calendar.holidays.len(),
        },
        "has_baseline": p.tasks.iter().any(|t| t.baseline(0).is_some()),
        "unsaved_changes": app.dirty,
        "undo_steps": app.undo_len(),
    })
}

fn list_tasks(app: &App, a: &Value) -> Result<Value, String> {
    let filter = opt_str(a, "filter")?.unwrap_or("all");
    let kind = match filter {
        "all" => TaskFilter::All,
        "critical" => TaskFilter::Critical,
        "late" => TaskFilter::Late,
        "slipping" => TaskFilter::Slipping,
        "milestones" => TaskFilter::Milestones,
        "summary" => TaskFilter::Summary,
        "incomplete" => TaskFilter::Incomplete,
        "completed" => TaskFilter::Completed,
        "manual" => TaskFilter::Manual,
        "estimated" => TaskFilter::Estimated,
        _ => return Err(format!("Unknown filter \"{filter}\": all, critical, late, slipping, milestones, summary, incomplete, completed, manual, estimated")),
    };
    let from = opt_date(a, "from")?;
    let to = opt_date(a, "to")?;
    let max_level = opt_u64(a, "max_outline_level")?;
    let search = opt_str(a, "search")?.map(str::to_lowercase);
    let resource = match opt_str(a, "resource")? {
        Some(n) => Some(app.doc.resource_by_name(n).ok_or_else(|| format!("No resource named \"{n}\""))?.uid),
        None => None,
    };
    let limit = opt_u64(a, "limit")?.map(|v| v as usize).unwrap_or(DEFAULT_LIMIT);
    let offset = opt_u64(a, "offset")?.map(|v| v as usize).unwrap_or(0);
    let spec = FilterSpec { kind, range: None, resource: None };
    let wbs = wbs_codes(app);
    let rows: Vec<usize> = (0..app.doc.tasks.len())
        .filter(|&i| {
            let t = &app.doc.tasks[i];
            (kind == TaskFilter::All || app.task_matches(i, &spec))
                && max_level.is_none_or(|m| t.outline_level as u64 <= m)
                // in the window: the task runs on some day between from and to
                && from.is_none_or(|f| t.finish >= f)
                && to.is_none_or(|e| t.start <= e)
                && search.as_ref().is_none_or(|s| t.name.to_lowercase().contains(s) || t.notes.to_lowercase().contains(s))
                && resource.is_none_or(|r| t.resources.contains(&r))
        })
        .collect();
    let total = rows.len();
    let page: Vec<Value> = rows.iter().skip(offset).take(limit).map(|&i| task_row(app, i, &wbs)).collect();
    let shown = page.len();
    let mut v = json!({ "total": total, "offset": offset, "returned": shown, "tasks": page });
    if offset + shown < total {
        v["next_offset"] = json!(offset + shown);
    }
    Ok(v)
}

fn critical_path(app: &App) -> Value {
    let wbs = wbs_codes(app);
    let mut rows: Vec<usize> = (0..app.doc.tasks.len()).filter(|&i| {
        let t = &app.doc.tasks[i];
        t.critical && !t.is_summary && t.active
    }).collect();
    rows.sort_by_key(|&i| (app.doc.tasks[i].start_dt, app.doc.tasks[i].finish_dt, i));
    let mpd = app.doc.calendar.minutes_per_day();
    json!({
        "count": rows.len(),
        "critical_days": days(app, rows.iter().map(|&i| app.doc.tasks[i].dur_minutes(mpd)).sum()),
        "project_finish": app.doc.tasks.iter().filter_map(|t| t.finish_dt).max().map(iso),
        "tasks": rows.iter().map(|&i| task_row(app, i, &wbs)).collect::<Vec<_>>(),
    })
}

fn list_resources(app: &App) -> Value {
    let p = &app.doc;
    json!({ "resources": p.resources.iter().enumerate().map(|(k, r)| json!({
        "id": k + 1,
        "uid": r.uid,
        "name": r.name,
        "type": r.rtype.label(),
        "material_label": r.material_label,
        "group": r.group,
        "max_units_percent": r.max_units,
        "std_rate": r.std_rate,
        "overtime_rate": r.ovt_rate,
        "cost_per_use": r.cost_per_use,
        "work_hours": (r.work_min as f64 / 60.0 * 100.0).round() / 100.0,
        "cost": (r.cost * 100.0).round() / 100.0,
        "overallocated": r.overallocated,
        "tasks": p.tasks.iter().filter(|t| t.resources.contains(&r.uid)).count(),
    })).collect::<Vec<_>>() })
}

fn progress_report(app: &App, a: &Value) -> Result<Value, String> {
    if let Some(d) = opt_date(a, "status_date")? {
        // a report as of another day, without changing the project
        let mut copy = App::with_ctx(&egui::Context::default(), None);
        copy.doc = app.doc.clone();
        copy.doc.status_date = Some(d);
        copy.recalc();
        return progress_report(&copy, &json!({}));
    }
    let p = &app.doc;
    let wbs = wbs_codes(app);
    let leaves: Vec<usize> = (0..p.tasks.len()).filter(|&i| !p.tasks[i].is_summary && p.tasks[i].active && p.tasks[i].external.is_none()).collect();
    let late: Vec<usize> = leaves.iter().copied().filter(|&i| app.status_text(i) == "Late").collect();
    let in_progress = leaves.iter().filter(|&&i| (1..100).contains(&p.tasks[i].percent_complete)).count();
    let complete = leaves.iter().filter(|&&i| p.tasks[i].percent_complete == 100).count();
    // finish later than baseline (working days), worst first
    let mut slipping: Vec<(i64, usize)> = leaves
        .iter()
        .filter_map(|&i| {
            let t = &p.tasks[i];
            let (bf, f) = (t.baseline(0)?.finish?, t.finish_dt?);
            (f > bf).then(|| (p.calendar.work_minutes(bf, f), i))
        })
        .collect();
    slipping.sort_by(|a, b| b.0.cmp(&a.0));
    let finish = p.tasks.iter().filter_map(|t| t.finish_dt).max();
    let base_finish = p.tasks.iter().filter_map(|t| t.baseline(0).and_then(|b| b.finish)).max();
    let mut s = summary(app);
    let o = s.as_object_mut().unwrap();
    o.insert("as_of".into(), json!(app.status_instant().date().to_string()));
    o.insert("leaf_tasks".into(), json!(leaves.len()));
    o.insert("in_progress".into(), json!(in_progress));
    o.insert("complete".into(), json!(complete));
    o.insert("not_started".into(), json!(leaves.len() - in_progress - complete));
    o.insert("late_count".into(), json!(late.len()));
    o.insert("late_tasks".into(), json!(late.iter().take(50).map(|&i| task_row(app, i, &wbs)).collect::<Vec<_>>()));
    if let (Some(f), Some(bf)) = (finish, base_finish) {
        o.insert("baseline_finish".into(), json!(iso(bf)));
        o.insert(
            "finish_variance_days".into(),
            json!(days(app, if f >= bf { p.calendar.work_minutes(bf, f) } else { -p.calendar.work_minutes(f, bf) })),
        );
    }
    o.insert(
        "slipping_tasks".into(),
        json!(slipping.iter().take(30).map(|&(m, i)| {
            let mut r = task_row(app, i, &wbs);
            r["slip_days"] = json!(days(app, m));
            r
        }).collect::<Vec<_>>()),
    );
    Ok(s)
}

fn compare_projects(app: &App, a: &Value) -> Result<Value, String> {
    let path = PathBuf::from(req_str(a, "other_path")?);
    let mut prev = crate::fileio::load(&path)?;
    let _ = crate::schedule::schedule(&mut prev);
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let c = crate::compare::compare(&app.doc, &prev, &name);
    let fmt = |v: &Option<crate::compare::Vals>| {
        v.as_ref().map(|v| json!({
            "start": v.start.map(iso),
            "finish": v.finish.map(iso),
            "duration_days": days(app, v.dur_min),
            "percent_complete": v.pct,
            "cost": (v.cost * 100.0).round() / 100.0,
        }))
    };
    let changed: Vec<Value> = c
        .rows
        .iter()
        .filter(|r| r.status != crate::compare::Status::Same)
        .map(|r| json!({ "status": r.status.label(), "name": r.name, "outline_level": r.level, "now": fmt(&r.cur), "before": fmt(&r.prev) }))
        .collect();
    Ok(json!({
        "compared_with": path.display().to_string(),
        "unchanged": c.rows.iter().filter(|r| r.status == crate::compare::Status::Same).count(),
        "differences": changed.len(),
        "tasks": changed.into_iter().take(300).collect::<Vec<_>>(),
    }))
}

/// Tasks whose dates moved, tasks added or deleted, and the project finish, between `before` and now.
fn schedule_diff(before: &crate::model::Project, app: &App) -> Value {
    let now = &app.doc;
    let old: HashMap<u32, &Task> = before.tasks.iter().map(|t| (t.uid, t)).collect();
    let new_uids: std::collections::HashSet<u32> = now.tasks.iter().map(|t| t.uid).collect();
    let mut moved = Vec::new();
    let mut added = Vec::new();
    for (i, t) in now.tasks.iter().enumerate() {
        match old.get(&t.uid) {
            None => added.push(json!({ "id": i + 1, "uid": t.uid, "name": t.name, "start": start_of(t), "finish": finish_of(t) })),
            Some(o) if o.start_dt != t.start_dt || o.finish_dt != t.finish_dt => moved.push(json!({
                "id": i + 1,
                "uid": t.uid,
                "name": t.name,
                "start": [start_of(o), start_of(t)],
                "finish": [finish_of(o), finish_of(t)],
            })),
            _ => {}
        }
    }
    let deleted: Vec<Value> = before.tasks.iter().filter(|t| !new_uids.contains(&t.uid)).map(|t| json!({ "uid": t.uid, "name": t.name })).collect();
    let f0 = before.tasks.iter().filter_map(|t| t.finish_dt).max();
    let f1 = now.tasks.iter().filter_map(|t| t.finish_dt).max();
    let mut v = json!({
        "project_finish": { "before": f0.map(iso), "after": f1.map(iso) },
        "moved_count": moved.len(),
        "added_count": added.len(),
        "deleted_count": deleted.len(),
    });
    if let (Some(a), Some(b)) = (f0, f1) {
        let cal = &now.calendar;
        v["project_finish"]["change_working_days"] = json!(days(app, if b >= a { cal.work_minutes(a, b) } else { -cal.work_minutes(b, a) }));
    }
    let o = v.as_object_mut().unwrap();
    o.insert("moved".into(), json!(moved.into_iter().take(DIFF_ROWS).collect::<Vec<_>>()));
    o.insert("added".into(), json!(added.into_iter().take(DIFF_ROWS).collect::<Vec<_>>()));
    o.insert("deleted".into(), json!(deleted.into_iter().take(DIFF_ROWS).collect::<Vec<_>>()));
    v
}

// ---------------------------------------------------------------------- changing

/// Index of the task an argument object names by `id` (row number) or `uid`.
// ------------------------------------------------------------------ BOQ and payments

/// import_boq: the items of a BOQ workbook added as tasks (boq.rs finds the columns).
fn import_boq(app: &mut App, a: &Value) -> Result<Value, String> {
    let path = opt_str(a, "path")?.ok_or("path is the BOQ file (.xlsx or .csv)")?;
    let sheets = crate::boq::read_file(std::path::Path::new(path))?;
    let pick = match opt_str(a, "sheet")? {
        Some(n) => sheets.iter().position(|s| s.name == n).ok_or_else(|| format!("No sheet \"{n}\". Sheets: {}", sheets.iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join(", ")))?,
        None => (0..sheets.len()).max_by_key(|&k| crate::boq::detect(&sheets[k].rows).map(|c| crate::boq::items(&sheets[k].rows, &c).len()).unwrap_or(0)).unwrap_or(0),
    };
    let rows = &sheets[pick].rows;
    let cols = crate::boq::detect(rows).ok_or("No BOQ header (a row with รายการ / Description and amounts) was found in the first 40 rows")?;
    let list = crate::boq::items(rows, &cols);
    if list.is_empty() {
        return Err("The sheet has no BOQ items under its header".into());
    }
    let days = opt_u64(a, "days")?.unwrap_or(1).min(365) as i32;
    let chain = opt_bool(a, "link_in_sequence")?.unwrap_or(false);
    let first = app.doc.tasks.len();
    let n = crate::boq::add_to_project(&mut app.doc, &list, days, chain)?;
    let letter = |k: Option<usize>| k.map(|k| ((b'A' + (k % 26) as u8) as char).to_string());
    Ok(json!({
        "sheet": sheets[pick].name,
        "header_row": cols.header_row + 1,
        "columns": { "item_no": letter(cols.no), "description": letter(cols.desc), "quantity": letter(cols.qty), "unit": letter(cols.unit), "amount": letter(cols.amount), "material": letter(cols.material), "labour": letter(cols.labour), "unit_price": letter(cols.unit_price) },
        "added_tasks": n,
        "first_id": first + 1,
        "groups": list.iter().filter(|i| i.group).count(),
        "total_amount": (list.iter().map(|i| i.amount).sum::<f64>() * 100.0).round() / 100.0,
    }))
}

fn payment_schedule(app: &App) -> Value {
    let s = crate::payments::schedule(&app.doc, app.status_instant().date());
    let plan = &app.doc.payments;
    json!({
        "contract_value": s.contract,
        "contract_value_is_project_cost": plan.contract_value <= 0.0,
        "advance_pct": plan.advance_pct,
        "retention_pct": plan.retention_pct,
        "advance_amount": s.advance_amount,
        "installments": s.rows.iter().map(|r| json!({
            "name": r.name,
            "task_id": r.task.map(|i| i + 1),
            "task": r.condition,
            "due": r.due.map(|d| d.to_string()),
            "baseline_due": r.baseline_due.map(|d| d.to_string()),
            "percent": (r.pct * 100.0).round() / 100.0,
            "amount": (r.amount * 100.0).round() / 100.0,
            "advance_recovered": (r.advance * 100.0).round() / 100.0,
            "retention": (r.retention * 100.0).round() / 100.0,
            "net": (r.net * 100.0).round() / 100.0,
            "status": r.status.label(),
        })).collect::<Vec<_>>(),
        "percent_total": (s.pct_sum * 100.0).round() / 100.0,
        "ready_to_bill_net": s.due_net,
        "received_net": s.paid_net,
        "retention_total": s.retention_total,
        "contract_value_in_thai_words": crate::thai::baht_text(s.contract),
    })
}

fn set_payment_schedule(app: &mut App, a: &Value) -> Result<Value, String> {
    let plan = &mut app.doc.payments;
    if let Some(v) = opt_f64(a, "contract_value")? {
        plan.contract_value = v.max(0.0);
    }
    if let Some(v) = opt_f64(a, "advance_pct")? {
        plan.advance_pct = v.clamp(0.0, 100.0);
    }
    if let Some(v) = opt_f64(a, "retention_pct")? {
        plan.retention_pct = v.clamp(0.0, 100.0);
    }
    if opt_bool(a, "from_milestones")?.unwrap_or(false) {
        let list = crate::payments::from_milestones(&app.doc, opt_str(a, "language")? != Some("en"));
        if list.is_empty() {
            return Err("The plan has no milestones to make installments from".into());
        }
        app.doc.payments.items = list;
    } else if let Some(items) = a.get("installments") {
        let items = items.as_array().ok_or("installments is a list of {name, percent or amount, task id, paid}")?;
        let mut out = Vec::new();
        for (k, it) in items.iter().enumerate() {
            let task_uid = match opt_u64(it, "task_id")? {
                Some(id) => Some(app.doc.tasks.get((id as usize).wrapping_sub(1)).map(|t| t.uid).ok_or_else(|| format!("installment {}: there is no task with id {id}", k + 1))?),
                None => None,
            };
            out.push(crate::model::Payment {
                name: opt_str(it, "name")?.map(str::to_string).unwrap_or_else(|| format!("งวดที่ {}", k + 1)),
                percent: opt_f64(it, "percent")?.unwrap_or(0.0),
                amount: opt_f64(it, "amount")?.unwrap_or(0.0),
                task_uid,
                paid: opt_bool(it, "paid")?.unwrap_or(false),
                note: opt_str(it, "note")?.unwrap_or("").to_string(),
            });
        }
        app.doc.payments.items = out;
    }
    Ok(payment_schedule(app))
}

fn task_index(app: &App, a: &Value) -> Result<usize, String> {
    if let Some(id) = opt_u64(a, "id")? {
        let n = app.doc.tasks.len();
        return if id >= 1 && id as usize <= n { Ok(id as usize - 1) } else { Err(format!("There is no task with id {id} (ids are 1 to {n})")) };
    }
    if let Some(uid) = opt_u64(a, "uid")? {
        return app.doc.index_of_uid(uid as u32).ok_or_else(|| format!("There is no task with uid {uid}"));
    }
    Err("Say which task with `id` (row number) or `uid`".into())
}

fn label(app: &App, i: usize) -> String {
    format!("task {} \"{}\"", i + 1, app.doc.tasks[i].name)
}

fn cell(app: &mut App, i: usize, col: TCol, text: &str, field: &str) -> Result<(), String> {
    app.apply_cell(i, col, text).map_err(|e| format!("{} {field}: {e}", label(app, i)))
}

/// A field given as a string or a number ("5d" or 5).
fn text_of(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Null => Some(String::new()),
        _ => None,
    }
}

/// Fields a task can be given (add_tasks and update_tasks), in the order they are applied.
fn apply_fields(app: &mut App, i: usize, f: &Value, with_links: bool) -> Result<(), String> {
    let o = f.as_object().ok_or("each task is an object")?;
    let get = |k: &str| -> Result<Option<String>, String> {
        match o.get(k) {
            None => Ok(None),
            Some(v) => text_of(v).map(Some).ok_or_else(|| format!("{k} must be text or a number")),
        }
    };
    if let Some(n) = get("name")? {
        if n.trim().is_empty() {
            return Err(format!("{}: a task needs a name", label(app, i)));
        }
        cell(app, i, TCol::Name, &n, "name")?;
    }
    if let Some(m) = get("mode")? {
        let manual = match m.to_lowercase().as_str() {
            "manual" => true,
            "auto" => false,
            _ => return Err("mode is \"auto\" or \"manual\"".into()),
        };
        cell(app, i, TCol::Mode, if manual { "Manually Scheduled" } else { "Auto Scheduled" }, "mode")?;
    }
    if let Some(v) = o.get("outline_level") {
        let lvl = v.as_u64().filter(|l| (1..=20).contains(l)).ok_or("outline_level is a whole number from 1")? as u8;
        let prev = if i == 0 { 0 } else { app.doc.tasks[i - 1].outline_level };
        if lvl > prev + 1 {
            return Err(format!("{}: outline_level {lvl} is more than one level below the row above (level {prev})", label(app, i)));
        }
        app.doc.tasks[i].outline_level = lvl;
        // the schedule works out which rows are summaries from the outline
        app.recalc();
    }
    if app.doc.tasks[i].is_summary && ["duration", "start", "finish", "actual_start", "actual_finish"].iter().any(|k| o.contains_key(*k)) {
        return Err(format!("{} is a summary task: its dates and duration come from its subtasks", label(app, i)));
    }
    for (k, col) in [
        ("duration", TCol::Duration),
        ("start", TCol::Start),
        ("finish", TCol::Finish),
        ("constraint_type", TCol::ConstraintType),
        ("constraint_date", TCol::ConstraintDate),
        ("deadline", TCol::Deadline),
        ("priority", TCol::Priority),
        ("wbs", TCol::Wbs),
        ("resources", TCol::Resources),
        ("actual_start", TCol::ActualStart),
        ("actual_finish", TCol::ActualFinish),
        ("percent_complete", TCol::Percent),
    ] {
        if let Some(v) = get(k)? {
            cell(app, i, col, &v, k)?;
        }
    }
    if let Some(n) = get("notes")? {
        app.doc.tasks[i].notes = n;
    }
    if let Some(v) = o.get("active") {
        app.doc.tasks[i].active = v.as_bool().ok_or("active is true or false")?;
    }
    if with_links {
        if let Some(p) = get("predecessors")? {
            cell(app, i, TCol::Predecessors, &p, "predecessors")?;
        }
    }
    Ok(())
}

/// Predecessor text with `@k` (k-th task of this call) and IDs from before the call turned into
/// the IDs rows have now.
fn map_predecessors(text: &str, old_ids: &HashMap<usize, u32>, batch: &[u32], app: &App) -> Result<String, String> {
    let uid_to_id: HashMap<u32, usize> = app.doc.tasks.iter().enumerate().map(|(i, t)| (t.uid, i + 1)).collect();
    let mut out = Vec::new();
    for raw in text.split([',', ';']) {
        let tok = raw.trim();
        if tok.is_empty() {
            continue;
        }
        if tok.contains('\\') {
            // a task of another project ("path\ID") is kept as typed
            out.push(tok.to_string());
            continue;
        }
        let (batch_ref, body) = match tok.strip_prefix('@') {
            Some(r) => (true, r),
            None => (false, tok),
        };
        let digits: String = body.chars().take_while(|c| c.is_ascii_digit()).collect();
        let n: usize = digits.parse().map_err(|_| format!("\"{tok}\" is not a predecessor (expected e.g. 3, 3FS+2d or @1)"))?;
        let uid = if batch_ref {
            *batch.get(n.wrapping_sub(1)).ok_or_else(|| format!("\"{tok}\": this call adds {} task(s)", batch.len()))?
        } else {
            *old_ids.get(&n).ok_or_else(|| format!("\"{tok}\": there is no task with id {n}"))?
        };
        let id = uid_to_id[&uid];
        out.push(format!("{id}{}", &body[digits.len()..]));
    }
    Ok(out.join(","))
}

fn add_tasks(app: &mut App, a: &Value) -> Result<Value, String> {
    let list = a.get("tasks").and_then(Value::as_array).filter(|l| !l.is_empty()).ok_or("tasks is a list of tasks to add")?;
    let old_ids: HashMap<usize, u32> = app.doc.tasks.iter().enumerate().map(|(i, t)| (i + 1, t.uid)).collect();
    let mut at = match opt_u64(a, "after_id")? {
        Some(0) => 0,
        Some(id) if (id as usize) <= app.doc.tasks.len() => id as usize,
        Some(id) => return Err(format!("There is no task with id {id}")),
        None => app.doc.tasks.len(),
    };
    let mut batch: Vec<u32> = Vec::new();
    for f in list {
        let name = f.get("name").and_then(Value::as_str).filter(|n| !n.trim().is_empty()).ok_or("every task needs a name")?;
        let mut t = app.doc.new_task(name.trim());
        t.outline_level = if at == 0 { 1 } else { app.doc.tasks[at - 1].outline_level };
        app.doc.tasks.insert(at, t);
        if let Some(c) = crate::wbs::code_for_new(&app.doc, at) {
            app.doc.tasks[at].wbs = c;
        }
        batch.push(app.doc.tasks[at].uid);
        apply_fields(app, at, f, false)?;
        // a new task's index may have moved if the outline changed nothing before it: find it again
        at = app.doc.index_of_uid(*batch.last().unwrap()).unwrap() + 1;
    }
    // links last, once every new task has its row
    for (k, f) in list.iter().enumerate() {
        if let Some(p) = f.get("predecessors").and_then(text_of).filter(|p| !p.trim().is_empty()) {
            let text = map_predecessors(&p, &old_ids, &batch, app)?;
            let i = app.doc.index_of_uid(batch[k]).unwrap();
            cell(app, i, TCol::Predecessors, &text, "predecessors")?;
        }
    }
    app.recalc();
    let wbs = wbs_codes(app);
    let rows: Vec<Value> = batch.iter().filter_map(|u| app.doc.index_of_uid(*u)).map(|i| task_row(app, i, &wbs)).collect();
    Ok(json!({ "added": rows }))
}

fn update_tasks(app: &mut App, a: &Value) -> Result<Value, String> {
    let list = a.get("updates").and_then(Value::as_array).filter(|l| !l.is_empty()).ok_or("updates is a list of {id or uid, fields to change}")?;
    // every task is found before any change, so ids keep meaning the rows as they were
    let targets: Vec<u32> = list.iter().map(|u| task_index(app, u).map(|i| app.doc.tasks[i].uid)).collect::<Result<_, _>>()?;
    for (u, uid) in list.iter().zip(&targets) {
        let i = app.doc.index_of_uid(*uid).unwrap();
        if app.doc.tasks[i].external.is_some() {
            return Err(format!("{} belongs to another project: open that project to change it", label(app, i)));
        }
        apply_fields(app, i, u, true)?;
    }
    app.recalc();
    let wbs = wbs_codes(app);
    Ok(json!({ "updated": targets.iter().filter_map(|u| app.doc.index_of_uid(*u)).map(|i| task_row(app, i, &wbs)).collect::<Vec<_>>() }))
}

fn delete_tasks(app: &mut App, a: &Value) -> Result<Value, String> {
    let refs = a.get("tasks").and_then(Value::as_array).filter(|l| !l.is_empty()).ok_or("tasks is a list of {id} or {uid}")?;
    let mut del = std::collections::BTreeSet::new();
    for r in refs {
        let i = task_index(app, r)?;
        // a summary goes with its subtasks, as in the program
        del.extend(i..app.doc.children_end(i));
    }
    let names: Vec<Value> = del.iter().map(|&i| json!({ "id": i + 1, "uid": app.doc.tasks[i].uid, "name": app.doc.tasks[i].name })).collect();
    let uids: std::collections::HashSet<u32> = del.iter().map(|&i| app.doc.tasks[i].uid).collect();
    for &i in del.iter().rev() {
        app.doc.tasks.remove(i);
    }
    for t in app.doc.tasks.iter_mut() {
        t.predecessors.retain(|l| !uids.contains(&l.pred_uid));
    }
    Ok(json!({ "deleted": names }))
}

/// A link's type and lag as a predecessor suffix: "FS+2d".
fn link_suffix(l: &Value) -> Result<String, String> {
    let kind = opt_str(l, "type")?.unwrap_or("FS").to_uppercase();
    if !matches!(kind.as_str(), "FS" | "SS" | "FF" | "SF") {
        return Err(format!("link type \"{kind}\" is FS, SS, FF or SF"));
    }
    let lag = l.get("lag").and_then(text_of).unwrap_or_default();
    let lag = lag.trim();
    let lag = if lag.is_empty() || lag == "0" || lag.starts_with(['+', '-']) { lag.to_string() } else { format!("+{lag}") };
    Ok(if lag.is_empty() || lag == "0" { kind } else { format!("{kind}{lag}") })
}

fn link_tasks(app: &mut App, a: &Value) -> Result<Value, String> {
    let links = a.get("links").and_then(Value::as_array).filter(|l| !l.is_empty()).ok_or("links is a list of {from, to, type, lag}")?;
    let mut done = Vec::new();
    for l in links {
        let from = task_index(app, &json!({ "id": l.get("from"), "uid": l.get("from_uid") }))?;
        let to = task_index(app, &json!({ "id": l.get("to"), "uid": l.get("to_uid") }))?;
        if from == to {
            return Err("A task cannot be linked to itself".into());
        }
        let from_uid = app.doc.tasks[from].uid;
        // the task's other links stay; a link from the same task is replaced
        let keep: Vec<String> = app.cell_text(to, TCol::Predecessors)
            .split(',')
            .map(str::trim)
            .filter(|tok| !tok.is_empty())
            .filter(|tok| tok.contains('\\') || tok.chars().take_while(|c| c.is_ascii_digit()).collect::<String>() != (from + 1).to_string())
            .map(str::to_string)
            .collect();
        let suffix = link_suffix(l)?;
        let text = keep.into_iter().chain(std::iter::once(format!("{}{suffix}", from + 1))).collect::<Vec<_>>().join(",");
        cell(app, to, TCol::Predecessors, &text, "link")?;
        let to_uid = app.doc.tasks[to].uid;
        done.push(json!({ "from": from + 1, "to": app.doc.index_of_uid(to_uid).map(|i| i + 1), "from_uid": from_uid, "to_uid": to_uid, "link": suffix }));
    }
    Ok(json!({ "linked": done }))
}

fn unlink_tasks(app: &mut App, a: &Value) -> Result<Value, String> {
    let links = a.get("links").and_then(Value::as_array).filter(|l| !l.is_empty()).ok_or("links is a list of {from, to}")?;
    let mut done = Vec::new();
    for l in links {
        let from = task_index(app, &json!({ "id": l.get("from"), "uid": l.get("from_uid") }))?;
        let to = task_index(app, &json!({ "id": l.get("to"), "uid": l.get("to_uid") }))?;
        let pu = app.doc.tasks[from].uid;
        let preds = &mut app.doc.tasks[to].predecessors;
        let n = preds.len();
        preds.retain(|x| x.pred_uid != pu);
        if preds.len() == n {
            return Err(format!("{} is not a predecessor of {}", label(app, from), label(app, to)));
        }
        done.push(json!({ "from": from + 1, "to": to + 1 }));
    }
    Ok(json!({ "unlinked": done }))
}

fn add_resources(app: &mut App, a: &Value) -> Result<Value, String> {
    let list = a.get("resources").and_then(Value::as_array).filter(|l| !l.is_empty()).ok_or("resources is a list of resources")?;
    let mut added = Vec::new();
    for r in list {
        let name = opt_str(r, "name")?.map(str::trim).filter(|n| !n.is_empty()).ok_or("every resource needs a name")?;
        if name.contains([',', ';', '[']) {
            return Err(format!("\"{name}\": a resource name cannot contain , ; or ["));
        }
        let existing = app.doc.resource_by_name(name).map(|r| r.uid);
        let uid = match existing {
            Some(u) => u,
            None => {
                let u = app.doc.alloc_res_uid();
                app.doc.resources.push(Resource::new(u, name));
                u
            }
        };
        let res = app.doc.resources.iter_mut().find(|x| x.uid == uid).unwrap();
        if let Some(t) = opt_str(r, "type")? {
            res.rtype = match t.to_lowercase().as_str() {
                "work" => ResourceType::Work,
                "material" => ResourceType::Material,
                "cost" => ResourceType::Cost,
                _ => return Err(format!("resource type \"{t}\" is work, material or cost")),
            };
        }
        if let Some(v) = opt_f64(r, "max_units_percent")? {
            res.max_units = v;
        }
        if let Some(v) = opt_f64(r, "std_rate")? {
            res.std_rate = v;
        }
        if let Some(v) = opt_f64(r, "overtime_rate")? {
            res.ovt_rate = v;
        }
        if let Some(v) = opt_f64(r, "cost_per_use")? {
            res.cost_per_use = v;
        }
        if let Some(v) = opt_str(r, "material_label")? {
            res.material_label = v.to_string();
        }
        if let Some(v) = opt_str(r, "group")? {
            res.group = v.to_string();
        }
        added.push(json!({ "name": name, "uid": uid, "existed": existing.is_some() }));
    }
    Ok(json!({ "resources": added }))
}

fn update_project(app: &mut App, a: &Value) -> Result<Value, String> {
    let p = &mut app.doc;
    for (k, field) in [("title", &mut p.title), ("manager", &mut p.manager), ("company", &mut p.company), ("author", &mut p.author)] {
        if let Some(v) = opt_str(a, k)? {
            *field = v.to_string();
        }
    }
    if let Some(d) = opt_date(a, "start")? {
        // as Project Information does: tasks without links or constraints start here
        p.start = p.calendar.next_working(d);
        p.start_dt = None;
    }
    match a.get("status_date") {
        Some(Value::Null) => p.status_date = None,
        Some(_) => p.status_date = opt_date(a, "status_date")?,
        None => {}
    }
    if let Some(days) = a.get("working_days") {
        let names = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];
        let list = days.as_array().ok_or("working_days is a list such as [\"Mon\",\"Tue\",\"Wed\",\"Thu\",\"Fri\",\"Sat\"]")?;
        let mut w = [false; 7];
        for d in list {
            let s = d.as_str().map(|s| s.to_lowercase()).unwrap_or_default();
            let k = names.iter().position(|n| s.starts_with(n)).ok_or_else(|| format!("\"{s}\" is not a weekday (Mon .. Sun)"))?;
            w[k] = true;
        }
        if !w.contains(&true) {
            return Err("A calendar needs at least one working day".into());
        }
        p.calendar.working_days = w;
    }
    if let Some(list) = a.get("add_holidays") {
        for h in list.as_array().ok_or("add_holidays is a list of {name, start, finish}")? {
            let start = opt_date(h, "start")?.ok_or("a holiday needs a start date")?;
            let finish = opt_date(h, "finish")?.unwrap_or(start);
            if finish < start {
                return Err("A holiday cannot finish before it starts".into());
            }
            let name = opt_str(h, "name")?.unwrap_or("Holiday").to_string();
            p.calendar.holidays.push(Holiday { name, start, finish });
        }
    }
    if let Some(list) = a.get("remove_holidays") {
        for h in list.as_array().ok_or("remove_holidays is a list of holiday names or dates")? {
            let key = h.as_str().unwrap_or_default();
            let date = NaiveDate::parse_from_str(key, "%Y-%m-%d").ok();
            p.calendar.holidays.retain(|x| x.name != key && date.is_none_or(|d| d < x.start || d > x.finish));
        }
    }
    let wd = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    Ok(json!({
        "title": p.title,
        "project_start": p.start.to_string(),
        "status_date": p.status_date.map(|d| d.to_string()),
        "working_days": wd.iter().zip(p.calendar.working_days).filter(|(_, w)| *w).map(|(d, _)| *d).collect::<Vec<_>>(),
        "holidays": p.calendar.holidays.iter().map(|h| json!({ "name": h.name, "start": h.start.to_string(), "finish": h.finish.to_string() })).collect::<Vec<_>>(),
    }))
}

// ---------------------------------------------------------------------- arguments

fn req_str<'a>(a: &'a Value, k: &str) -> Result<&'a str, String> {
    opt_str(a, k)?.ok_or_else(|| format!("{k} is required"))
}

fn opt_str<'a>(a: &'a Value, k: &str) -> Result<Option<&'a str>, String> {
    match a.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s)),
        Some(_) => Err(format!("{k} must be text")),
    }
}

fn opt_bool(a: &Value, k: &str) -> Result<Option<bool>, String> {
    match a.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(_) => Err(format!("{k} must be true or false")),
    }
}

fn opt_u64(a: &Value, k: &str) -> Result<Option<u64>, String> {
    match a.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_u64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok())).map(Some).ok_or_else(|| format!("{k} must be a whole number")),
    }
}

fn opt_f64(a: &Value, k: &str) -> Result<Option<f64>, String> {
    match a.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_f64().filter(|x| x.is_finite()).map(Some).ok_or_else(|| format!("{k} must be a number")),
    }
}

fn opt_date(a: &Value, k: &str) -> Result<Option<NaiveDate>, String> {
    match opt_str(a, k)? {
        None => Ok(None),
        Some(s) => NaiveDate::parse_from_str(s.trim().get(..10).unwrap_or(s), "%Y-%m-%d").map(Some).map_err(|_| format!("{k} must be a date like 2026-11-02")),
    }
}

// ---------------------------------------------------------------------- tool list

fn doc_prop() -> Value {
    json!({ "type": "string", "description": "Project handle from open_project / new_project (may be left out while only one project is open)" })
}

fn dry_run_prop() -> Value {
    json!({ "type": "boolean", "description": "Report what the change would do without keeping it" })
}

fn task_fields(with_links: bool) -> Map<String, Value> {
    let s = |d: &str| json!({ "type": "string", "description": d });
    let mut m = Map::new();
    m.insert("name".into(), s("Task name"));
    m.insert("outline_level".into(), json!({ "type": "integer", "minimum": 1, "description": "1 = top level; a row one level deeper than the row above becomes its subtask (the row above turns into a summary)" }));
    m.insert("duration".into(), json!({ "type": ["string", "number"], "description": "e.g. 5, \"5d\", \"0.5d\", \"4h\", \"2w\", \"3ed\"; 0 = milestone. Not for summary tasks" }));
    m.insert("start".into(), s("YYYY-MM-DD. On an auto-scheduled task this sets Start No Earlier Than (prefer links)"));
    m.insert("finish".into(), s("YYYY-MM-DD. On an auto-scheduled task this sets Finish No Earlier Than"));
    m.insert("mode".into(), json!({ "type": "string", "enum": ["auto", "manual"] }));
    m.insert("resources".into(), s("Resource names separated by commas, e.g. \"คนงาน, วิศวกร\" (unknown names become new work resources)"));
    m.insert("percent_complete".into(), json!({ "type": "number", "minimum": 0, "maximum": 100, "description": "On a summary task it is passed on to the subtasks" }));
    m.insert("actual_start".into(), s("YYYY-MM-DD, or \"NA\" to clear"));
    m.insert("actual_finish".into(), s("YYYY-MM-DD, or \"NA\" to clear"));
    m.insert("deadline".into(), s("YYYY-MM-DD, or \"NA\" to clear"));
    m.insert("constraint_type".into(), s("As Soon As Possible, As Late As Possible, Must Start On, Must Finish On, Start No Earlier Than, Start No Later Than, Finish No Earlier Than, Finish No Later Than"));
    m.insert("constraint_date".into(), s("YYYY-MM-DD, or \"NA\" to clear the constraint"));
    m.insert("priority".into(), json!({ "type": "integer", "minimum": 0, "maximum": 1000 }));
    m.insert("wbs".into(), s("WBS code"));
    m.insert("notes".into(), s("Task notes"));
    m.insert("active".into(), json!({ "type": "boolean", "description": "false = inactive (kept but not scheduled into the project)" }));
    m.insert(
        "predecessors".into(),
        s(if with_links {
            "Replaces the task's predecessors, as typed in the Predecessors column: \"3\", \"3FS+2d\", \"4SS, 7FF-1d\" (ids as listed now)"
        } else {
            "As typed in the Predecessors column, e.g. \"3FS+2d, 5\". Numbers are task ids as they were before this call; @k is the k-th task of this call (\"@1\", \"@2SS+1d\")"
        }),
    );
    m
}

fn tool(name: &str, title: &str, description: &str, props: Value, required: &[&str], read_only: bool) -> Value {
    let mut p = props.as_object().cloned().unwrap_or_default();
    if name != "open_project" && name != "new_project" && name != "list_open_projects" {
        p.insert("doc".into(), doc_prop());
    }
    if !read_only && !matches!(name, "save" | "export" | "undo" | "close_project" | "open_project" | "new_project") {
        p.insert("dry_run".into(), dry_run_prop());
    }
    json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": { "type": "object", "properties": p, "required": required },
        "annotations": { "readOnlyHint": read_only, "destructiveHint": name == "delete_tasks", "openWorldHint": false },
    })
}

fn tools() -> Vec<Value> {
    let id_ref = json!({ "type": "object", "properties": { "id": { "type": "integer", "description": "Row id" }, "uid": { "type": "integer", "description": "Unique id" } } });
    let link = json!({ "type": "object", "properties": {
        "from": { "type": "integer", "description": "Predecessor id" },
        "to": { "type": "integer", "description": "Successor id" },
        "from_uid": { "type": "integer" },
        "to_uid": { "type": "integer" },
        "type": { "type": "string", "enum": ["FS", "SS", "FF", "SF"], "default": "FS" },
        "lag": { "type": ["string", "number"], "description": "e.g. \"2d\", \"-1d\" (lead), \"4h\", \"50%\"" },
    } });
    let mut update = task_fields(true);
    update.insert("id".into(), json!({ "type": "integer", "description": "Row id of the task to change" }));
    update.insert("uid".into(), json!({ "type": "integer", "description": "Or its unique id" }));
    vec![
        tool("open_project", "Open a project", "Open a .neramit, .mpp, .mpt or .xml project file and return its summary and `doc` handle.", json!({ "path": { "type": "string", "description": "Full path of the file" } }), &["path"], true),
        tool("new_project", "New project", "Start a new, empty project in memory (save writes it to a file).", json!({
            "title": { "type": "string" },
            "start": { "type": "string", "description": "Project start, YYYY-MM-DD" },
            "path": { "type": "string", "description": "Where save will write it (.neramit), optional" },
        }), &[], false),
        tool("list_open_projects", "List open projects", "The projects open in this session with their `doc` handles.", json!({}), &[], true),
        tool("close_project", "Close a project", "Close a project. Refuses when it has unsaved changes unless discard_changes is true.", json!({ "discard_changes": { "type": "boolean" } }), &[], false),
        tool("project_summary", "Project summary", "Dates, size, progress, calendar and cost of the project.", json!({}), &[], true),
        tool("list_tasks", "List tasks", "Tasks in outline order with dates, duration, links, resources, slack and status. Pages through long projects with limit / offset.", json!({
            "filter": { "type": "string", "enum": ["all", "critical", "late", "slipping", "milestones", "summary", "incomplete", "completed", "manual", "estimated"], "default": "all" },
            "from": { "type": "string", "description": "Only tasks running on or after this date (YYYY-MM-DD)" },
            "to": { "type": "string", "description": "Only tasks running on or before this date" },
            "max_outline_level": { "type": "integer", "description": "e.g. 2 for the top two outline levels" },
            "search": { "type": "string", "description": "Text in the task name or notes" },
            "resource": { "type": "string", "description": "Only tasks using this resource" },
            "limit": { "type": "integer", "default": DEFAULT_LIMIT },
            "offset": { "type": "integer", "default": 0 },
        }), &[], true),
        tool("get_task", "Task details", "Everything about one task: links both ways, constraint, deadline, baseline and variance, actuals, work, cost, notes.", json!({ "id": { "type": "integer" }, "uid": { "type": "integer" } }), &[], true),
        tool("critical_path", "Critical path", "The critical tasks in date order (the chain that decides the project finish).", json!({}), &[], true),
        tool("list_resources", "List resources", "Resources with rates, work, cost and whether they are overallocated.", json!({}), &[], true),
        tool("progress_report", "Progress report", "Progress as of the status date (or a given date): complete / in progress / late tasks, and slippage against the baseline.", json!({
            "status_date": { "type": "string", "description": "Report as of this date without changing the project (YYYY-MM-DD)" },
        }), &[], true),
        tool("compare_projects", "Compare with another version", "Tasks added, deleted or changed (dates, duration, % complete, cost) compared with another version of the project file.", json!({
            "other_path": { "type": "string", "description": "The earlier (or other) version's file" },
        }), &["other_path"], true),
        tool("add_tasks", "Add tasks", "Add tasks (a whole outline at once if needed), then their links. Returns the new tasks and what moved.", json!({
            "tasks": { "type": "array", "items": { "type": "object", "properties": task_fields(false), "required": ["name"] } },
            "after_id": { "type": "integer", "description": "Insert directly after this row (0 = at the top); default: at the end" },
        }), &["tasks"], false),
        tool("update_tasks", "Change tasks", "Change fields of one or more tasks (ids refer to the rows as they are before the call).", json!({
            "updates": { "type": "array", "items": { "type": "object", "properties": update } },
        }), &["updates"], false),
        tool("delete_tasks", "Delete tasks", "Delete tasks; a summary task is deleted with its subtasks. Links to them are removed.", json!({
            "tasks": { "type": "array", "items": id_ref },
        }), &["tasks"], false),
        tool("link_tasks", "Link tasks", "Add links (a link between the same two tasks is replaced). Refuses circular links and links between a summary and its own subtasks.", json!({
            "links": { "type": "array", "items": link },
        }), &["links"], false),
        tool("unlink_tasks", "Unlink tasks", "Remove links between tasks.", json!({
            "links": { "type": "array", "items": link },
        }), &["links"], false),
        tool("add_resources", "Add resources", "Add resources (or change ones with the same name).", json!({
            "resources": { "type": "array", "items": { "type": "object", "properties": {
                "name": { "type": "string" },
                "type": { "type": "string", "enum": ["work", "material", "cost"], "default": "work" },
                "max_units_percent": { "type": "number", "description": "100 = one full-time person, 300 = three" },
                "std_rate": { "type": "number", "description": "Cost per hour (work) or per unit (material)" },
                "overtime_rate": { "type": "number" },
                "cost_per_use": { "type": "number" },
                "material_label": { "type": "string", "description": "Unit of a material, e.g. ตัน, ลบ.ม." },
                "group": { "type": "string" },
            }, "required": ["name"] } },
        }), &["resources"], false),
        tool("update_project", "Change project information", "Title, start, status date, working days and holidays.", json!({
            "title": { "type": "string" },
            "manager": { "type": "string" },
            "company": { "type": "string" },
            "author": { "type": "string" },
            "start": { "type": "string", "description": "Project start, YYYY-MM-DD" },
            "status_date": { "type": ["string", "null"], "description": "YYYY-MM-DD; null clears it" },
            "working_days": { "type": "array", "items": { "type": "string" }, "description": "e.g. [\"Mon\",\"Tue\",\"Wed\",\"Thu\",\"Fri\",\"Sat\"]" },
            "add_holidays": { "type": "array", "items": { "type": "object", "properties": { "name": { "type": "string" }, "start": { "type": "string" }, "finish": { "type": "string" } }, "required": ["start"] } },
            "remove_holidays": { "type": "array", "items": { "type": "string" }, "description": "Holiday names or dates" },
        }), &[], false),
        tool("import_boq", "Import a BOQ", "Add the items of a bill of quantities (Excel .xlsx or CSV, Thai or English headers such as ลำดับ / รายการ / จำนวน / หน่วย / ค่าวัสดุ / ค่าแรง / รวม) at the end of the plan: groups become summary tasks, items become tasks with their amount as fixed cost. Subtotal, VAT and Factor F rows are skipped.", json!({
            "path": { "type": "string", "description": "The .xlsx or .csv file" },
            "sheet": { "type": "string", "description": "Sheet name (default: the sheet with the most items)" },
            "days": { "type": "integer", "minimum": 0, "default": 1, "description": "Duration of each item in working days" },
            "link_in_sequence": { "type": "boolean", "default": false, "description": "Link the items finish-to-start one after another" },
        }), &["path"], false),
        tool("list_boq", "List structured BOQ", "Read native BOQ quantities, units and contract amounts with stable IDs. These records are saved only to .neramit.", json!({}), &[], true),
        tool("list_progress", "Read physical progress observations", "Read dated physical progress history; through limits records to a cutoff. Missing data stays missing.", json!({
            "id": { "type": "integer", "description": "Optional task row" },
            "uid": { "type": "integer", "description": "Optional task UID" },
            "through": { "type": "string", "description": "Include observations through YYYY-MM-DD" },
        }), &[], true),
        tool("record_progress", "Record physical site progress", "Append a dated physical-progress entry for a local leaf task. With a single BOQ item, cumulative quantity can determine physical %. Does not rewrite schedule % Complete. Supports dry_run.", json!({
            "id": { "type": "integer" }, "uid": { "type": "integer" },
            "as_of": { "type": "string", "description": "Actual survey date YYYY-MM-DD" },
            "percent_complete": { "type": "number", "minimum": 0, "maximum": 100 },
            "cumulative_quantity": { "type": "number", "minimum": 0 },
            "note": { "type": "string", "description": "Required for a downward correction" },
        }), &["as_of"], false),
        tool("list_actual_costs", "Read posted actual costs", "List dated costs entered by the user. Does not include estimated resource-rate costs.", json!({}), &[], true),
        tool("record_actual_cost", "Post actual cost", "Append an explicitly dated actual cost to the native ledger. Does not rewrite estimated task costs. Supports dry_run.", json!({
            "id": { "type": "integer" }, "uid": { "type": "integer" },
            "incurred_on": { "type": "string", "description": "YYYY-MM-DD" },
            "amount": { "type": "number", "minimum": 0 },
            "category": { "type": "string" }, "reference": { "type": "string" },
        }), &["incurred_on","amount","category"], false),
        tool("payment_schedule", "Payment schedule", "The contract's installments (งวดงาน): amount, advance recovered, retention, net, due date from the linked task and status (Paid, Ready to bill, Work late, Planned).", json!({}), &[], true),
        tool("set_payment_schedule", "Set payment schedule", "Change the contract terms and installments. Give installments, or from_milestones to make one per milestone sized by the value of the work before it.", json!({
            "contract_value": { "type": "number", "description": "0 = the project's total cost" },
            "advance_pct": { "type": "number", "description": "Advance payment, % of the contract, recovered from each installment" },
            "retention_pct": { "type": "number", "description": "Retention, % withheld from each installment" },
            "from_milestones": { "type": "boolean" },
            "language": { "type": "string", "enum": ["th", "en"], "default": "th", "description": "Names of installments made from milestones" },
            "installments": { "type": "array", "items": { "type": "object", "properties": {
                "name": { "type": "string" },
                "percent": { "type": "number" },
                "amount": { "type": "number", "description": "A fixed amount instead of percent" },
                "task_id": { "type": "integer", "description": "Row of the task (usually a milestone) whose completion makes it due" },
                "paid": { "type": "boolean" },
                "note": { "type": "string" },
            } } },
        }), &[], false),
        tool("set_baseline", "Set baseline", "Save the current dates, durations, work and cost of every task as a baseline.", json!({
            "number": { "type": "integer", "minimum": 0, "maximum": 10, "default": 0, "description": "0 = Baseline, 1-10 = Baseline 1-10" },
        }), &[], false),
        tool("level_resources", "Level resources", "Delay tasks so no resource is overallocated (Resource > Level All).", json!({}), &[], false),
        tool("undo", "Undo", "Undo the last changes made in this session.", json!({ "steps": { "type": "integer", "default": 1 } }), &[], false),
        tool("save", "Save", "Write the project to its file, or to path (.neramit, .xml, or a new .mpp). Never overwrites the .mpp it was opened from, nor a file changed on disk since it was opened, unless overwrite is true where allowed.", json!({
            "path": { "type": "string", "description": "Save as this file instead" },
            "overwrite": { "type": "boolean", "description": "Replace an existing file" },
        }), &[], false),
        tool("export", "Export", "Write a PDF (Gantt chart), Excel workbook, CSV or XML copy of the project.", json!({
            "path": { "type": "string", "description": "File ending in .pdf, .xlsx, .csv or .xml" },
            "overwrite": { "type": "boolean" },
        }), &["path"], false),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(s: &mut Server, name: &str, args: Value) -> Value {
        let line = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": name, "arguments": args } }).to_string();
        let r = s.message(&line).unwrap();
        let res = &r["result"];
        assert_eq!(res["isError"], json!(false), "{name}: {}", res["structuredContent"]);
        res["structuredContent"].clone()
    }

    fn call_err(s: &mut Server, name: &str, args: Value) -> String {
        let line = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": name, "arguments": args } }).to_string();
        let r = s.message(&line).unwrap();
        assert_eq!(r["result"]["isError"], json!(true));
        r["result"]["structuredContent"]["error"].as_str().unwrap().to_string()
    }

    fn server() -> Server {
        crate::app::NO_PERSIST.store(true, std::sync::atomic::Ordering::Relaxed);
        Server { docs: Vec::new(), next: 1 }
    }

    #[test]
    fn handshake_and_tool_list() {
        let mut s = server();
        let r = s.message(r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"1"}}}"#).unwrap();
        assert_eq!(r["result"]["protocolVersion"], "2025-06-18");
        assert!(s.message(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).is_none());
        let r = s.message(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#).unwrap();
        let names: Vec<&str> = r["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"add_tasks") && names.contains(&"save"));
        assert!(names.contains(&"record_progress") && names.contains(&"list_boq") && names.contains(&"record_actual_cost"));
        let r = s.message(r#"{"jsonrpc":"2.0","id":3,"method":"nope"}"#).unwrap();
        assert_eq!(r["error"]["code"], -32601);
        let r = s.message("{bad").unwrap();
        assert_eq!(r["error"]["code"], -32700);
    }

    #[test]
    fn build_a_schedule_from_scratch() {
        let mut s = server();
        let v = call(&mut s, "new_project", json!({ "title": "อาคาร A", "start": "2026-11-02" }));
        assert_eq!(v["doc"], "d1");
        let v = call(&mut s, "add_tasks", json!({ "tasks": [
            { "name": "งานฐานราก", "outline_level": 1 },
            { "name": "ขุดดิน", "outline_level": 2, "duration": "3d", "resources": "คนงาน" },
            { "name": "เทฐานราก", "outline_level": 2, "duration": 2, "predecessors": "@2" },
            { "name": "ตรวจรับ", "outline_level": 2, "duration": 0, "predecessors": "@3FS+1d" },
        ] }));
        let added = v["added"].as_array().unwrap();
        assert_eq!(added.len(), 4);
        assert_eq!(added[0]["summary"], true);
        assert_eq!(added[1]["start"], "2026-11-02T08:00");
        assert_eq!(added[2]["start"], "2026-11-05T08:00");
        assert_eq!(added[2]["predecessors"], "2");
        assert_eq!(added[3]["milestone"], true);
        assert_eq!(added[3]["finish"], "2026-11-09T17:00");
        assert_eq!(v["changes"]["added_count"], 4);

        // a dry run reports the move and keeps nothing
        let v = call(&mut s, "update_tasks", json!({ "updates": [{ "id": 2, "duration": "5d" }], "dry_run": true }));
        assert_eq!(v["changes"]["project_finish"]["change_working_days"], 2.0);
        let t = call(&mut s, "get_task", json!({ "id": 2 }));
        assert_eq!(t["duration_days"], 3.0);

        call(&mut s, "update_tasks", json!({ "updates": [{ "id": 2, "duration": "5d", "percent_complete": 40 }] }));
        let cp = call(&mut s, "critical_path", json!({}));
        assert_eq!(cp["count"], 3);

        // links: circular ones are refused and nothing changes
        let e = call_err(&mut s, "link_tasks", json!({ "links": [{ "from": 4, "to": 2 }] }));
        assert!(e.contains("circular"), "{e}");
        let v = call(&mut s, "link_tasks", json!({ "links": [{ "from": 2, "to": 3, "type": "SS", "lag": "1d" }] }));
        assert_eq!(v["linked"][0]["link"], "SS+1d");
        assert_eq!(call(&mut s, "get_task", json!({ "id": 3 }))["predecessors"], "2SS+1 day");

        // inserting above shifts ids; ids in predecessors still mean the rows as they were
        call(&mut s, "add_tasks", json!({ "after_id": 0, "tasks": [{ "name": "เตรียมพื้นที่", "duration": "1d" }], "dry_run": false }));
        let v = call(&mut s, "add_tasks", json!({ "after_id": 1, "tasks": [{ "name": "รั้ว", "duration": "2d", "predecessors": "3" }] }));
        assert_eq!(v["added"][0]["id"], 2);
        assert_eq!(v["added"][0]["predecessors"], "4");

        call(&mut s, "undo", json!({ "steps": 2 }));
        assert_eq!(call(&mut s, "project_summary", json!({}))["tasks"], 4);

        let e = call_err(&mut s, "close_project", json!({}));
        assert!(e.contains("unsaved"));
        let dir = std::env::temp_dir().join(format!("neramit-mcp-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.neramit");
        let v = call(&mut s, "save", json!({ "path": path.display().to_string(), "overwrite": true }));
        assert!(v["saved"].as_str().unwrap().ends_with("a.neramit"));
        call(&mut s, "export", json!({ "path": dir.join("a.xlsx").display().to_string(), "overwrite": true }));
        call(&mut s, "close_project", json!({}));
        let v = call(&mut s, "open_project", json!({ "path": path.display().to_string() }));
        assert_eq!(v["tasks"], 4);
        assert_eq!(v["title"], "อาคาร A");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
