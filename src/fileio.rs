//! File formats: native Neramit Planner (.neramit, JSON), project XML and CSV export.

use crate::model::*;
use chrono::NaiveDate;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;

#[derive(serde::Serialize, serde::Deserialize)]
struct NativeFile {
    format: String,
    version: u32,
    project: Project,
}

/// XML ExtendedAttribute values -> custom field values.
fn custom_values(cf: &[(u32, String)], resource: bool) -> crate::custom::Values {
    let mut out: crate::custom::Values = Vec::new();
    for (id, v) in cf {
        if let Some((f, r)) = crate::custom::by_field_id(*id) {
            if r == resource {
                if let Some(val) = crate::custom::from_xml(f, v) {
                    crate::custom::set(&mut out, f, Some(val));
                }
            }
        }
    }
    out
}

pub fn load(path: &Path) -> Result<Project, String> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let bytes = std::fs::read(path).map_err(|e| format!("Cannot read file: {e}"))?;
    match ext.as_str() {
        "xml" => load_xml_doc(&bytes),
        "mpp" | "mpt" => crate::mpp::load_mpp(&bytes),
        _ => {
            // any native JSON project document (also those of earlier versions under another tag)
            let f: NativeFile = serde_json::from_slice(&bytes).map_err(|e| format!("Invalid Neramit Planner file: {e}"))?;
            if f.format.is_empty() {
                return Err("Not a Neramit Planner file".into());
            }
            let mut p = f.project;
            for t in p.tasks.iter_mut() {
                t.normalize_constraint();
            }
            p.next_uid = p.next_uid.max(p.tasks.iter().map(|t| t.uid + 1).max().unwrap_or(1));
            p.next_res_uid = p.next_res_uid.max(p.resources.iter().map(|r| r.uid + 1).max().unwrap_or(1));
            Ok(p)
        }
    }
}

pub fn save(path: &Path, p: &Project) -> Result<(), String> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if ext == "xml" && !p.controls.is_empty() {
        return Err("Dated progress, actual costs and structured BOQ are native-only. Save as .neramit to prevent data loss; XML export is blocked until a tested adapter exists.".into());
    }
    let data = match ext.as_str() {
        "mpp" | "mpt" => return Err("Neramit Planner cannot write .mpp files here. Save as .neramit or XML (*.xml) instead.".into()),
        "xml" => to_xml_doc(p, path),
        "csv" => to_csv(p),
        _ => serde_json::to_string_pretty(&NativeFile { format: "Neramit Planner".into(), version: 2, project: p.clone() })
            .map_err(|e| e.to_string())?,
    };
    // write atomically: temp file then rename
    let tmp = path.with_extension(format!("{ext}.tmp"));
    std::fs::write(&tmp, data.as_bytes()).map_err(|e| format!("Cannot write file: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Cannot save file: {e}")
    })
}

// ------------------------------------------------------------------ CSV

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn to_csv(p: &Project) -> String {
    let mut out = String::from("\u{feff}ID,Outline Level,Task Mode,Name,Duration,Start,Finish,Predecessors,Resource Names,% Complete,Notes\r\n");
    let ids: HashMap<u32, usize> = p.tasks.iter().enumerate().map(|(i, t)| (t.uid, i + 1)).collect();
    for (i, t) in p.tasks.iter().enumerate() {
        let preds = format_predecessors(&t.predecessors, &ids);
        let _ = write!(
            out,
            "{},{},{},{},{},{},{},{},{},{}%,{}\r\n",
            i + 1,
            t.outline_level,
            if t.manual { "Manually Scheduled" } else { "Auto Scheduled" },
            csv_field(&t.name),
            csv_field(&format_duration(t.duration)),
            t.start.format("%Y-%m-%d"),
            t.finish.format("%Y-%m-%d"),
            csv_field(&preds),
            csv_field(&p.resource_names(t)),
            t.percent_complete,
            csv_field(&t.notes)
        );
    }
    out
}

pub fn format_duration(d: i32) -> String {
    if d == 1 {
        "1 day".into()
    } else {
        format!("{d} days")
    }
}

pub fn format_predecessors(links: &[Link], ids: &HashMap<u32, usize>) -> String {
    let o = ProjectOptions::default();
    format_predecessors_with(links, ids, crate::util::DurUnits { mpd: 480, mpw: 2400, dpm: 20.0 }, &o, false)
}

/// Predecessor text as MPP shows it: "2", "3SS+4 hrs", "5FF-50%", "6FS+3 edays".
pub fn format_predecessors_with(links: &[Link], ids: &HashMap<u32, usize>, du: crate::util::DurUnits, o: &ProjectOptions, thai: bool) -> String {
    links
        .iter()
        .filter_map(|l| {
            let id = ids.get(&l.pred_uid)?;
            let mut s = id.to_string();
            let lag = crate::util::fmt_lag(l, du, o, thai);
            if l.kind != LinkType::FS || !lag.is_empty() {
                s.push_str(l.kind.code());
            }
            let _ = write!(s, "{lag}");
            Some(s)
        })
        .collect::<Vec<_>>()
        .join(",")
}

// ------------------------------------------------------------------ XML writer

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            c if (c as u32) < 0x20 && c != '\n' && c != '\r' && c != '\t' => {}
            c => o.push(c),
        }
    }
    o
}

fn dt(d: NaiveDate, time: &str) -> String {
    format!("{}T{}", d.format("%Y-%m-%d"), time)
}

/// The comment Neramit Planner keeps its drawings in (Project XML has no element for them).
const DRAWINGS_TAG: &str = "Neramit Planner drawings:";
/// The comment Neramit Planner keeps its payment schedule in (hex of the JSON, tasks by UID).
const PAYMENTS_TAG: &str = "Neramit Planner payments:";

thread_local! {
    /// reading another project for its external task (its own links to other projects are not
    /// followed: a project linked both ways would never end)
    static READING_EXTERNAL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The external task `id` of the project at `path` as this project shows it (grey, its dates as
/// that file has them; a file that cannot be read gives its name and the project's start).
pub fn external_task(path: &str, id: u32, uid: u32, level: u8, pstart: NaiveDate, mpd: i64) -> Task {
    let file = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.to_string());
    let mut t = Task::new(uid, &format!("{file}\\{id}"), pstart);
    t.outline_level = level;
    t.manual = false;
    let mut ext_uid = id;
    if !READING_EXTERNAL.with(|r| r.get()) {
        READING_EXTERNAL.with(|r| r.set(true));
        let other = load(std::path::Path::new(path));
        READING_EXTERNAL.with(|r| r.set(false));
        if let Ok(mut q) = other {
            let _ = crate::schedule::schedule(&mut q);
            // (the standard IDs count that project's own external tasks too)
            if let Some(x) = q.tasks.get(id as usize - 1).filter(|_| id >= 1) {
                t.name = x.name.clone();
                let qmpd = q.calendar.minutes_per_day();
                t.set_duration_minutes(x.dur_minutes(qmpd), x.dur_unit, x.dur_elapsed, mpd);
                t.milestone = x.milestone;
                t.percent_complete = x.percent_complete;
                t.start_dt = x.start_dt;
                t.finish_dt = x.finish_dt;
                t.start = x.start;
                t.finish = x.finish;
                ext_uid = x.src_uid.unwrap_or(x.uid);
                // (MPP keeps the external task's WBS code with it, var 0x0B400010)
                t.wbs = if x.wbs.is_empty() { q.outline_numbers().get(id as usize - 1).cloned().unwrap_or_default() } else { x.wbs.clone() };
            }
        }
    }
    t.external = Some(crate::model::ExternalTask { path: path.to_string(), uid: ext_uid, id });
    t
}

fn tag(out: &mut String, name: &str, val: impl std::fmt::Display) {
    let _ = writeln!(out, "<{name}>{val}</{name}>");
}

fn to_xml_doc(p: &Project, path: &Path) -> String {
    let hpd = p.calendar.hours_per_day;
    let mpd = (hpd * 60.0).round() as i64;
    let work_days_week = p.calendar.working_days.iter().filter(|w| **w).count() as i64;
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("Project.xml");
    let mut o = String::new();
    o.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    o.push_str("<Project xmlns=\"http://schemas.microsoft.com/project\">\n");
    tag(&mut o, "SaveVersion", 14);
    tag(&mut o, "Name", esc(file_name));
    tag(&mut o, "Title", esc(&p.title));
    tag(&mut o, "Company", esc(&p.company));
    tag(&mut o, "Manager", esc(&p.manager));
    tag(&mut o, "Author", esc(&p.author));
    tag(&mut o, "ScheduleFromStart", 1);
    tag(&mut o, "StartDate", p.start_dt.filter(|t| t.date() == p.start).map(iso).unwrap_or_else(|| dt(p.start, "08:00:00")));
    tag(&mut o, "FinishDate", dt(p.finish_date(), "17:00:00"));
    tag(&mut o, "CalendarUID", 1);
    tag(&mut o, "DefaultStartTime", "08:00:00");
    tag(&mut o, "DefaultFinishTime", "17:00:00");
    tag(&mut o, "MinutesPerDay", mpd);
    tag(&mut o, "MinutesPerWeek", mpd * work_days_week);
    tag(&mut o, "DaysPerMonth", 20);
    tag(&mut o, "DefaultTaskType", p.options.default_task_type as u8);
    tag(&mut o, "DurationFormat", 7);
    tag(&mut o, "WorkFormat", 2);
    tag(&mut o, "NewTasksEffortDriven", p.options.new_tasks_effort_driven as u8);
    tag(&mut o, "NewTasksEstimated", p.options.new_tasks_estimated as u8);
    tag(&mut o, "SplitsInProgressTasks", p.options.split_in_progress as u8);
    tag(&mut o, "WeekStartDay", 0);
    if let Some(sd) = p.status_date {
        tag(&mut o, "StatusDate", dt(sd, "17:00:00"));
    }
    tag(&mut o, "Autolink", p.options.autolink as u8);
    // Without this flag MPP applies its own default (often "manual") while importing
    tag(&mut o, "NewTasksAreManual", (!p.new_tasks_auto) as u8);

    // (before WBSMasks and ExtendedAttributes: MPP reads the elements in the schema's order)
    {
        // lookup tables: MPP14 keep them as one-level outline codes, each field naming
        // its table (Ltuid; MPP reads no ValueList of its own)
        let lists: Vec<&crate::custom::CustomFieldDef> = p.custom_defs.iter().filter(|d| !d.values.is_empty() && crate::cfdefs::type_codes(d.field.kind).is_some()).collect();
        if !lists.is_empty() {
            o.push_str("<OutlineCodes>\n");
            let du = crate::util::DurUnits::of(p);
            let mut value_id = 0;
            for d in &lists {
                let (_, _, code, _) = crate::cfdefs::type_codes(d.field.kind).unwrap();
                o.push_str("<OutlineCode>\n");
                tag(&mut o, "Guid", crate::cfdefs::xml_table_guid(d.field.field_id(d.resource), 0));
                tag(&mut o, "Enterprise", 0);
                tag(&mut o, "ShowIndent", 1);
                tag(&mut o, "ResourceSubstitutionEnabled", 0);
                tag(&mut o, "LeafOnly", 0);
                tag(&mut o, "AllLevelsRequired", 0);
                tag(&mut o, "OnlyTableValuesAllowed", 1);
                o.push_str("<Masks>\n<Mask>\n");
                tag(&mut o, "Level", 1);
                tag(&mut o, "Type", crate::cfdefs::mask_sequence(d.field.kind));
                tag(&mut o, "Length", 0);
                tag(&mut o, "Separator", ".");
                o.push_str("</Mask>\n</Masks>\n<Values>\n");
                for (k, (v, desc)) in d.values.iter().enumerate() {
                    value_id += 1;
                    o.push_str("<Value>\n");
                    tag(&mut o, "ValueID", value_id);
                    tag(&mut o, "FieldGUID", crate::cfdefs::xml_table_guid(d.field.field_id(d.resource), k as u32 + 1));
                    tag(&mut o, "ParentValueID", 0);
                    tag(&mut o, "Type", code);
                    tag(&mut o, "IsCollapsed", 0);
                    tag(&mut o, "Value", esc(&crate::cfdefs::xml_value(d.field.kind, v, du)));
                    if !desc.is_empty() {
                        tag(&mut o, "Description", esc(desc));
                    }
                    o.push_str("</Value>\n");
                }
                o.push_str("</Values>\n</OutlineCode>\n");
            }
            o.push_str("</OutlineCodes>\n");
        }
    }

    // Project > WBS > Define Code
    if let Some(m) = &p.wbs_mask {
        o.push_str("<WBSMasks>\n");
        tag(&mut o, "VerifyUniqueCodes", m.verify_unique as u8);
        tag(&mut o, "GenerateCodes", m.generate as u8);
        tag(&mut o, "Prefix", esc(&m.prefix));
        o.push_str("<Masks>\n");
        for (k, l) in m.levels.iter().enumerate() {
            o.push_str("<WBSMask>\n");
            tag(&mut o, "Level", k + 1);
            tag(&mut o, "Type", l.seq);
            tag(&mut o, "Length", l.len);
            tag(&mut o, "Separator", esc(&l.sep));
            o.push_str("</WBSMask>\n");
        }
        o.push_str("</Masks>\n</WBSMasks>\n");
    }

    // Project > Custom Fields: the renamed / listed / calculated fields and every field in use
    {
        let mut used: Vec<(crate::custom::CustomField, bool)> = p.custom_defs.iter().map(|d| (d.field, d.resource)).collect();
        for t in &p.tasks {
            used.extend(t.custom.iter().map(|x| (x.0, false)));
        }
        for r in &p.resources {
            used.extend(r.custom.iter().map(|x| (x.0, true)));
        }
        used.sort();
        used.dedup();
        if !used.is_empty() {
            o.push_str("<ExtendedAttributes>\n");
            for (f, res) in used {
                o.push_str("<ExtendedAttribute>\n");
                tag(&mut o, "FieldID", f.field_id(res));
                tag(&mut o, "FieldName", f.name());
                if let Some(d) = crate::custom::def(p, f, res) {
                    if !d.alias.trim().is_empty() {
                        tag(&mut o, "Alias", esc(&d.alias));
                    }
                    if !d.values.is_empty() && crate::cfdefs::type_codes(f.kind).is_some() {
                        tag(&mut o, "Ltuid", crate::cfdefs::xml_table_guid(f.field_id(res), 0));
                    }
                    if !d.formula.trim().is_empty() {
                        tag(&mut o, "Formula", esc(&d.formula));
                    }
                    if d.restrict {
                        tag(&mut o, "RestrictValues", 1);
                    }
                    if !d.values.is_empty() {
                        o.push_str("<ValueList>\n");
                        for (k, (v, desc)) in d.values.iter().enumerate() {
                            o.push_str("<Value>\n");
                            tag(&mut o, "ID", k + 1);
                            tag(&mut o, "Value", esc(v));
                            tag(&mut o, "Description", esc(desc));
                            o.push_str("</Value>\n");
                        }
                        o.push_str("</ValueList>\n");
                    }
                }
                o.push_str("</ExtendedAttribute>\n");
            }
            o.push_str("</ExtendedAttributes>\n");
        }
    }

    // calendar
    o.push_str("<Calendars>\n<Calendar>\n");
    tag(&mut o, "UID", 1);
    tag(&mut o, "Name", esc(&p.calendar.name));
    tag(&mut o, "IsBaseCalendar", 1);
    tag(&mut o, "BaseCalendarUID", -1);
    o.push_str("<WeekDays>\n");
    // XML DayType: 1 = Sunday .. 7 = Saturday; our index 0 = Monday
    for day_type in 1..=7 {
        let idx = (day_type + 5) % 7; // Sunday(1)->6, Monday(2)->0 ...
        let working = p.calendar.working_days[idx];
        o.push_str("<WeekDay>\n");
        tag(&mut o, "DayType", day_type);
        tag(&mut o, "DayWorking", working as u8);
        if working {
            let times: Vec<(u16, u16)> = match p.calendar.work_times.get(idx) {
                Some(v) if !v.is_empty() => v.clone(),
                _ => STANDARD_DAY.to_vec(),
            };
            o.push_str("<WorkingTimes>\n");
            for (a, b) in times {
                let hm = |m: u16| if m >= 24 * 60 { "00:00:00".to_string() } else { format!("{:02}:{:02}:00", m / 60, m % 60) };
                let _ = writeln!(o, "<WorkingTime><FromTime>{}</FromTime><ToTime>{}</ToTime></WorkingTime>", hm(a), hm(b));
            }
            o.push_str("</WorkingTimes>\n");
        }
        o.push_str("</WeekDay>\n");
    }
    for h in &p.calendar.holidays {
        o.push_str("<WeekDay>\n");
        tag(&mut o, "DayType", 0);
        tag(&mut o, "DayWorking", 0);
        let _ = writeln!(
            o,
            "<TimePeriod><FromDate>{}</FromDate><ToDate>{}</ToDate></TimePeriod>",
            dt(h.start, "00:00:00"),
            dt(h.finish, "23:59:00")
        );
        o.push_str("</WeekDay>\n");
    }
    o.push_str("</WeekDays>\n</Calendar>\n</Calendars>\n");

    // tasks
    let numbers = p.outline_numbers();
    // sprints by the UIDs the standard files give them ("No Sprint" is 1)
    let sprint_fuid: HashMap<u32, u32> = p.sprints.iter().map(|s| s.uid).zip(crate::sprintio::file_uids(&p.sprints)).collect();
    o.push_str("<Tasks>\n");
    // project summary task (UID 0)
    o.push_str("<Task>\n");
    tag(&mut o, "UID", 0);
    tag(&mut o, "ID", 0);
    tag(&mut o, "Name", esc(if p.title.is_empty() { file_name } else { &p.title }));
    tag(&mut o, "Type", 1);
    tag(&mut o, "IsNull", 0);
    tag(&mut o, "OutlineNumber", 0);
    tag(&mut o, "OutlineLevel", 0);
    tag(&mut o, "Start", dt(p.start, "08:00:00"));
    tag(&mut o, "Finish", dt(p.finish_date(), "17:00:00"));
    tag(&mut o, "Summary", 1);
    o.push_str("</Task>\n");
    let mut xml_id = 0;
    for (i, t) in p.tasks.iter().enumerate() {
        // a task of another project is no task of this file (as MPP writes it): the links
        // from it name it (CrossProjectName)
        if t.external.is_some() {
            continue;
        }
        xml_id += 1;
        let mins = t.dur_minutes(mpd);
        o.push_str("<Task>\n");
        tag(&mut o, "UID", t.uid);
        tag(&mut o, "ID", xml_id);
        tag(&mut o, "Name", esc(&t.name));
        tag(&mut o, "Active", t.active as u8);
        tag(&mut o, "Manual", t.manual as u8);
        tag(&mut o, "Type", t.task_type.code());
        tag(&mut o, "IsNull", 0);
        if !t.contact.is_empty() {
            tag(&mut o, "Contact", esc(&t.contact));
        }
        if !t.hyperlink.is_empty() || !t.hyperlink_address.is_empty() {
            tag(&mut o, "Hyperlink", esc(&t.hyperlink));
            tag(&mut o, "HyperlinkAddress", esc(&t.hyperlink_address));
            tag(&mut o, "HyperlinkSubAddress", esc(&t.hyperlink_sub));
        }
        if !t.wbs.is_empty() {
            tag(&mut o, "WBS", esc(&t.wbs));
        }
        tag(&mut o, "OutlineNumber", &numbers[i]);
        tag(&mut o, "OutlineLevel", t.outline_level);
        tag(&mut o, "Priority", t.priority);
        if t.manual {
            for (k, name) in ["StartText", "FinishText", "DurationText"].iter().enumerate() {
                if let Some(s) = &t.manual_text[k] {
                    tag(&mut o, name, esc(s));
                }
            }
        }
        let fin_time = if t.duration == 0 { "08:00:00" } else { "17:00:00" };
        // work hours of the task = sum of its work-resource assignments
        let done_min = t.actual_minutes(mpd).min(mins);
        // Element order follows the XML schema (the standard reader is order sensitive).
        let start_s = t.start_dt.map(iso).unwrap_or_else(|| dt(t.start, "08:00:00"));
        let finish_s = t.finish_dt.map(iso).unwrap_or_else(|| dt(t.finish, fin_time));
        tag(&mut o, "Start", &start_s);
        tag(&mut o, "Finish", &finish_s);
        tag(&mut o, "Duration", iso_dur(mins));
        tag(&mut o, "ManualStart", &start_s);
        tag(&mut o, "ManualFinish", &finish_s);
        tag(&mut o, "ManualDuration", iso_dur(mins));
        // a summary keeps the format MPP gave it (mostly 21, "none"; 53 when estimated), and
        // one made in Neramit Planner gets 21 as in MPP
        let kept = t.summary_fmt.map(|f| f as i64 & 0x1F);
        let fmt = if t.is_summary {
            kept.unwrap_or(21)
        } else if kept == Some(21) {
            // a manually scheduled task without a duration format of its own ("null")
            21
        } else {
            t.dur_unit.code(t.dur_elapsed)
        };
        tag(&mut o, "DurationFormat", fmt + if t.estimated { 32 } else { 0 });
        tag(&mut o, "Work", iso_dur(t.work_min));
        tag(&mut o, "EffortDriven", t.effort_driven as u8);
        tag(&mut o, "Estimated", t.estimated as u8);
        tag(&mut o, "Milestone", t.is_milestone() as u8);
        tag(&mut o, "Summary", t.is_summary as u8);
        tag(&mut o, "Critical", t.critical as u8);
        tag(&mut o, "FixedCost", (t.fixed_cost * 100.0).round() as i64);
        tag(&mut o, "FixedCostAccrual", t.fixed_cost_accrual.code());
        tag(&mut o, "PercentComplete", t.percent_complete);
        tag(&mut o, "Cost", (t.cost * 100.0).round() as i64);
        if let Some(a) = t.actual_start {
            tag(&mut o, "ActualStart", iso(a));
            if let Some(f) = t.actual_finish {
                tag(&mut o, "ActualFinish", iso(f));
            }
        }
        // Duration = ActualDuration + RemainingDuration; omitting these makes MPP read 0 days
        tag(&mut o, "ActualDuration", iso_dur(done_min));
        tag(&mut o, "ActualCost", (t.actual_cost * 100.0).round() as i64);
        tag(&mut o, "RemainingDuration", iso_dur((mins - done_min).max(0)));
        tag(&mut o, "RemainingCost", ((t.cost - t.actual_cost).max(0.0) * 100.0).round() as i64);
        let ctype = if t.constraint_type.needs_date() && t.constraint_date.is_none() { ConstraintType::Asap } else { t.constraint_type };
        tag(&mut o, "ConstraintType", ctype.code());
        tag(&mut o, "CalendarUID", -1);
        if ctype.needs_date() {
            if let Some(c) = t.constraint_instant(&p.calendar) {
                tag(&mut o, "ConstraintDate", iso(c));
            }
        }
        if let Some(d) = t.deadline {
            tag(&mut o, "Deadline", dt(d, "17:00:00"));
        }
        if t.level_delay_min > 0 {
            tag(&mut o, "LevelingDelay", t.level_delay_min * 10);
            tag(&mut o, "LevelingDelayFormat", 7);
        }
        if !t.notes.is_empty() {
            tag(&mut o, "Notes", esc(&t.notes));
        }
        tag(&mut o, "HideBar", t.hide_bar as u8);
        tag(&mut o, "Rollup", t.rollup as u8);
        if t.physical_percent > 0 {
            tag(&mut o, "PhysicalPercentComplete", t.physical_percent);
        }
        for l in &t.predecessors {
            let ty = match l.kind {
                LinkType::FF => 0,
                LinkType::FS => 1,
                LinkType::SF => 2,
                LinkType::SS => 3,
            };
            o.push_str("<PredecessorLink>\n");
            tag(&mut o, "PredecessorUID", l.pred_uid);
            tag(&mut o, "Type", ty);
            match p.tasks.iter().find(|x| x.uid == l.pred_uid).and_then(|x| x.external.as_ref()) {
                Some(x) => {
                    tag(&mut o, "CrossProject", 1);
                    tag(&mut o, "CrossProjectName", esc(&format!("{}\\{}", x.path, x.id)));
                }
                None => tag(&mut o, "CrossProject", 0),
            }
            if let Some(pct) = l.lag_pct {
                tag(&mut o, "LinkLag", pct.round() as i64);
                tag(&mut o, "LagFormat", if l.elapsed { 20 } else { 19 });
            } else {
                tag(&mut o, "LinkLag", l.lag_minutes(mpd, 0) * 10);
                tag(&mut o, "LagFormat", l.lag_unit.code(l.elapsed));
            }
            o.push_str("</PredecessorLink>\n");
        }
        if t.sprint.is_some() || t.board_status == 1 {
            tag(&mut o, "SprintUID", t.sprint.and_then(|u| sprint_fuid.get(&u)).map(|u| *u as i64).unwrap_or(-1));
            tag(&mut o, "BoardColumnUID", if t.board_status == 1 { crate::sprintio::NEXT_UP_UID as i64 } else { -1 });
        }
        for (f, v) in &t.custom {
            o.push_str("<ExtendedAttribute>\n");
            tag(&mut o, "FieldID", f.field_id(false));
            tag(&mut o, "Value", esc(&crate::custom::xml_value(v)));
            if let crate::custom::CfValue::Duration(_) = v {
                tag(&mut o, "DurationFormat", 7);
            }
            o.push_str("</ExtendedAttribute>\n");
        }
        for b in &t.baselines {
            o.push_str("<Baseline>\n");
            tag(&mut o, "Number", b.number);
            if let Some(s) = b.start {
                tag(&mut o, "Start", iso(s));
            }
            if let Some(f) = b.finish {
                tag(&mut o, "Finish", iso(f));
            }
            if let Some(d) = b.dur_min {
                tag(&mut o, "Duration", iso_dur(d));
            }
            if let Some(w) = b.work_min {
                tag(&mut o, "Work", iso_dur(w));
            }
            if let Some(c) = b.cost {
                tag(&mut o, "Cost", (c * 100.0).round() as i64);
            }
            o.push_str("</Baseline>\n");
        }
        o.push_str("</Task>\n");
    }
    o.push_str("</Tasks>\n");

    // resources
    o.push_str("<Resources>\n");
    for (i, r) in p.resources.iter().enumerate() {
        o.push_str("<Resource>\n");
        tag(&mut o, "UID", r.uid);
        tag(&mut o, "ID", i + 1);
        tag(&mut o, "Name", esc(&r.name));
        tag(&mut o, "Type", match r.rtype { ResourceType::Material => 0, ResourceType::Work => 1, ResourceType::Cost => 2 });
        tag(&mut o, "IsNull", 0);
        tag(&mut o, "Initials", esc(&r.initials));
        if r.rtype == ResourceType::Material && !r.material_label.is_empty() {
            tag(&mut o, "MaterialLabel", esc(&r.material_label));
        }
        if !r.code.is_empty() {
            tag(&mut o, "Code", esc(&r.code));
        }
        if !r.group.is_empty() {
            tag(&mut o, "Group", esc(&r.group));
        }
        if !r.email.is_empty() {
            tag(&mut o, "EmailAddress", esc(&r.email));
        }
        if r.rtype == ResourceType::Work {
            tag(&mut o, "MaxUnits", format!("{:.2}", r.max_units / 100.0));
        }
        tag(&mut o, "AccrueAt", r.accrue_at.code());
        // MPP: a material's work is its quantity, an hour a unit; a cost resource has none
        match r.rtype {
            ResourceType::Work => tag(&mut o, "Work", iso_dur(r.work_min)),
            ResourceType::Material => {
                let q: f64 = p.tasks.iter().filter(|t| !t.is_summary).flat_map(|t| t.assignments.iter()).filter(|a| a.res_uid == r.uid).map(|a| a.units).sum();
                tag(&mut o, "Work", iso_dur((q * 60.0).round() as i64));
            }
            ResourceType::Cost => {}
        }
        tag(&mut o, "StandardRate", r.std_rate);
        // 2 = per hour, 8 = per unit of a material
        tag(&mut o, "StandardRateFormat", if r.rtype == ResourceType::Material { 8 } else { 2 });
        tag(&mut o, "Cost", (r.cost * 100.0).round() as i64);
        tag(&mut o, "OvertimeRate", r.ovt_rate);
        tag(&mut o, "OvertimeRateFormat", 2);
        // XML costs are in cents
        tag(&mut o, "CostPerUse", (r.cost_per_use * 100.0).round() as i64);
        if r.rtype == ResourceType::Cost {
            tag(&mut o, "IsCostResource", 1);
        }
        if !r.notes.is_empty() {
            tag(&mut o, "Notes", esc(&r.notes));
        }
        for (f, v) in &r.custom {
            o.push_str("<ExtendedAttribute>\n");
            tag(&mut o, "FieldID", f.field_id(true));
            tag(&mut o, "Value", esc(&crate::custom::xml_value(v)));
            o.push_str("</ExtendedAttribute>\n");
        }
        o.push_str("</Resource>\n");
    }
    o.push_str("</Resources>\n");

    // assignments
    o.push_str("<Assignments>\n");
    let mut auid = 1;
    for t in &p.tasks {
        if t.is_summary {
            continue;
        }
        let pieces = if t.splits.is_empty() { Vec::new() } else { t.segments(&p.calendar) };
        let timephased = |o: &mut String, uid: usize, work: i64| {
            // one element per piece and per gap, the standard Type 1 (planned work), Unit 1
            let total: i64 = pieces.iter().map(|(a, b)| p.calendar.work_minutes(*a, *b)).sum::<i64>().max(1);
            for (k, (a, b)) in pieces.iter().enumerate() {
                let m = p.calendar.work_minutes(*a, *b);
                let _ = writeln!(o, "<TimephasedData><Type>1</Type><UID>{uid}</UID><Start>{}</Start><Finish>{}</Finish><Unit>1</Unit><Value>{}</Value></TimephasedData>", iso(*a), iso(*b), iso_dur(work * m / total));
                if let Some((na, _)) = pieces.get(k + 1) {
                    let _ = writeln!(o, "<TimephasedData><Type>1</Type><UID>{uid}</UID><Start>{}</Start><Finish>{}</Finish><Unit>1</Unit><Value>PT0H0M0S</Value></TimephasedData>", iso(*b), iso(*na));
                }
            }
        };
        if !pieces.is_empty() && !t.assignments.iter().any(|a| p.resource_by_uid(a.res_uid).map(|r| r.rtype == ResourceType::Work).unwrap_or(false)) {
            // an unassigned split task: MPP keeps its pieces on the placeholder assignment
            o.push_str("<Assignment>\n");
            tag(&mut o, "UID", auid);
            tag(&mut o, "TaskUID", t.uid);
            tag(&mut o, "ResourceUID", -65535);
            tag(&mut o, "Finish", t.finish_dt.map(iso).unwrap_or_default());
            tag(&mut o, "Start", t.start_dt.map(iso).unwrap_or_default());
            tag(&mut o, "Units", "1.00");
            tag(&mut o, "Work", iso_dur(t.dur_minutes(mpd)));
            timephased(&mut o, auid, t.dur_minutes(mpd));
            o.push_str("</Assignment>\n");
            auid += 1;
        }
        for a in &t.assignments {
            let Some(r) = p.resource_by_uid(a.res_uid) else { continue };
            // Assignment actual work is deliberately not written: MPP then raises a modal
            // rescheduling prompt on open. Progress stays in the task-level fields.
            let fin_time = if t.duration == 0 { "08:00:00" } else { "17:00:00" };
            let cost = match r.rtype {
                ResourceType::Work => a.work_min.max(0) as f64 / 60.0 * r.std_rate + r.cost_per_use * a.units,
                ResourceType::Material => a.units * r.std_rate + r.cost_per_use,
                ResourceType::Cost => a.cost,
            };
            o.push_str("<Assignment>\n");
            tag(&mut o, "UID", auid);
            tag(&mut o, "TaskUID", t.uid);
            tag(&mut o, "ResourceUID", r.uid);
            tag(&mut o, "Cost", (cost * 100.0).round() as i64);
            tag(&mut o, "Finish", t.finish_dt.map(iso).unwrap_or_else(|| dt(t.finish, fin_time)));
            if r.rtype == ResourceType::Work {
                tag(&mut o, "RemainingWork", iso_dur(a.work_min.max(0)));
            }
            tag(&mut o, "Start", t.start_dt.map(iso).unwrap_or_else(|| dt(t.start, "08:00:00")));
            tag(&mut o, "Units", format!("{:.2}", a.units));
            match r.rtype {
                // MPP: a material's quantity is also its work, an hour a unit
                ResourceType::Material => tag(&mut o, "Work", iso_dur((a.units * 60.0).round() as i64)),
                ResourceType::Cost => tag(&mut o, "Work", iso_dur(0)),
                ResourceType::Work => {}
            }
            if r.rtype == ResourceType::Work {
                tag(&mut o, "Work", iso_dur(a.work_min.max(0)));
                if !pieces.is_empty() {
                    timephased(&mut o, auid, a.work_min.max(0));
                }
            }
            o.push_str("</Assignment>\n");
            auid += 1;
        }
    }
    o.push_str("</Assignments>\n");
    // the Task Board's columns (the standard four) and Project > Manage Sprints
    o.push_str("<BoardColumns>\n");
    for (k, name) in ["Not Started", "Next up", "In progress", "Done"].iter().enumerate() {
        o.push_str("<BoardColumn>\n");
        tag(&mut o, "UID", k + 1);
        tag(&mut o, "ID", k);
        tag(&mut o, "Name", name);
        o.push_str("</BoardColumn>\n");
    }
    o.push_str("</BoardColumns>\n<Sprints>\n<Sprint>\n");
    tag(&mut o, "UID", crate::sprintio::NO_SPRINT_UID);
    tag(&mut o, "ID", 0);
    tag(&mut o, "Name", "No Sprint");
    tag(&mut o, "DurationUnits", 39);
    tag(&mut o, "Duration", "PT0H0M0S");
    o.push_str("</Sprint>\n");
    for (k, s) in p.sprints.iter().enumerate() {
        o.push_str("<Sprint>\n");
        tag(&mut o, "UID", sprint_fuid.get(&s.uid).copied().unwrap_or(s.uid));
        tag(&mut o, "ID", k + 1);
        tag(&mut o, "Name", esc(&s.name));
        tag(&mut o, "Start", dt(s.start, "08:00:00"));
        tag(&mut o, "Finish", dt(s.finish, "17:00:00"));
        tag(&mut o, "DurationUnits", 9);
        tag(&mut o, "Duration", iso_dur(crate::sprintio::sprint_tenths(s) as i64 / 10));
        o.push_str("</Sprint>\n");
    }
    o.push_str("</Sprints>\n");
    // Format > Drawing: Neramit Planner's own, in a comment MPP passes over (hex of the JSON)
    if !p.drawings.is_empty() {
        if let Ok(j) = serde_json::to_vec(&p.drawings) {
            let hex: String = j.iter().map(|b| format!("{b:02x}")).collect();
            let _ = writeln!(o, "<!--{DRAWINGS_TAG}{hex}-->");
        }
    }
    if !p.payments.is_empty() {
        if let Ok(j) = serde_json::to_vec(&p.payments) {
            let hex: String = j.iter().map(|b| format!("{b:02x}")).collect();
            let _ = writeln!(o, "<!--{PAYMENTS_TAG}{hex}-->");
        }
    }
    o.push_str("</Project>\n");
    o
}

// ------------------------------------------------------------------ XML reader

#[derive(Default, Clone)]
struct XTask {
    /// custom fields: (field number, XML value)
    cf: Vec<(u32, String)>,
    /// Hyperlink, HyperlinkAddress, HyperlinkSubAddress
    hyperlink: (String, String, String),
    uid: i64,
    id: i64,
    name: String,
    manual: bool,
    outline_level: i64,
    start: Option<NaiveDate>,
    finish: Option<NaiveDate>,
    duration_h: f64,
    dur_fmt: i64,
    manual_start: Option<chrono::NaiveDateTime>,
    manual_finish: Option<chrono::NaiveDateTime>,
    milestone: bool,
    percent: i64,
    constraint_type: i64,
    constraint_date: Option<NaiveDate>,
    constraint_dt: Option<chrono::NaiveDateTime>,
    notes: String,
    active: bool,
    is_null: bool,
    links: Vec<(i64, i64, i64, i64)>, // pred uid, type, lag tenths of minutes, lag format
    priority: Option<i64>,
    deadline: Option<NaiveDate>,
    estimated: bool,
    hide_bar: bool,
    rollup: bool,
    ttype: i64,
    effort_driven: bool,
    wbs: String,
    contact: String,
    actual_start: Option<chrono::NaiveDateTime>,
    actual_finish: Option<chrono::NaiveDateTime>,
    actual_dur_min: Option<i64>,
    physical_percent: i64,
    baselines: Vec<crate::model::Baseline>,
    fixed_cost: f64,
    fixed_cost_accrual: i64,
    level_delay: i64,
    start_dt: Option<chrono::NaiveDateTime>,
    /// StartText, FinishText, DurationText
    texts: [Option<String>; 3],
    /// links from tasks of other projects: ("path\ID", type, lag, lag format)
    xlinks: Vec<(String, i64, i64, i64)>,
    sprint_uid: i64,
    board_column_uid: i64,
}

#[derive(Default, Clone)]
struct XRes {
    uid: i64,
    name: String,
    rtype: i64,
    initials: String,
    group: String,
    material_label: String,
    max_units: f64,
    std_rate: f64,
    ovt_rate: f64,
    cost_per_use: f64,
    is_null: bool,
    accrue_at: i64,
    code: String,
    is_cost: bool,
    notes: String,
    email: String,
    /// custom fields: (field number, XML value)
    cf: Vec<(u32, String)>,
}

#[derive(Default)]
struct XCal {
    uid: i64,
    name: String,
    days: HashMap<i64, bool>,
    /// working times per DayType (minutes from midnight)
    times: HashMap<i64, Vec<(u16, u16)>>,
    holidays: Vec<(NaiveDate, NaiveDate)>,
}

fn parse_datetime(s: &str) -> Option<chrono::NaiveDateTime> {
    chrono::NaiveDateTime::parse_from_str(s.trim(), "%Y-%m-%dT%H:%M:%S").ok()
}

fn iso(t: chrono::NaiveDateTime) -> String {
    t.format("%Y-%m-%dT%H:%M:%S").to_string()
}

/// Minutes as an XML duration (PT16H30M0S).
fn iso_dur(m: i64) -> String {
    let m = m.max(0);
    format!("PT{}H{}M0S", m / 60, m % 60)
}

fn parse_date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s.get(0..10)?, "%Y-%m-%d").ok()
}

/// Parse ISO-8601 duration like PT40H0M0S into hours.
fn parse_iso_hours(s: &str) -> f64 {
    let mut hours = 0.0;
    let mut num = String::new();
    let mut in_time = false;
    for c in s.chars() {
        match c {
            'P' => {}
            'T' => in_time = true,
            '0'..='9' | '.' => num.push(c),
            'D' => {
                hours += num.parse::<f64>().unwrap_or(0.0) * 24.0;
                num.clear();
            }
            'H' => {
                hours += num.parse::<f64>().unwrap_or(0.0);
                num.clear();
            }
            'M' if in_time => {
                hours += num.parse::<f64>().unwrap_or(0.0) / 60.0;
                num.clear();
            }
            'S' => {
                hours += num.parse::<f64>().unwrap_or(0.0) / 3600.0;
                num.clear();
            }
            _ => num.clear(),
        }
    }
    hours
}

fn load_xml_doc(bytes: &[u8]) -> Result<Project, String> {
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut path: Vec<String> = Vec::new();
    let mut text = String::new();

    let mut proj_fields: HashMap<String, String> = HashMap::new();
    let mut tasks: Vec<XTask> = Vec::new();
    let mut res: Vec<XRes> = Vec::new();
    let mut assigns: Vec<(i64, i64)> = Vec::new();
    // (task uid, resource uid) -> units, work minutes, cost
    let mut assign_detail: HashMap<(i64, i64), (f64, i64, f64)> = HashMap::new();
    let mut cur_ad = (1.0f64, -1i64, 0.0f64);
    // timephased planned work: task uid -> (start, finish, minutes)
    let mut timephased: HashMap<i64, Vec<(chrono::NaiveDateTime, chrono::NaiveDateTime, i64)>> = HashMap::new();
    let mut cur_tp: (i64, Option<chrono::NaiveDateTime>, Option<chrono::NaiveDateTime>, i64) = (0, None, None, 0);
    let mut cur_tps: Vec<(chrono::NaiveDateTime, chrono::NaiveDateTime, i64)> = Vec::new();
    let mut cals: Vec<XCal> = Vec::new();
    let mut cur_t = XTask::default();
    let mut wbs: Option<crate::wbs::WbsMask> = None;
    // custom fields: definitions, and the values of the task / resource being read
    let mut cf_defs: Vec<crate::custom::CustomFieldDef> = Vec::new();
    let mut cf_cur: (u32, String, String, String, bool, Vec<(String, String)>) = Default::default();
    // MPP14 lookup tables: outline code GUID -> values; each field's Ltuid
    let mut oc_lists: HashMap<String, Vec<(String, String)>> = HashMap::new();
    let mut oc_cur: (String, Vec<(String, String)>) = Default::default();
    let mut oc_val: (String, String, i64) = Default::default();
    let mut cf_ltuid = String::new();
    let mut cf_links: Vec<(u32, String)> = Vec::new();
    let mut cf_val: (u32, String) = (0, String::new());
    let mut cf_list_item: (String, String) = Default::default();
    let mut cur_r = XRes::default();
    let mut cur_a = (0i64, 0i64);
    let mut cur_c = XCal::default();
    let mut xml_drawings: Vec<crate::drawing::Drawing> = Vec::new();
    let mut xml_payments: Option<crate::model::PaymentPlan> = None;
    // Project > Manage Sprints: (UID, ID, name, start, finish, duration hours); board columns (UID, ID)
    let mut x_sprint: (i64, i64, String, Option<NaiveDate>, Option<NaiveDate>, i64) = Default::default();
    let mut x_sprints: Vec<(i64, i64, String, Option<NaiveDate>, Option<NaiveDate>, i64)> = Vec::new();
    let mut x_col: (i64, i64) = (0, -1);
    let mut x_next_up: i64 = crate::sprintio::NEXT_UP_UID as i64;
    let mut cur_link = (0i64, 1i64, 0i64, 7i64);
    // a link to a task of another project: (CrossProject, CrossProjectName "path\ID")
    let mut cur_cross: (bool, String) = Default::default();
    let mut cur_bl = crate::model::Baseline::default();
    let mut wd_type = -1i64;
    let mut wd_working = false;
    let mut wd_times: Vec<(u16, u16)> = Vec::new();
    let mut wt_from: Option<u16> = None;
    let mut tp_from: Option<NaiveDate> = None;
    let mut tp_to: Option<NaiveDate> = None;
    let mut saw_project = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => return Err(format!("XML error at position {}: {e}", reader.error_position())),
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if path.is_empty() && name == "Project" {
                    saw_project = true;
                }
                path.push(name.clone());
                text.clear();
                let p = path.join("/");
                match p.as_str() {
                    "Project/Tasks/Task" => cur_t = XTask { active: true, sprint_uid: -1, board_column_uid: -1, ..Default::default() },
                    "Project/Sprints/Sprint" => x_sprint = (0, -1, String::new(), None, None, 0),
                    "Project/BoardColumns/BoardColumn" => x_col = (0, -1),
                    "Project/Tasks/Task/PredecessorLink" => {
                        cur_link = (0, 1, 0, 7);
                        cur_cross = Default::default();
                    }
                    "Project/Tasks/Task/Baseline" => cur_bl = crate::model::Baseline::default(),
                    "Project/Resources/Resource" => cur_r = XRes { max_units: 1.0, ..Default::default() },
                    "Project/Assignments/Assignment" => {
                        cur_a = (0, 0);
                        cur_ad = (1.0, -1, 0.0);
                    }
                    "Project/Assignments/Assignment/TimephasedData" => cur_tp = (0, None, None, 0),
                    "Project/Calendars/Calendar" => cur_c = XCal::default(),
                    "Project/Calendars/Calendar/WeekDays/WeekDay" | "Project/Calendars/Calendar/Exceptions/Exception" => {
                        wd_type = -1;
                        wd_working = false;
                        wd_times.clear();
                        tp_from = None;
                        tp_to = None;
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(e)) => {
                text.push_str(&e.unescape().map_err(|e| e.to_string())?);
            }
            Ok(Event::CData(e)) => {
                text.push_str(&String::from_utf8_lossy(&e));
            }
            Ok(Event::Comment(e)) => {
                let c = String::from_utf8_lossy(&e).to_string();
                // files saved under an earlier program name carry "<name> drawings:"
                if let Some((_, hex)) = c.trim().split_once(" drawings:") {
                    let bytes: Vec<u8> = (0..hex.len() / 2).filter_map(|i| u8::from_str_radix(hex.get(2 * i..2 * i + 2)?, 16).ok()).collect();
                    xml_drawings = serde_json::from_slice(&bytes).unwrap_or_default();
                } else if let Some((_, hex)) = c.trim().split_once(" payments:") {
                    let bytes: Vec<u8> = (0..hex.len() / 2).filter_map(|i| u8::from_str_radix(hex.get(2 * i..2 * i + 2)?, 16).ok()).collect();
                    xml_payments = serde_json::from_slice(&bytes).ok();
                }
            }
            Ok(Event::End(_)) => {
                let p = path.join("/");
                let v = text.trim().to_string();
                let leaf = path.last().cloned().unwrap_or_default();
                let num = || v.parse::<f64>().unwrap_or(0.0);
                let int = || v.parse::<f64>().map(|f| f as i64).unwrap_or(0);
                let parent = if path.len() >= 2 { path[..path.len() - 1].join("/") } else { String::new() };
                match parent.as_str() {
                    "Project" if !leaf.is_empty() => {
                        proj_fields.insert(leaf.clone(), v.clone());
                    }
                    "Project/OutlineCodes/OutlineCode" if leaf == "Guid" => oc_cur.0 = v.to_uppercase(),
                    "Project/OutlineCodes/OutlineCode/Values/Value" => match leaf.as_str() {
                        "Value" => oc_val.0 = v.clone(),
                        "Description" => oc_val.1 = v.clone(),
                        "Type" => oc_val.2 = int(),
                        _ => {}
                    },
                    "Project/OutlineCodes/OutlineCode/Values" if leaf == "Value" => {
                        let (value, desc, ty) = std::mem::take(&mut oc_val);
                        oc_cur.1.push((crate::cfdefs::from_xml_value(ty, &value), desc));
                    }
                    "Project/OutlineCodes" if leaf == "OutlineCode" => {
                        let (g, vals) = std::mem::take(&mut oc_cur);
                        oc_lists.insert(g, vals);
                    }
                    "Project/ExtendedAttributes/ExtendedAttribute" if leaf == "Ltuid" => cf_ltuid = v.to_uppercase(),
                    "Project/ExtendedAttributes/ExtendedAttribute" => match leaf.as_str() {
                        "FieldID" => cf_cur.0 = int() as u32,
                        "FieldName" => cf_cur.1 = v.clone(),
                        "Alias" => cf_cur.2 = v.clone(),
                        "Formula" => cf_cur.3 = v.clone(),
                        "RestrictValues" => cf_cur.4 = v == "1",
                        _ => {}
                    },
                    "Project/ExtendedAttributes/ExtendedAttribute/ValueList/Value" => match leaf.as_str() {
                        "Value" => cf_list_item.0 = v.clone(),
                        "Description" => cf_list_item.1 = v.clone(),
                        _ => {}
                    },
                    "Project/ExtendedAttributes/ExtendedAttribute/ValueList" if leaf == "Value" => {
                        cf_cur.5.push(std::mem::take(&mut cf_list_item));
                    }
                    "Project/ExtendedAttributes" if leaf == "ExtendedAttribute" => {
                        let c = std::mem::take(&mut cf_cur);
                        let lt = std::mem::take(&mut cf_ltuid);
                        if !lt.is_empty() {
                            cf_links.push((c.0, lt));
                        }
                        if let Some((f, res)) = crate::custom::by_field_id(c.0) {
                            if !c.2.is_empty() || !c.3.is_empty() || !c.5.is_empty() {
                                let mut d = crate::custom::CustomFieldDef::new(f, res);
                                d.alias = c.2;
                                d.formula = c.3;
                                d.restrict = c.4;
                                d.values = c.5;
                                cf_defs.push(d);
                            }
                        }
                    }
                    "Project/Tasks/Task/ExtendedAttribute" | "Project/Resources/Resource/ExtendedAttribute" => match leaf.as_str() {
                        "FieldID" => cf_val.0 = int() as u32,
                        "Value" => cf_val.1 = v.clone(),
                        _ => {}
                    },
                    "Project/Tasks/Task" if leaf == "ExtendedAttribute" => cur_t.cf.push(std::mem::take(&mut cf_val)),
                    "Project/Resources/Resource" if leaf == "ExtendedAttribute" => cur_r.cf.push(std::mem::take(&mut cf_val)),
                    "Project/WBSMasks" => {
                        let m = wbs.get_or_insert_with(|| crate::wbs::WbsMask { levels: Vec::new(), ..Default::default() });
                        match leaf.as_str() {
                            "VerifyUniqueCodes" => m.verify_unique = v == "1",
                            "GenerateCodes" => m.generate = v == "1",
                            "Prefix" => m.prefix = v.clone(),
                            _ => {}
                        }
                    }
                    "Project/WBSMasks/Masks/WBSMask" => {
                        let m = wbs.get_or_insert_with(|| crate::wbs::WbsMask { levels: Vec::new(), ..Default::default() });
                        match leaf.as_str() {
                            "Level" => m.levels.push(crate::wbs::WbsLevel { seq: 0, len: 0, sep: ".".into() }),
                            "Type" => {
                                if let Some(l) = m.levels.last_mut() {
                                    l.seq = int().clamp(0, 3) as u8;
                                }
                            }
                            "Length" => {
                                if let Some(l) = m.levels.last_mut() {
                                    l.len = int().clamp(0, 10) as u8;
                                }
                            }
                            "Separator" => {
                                if let Some(l) = m.levels.last_mut() {
                                    l.sep = v.clone();
                                }
                            }
                            _ => {}
                        }
                    }
                    "Project/Sprints/Sprint" => match leaf.as_str() {
                        "UID" => x_sprint.0 = int(),
                        "ID" => x_sprint.1 = int(),
                        "Name" => x_sprint.2 = v.clone(),
                        "Start" => x_sprint.3 = parse_date(&v),
                        "Finish" => x_sprint.4 = parse_date(&v),
                        "Duration" => x_sprint.5 = parse_iso_hours(&v).round() as i64,
                        _ => {}
                    },
                    "Project/Sprints" if leaf == "Sprint" => x_sprints.push(std::mem::take(&mut x_sprint)),
                    "Project/BoardColumns/BoardColumn" => match leaf.as_str() {
                        "UID" => x_col.0 = int(),
                        "ID" => x_col.1 = int(),
                        _ => {}
                    },
                    "Project/BoardColumns" if leaf == "BoardColumn" => {
                        if x_col.1 == 1 {
                            x_next_up = x_col.0;
                        }
                    }
                    "Project/Tasks/Task" => match leaf.as_str() {
                        "SprintUID" => cur_t.sprint_uid = int(),
                        "BoardColumnUID" => cur_t.board_column_uid = int(),
                        "UID" => cur_t.uid = int(),
                        "ID" => cur_t.id = int(),
                        "Name" => cur_t.name = v.clone(),
                        "Manual" => cur_t.manual = v == "1",
                        "OutlineLevel" => cur_t.outline_level = int(),
                        "Start" => {
                            cur_t.start = parse_date(&v);
                            cur_t.start_dt = parse_datetime(&v);
                        }
                        "Finish" => cur_t.finish = parse_date(&v),
                        "Duration" => cur_t.duration_h = parse_iso_hours(&v),
                        "DurationFormat" => cur_t.dur_fmt = int(),
                        "ManualStart" => cur_t.manual_start = parse_datetime(&v),
                        "StartText" => cur_t.texts[0] = Some(v.clone()),
                        "FinishText" => cur_t.texts[1] = Some(v.clone()),
                        "DurationText" => cur_t.texts[2] = Some(v.clone()),
                        "ManualFinish" => cur_t.manual_finish = parse_datetime(&v),
                        "Milestone" => cur_t.milestone = v == "1",
                        "PercentComplete" => cur_t.percent = int(),
                        "ConstraintType" => cur_t.constraint_type = int(),
                        "ConstraintDate" => {
                            cur_t.constraint_date = parse_date(&v);
                            cur_t.constraint_dt = parse_datetime(&v);
                        }
                        "Notes" => cur_t.notes = v.clone(),
                        "Active" => cur_t.active = v != "0",
                        "IsNull" => cur_t.is_null = v == "1",
                        "Priority" => cur_t.priority = Some(int()),
                        "Deadline" => cur_t.deadline = parse_date(&v),
                        "Estimated" => cur_t.estimated = v == "1",
                        "HideBar" => cur_t.hide_bar = v == "1",
                        "Rollup" => cur_t.rollup = v == "1",
                        "Type" => cur_t.ttype = int(),
                        "EffortDriven" => cur_t.effort_driven = v == "1",
                        "WBS" => cur_t.wbs = v.clone(),
                        "Contact" => cur_t.contact = v.clone(),
                        "Hyperlink" => cur_t.hyperlink.0 = v.clone(),
                        "HyperlinkAddress" => cur_t.hyperlink.1 = v.clone(),
                        "HyperlinkSubAddress" => cur_t.hyperlink.2 = v.clone(),
                        "ActualStart" => cur_t.actual_start = parse_datetime(&v),
                        "ActualFinish" => cur_t.actual_finish = parse_datetime(&v),
                        "ActualDuration" => cur_t.actual_dur_min = Some((parse_iso_hours(&v) * 60.0).round() as i64),
                        "PhysicalPercentComplete" => cur_t.physical_percent = int(),
                        "FixedCost" => cur_t.fixed_cost = num() / 100.0,
                        "FixedCostAccrual" => cur_t.fixed_cost_accrual = int(),
                        "LevelingDelay" => cur_t.level_delay = int(),
                        _ => {}
                    },
                    "Project/Tasks/Task/Baseline" => match leaf.as_str() {
                        "Number" => cur_bl.number = int().clamp(0, 10) as u8,
                        "Start" => cur_bl.start = parse_datetime(&v),
                        "Finish" => cur_bl.finish = parse_datetime(&v),
                        "Duration" => cur_bl.dur_min = Some((parse_iso_hours(&v) * 60.0).round() as i64),
                        "Work" => cur_bl.work_min = Some((parse_iso_hours(&v) * 60.0).round() as i64),
                        // XML costs are in cents
                        "Cost" => cur_bl.cost = Some(num() / 100.0),
                        _ => {}
                    },
                    "Project/Tasks/Task/PredecessorLink" => match leaf.as_str() {
                        "PredecessorUID" => cur_link.0 = int(),
                        "CrossProject" => cur_cross.0 = v == "1",
                        "CrossProjectName" => cur_cross.1 = v.clone(),
                        "Type" => cur_link.1 = int(),
                        "LinkLag" => cur_link.2 = int(),
                        "LagFormat" => cur_link.3 = int(),
                        _ => {}
                    },
                    "Project/Resources/Resource" => match leaf.as_str() {
                        "UID" => cur_r.uid = int(),
                        "Name" => cur_r.name = v.clone(),
                        "Type" => cur_r.rtype = int(),
                        "Initials" => cur_r.initials = v.clone(),
                        "Group" => cur_r.group = v.clone(),
                        "MaterialLabel" => cur_r.material_label = v.clone(),
                        "MaxUnits" => cur_r.max_units = num(),
                        "StandardRate" => cur_r.std_rate = num(),
                        "OvertimeRate" => cur_r.ovt_rate = num(),
                        // XML costs are in cents
                        "CostPerUse" => cur_r.cost_per_use = num() / 100.0,
                        "AccrueAt" => cur_r.accrue_at = int(),
                        "Code" => cur_r.code = v.clone(),
                        "Notes" => cur_r.notes = v.clone(),
                        "EmailAddress" => cur_r.email = v.clone(),
                        "IsCostResource" => cur_r.is_cost = v == "1",
                        "IsNull" => cur_r.is_null = v == "1",
                        _ => {}
                    },
                    "Project/Assignments/Assignment" => match leaf.as_str() {
                        "TaskUID" => cur_a.0 = int(),
                        "ResourceUID" => cur_a.1 = int(),
                        "Units" => cur_ad.0 = num(),
                        "Work" => cur_ad.1 = (parse_iso_hours(&v) * 60.0).round() as i64,
                        "Cost" => cur_ad.2 = num() / 100.0,
                        _ => {}
                    },
                    "Project/Assignments/Assignment/TimephasedData" => match leaf.as_str() {
                        "Type" => cur_tp.0 = int(),
                        "Start" => cur_tp.1 = parse_datetime(&v),
                        "Finish" => cur_tp.2 = parse_datetime(&v),
                        "Value" => cur_tp.3 = (parse_iso_hours(&v) * 60.0).round() as i64,
                        _ => {}
                    },
                    "Project/Calendars/Calendar" => match leaf.as_str() {
                        "UID" => cur_c.uid = int(),
                        "Name" => cur_c.name = v.clone(),
                        _ => {}
                    },
                    "Project/Calendars/Calendar/WeekDays/WeekDay" | "Project/Calendars/Calendar/Exceptions/Exception" => match leaf.as_str() {
                        "DayType" => wd_type = int(),
                        "DayWorking" => wd_working = v == "1",
                        _ => {}
                    },
                    "Project/Calendars/Calendar/WeekDays/WeekDay/WorkingTimes/WorkingTime" => {
                        let minutes = |s: &str| -> Option<u16> {
                            let mut it = s.trim().split(':');
                            let h: u16 = it.next()?.parse().ok()?;
                            let m: u16 = it.next()?.parse().ok()?;
                            Some(h * 60 + m)
                        };
                        match leaf.as_str() {
                            "FromTime" => wt_from = minutes(&v),
                            "ToTime" => {
                                if let (Some(a), Some(b)) = (wt_from, minutes(&v)) {
                                    // "00:00" as an end is midnight at the end of the day
                                    let b = if b == 0 { 24 * 60 } else { b };
                                    if b > a {
                                        wd_times.push((a, b));
                                    }
                                }
                                wt_from = None;
                            }
                            _ => {}
                        }
                    }
                    "Project/Calendars/Calendar/WeekDays/WeekDay/TimePeriod" | "Project/Calendars/Calendar/Exceptions/Exception/TimePeriod" => {
                        match leaf.as_str() {
                            "FromDate" => tp_from = parse_date(&v),
                            "ToDate" => tp_to = parse_date(&v),
                            _ => {}
                        }
                    }
                    _ => {}
                }
                match p.as_str() {
                    "Project/Tasks/Task" => tasks.push(std::mem::take(&mut cur_t)),
                    "Project/Tasks/Task/PredecessorLink" => {
                        if cur_cross.0 && !cur_cross.1.is_empty() {
                            cur_t.xlinks.push((std::mem::take(&mut cur_cross.1), cur_link.1, cur_link.2, cur_link.3));
                        } else {
                            cur_t.links.push(cur_link);
                        }
                    }
                    "Project/Tasks/Task/Baseline" => {
                        if cur_bl.start.is_some() || cur_bl.finish.is_some() || cur_bl.dur_min.is_some() {
                            cur_t.baselines.push(std::mem::take(&mut cur_bl));
                        }
                    }
                    "Project/Resources/Resource" => res.push(std::mem::take(&mut cur_r)),
                    "Project/Assignments/Assignment" => {
                        assigns.push(cur_a);
                        assign_detail.insert(cur_a, cur_ad);
                        // the task's first assignment carries its pieces
                        let tps = std::mem::take(&mut cur_tps);
                        if !tps.is_empty() {
                            timephased.entry(cur_a.0).or_insert(tps);
                        }
                    }
                    "Project/Assignments/Assignment/TimephasedData" => {
                        // planned work of the first assignment of the task seen (type 1)
                        if let (1, Some(a), Some(b)) = (cur_tp.0, cur_tp.1, cur_tp.2) {
                            cur_tps.push((a, b, cur_tp.3));
                        }
                    }
                    "Project/Calendars/Calendar" => cals.push(std::mem::take(&mut cur_c)),
                    "Project/Calendars/Calendar/WeekDays/WeekDay" => {
                        if (1..=7).contains(&wd_type) {
                            cur_c.days.insert(wd_type, wd_working);
                            if wd_working && !wd_times.is_empty() {
                                cur_c.times.insert(wd_type, wd_times.clone());
                            }
                        } else if wd_type == 0 && !wd_working {
                            if let (Some(a), Some(b)) = (tp_from, tp_to) {
                                cur_c.holidays.push((a, b));
                            }
                        }
                    }
                    "Project/Calendars/Calendar/Exceptions/Exception" => {
                        if !wd_working {
                            if let (Some(a), Some(b)) = (tp_from, tp_to) {
                                cur_c.holidays.push((a, b));
                            }
                        }
                    }
                    _ => {}
                }
                path.pop();
                text.clear();
            }
            _ => {}
        }
        buf.clear();
    }
    if !saw_project {
        return Err("This XML file is not a project XML document.".into());
    }

    let mut p = Project::new();
    p.wbs_mask = wbs.filter(|m| !m.levels.is_empty() || !m.prefix.is_empty());
    // a field's lookup table among the outline codes
    for (id, lt) in cf_links {
        let Some(vals) = oc_lists.get(&lt).filter(|v| !v.is_empty()) else { continue };
        let Some((f, res)) = crate::custom::by_field_id(id) else { continue };
        match cf_defs.iter_mut().find(|d| d.field == f && d.resource == res) {
            Some(d) if d.values.is_empty() => d.values = vals.clone(),
            Some(_) => {}
            None => {
                let mut d = crate::custom::CustomFieldDef::new(f, res);
                d.values = vals.clone();
                cf_defs.push(d);
            }
        }
    }
    p.custom_defs = cf_defs;
    p.drawings = std::mem::take(&mut xml_drawings);
    // sprints in ID order ("No Sprint", ID 0, left out); one without dates follows the one before
    x_sprints.sort_by_key(|s| s.1);
    let mut next = p.calendar.next_working(p.start);
    for (uid, id, name, start, finish, hours) in x_sprints {
        if id <= 0 || uid <= crate::sprintio::NO_SPRINT_UID as i64 {
            continue;
        }
        let s = start.unwrap_or(next);
        let f = finish.unwrap_or_else(|| crate::sprintio::sprint_end(&p.calendar, s, hours * 60)).max(s);
        p.sprints.push(crate::model::Sprint { uid: uid as u32, name, start: s, finish: f });
        next = p.calendar.next_working(f + chrono::Duration::days(1));
    }
    let sprint_uids: Vec<u32> = p.sprints.iter().map(|s| s.uid).collect();
    p.title = proj_fields.get("Title").cloned().unwrap_or_default();
    p.author = proj_fields.get("Author").cloned().unwrap_or_default();
    p.company = proj_fields.get("Company").cloned().unwrap_or_default();
    p.manager = proj_fields.get("Manager").cloned().unwrap_or_default();
    if let Some(d) = proj_fields.get("StartDate").and_then(|s| parse_date(s)) {
        p.start = d;
        p.start_dt = proj_fields.get("StartDate").and_then(|s| parse_datetime(s));
    }
    if let Some(d) = proj_fields.get("StatusDate").and_then(|s| parse_date(s)) {
        p.status_date = Some(d);
    }
    let mpd = proj_fields.get("MinutesPerDay").and_then(|s| s.parse::<f64>().ok()).filter(|v| *v > 0.0).unwrap_or(480.0);
    p.calendar.hours_per_day = mpd / 60.0;
    if let Some(nm) = proj_fields.get("NewTasksAreManual") {
        p.new_tasks_auto = nm != "1";
    }
    if let Some(t) = proj_fields.get("DefaultTaskType") {
        p.options.default_task_type = match t.as_str() {
            "1" => TaskType::FixedDuration,
            "2" => TaskType::FixedWork,
            _ => TaskType::FixedUnits,
        };
    }
    let cal_uid = proj_fields.get("CalendarUID").and_then(|s| s.parse::<i64>().ok()).unwrap_or(1);
    if let Some(c) = cals.iter().find(|c| c.uid == cal_uid).or(cals.first()) {
        if !c.name.is_empty() {
            p.calendar.name = c.name.clone();
        }
        p.calendar.work_times = vec![Vec::new(); 7];
        for (dt, w) in &c.days {
            let idx = ((*dt + 5) % 7) as usize;
            p.calendar.working_days[idx] = *w;
            if let Some(t) = c.times.get(dt) {
                p.calendar.work_times[idx] = t.clone();
            }
        }
        // MPP writes each exception twice (legacy WeekDay entry + Exceptions element)
        for (a, b) in &c.holidays {
            if !p.calendar.holidays.iter().any(|h| h.start == *a && h.finish == *b) {
                p.calendar.holidays.push(Holiday { name: "Exception".into(), start: *a, finish: *b });
            }
        }
    }

    // resources
    let mut res_map: HashMap<i64, u32> = HashMap::new();
    for r in res.iter().filter(|r| !r.is_null && r.uid > 0 && !r.name.is_empty()) {
        let uid = p.alloc_res_uid();
        res_map.insert(r.uid, uid);
        let mut nr = Resource::new(uid, &r.name);
        nr.rtype = match r.rtype {
            0 => ResourceType::Material,
            2 => ResourceType::Cost,
            _ if r.is_cost => ResourceType::Cost,
            _ => ResourceType::Work,
        };
        nr.accrue_at = Accrue::from_code(r.accrue_at);
        nr.code = r.code.clone();
        nr.notes = r.notes.clone();
        nr.custom = custom_values(&r.cf, true);
        nr.email = r.email.clone();
        if !r.initials.is_empty() {
            nr.initials = r.initials.clone();
        }
        nr.group = r.group.clone();
        nr.material_label = r.material_label.clone();
        nr.max_units = r.max_units * 100.0;
        nr.std_rate = r.std_rate;
        nr.ovt_rate = r.ovt_rate;
        nr.cost_per_use = r.cost_per_use;
        p.resources.push(nr);
    }

    // tasks (ordered by ID)
    tasks.retain(|t| t.outline_level >= 1 && !t.is_null);
    tasks.sort_by_key(|t| t.id);
    let mut uid_map: HashMap<i64, u32> = HashMap::new();
    for xt in &tasks {
        let uid = p.alloc_uid();
        uid_map.insert(xt.uid, uid);
    }
    if let Some(mut plan) = xml_payments.take() {
        for it in plan.items.iter_mut() {
            it.task_uid = it.task_uid.and_then(|u| uid_map.get(&(u as i64)).copied());
        }
        p.payments = plan;
    }
    let hpd = p.calendar.hours_per_day;
    for xt in &tasks {
        let start = xt.start.unwrap_or(p.start);
        let mut t = Task::new(uid_map[&xt.uid], &xt.name, start);
        t.custom = custom_values(&xt.cf, false);
        (t.hyperlink, t.hyperlink_address, t.hyperlink_sub) = xt.hyperlink.clone();
        t.manual = xt.manual;
        t.outline_level = xt.outline_level.clamp(1, 250) as u8;
        let (unit, elapsed) = DurUnit::from_code(if xt.dur_fmt == 0 { 7 } else { xt.dur_fmt });
        t.set_duration_minutes((xt.duration_h * 60.0).round() as i64, unit, elapsed, (hpd * 60.0).round() as i64);
        if t.manual {
            t.start_dt = xt.manual_start;
            t.finish_dt = xt.manual_finish;
            t.manual_text = xt.texts.clone();
            t.sync_manual_text();
        }
        t.finish = xt.finish.unwrap_or(start);
        t.milestone = xt.milestone && t.duration == 0;
        t.percent_complete = xt.percent.clamp(0, 100) as u8;
        t.notes = xt.notes.clone();
        t.active = xt.active;
        t.priority = xt.priority.unwrap_or(500).clamp(0, 1000) as u16;
        t.deadline = xt.deadline;
        t.estimated = xt.estimated;
        t.hide_bar = xt.hide_bar;
        t.rollup = xt.rollup;
        t.task_type = TaskType::from_code(xt.ttype);
        t.effort_driven = xt.effort_driven;
        t.wbs = xt.wbs.clone();
        t.contact = xt.contact.clone();
        t.actual_start = xt.actual_start;
        t.actual_finish = xt.actual_finish.filter(|_| xt.actual_start.is_some());
        t.actual_dur_min = xt.actual_dur_min.filter(|m| *m > 0);
        t.physical_percent = xt.physical_percent.clamp(0, 100) as u8;
        t.baselines = xt.baselines.clone();
        if xt.sprint_uid > crate::sprintio::NO_SPRINT_UID as i64 && sprint_uids.contains(&(xt.sprint_uid as u32)) {
            t.sprint = Some(xt.sprint_uid as u32);
        }
        if xt.board_column_uid >= 0 && xt.board_column_uid == x_next_up {
            t.board_status = 1;
        }
        t.baselines.sort_by_key(|b| b.number);
        if !t.manual {
            t.constraint_type = ConstraintType::from_code(xt.constraint_type);
            t.constraint_date = xt.constraint_date;
            t.constraint_dt = xt.constraint_dt;
            t.normalize_constraint();
        }
        for (pu, ty, lag, fmt) in &xt.links {
            let Some(pred) = uid_map.get(pu) else { continue };
            let kind = match ty {
                0 => LinkType::FF,
                2 => LinkType::SF,
                3 => LinkType::SS,
                _ => LinkType::FS,
            };
            let mut link = Link::new(*pred, kind);
            if matches!(fmt, 19 | 20) {
                link.lag_pct = Some(*lag as f64);
            } else {
                let (unit, el) = DurUnit::from_code(*fmt);
                link.set_lag_minutes(*lag / 10, unit, el, (hpd * 60.0).round() as i64);
            }
            t.predecessors.push(link);
            // elapsed formats (em, eh, ed, ew, emo) count 24-hour days
            let elapsed = matches!(fmt, 4 | 6 | 8 | 10 | 12);
            let _lag_days = if matches!(fmt, 19 | 20) {
                // percent of the predecessor's duration
                let pd = tasks.iter().find(|x| x.uid == *pu).map(|x| (x.duration_h / hpd).round()).unwrap_or(0.0);
                (pd * *lag as f64 / 100.0).round() as i32
            } else if elapsed {
                (*lag as f64 / (1440.0 * 10.0)).round() as i32
            } else {
                (*lag as f64 / (mpd * 10.0)).round() as i32
            };
            let _ = (_lag_days, elapsed);
        }
        for (tu, ru) in &assigns {
            if *tu == xt.uid {
                if let Some(r) = res_map.get(ru) {
                    if !t.resources.contains(r) {
                        t.resources.push(*r);
                        let (units, work, cost) = assign_detail.get(&(*tu, *ru)).copied().unwrap_or((1.0, -1, 0.0));
                        t.assignments.push(crate::model::Assignment { res_uid: *r, units, work_min: work, cost });
                    }
                }
            }
        }
        // splits: periods without work inside the planned work that cover working time
        if let (Some(segs), Some(st)) = (timephased.get(&xt.uid), xt.start_dt) {
            let mut segs = segs.clone();
            segs.sort_by_key(|s| s.0);
            let first = segs.iter().position(|s| s.2 > 0);
            let last = segs.iter().rposition(|s| s.2 > 0);
            if let (Some(fw), Some(lw)) = (first, last) {
                let mut gaps_before = 0;
                for s in segs.get(fw + 1..lw).unwrap_or(&[]) {
                    if s.2 == 0 {
                        let gap = p.calendar.work_minutes(s.0, s.1);
                        if gap > 0 {
                            let pos = p.calendar.work_minutes(st, s.0) - gaps_before;
                            if pos > 0 {
                                t.splits.push((pos, gap));
                                gaps_before += gap;
                            }
                        }
                    }
                }
            }
        }
        t.summary_fmt = Some((xt.dur_fmt & 0x3F) as u8).filter(|f| *f > 0);
        t.fixed_cost = xt.fixed_cost;
        t.fixed_cost_accrual = Accrue::from_code(xt.fixed_cost_accrual);
        t.level_delay_min = xt.level_delay / 10;
        if !t.assignments.is_empty() {
            t.work_ref_dur = t.dur_minutes(p.calendar.minutes_per_day());
        }
        p.tasks.push(t);
    }
    // links to tasks of other projects (Project XML names them "path\ID"): their external tasks,
    // read from those files, above the first task they drive
    {
        let hpd_min = (hpd * 60.0).round() as i64;
        let mut ghosts: Vec<(String, u32, u32)> = Vec::new(); // (path, id, uid here)
        let mut inserts: Vec<(usize, Task)> = Vec::new();
        for xt in &tasks {
            let Some(&my_uid) = uid_map.get(&xt.uid) else { continue };
            for (name, ty, lag, fmt) in &xt.xlinks {
                let Some((path, id)) = name.rsplit_once('\\').and_then(|(a, b)| Some((a.to_string(), b.trim().parse::<u32>().ok()?))) else { continue };
                let ghost_uid = match ghosts.iter().find(|g| g.0.eq_ignore_ascii_case(&path) && g.1 == id) {
                    Some(g) => g.2,
                    None => {
                        let uid = p.alloc_uid();
                        let at = p.tasks.iter().position(|t| t.uid == my_uid).unwrap_or(p.tasks.len());
                        let level = p.tasks.get(at).map(|t| t.outline_level).unwrap_or(1);
                        inserts.push((at, external_task(&path, id, uid, level, p.start, hpd_min)));
                        ghosts.push((path.clone(), id, uid));
                        uid
                    }
                };
                let kind = match ty {
                    0 => LinkType::FF,
                    2 => LinkType::SF,
                    3 => LinkType::SS,
                    _ => LinkType::FS,
                };
                let mut link = Link::new(ghost_uid, kind);
                if matches!(fmt, 19 | 20) {
                    link.lag_pct = Some(*lag as f64);
                } else {
                    let (unit, el) = DurUnit::from_code(*fmt);
                    link.set_lag_minutes(*lag / 10, unit, el, hpd_min);
                }
                if let Some(t) = p.tasks.iter_mut().find(|t| t.uid == my_uid) {
                    t.predecessors.push(link);
                }
            }
        }
        inserts.sort_by_key(|x| std::cmp::Reverse(x.0));
        for (at, g) in inserts {
            p.tasks.insert(at.min(p.tasks.len()), g);
        }
    }
    if let Some(v) = proj_fields.get("NewTasksEstimated") {
        p.options.new_tasks_estimated = v == "1";
    }
    if let Some(v) = proj_fields.get("NewTasksEffortDriven") {
        p.options.new_tasks_effort_driven = v == "1";
    }
    if let Some(v) = proj_fields.get("SplitsInProgressTasks") {
        p.options.split_in_progress = v == "1";
    }
    if let Some(v) = proj_fields.get("Autolink") {
        p.options.autolink = v == "1";
    }
    if let Some(v) = proj_fields.get("CurrencySymbol") {
        if !v.is_empty() {
            p.options.currency_symbol = v.clone();
        }
    }
    if let Some(v) = proj_fields.get("WeekStartDay").and_then(|s| s.parse::<u8>().ok()) {
        p.options.week_starts_on = v.saturating_sub(1).min(6);
    }
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_doc_roundtrip() {
        let mut p = Project::new();
        p.start = NaiveDate::from_ymd_opt(2026, 10, 19).unwrap();
        p.title = "Test & <Demo>".into();
        let mut a = p.new_task("Design");
        a.duration = 3;
        let mut b = p.new_task("Build");
        b.duration = 5;
        b.predecessors.push(Link { pred_uid: a.uid, kind: LinkType::FS, lag: 2, elapsed: false, ..Default::default() });
        let r = Resource::new(p.alloc_res_uid(), "สมชาย");
        b.resources.push(r.uid);
        p.resources.push(r);
        p.tasks.push(a);
        p.tasks.push(b);
        crate::schedule::schedule(&mut p).unwrap();
        let xml = to_xml_doc(&p, Path::new("x.xml"));
        let mut q = load_xml_doc(xml.as_bytes()).unwrap();
        crate::schedule::schedule(&mut q).unwrap();
        assert_eq!(q.title, p.title);
        assert_eq!(q.tasks.len(), 2);
        assert_eq!(q.tasks[1].duration, 5);
        assert_eq!(q.tasks[1].predecessors[0].lag, 2);
        assert_eq!(q.tasks[1].start, p.tasks[1].start);
        assert_eq!(q.resources[0].name, "สมชาย");
        assert_eq!(q.tasks[1].resources.len(), 1);
    }
}
