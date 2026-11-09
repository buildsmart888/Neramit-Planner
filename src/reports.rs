//! Report tab: the standard built-in reports drawn as a page - dashboards (Project, Cost and Work
//! Overview, Upcoming Tasks, Burndown), resources (Resource Overview, Overallocated Resources),
//! costs (Cash Flow, Cost Overruns, Earned Value, Task / Resource Cost Overview), in progress
//! (Critical, Late and Slipping Tasks, Milestones), the task board's status and a custom table of
//! the current columns. The numbers come from the schedule; nothing is edited here.

use crate::app::App;
use crate::i18n::tr;
use crate::model::{ResourceType, Task};
use crate::theme;
use chrono::{Datelike, NaiveDate};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum ReportKind {
    ProjectOverview,
    CostOverview,
    WorkOverview,
    UpcomingTasks,
    Burndown,
    ResourceOverview,
    OverallocatedResources,
    CashFlow,
    CostOverruns,
    EarnedValue,
    TaskCostOverview,
    ResourceCostOverview,
    CriticalTasks,
    LateTasks,
    MilestoneReport,
    SlippingTasks,
    BoardStatus,
    GettingStarted,
    CustomTable,
    /// Report > Compare Projects
    Comparison,
    /// cumulative planned against actual progress, weighted (In Progress)
    SCurve,
    /// installments of the contract (Costs)
    Payments,
}

impl ReportKind {
    pub fn label(self) -> &'static str {
        match self {
            ReportKind::ProjectOverview => "Project Overview",
            ReportKind::CostOverview => "Cost Overview",
            ReportKind::WorkOverview => "Work Overview",
            ReportKind::UpcomingTasks => "Upcoming Tasks",
            ReportKind::Burndown => "Burndown",
            ReportKind::ResourceOverview => "Resource Overview",
            ReportKind::OverallocatedResources => "Overallocated Resources",
            ReportKind::CashFlow => "Cash Flow",
            ReportKind::CostOverruns => "Cost Overruns",
            ReportKind::EarnedValue => "Earned Value Report",
            ReportKind::TaskCostOverview => "Task Cost Overview",
            ReportKind::ResourceCostOverview => "Resource Cost Overview",
            ReportKind::CriticalTasks => "Critical Tasks",
            ReportKind::LateTasks => "Late Tasks",
            ReportKind::MilestoneReport => "Milestone Report",
            ReportKind::SlippingTasks => "Slipping Tasks",
            ReportKind::BoardStatus => "Task Status",
            ReportKind::GettingStarted => "Getting Started",
            ReportKind::CustomTable => "Custom Report",
            ReportKind::Comparison => "Comparison Report",
            ReportKind::SCurve => "S-Curve",
            ReportKind::Payments => "Payment Schedule",
        }
    }

    /// The Report tab menus: Dashboards, Resources, Costs, In Progress.
    pub fn menu(key: &str) -> &'static [ReportKind] {
        match key {
            "rep_dash" => &[ReportKind::Burndown, ReportKind::CostOverview, ReportKind::ProjectOverview, ReportKind::UpcomingTasks, ReportKind::WorkOverview],
            "rep_res" => &[ReportKind::OverallocatedResources, ReportKind::ResourceOverview],
            "rep_cost" => &[ReportKind::CashFlow, ReportKind::CostOverruns, ReportKind::EarnedValue, ReportKind::Payments, ReportKind::ResourceCostOverview, ReportKind::TaskCostOverview],
            "rep_prog" => &[ReportKind::CriticalTasks, ReportKind::LateTasks, ReportKind::MilestoneReport, ReportKind::SCurve, ReportKind::SlippingTasks],
            _ => &[],
        }
    }
}

const ORANGE: Color32 = Color32::from_rgb(0xED, 0x7D, 0x31);
const BLUE: Color32 = Color32::from_rgb(0x44, 0x72, 0xC4);
const GREY: Color32 = Color32::from_rgb(0xA5, 0xA5, 0xA5);

pub fn report_view(app: &mut App, ui: &mut Ui, area: Rect) {
    ui.painter().rect_filled(area, 0.0, Color32::WHITE);
    let kind = app.report;
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area.shrink2(Vec2::new(24.0, 12.0))).layout(egui::Layout::top_down(egui::Align::LEFT)));
    egui::ScrollArea::both().id_salt(("report", kind as u8)).auto_shrink([false, false]).show(&mut child, |ui| {
        ui.add_space(6.0);
        let title = if crate::i18n::is_thai() { tr(kind.label()).to_string() } else { kind.label().to_uppercase() };
        ui.label(RichText::new(title).size(26.0).color(theme::GREEN));
        let title = if app.doc.title.trim().is_empty() { String::new() } else { crate::thai::shape(&app.doc.title).to_string() };
        ui.label(RichText::new(format!("{title}   {} - {}", app.fmt_date(app.doc.start), app.fmt_finish(app.doc.finish_date()))).color(theme::TEXT_DIM));
        ui.add_space(10.0);
        match kind {
            ReportKind::ProjectOverview => project_overview(app, ui),
            ReportKind::CostOverview => cost_overview(app, ui),
            ReportKind::WorkOverview => work_overview(app, ui),
            ReportKind::UpcomingTasks => upcoming(app, ui),
            ReportKind::Burndown => burndown(app, ui),
            ReportKind::ResourceOverview => resource_overview(app, ui),
            ReportKind::OverallocatedResources => overallocated(app, ui),
            ReportKind::CashFlow => cash_flow(app, ui),
            ReportKind::CostOverruns => cost_overruns(app, ui),
            ReportKind::EarnedValue => earned_value(app, ui),
            ReportKind::TaskCostOverview => task_cost(app, ui),
            ReportKind::ResourceCostOverview => resource_cost(app, ui),
            ReportKind::CriticalTasks => task_list(app, ui, "Critical tasks that are not complete.", |_, t| t.critical && t.percent_complete < 100 && !t.is_summary),
            ReportKind::LateTasks => {
                let today = app.status_instant().date();
                task_list(app, ui, "Tasks that should have finished by the status date.", move |_, t| !t.is_summary && t.percent_complete < 100 && t.finish < today)
            }
            ReportKind::MilestoneReport => milestones(app, ui),
            ReportKind::SlippingTasks => task_list(app, ui, "Tasks finishing after their baseline finish.", |_, t| {
                !t.is_summary && t.percent_complete < 100 && t.baseline(0).and_then(|b| b.finish).zip(t.finish_dt).map(|(b, f)| f > b).unwrap_or(false)
            }),
            ReportKind::BoardStatus => board_status(app, ui),
            ReportKind::GettingStarted => getting_started(ui),
            ReportKind::CustomTable => custom_table(app, ui),
            ReportKind::Comparison => crate::compare::comparison_page(app, ui),
            ReportKind::SCurve => s_curve(app, ui),
            ReportKind::Payments => payment_schedule(app, ui),
        }
        ui.add_space(30.0);
    });
}

// ================================================================ building blocks

fn heading(ui: &mut Ui, s: &str) {
    ui.add_space(12.0);
    ui.label(RichText::new(tr(s)).size(16.0).strong().color(theme::TEXT));
    ui.add_space(4.0);
}

fn note(ui: &mut Ui, s: &str) {
    ui.label(RichText::new(tr(s)).color(theme::TEXT_DIM));
}

/// A row of big numbers with their captions.
fn kpis(ui: &mut Ui, items: &[(String, &str)]) {
    ui.horizontal(|ui| {
        for (v, cap) in items {
            ui.vertical(|ui| {
                ui.set_min_width(150.0);
                ui.label(RichText::new(v).size(24.0).color(theme::GREEN));
                ui.label(RichText::new(tr(cap)).color(theme::TEXT_DIM));
            });
            ui.add_space(12.0);
        }
    });
}

/// A table with a header row (right-aligned columns after the first two).
fn table(ui: &mut Ui, id: &str, headers: &[&str], rows: &[Vec<String>]) {
    if rows.is_empty() {
        note(ui, "There is nothing to show.");
        return;
    }
    egui::Grid::new(id).striped(true).spacing([18.0, 4.0]).show(ui, |ui| {
        for h in headers {
            ui.label(RichText::new(tr(h)).strong().color(theme::GREEN));
        }
        ui.end_row();
        for r in rows.iter().take(500) {
            for (k, c) in r.iter().enumerate() {
                let text = crate::thai::shape(c).to_string();
                if k >= 2 {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(text);
                    });
                } else {
                    ui.label(text);
                }
            }
            ui.end_row();
        }
    });
}

/// Grouped vertical bars: one group per label, one bar per series.
fn bar_chart(ui: &mut Ui, h: f32, labels: &[String], series: &[(&str, Color32, Vec<f64>)], fmt: impl Fn(f64) -> String) {
    let w = ui.available_width().min(960.0);
    let (r, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    let p = ui.painter_at(r);
    let plot = Rect::from_min_max(Pos2::new(r.left() + 70.0, r.top() + 22.0), Pos2::new(r.right() - 10.0, r.bottom() - 46.0));
    // (an axis of 0 % - 100 % for percentages, at least 1 otherwise)
    let floor = if series.iter().any(|s| s.0.contains('%')) { 100.0 } else { 1.0 };
    let max = series.iter().flat_map(|s| s.2.iter().copied()).fold(0.0_f64, f64::max).max(floor);
    // legend
    let mut lx = plot.left();
    for (name, col, _) in series {
        p.rect_filled(Rect::from_min_size(Pos2::new(lx, r.top() + 4.0), Vec2::splat(10.0)), 1.0, *col);
        let g = p.layout_no_wrap(tr(name).to_string(), FontId::proportional(11.0), theme::TEXT);
        let gw = g.size().x;
        p.galley(Pos2::new(lx + 14.0, r.top() + 2.0), g, theme::TEXT);
        lx += gw + 30.0;
    }
    // axis and gridlines
    for k in 0..=4 {
        let v = max * k as f64 / 4.0;
        let y = plot.bottom() - plot.height() * k as f32 / 4.0;
        p.line_segment([Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)], Stroke::new(1.0_f32, Color32::from_rgb(0xE6, 0xE6, 0xE6)));
        p.text(Pos2::new(plot.left() - 6.0, y), Align2::RIGHT_CENTER, fmt(v), FontId::proportional(10.0), theme::TEXT_DIM);
    }
    let n = labels.len().max(1);
    let gw = plot.width() / n as f32;
    let bw = (gw * 0.7 / series.len().max(1) as f32).min(40.0);
    for (k, label) in labels.iter().enumerate() {
        let gx = plot.left() + gw * k as f32 + (gw - bw * series.len() as f32) / 2.0;
        for (s, (_, col, vals)) in series.iter().enumerate() {
            let v = vals.get(k).copied().unwrap_or(0.0).max(0.0);
            let bh = (v / max) as f32 * plot.height();
            let x = gx + bw * s as f32;
            p.rect_filled(Rect::from_min_max(Pos2::new(x, plot.bottom() - bh), Pos2::new(x + bw - 2.0, plot.bottom())), 0.0, *col);
        }
        let mut text = crate::thai::shape(label).to_string();
        if text.chars().count() > 18 {
            text = text.chars().take(17).collect::<String>() + "...";
        }
        let lp = p.with_clip_rect(Rect::from_min_max(Pos2::new(plot.left() + gw * k as f32, plot.bottom()), Pos2::new(plot.left() + gw * (k + 1) as f32, r.bottom())));
        lp.text(Pos2::new(plot.left() + gw * (k as f32 + 0.5), plot.bottom() + 6.0), Align2::CENTER_TOP, text, FontId::proportional(10.5), theme::TEXT);
    }
    p.line_segment([Pos2::new(plot.left(), plot.bottom()), Pos2::new(plot.right(), plot.bottom())], Stroke::new(1.0_f32, theme::TEXT_DIM));
}

/// Lines over time (one point per period label).
fn line_chart(ui: &mut Ui, h: f32, labels: &[String], series: &[(&str, Color32, Vec<f64>)], fmt: impl Fn(f64) -> String) {
    let w = ui.available_width().min(960.0);
    let (r, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    let p = ui.painter_at(r);
    let plot = Rect::from_min_max(Pos2::new(r.left() + 70.0, r.top() + 22.0), Pos2::new(r.right() - 10.0, r.bottom() - 30.0));
    let max = series.iter().flat_map(|s| s.2.iter().copied()).fold(0.0_f64, f64::max).max(1e-9);
    let mut lx = plot.left();
    for (name, col, _) in series {
        p.line_segment([Pos2::new(lx, r.top() + 9.0), Pos2::new(lx + 14.0, r.top() + 9.0)], Stroke::new(2.5_f32, *col));
        let g = p.layout_no_wrap(tr(name).to_string(), FontId::proportional(11.0), theme::TEXT);
        let gw = g.size().x;
        p.galley(Pos2::new(lx + 18.0, r.top() + 2.0), g, theme::TEXT);
        lx += gw + 34.0;
    }
    for k in 0..=4 {
        let v = max * k as f64 / 4.0;
        let y = plot.bottom() - plot.height() * k as f32 / 4.0;
        p.line_segment([Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)], Stroke::new(1.0_f32, Color32::from_rgb(0xE6, 0xE6, 0xE6)));
        p.text(Pos2::new(plot.left() - 6.0, y), Align2::RIGHT_CENTER, fmt(v), FontId::proportional(10.0), theme::TEXT_DIM);
    }
    let n = labels.len().max(2);
    let x_of = |k: usize| plot.left() + plot.width() * k as f32 / (n - 1) as f32;
    let step = (labels.len() / 12).max(1);
    for (k, l) in labels.iter().enumerate() {
        if k % step == 0 {
            let align = if k + 1 == labels.len() && labels.len() > 1 { Align2::RIGHT_TOP } else { Align2::CENTER_TOP };
            p.text(Pos2::new(x_of(k), plot.bottom() + 6.0), align, crate::thai::shape(l), FontId::proportional(10.0), theme::TEXT_DIM);
        }
    }
    for (_, col, vals) in series {
        let pts: Vec<Pos2> = vals.iter().enumerate().map(|(k, v)| Pos2::new(x_of(k), plot.bottom() - (v / max) as f32 * plot.height())).collect();
        if pts.len() >= 2 {
            p.add(egui::Shape::line(pts, Stroke::new(2.0_f32, *col)));
        }
    }
    p.line_segment([Pos2::new(plot.left(), plot.bottom()), Pos2::new(plot.right(), plot.bottom())], Stroke::new(1.0_f32, theme::TEXT_DIM));
}

// ================================================================ numbers

fn leaves(app: &App) -> Vec<&Task> {
    app.doc.tasks.iter().filter(|t| !t.is_summary && t.active).collect()
}

fn top(app: &App) -> Vec<&Task> {
    app.doc.tasks.iter().filter(|t| t.outline_level == 1 && t.active).collect()
}

fn hours(m: i64) -> f64 {
    m as f64 / 60.0
}

fn hrs(v: f64) -> String {
    format!("{v:.0} {}", tr("hrs"))
}

fn money(v: f64) -> String {
    crate::util::fmt_money(v)
}

/// Chart axis amounts, short enough for the axis: "฿850,000", "฿4.3M" ("฿4.3 ล้าน").
fn money_axis(v: f64) -> String {
    let sym = crate::util::currency_symbol();
    let a = v.abs();
    let sign = if v < 0.0 { "-" } else { "" };
    if a >= 999_500.0 {
        let m = a / 1_000_000.0;
        let n = if m >= 100.0 { format!("{m:.0}") } else { format!("{m:.1}") };
        if crate::i18n::is_thai() { format!("{sign}{sym}{n} ล้าน") } else { format!("{sign}{sym}{n}M") }
    } else {
        crate::util::format_money(v, &sym, 0, 0)
    }
}

/// The amount in words under a total, for a Thai user working in baht ("(หนึ่งร้อยบาทถ้วน)").
fn baht_words(ui: &mut Ui, label: &str, v: f64) {
    if crate::i18n::is_thai() && crate::util::currency_symbol() == "฿" && v.abs() >= 0.005 {
        let s = format!("{} ({})", tr(label), crate::thai::baht_text(v));
        ui.label(RichText::new(crate::thai::shape(&s)).color(theme::TEXT_DIM));
    }
}

fn actual_work(t: &Task) -> f64 {
    hours(t.work_min) * t.percent_complete as f64 / 100.0
}

fn baseline_cost(t: &Task) -> f64 {
    t.baseline(0).and_then(|b| b.cost).unwrap_or(0.0)
}

fn baseline_work(t: &Task) -> f64 {
    t.baseline(0).and_then(|b| b.work_min).map(hours).unwrap_or(0.0)
}

/// Monthly periods from the project start to its finish (at most 60).
fn months(app: &App) -> Vec<NaiveDate> {
    let s = app.doc.tasks.iter().map(|t| t.start).min().unwrap_or(app.doc.start);
    let f = app.doc.tasks.iter().map(|t| t.finish).max().unwrap_or(s);
    let mut d = NaiveDate::from_ymd_opt(s.year(), s.month(), 1).unwrap_or(s);
    let mut out = Vec::new();
    while d <= f && out.len() < 60 {
        out.push(d);
        d = if d.month() == 12 { NaiveDate::from_ymd_opt(d.year() + 1, 1, 1).unwrap() } else { NaiveDate::from_ymd_opt(d.year(), d.month() + 1, 1).unwrap() };
    }
    out
}

/// Share of a task between two dates (its value spread evenly over its working days).
fn share(app: &App, t: &Task, a: NaiveDate, b: NaiveDate) -> f64 {
    let cal = &app.doc.calendar;
    let total: i64 = day_count(cal, t.start, t.finish);
    if total == 0 {
        return if t.start >= a && t.start < b { 1.0 } else { 0.0 };
    }
    let (x, y) = (t.start.max(a), t.finish.min(b - chrono::Duration::days(1)));
    if y < x {
        return 0.0;
    }
    day_count(cal, x, y) as f64 / total as f64
}

fn day_count(cal: &crate::model::Calendar, a: NaiveDate, b: NaiveDate) -> i64 {
    let mut n = 0;
    let mut d = a;
    while d <= b && n < 20_000 {
        if cal.is_working(d) {
            n += 1;
        }
        d += chrono::Duration::days(1);
    }
    n
}

/// "Jan 26" / "ม.ค. 69" (the year of the display calendar).
fn month_label(app: &App, d: NaiveDate) -> String {
    format!("{} {:02}", crate::thai::shape(app.loc.month_abbr(d)), app.loc.year(d).rem_euclid(100))
}

// ================================================================ reports

fn project_overview(app: &App, ui: &mut Ui) {
    let ps = app.project_summary();
    let lv = leaves(app);
    let today = app.status_instant().date();
    let late = lv.iter().filter(|t| t.percent_complete < 100 && t.finish < today).count();
    kpis(ui, &[(format!("{}%", ps.percent_complete), "% complete"), (lv.len().to_string(), "Tasks"), (lv.iter().filter(|t| t.percent_complete == 100).count().to_string(), "Completed"), (late.to_string(), "Late tasks"), (money(ps.cost), "Cost")]);
    baht_words(ui, "Cost", ps.cost);
    heading(ui, "% Complete");
    let tops = top(app);
    let labels: Vec<String> = tops.iter().map(|t| t.name.clone()).collect();
    bar_chart(ui, 240.0, &labels, &[("% Complete", theme::GREEN, tops.iter().map(|t| t.percent_complete as f64).collect())], |v| format!("{v:.0}%"));
    heading(ui, "Milestones due");
    let rows: Vec<Vec<String>> = lv.iter().filter(|t| t.is_milestone() && t.percent_complete < 100).take(20).map(|t| vec![t.name.clone(), app.fmt_finish(t.finish)]).collect();
    table(ui, "po_ms", &["Name", "Finish"], &rows);
    heading(ui, "Late tasks");
    let rows: Vec<Vec<String>> = lv
        .iter()
        .filter(|t| t.percent_complete < 100 && t.finish < today)
        .take(20)
        .map(|t| vec![t.name.clone(), app.fmt_date(t.start), app.fmt_finish(t.finish), format!("{}%", t.percent_complete), app.doc.resource_names(t)])
        .collect();
    table(ui, "po_late", &["Name", "Start", "Finish", "% Complete", "Resource Names"], &rows);
}

fn cost_overview(app: &App, ui: &mut Ui) {
    let ps = app.project_summary();
    kpis(ui, &[(money(ps.cost), "Cost"), (money(ps.cost - ps.actual_cost), "Remaining Cost"), (money(ps.actual_cost), "Actual Cost"), (format!("{}%", ps.percent_complete), "% complete")]);
    baht_words(ui, "Cost", ps.cost);
    heading(ui, "Cost status");
    let tops = top(app);
    let labels: Vec<String> = tops.iter().map(|t| t.name.clone()).collect();
    bar_chart(
        ui,
        260.0,
        &labels,
        &[
            ("Actual Cost", BLUE, tops.iter().map(|t| t.actual_cost).collect()),
            ("Remaining Cost", ORANGE, tops.iter().map(|t| (t.cost - t.actual_cost).max(0.0)).collect()),
            ("Baseline Cost", GREY, tops.iter().map(|t| baseline_cost(t)).collect()),
        ],
        money_axis,
    );
    heading(ui, "Cost details");
    let rows: Vec<Vec<String>> = tops.iter().map(|t| vec![t.name.clone(), String::new(), money(t.actual_cost), money((t.cost - t.actual_cost).max(0.0)), money(baseline_cost(t)), money(t.cost), money(t.cost - baseline_cost(t))]).collect();
    table(ui, "co_t", &["Name", "", "Actual Cost", "Remaining Cost", "Baseline Cost", "Cost", "Cost Variance"], &rows);
}

fn work_overview(app: &App, ui: &mut Ui) {
    let ps = app.project_summary();
    let aw = actual_work(&ps);
    kpis(ui, &[(hrs(hours(ps.work_min)), "Work"), (hrs(hours(ps.work_min) - aw), "Remaining Work"), (hrs(aw), "Actual Work"), (format!("{}%", ps.percent_complete), "% complete")]);
    heading(ui, "Work stats");
    let tops = top(app);
    let labels: Vec<String> = tops.iter().map(|t| t.name.clone()).collect();
    bar_chart(
        ui,
        260.0,
        &labels,
        &[
            ("Actual Work", BLUE, tops.iter().map(|t| actual_work(t)).collect()),
            ("Remaining Work", ORANGE, tops.iter().map(|t| hours(t.work_min) - actual_work(t)).collect()),
            ("Baseline Work", GREY, tops.iter().map(|t| baseline_work(t)).collect()),
        ],
        hrs,
    );
    heading(ui, "Resource stats");
    let res: Vec<&crate::model::Resource> = app.doc.resources.iter().filter(|r| r.rtype == ResourceType::Work).collect();
    let labels: Vec<String> = res.iter().map(|r| r.name.clone()).collect();
    bar_chart(ui, 220.0, &labels, &[("Work", theme::GREEN, res.iter().map(|r| hours(r.work_min)).collect())], hrs);
}

fn upcoming(app: &App, ui: &mut Ui) {
    let today = app.status_instant().date();
    let end = today + chrono::Duration::days(7);
    let lv = leaves(app);
    heading(ui, "Work completed this week");
    let rows: Vec<Vec<String>> = lv.iter().filter(|t| t.actual_finish.map(|f| f.date() >= today - chrono::Duration::days(7) && f.date() <= today).unwrap_or(false)).map(|t| vec![t.name.clone(), app.fmt_finish(t.finish), "100%".into()]).collect();
    table(ui, "up_done", &["Name", "Finish", "% Complete"], &rows);
    heading(ui, "Tasks starting soon");
    let rows: Vec<Vec<String>> = lv
        .iter()
        .filter(|t| t.percent_complete < 100 && t.start >= today && t.start <= end)
        .map(|t| vec![t.name.clone(), app.fmt_date(t.start), app.fmt_finish(t.finish), app.doc.resource_names(t)])
        .collect();
    table(ui, "up_soon", &["Name", "Start", "Finish", "Resource Names"], &rows);
    heading(ui, "Tasks in progress");
    let rows: Vec<Vec<String>> = lv.iter().filter(|t| t.percent_complete > 0 && t.percent_complete < 100).map(|t| vec![t.name.clone(), app.fmt_finish(t.finish), format!("{}%", t.percent_complete)]).collect();
    table(ui, "up_prog", &["Name", "Finish", "% Complete"], &rows);
}

fn burndown(app: &App, ui: &mut Ui) {
    let ms = months(app);
    let lv = leaves(app);
    let total: f64 = lv.iter().map(|t| hours(t.work_min)).sum();
    let base_total: f64 = lv.iter().map(|t| baseline_work(t)).sum();
    let today = app.status_instant().date();
    let mut planned = Vec::new();
    let mut actual = Vec::new();
    let mut base = Vec::new();
    for (k, m) in ms.iter().enumerate() {
        let end = ms.get(k + 1).copied().unwrap_or(*m + chrono::Duration::days(31));
        let done_by: f64 = lv.iter().map(|t| hours(t.work_min) * share(app, t, app.doc.start.min(t.start), end)).sum();
        planned.push((total - done_by).max(0.0));
        if *m <= today {
            let act: f64 = lv.iter().map(|t| actual_work(t) * share(app, t, app.doc.start.min(t.start), end.min(today + chrono::Duration::days(1)))).sum();
            actual.push((total - act).max(0.0));
        }
        let bdone: f64 = lv
            .iter()
            .filter_map(|t| {
                let b = t.baseline(0)?;
                let (bs, bf) = (b.start?.date(), b.finish?.date());
                let mut bt = (*t).clone();
                bt.start = bs;
                bt.finish = bf;
                Some(baseline_work(t) * share(app, &bt, bs.min(app.doc.start), end))
            })
            .sum();
        base.push((base_total - bdone).max(0.0));
    }
    let labels: Vec<String> = ms.iter().map(|d| month_label(app, *d)).collect();
    heading(ui, "Work burndown");
    note(ui, "Remaining work by month: as scheduled, as done so far, and as in the baseline.");
    let mut series = vec![("Remaining Work", ORANGE, planned), ("Remaining Actual Work", BLUE, actual)];
    if base_total > 0.0 {
        series.push(("Baseline Remaining Work", GREY, base));
    }
    line_chart(ui, 300.0, &labels, &series, hrs);
    heading(ui, "Task burndown");
    let mut rem_tasks = Vec::new();
    for (k, _) in ms.iter().enumerate() {
        let end = ms.get(k + 1).copied().unwrap_or(ms[k] + chrono::Duration::days(31));
        rem_tasks.push(lv.iter().filter(|t| t.finish >= end).count() as f64);
    }
    line_chart(ui, 240.0, &labels, &[("Remaining Tasks", ORANGE, rem_tasks)], |v| format!("{v:.0}"));
}

fn resource_overview(app: &App, ui: &mut Ui) {
    let res = &app.doc.resources;
    kpis(ui, &[(res.len().to_string(), "Resources"), (res.iter().filter(|r| r.overallocated).count().to_string(), "Overallocated")]);
    heading(ui, "Resource stats");
    let work: Vec<&crate::model::Resource> = res.iter().filter(|r| r.rtype == ResourceType::Work).collect();
    let labels: Vec<String> = work.iter().map(|r| r.name.clone()).collect();
    let actual = |r: &crate::model::Resource| -> f64 {
        app.doc.tasks.iter().filter(|t| !t.is_summary).flat_map(|t| t.assignments.iter().filter(|a| a.res_uid == r.uid).map(move |a| hours(a.work_min.max(0)) * t.percent_complete as f64 / 100.0)).sum()
    };
    bar_chart(
        ui,
        240.0,
        &labels,
        &[("Actual Work", BLUE, work.iter().map(|r| actual(r)).collect()), ("Remaining Work", ORANGE, work.iter().map(|r| hours(r.work_min) - actual(r)).collect())],
        hrs,
    );
    heading(ui, "Resource status");
    let rows: Vec<Vec<String>> = res
        .iter()
        .map(|r| {
            let w = hours(r.work_min);
            let a = actual(r);
            vec![r.name.clone(), tr(r.rtype.label()).to_string(), hrs(a), hrs(w - a), money(r.cost), if r.overallocated { tr("Yes").to_string() } else { tr("No").to_string() }]
        })
        .collect();
    table(ui, "ro_t", &["Name", "Type", "Actual Work", "Remaining Work", "Cost", "Overallocated"], &rows);
}

fn overallocated(app: &App, ui: &mut Ui) {
    let res: Vec<&crate::model::Resource> = app.doc.resources.iter().filter(|r| r.overallocated).collect();
    if res.is_empty() {
        note(ui, "No resource is overallocated.");
        return;
    }
    for r in res {
        heading(ui, &crate::thai::shape(&r.name));
        let rows: Vec<Vec<String>> = app
            .doc
            .tasks
            .iter()
            .filter(|t| !t.is_summary && t.overallocated && t.assignments.iter().any(|a| a.res_uid == r.uid))
            .map(|t| vec![t.name.clone(), app.fmt_date(t.start), app.fmt_finish(t.finish), hrs(hours(t.work_min))])
            .collect();
        table(ui, &format!("oa_{}", r.uid), &["Name", "Start", "Finish", "Work"], &rows);
    }
}

fn cash_flow(app: &App, ui: &mut Ui) {
    let ms = months(app);
    let lv = leaves(app);
    let mut cost = Vec::new();
    let mut cum = Vec::new();
    let mut acc = 0.0;
    for (k, m) in ms.iter().enumerate() {
        let end = ms.get(k + 1).copied().unwrap_or(*m + chrono::Duration::days(31));
        let c: f64 = lv.iter().map(|t| t.cost * share(app, t, *m, end)).sum();
        acc += c;
        cost.push(c);
        cum.push(acc);
    }
    let labels: Vec<String> = ms.iter().map(|d| month_label(app, *d)).collect();
    heading(ui, "Cost by month");
    bar_chart(ui, 260.0, &labels, &[("Cost", BLUE, cost.clone())], money_axis);
    heading(ui, "Cumulative Cost");
    line_chart(ui, 240.0, &labels, &[("Cumulative Cost", ORANGE, cum.clone())], money_axis);
    let rows: Vec<Vec<String>> = ms.iter().zip(cost.iter().zip(cum.iter())).map(|(m, (c, a))| vec![month_label(app, *m), String::new(), money(*c), money(*a)]).collect();
    table(ui, "cf_t", &["Month", "", "Cost", "Cumulative Cost"], &rows);
}

fn cost_overruns(app: &App, ui: &mut Ui) {
    let lv = leaves(app);
    let over: Vec<&&Task> = lv.iter().filter(|t| t.cost - baseline_cost(t) > 0.005 && baseline_cost(t) > 0.0).collect();
    kpis(ui, &[(over.len().to_string(), "Tasks over budget"), (money(over.iter().map(|t| t.cost - baseline_cost(t)).sum()), "Cost Variance")]);
    heading(ui, "Task cost variance");
    let rows: Vec<Vec<String>> = over.iter().map(|t| vec![t.name.clone(), String::new(), money(baseline_cost(t)), money(t.cost), money(t.cost - baseline_cost(t))]).collect();
    table(ui, "ov_t", &["Name", "", "Baseline Cost", "Cost", "Cost Variance"], &rows);
    if lv.iter().all(|t| t.baseline(0).is_none()) {
        note(ui, "Set a baseline (Project > Set Baseline) to compare costs with it.");
    }
}

fn earned_value(app: &App, ui: &mut Ui) {
    let lv = leaves(app);
    if lv.iter().all(|t| t.baseline(0).is_none()) {
        note(ui, "Earned value needs a baseline: Project > Set Baseline.");
        return;
    }
    let status = app.status_instant().date();
    // planned value at a date: the baseline cost due by then (spread over the baseline dates)
    let pv_at = |d: NaiveDate| -> f64 {
        lv.iter()
            .filter_map(|t| {
                let b = t.baseline(0)?;
                let (bs, bf) = (b.start?.date(), b.finish?.date());
                let mut bt = (*t).clone();
                bt.start = bs;
                bt.finish = bf;
                Some(b.cost.unwrap_or(0.0) * share(app, &bt, bs.min(app.doc.start), d + chrono::Duration::days(1)))
            })
            .sum()
    };
    let ev: f64 = lv.iter().map(|t| baseline_cost(t) * t.percent_complete as f64 / 100.0).sum();
    let ac: f64 = lv.iter().map(|t| t.actual_cost).sum();
    let pv = pv_at(status);
    let bac: f64 = lv.iter().map(|t| baseline_cost(t)).sum();
    let spi = if pv > 0.0 { Some(ev / pv) } else { None };
    let cpi = if ac > 0.0 { Some(ev / ac) } else { None };
    let eac = cpi.filter(|&x| x > 0.0).map(|x| bac / x);
    let ratio = |v: Option<f64>| v.map(|x| format!("{x:.2}")).unwrap_or_else(|| "N/A".into());
    let estimate = |v: Option<f64>| v.map(money).unwrap_or_else(|| "N/A".into());
    kpis(ui, &[(money(pv), "Planned Value - PV (BCWS)"), (money(ev), "Earned Value - EV (BCWP)"), (money(ac), "AC (ACWP)")]);
    kpis(ui, &[(ratio(spi), "SPI"), (ratio(cpi), "CPI"), (money(ev - pv), "SV"), (money(ev - ac), "CV"), (estimate(eac), "EAC")]);
    note(ui, "Actual cost is currently estimated from resource rates and task completion; it is not a certified actual-cost ledger.");
    heading(ui, "Earned value over time");
    let ms = months(app);
    let labels: Vec<String> = ms.iter().map(|d| month_label(app, *d)).collect();
    let pvs: Vec<f64> = ms.iter().enumerate().map(|(k, m)| pv_at(ms.get(k + 1).map(|n| *n - chrono::Duration::days(1)).unwrap_or(*m + chrono::Duration::days(30)))).collect();
    line_chart(ui, 280.0, &labels, &[("Planned Value - PV (BCWS)", GREY, pvs)], money_axis);
    note(ui, "Historical EV and AC curves are unavailable without dated progress and cost entries. Current EV/AC above are point-in-time estimates, not historical values.");
}

fn task_cost(app: &App, ui: &mut Ui) {
    let ps = app.project_summary();
    kpis(ui, &[(money(ps.cost), "Cost"), (money(ps.actual_cost), "Actual Cost"), (money(ps.cost - ps.actual_cost), "Remaining Cost")]);
    baht_words(ui, "Cost", ps.cost);
    heading(ui, "Cost status");
    let rows: Vec<Vec<String>> = top(app).iter().map(|t| vec![t.name.clone(), String::new(), money(t.fixed_cost), money(t.actual_cost), money((t.cost - t.actual_cost).max(0.0)), money(t.cost), money(baseline_cost(t)), money(t.cost - baseline_cost(t))]).collect();
    table(ui, "tc_t", &["Name", "", "Fixed Cost", "Actual Cost", "Remaining Cost", "Cost", "Baseline Cost", "Cost Variance"], &rows);
    heading(ui, "Cost distribution");
    let tops = top(app);
    let labels: Vec<String> = tops.iter().map(|t| t.name.clone()).collect();
    bar_chart(ui, 240.0, &labels, &[("Cost", BLUE, tops.iter().map(|t| t.cost).collect())], money_axis);
}

fn resource_cost(app: &App, ui: &mut Ui) {
    let res = &app.doc.resources;
    let total: f64 = res.iter().map(|r| r.cost).sum();
    kpis(ui, &[(money(total), "Cost"), (res.len().to_string(), "Resources")]);
    heading(ui, "Cost status");
    let labels: Vec<String> = res.iter().map(|r| r.name.clone()).collect();
    bar_chart(ui, 240.0, &labels, &[("Cost", BLUE, res.iter().map(|r| r.cost).collect())], money_axis);
    let rows: Vec<Vec<String>> = res.iter().map(|r| vec![r.name.clone(), tr(r.rtype.label()).to_string(), money(r.std_rate), money(r.cost_per_use), money(r.cost)]).collect();
    table(ui, "rc_t", &["Name", "Type", "Standard Rate", "Cost/Use", "Cost"], &rows);
}

fn task_list(app: &App, ui: &mut Ui, what: &str, keep: impl Fn(&App, &Task) -> bool) {
    note(ui, what);
    let rows: Vec<Vec<String>> = app
        .doc
        .tasks
        .iter()
        .filter(|t| keep(app, t))
        .map(|t| vec![t.name.clone(), app.fmt_date(t.start), app.fmt_finish(t.finish), app.task_duration_text(t), format!("{}%", t.percent_complete), app.doc.resource_names(t)])
        .collect();
    kpis(ui, &[(rows.len().to_string(), "Tasks")]);
    table(ui, "tl_t", &["Name", "Start", "Finish", "Duration", "% Complete", "Resource Names"], &rows);
}

fn milestones(app: &App, ui: &mut Ui) {
    let today = app.status_instant().date();
    let ms: Vec<&Task> = app.doc.tasks.iter().filter(|t| t.is_milestone()).collect();
    let late: Vec<&&Task> = ms.iter().filter(|t| t.percent_complete < 100 && t.finish < today).collect();
    let soon: Vec<&&Task> = ms.iter().filter(|t| t.percent_complete < 100 && t.finish >= today).collect();
    let done: Vec<&&Task> = ms.iter().filter(|t| t.percent_complete == 100).collect();
    kpis(ui, &[(late.len().to_string(), "Late milestones"), (soon.len().to_string(), "Milestones upcoming"), (done.len().to_string(), "Completed milestones")]);
    for (title, list) in [("Late milestones", late), ("Milestones upcoming", soon), ("Completed milestones", done)] {
        heading(ui, title);
        let rows: Vec<Vec<String>> = list.iter().map(|t| vec![t.name.clone(), app.fmt_finish(t.finish), t.baseline(0).and_then(|b| b.finish).map(|f| app.fmt_finish(f.date())).unwrap_or_else(|| "NA".into())]).collect();
        table(ui, &format!("ms_{title}"), &["Name", "Finish", "Baseline Finish"], &rows);
    }
}

fn board_status(app: &App, ui: &mut Ui) {
    let lv = leaves(app);
    let counts: Vec<f64> = (0..4).map(|c| lv.iter().filter(|t| crate::board::column_of(t) == c).count() as f64).collect();
    let labels: Vec<String> = crate::board::COLUMNS.iter().map(|s| tr(s).to_string()).collect();
    kpis(ui, &[(lv.len().to_string(), "Tasks"), (format!("{:.0}", counts[3]), "Done")]);
    heading(ui, "Task status");
    bar_chart(ui, 240.0, &labels, &[("Tasks", theme::GREEN, counts)], |v| format!("{v:.0}"));
    heading(ui, "Work status");
    let work: Vec<f64> = (0..4).map(|c| lv.iter().filter(|t| crate::board::column_of(t) == c).map(|t| hours(t.work_min)).sum()).collect();
    bar_chart(ui, 240.0, &labels, &[("Work", BLUE, work)], hrs);
}

fn getting_started(ui: &mut Ui) {
    for (h, body) in [
        ("Reports", "The Report tab shows the project as charts and tables: dashboards for the whole project, its resources and costs, and the tasks in progress. Pick one from Dashboards, Resources, Costs or In Progress."),
        ("Keep them current", "Reports follow the schedule: update progress (Task > % Complete, Project > Update Project) and set a baseline (Project > Set Baseline) to see variances, slipping tasks and earned value."),
        ("Custom", "New Report > Table lists every task with the columns of the current table (View > Tables)."),
        ("Status date", "Late tasks, upcoming tasks and earned value are measured at the status date (Project > Status Date), or today when it is not set."),
    ] {
        heading(ui, h);
        note(ui, body);
    }
}

fn custom_table(app: &App, ui: &mut Ui) {
    let cols: Vec<crate::app::TCol> = app.task_cols.iter().map(|c| c.0).filter(|c| !matches!(c, crate::app::TCol::Indicators | crate::app::TCol::Mode)).collect();
    let headers: Vec<String> = cols.iter().map(|c| app.col_title(*c)).collect();
    let rows: Vec<Vec<String>> = app.visible_rows().into_iter().map(|i| cols.iter().map(|c| app.cell_text(i, *c)).collect()).collect();
    egui::Grid::new("custom_report").striped(true).spacing([18.0, 4.0]).show(ui, |ui| {
        for h in &headers {
            ui.label(RichText::new(h.as_str()).strong().color(theme::GREEN));
        }
        ui.end_row();
        for r in rows.iter().take(2000) {
            for c in r {
                ui.label(crate::thai::shape(c).to_string());
            }
            ui.end_row();
        }
    });
}

// ================================================================ S-Curve

fn pct(v: f64) -> String {
    format!("{v:.2}%")
}

fn s_curve(app: &App, ui: &mut Ui) {
    let id = egui::Id::unique("scurve_opts");
    let (mut basis, mut period) = ui.data_mut(|d| *d.get_persisted_mut_or(id, (0u8, 0u8)));
    ui.horizontal(|ui| {
        ui.label(tr("Weight by:"));
        egui::ComboBox::from_id_salt("sc_basis").selected_text(tr(["Automatic", "Cost", "Work", "Duration"][basis as usize % 4])).show_ui(ui, |ui| {
            for (k, l) in ["Automatic", "Cost", "Work", "Duration"].iter().enumerate() {
                ui.selectable_value(&mut basis, k as u8, tr(l));
            }
        });
        ui.add_space(12.0);
        ui.label(tr("Period:"));
        egui::ComboBox::from_id_salt("sc_period").selected_text(tr(["Automatic", "Weekly", "Monthly"][period as usize % 3])).show_ui(ui, |ui| {
            for (k, l) in ["Automatic", "Weekly", "Monthly"].iter().enumerate() {
                ui.selectable_value(&mut period, k as u8, tr(l));
            }
        });
    });
    ui.data_mut(|d| d.insert_persisted(id, (basis, period)));
    if app.doc.tasks.iter().all(|t| t.is_summary || !t.active) {
        note(ui, "There is nothing to show.");
        return;
    }
    let status = app.status_instant().date();
    let sc = crate::scurve::compute(&app.doc, status, basis, period);
    note(ui, "Actual curve is an estimate reconstructed from current task % complete; it is not a dated site progress history.");
    let diff = sc.actual_at_status - sc.plan_at_status;
    let verdict = if diff.abs() < 0.005 { "On plan" } else if diff > 0.0 { "Ahead of plan" } else { "Behind plan" };
    ui.add_space(6.0);
    kpis(ui, &[(pct(sc.plan_at_status), "Planned progress to date"), (pct(sc.actual_at_status), "Actual progress to date"), (format!("{:+.2}%", diff), verdict)]);
    note(ui, &format!("{}  ·  {} {}", tr(sc.basis.label()), tr("Status date:"), app.fmt_date(status)));
    // labels: the start, then the last day of each period
    let label = |k: usize| -> String {
        let d = sc.ends[k];
        if k == 0 {
            app.loc.month_day(d)
        } else if sc.weekly {
            app.loc.month_day(d - chrono::Duration::days(1))
        } else {
            month_label(app, d - chrono::Duration::days(1))
        }
    };
    let labels: Vec<String> = (0..sc.ends.len()).map(label).collect();
    heading(ui, "Cumulative progress");
    let has_base = sc.current.is_some();
    let mut series = vec![(if has_base { "Plan (baseline)" } else { "Plan" }, GREY, sc.plan.clone()), ("Actual", BLUE, sc.actual.clone())];
    if let Some(c) = &sc.current {
        series.push(("Current schedule", ORANGE, c.clone()));
    }
    line_chart(ui, 320.0, &labels, &series, |v| format!("{v:.0}%"));
    if !has_base {
        note(ui, "Set a baseline (Project > Set Baseline) to keep the plan fixed while the schedule changes.");
    }

    heading(ui, "Progress by period");
    let mut rows = Vec::new();
    for k in 1..sc.ends.len() {
        let p_in = sc.plan[k] - sc.plan[k - 1];
        let (a_in, a_cum, var) = match (sc.actual.get(k), sc.actual.get(k - 1)) {
            // the period holding the status date compares with the plan at the status date
            (Some(a), Some(b)) => (pct(a - b), pct(*a), pct(a - sc.plan[k].min(sc.plan_at_status))),
            _ => (String::new(), String::new(), String::new()),
        };
        rows.push(vec![labels[k].clone(), String::new(), pct(p_in), pct(sc.plan[k]), a_in, a_cum, var]);
    }
    let headers = ["Period", "", "Planned", "Cumulative Planned", "Actual", "Cumulative Actual", "Variance"];
    copy_button(ui, "sc_copy_p", &headers, &rows);
    table(ui, "sc_t", &headers, &rows);

    heading(ui, "Weight and progress of each phase");
    let rows: Vec<Vec<String>> = sc.groups.iter().map(|g| vec![g.name.clone(), String::new(), pct(g.weight), pct(g.plan), pct(g.actual), pct(g.actual - g.plan)]).collect();
    let headers = ["Name", "", "Weight", "Planned", "Actual", "Variance"];
    copy_button(ui, "sc_copy_g", &headers, &rows);
    table(ui, "sc_g", &headers, &rows);
}

// ================================================================ Payment Schedule

fn payment_schedule(app: &App, ui: &mut Ui) {
    let status = app.status_instant().date();
    let s = crate::payments::schedule(&app.doc, status);
    if s.rows.is_empty() {
        note(ui, "There are no installments yet: Project > Payment Schedule.");
        return;
    }
    kpis(ui, &[(money(s.contract), "Contract value"), (money(s.due_net), "Ready to bill (net)"), (money(s.paid_net), "Received (net)"), (money(s.net_total - s.paid_net), "Still to receive (net)")]);
    baht_words(ui, "Contract value", s.contract);
    if (s.pct_sum - 100.0).abs() > 0.01 {
        ui.label(RichText::new(crate::i18n::trf("The installments add up to {} of the contract.", &[&format!("{:.2}%", s.pct_sum)])).color(Color32::from_rgb(0xC4, 0x31, 0x4B)));
    }
    let plan = &app.doc.payments;
    if plan.advance_pct > 0.0 || plan.retention_pct > 0.0 {
        note(ui, &crate::i18n::trf("Advance payment {} recovered and retention {} withheld from each installment.", &[&format!("{:.2}%", plan.advance_pct), &format!("{:.2}%", plan.retention_pct)]));
    }
    heading(ui, "Installments");
    let mut rows: Vec<Vec<String>> = Vec::new();
    if s.advance_amount > 0.0 {
        rows.push(vec![tr("Advance payment").to_string(), tr("Contract signed").to_string(), app.fmt_date(app.doc.start), pct(plan.advance_pct), money(s.advance_amount), String::new(), String::new(), money(s.advance_amount), String::new()]);
    }
    for r in &s.rows {
        let due = match (r.due, r.baseline_due) {
            (Some(d), Some(b)) if b != d => format!("{} ({} {})", app.fmt_date(d), tr("baseline"), app.fmt_date(b)),
            (Some(d), _) => app.fmt_date(d),
            _ => String::new(),
        };
        rows.push(vec![r.name.clone(), r.condition.clone(), due, pct(r.pct), money(r.amount), money(-r.advance), money(-r.retention), money(r.net), tr(r.status.label()).to_string()]);
    }
    rows.push(vec![tr("Total").to_string(), String::new(), String::new(), pct(s.pct_sum), money(s.rows.iter().map(|r| r.amount).sum()), money(-s.rows.iter().map(|r| r.advance).sum::<f64>()), money(-s.retention_total), money(s.net_total + s.advance_amount), String::new()]);
    let headers = ["Installment", "Due when this task is done", "Due", "Share of contract", "Amount", "Advance recovered", "Retention", "Net", "Status"];
    copy_button(ui, "pay_copy", &headers, &rows);
    table(ui, "pay_t", &headers, &rows);
    if s.retention_total > 0.0 {
        note(ui, &crate::i18n::trf("Retention withheld in all: {} (returned after the warranty period).", &[&money(s.retention_total)]));
    }

    // money in (advance + net installments when their work is due) against the cost going out
    heading(ui, "Money in and out");
    let ms = months(app);
    let lv = leaves(app);
    let labels: Vec<String> = ms.iter().map(|d| month_label(app, *d)).collect();
    let (mut inc, mut out) = (Vec::new(), Vec::new());
    let (mut a, mut c) = (0.0, 0.0);
    for (k, m) in ms.iter().enumerate() {
        let end = ms.get(k + 1).copied().unwrap_or(*m + chrono::Duration::days(31));
        if k == 0 {
            a += s.advance_amount;
        }
        a += s.rows.iter().filter(|r| r.due.is_some_and(|d| d >= *m && d < end)).map(|r| r.net).sum::<f64>();
        c += lv.iter().map(|t| t.cost * share(app, t, *m, end)).sum::<f64>();
        inc.push(a);
        out.push(c);
    }
    line_chart(ui, 280.0, &labels, &[("Cumulative income", theme::GREEN, inc), ("Cumulative Cost", ORANGE, out)], money_axis);
    note(ui, "Income counts each installment in the month its task is due to finish.");
}

/// "Copy table": tab-separated text that pastes into Excel as cells.
fn copy_button(ui: &mut Ui, id: &str, headers: &[&str], rows: &[Vec<String>]) {
    let resp = ui.push_id(id, |ui| ui.small_button(tr("Copy table"))).inner;
    if resp.clicked() {
        let keep: Vec<usize> = (0..headers.len()).filter(|&k| !headers[k].is_empty()).collect();
        let mut out = keep.iter().map(|&k| tr(headers[k]).to_string()).collect::<Vec<_>>().join("\t");
        for r in rows {
            out.push('\n');
            out.push_str(&keep.iter().map(|&k| r.get(k).cloned().unwrap_or_default()).collect::<Vec<_>>().join("\t"));
        }
        ui.ctx().copy_text(out);
    }
}
