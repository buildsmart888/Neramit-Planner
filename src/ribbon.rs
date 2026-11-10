//! Title bar, ribbon tabs/groups and the File backstage.

use crate::i18n::{tr};
use crate::app::{App, Backstage, RibbonTab, SortKey, TCol, TaskFilter, TaskGroup, View};
use crate::dialogs::{self, Dialog};
use crate::model::ResourceType;
use crate::theme::{self, paint_icon, Icon};
use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Stroke, Ui, Vec2};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum Cmd {
    New,
    Open,
    OpenRecent(std::path::PathBuf),
    Save,
    SaveAs,
    #[serde(skip)]
    Export(&'static str),
    /// the view as a picture: to the clipboard (false) or a PNG file (true)
    Picture(bool),
    /// Project > Import BOQ (bill of quantities in Excel / CSV)
    ImportBoq,
    /// Project > Payment Schedule (งวดงาน)
    Payments,
    CloseDoc,
    Exit,
    Undo,
    Redo,
    View(View),
    Paste,
    Cut,
    Copy,
    Bold,
    Italic,
    Underline,
    Percent(u8),
    MarkOnTrack,
    RespectLinks,
    Inactivate,
    Link,
    Unlink,
    Indent,
    Outdent,
    Manual,
    Auto,
    Inspect,
    MoveUp,
    MoveDown,
    /// Task > Move: 0 = 1 day, 1 = 1 week, 2 = 4 weeks forward
    MoveTask(u8),
    MoveTaskCustom,
    MoveProject,
    /// View > Tables
    Table(u8),
    LevelAll,
    LevelResource,
    ClearLeveling,
    /// View > Group by
    Group(TaskGroup),
    /// Task > Split Task
    SplitTask,
    /// File > Print > Print
    PrintNow,
    /// Ctrl+P: the Print page
    PrintPage,
    /// View > Filter / Highlight (true = highlight)
    Filter(TaskFilter, bool),
    RelatedSummaryRows,
    Sort(SortKey),
    SetBaseline,
    ClearBaseline,
    UpdateTasks,
    UpdateProject,
    InsertTask,
    InsertSummary,
    InsertMilestone,
    TaskInfo,
    Notes,
    AddToTimeline,
    ScrollToTask,
    Find,
    Clear,
    FillDown,
    AssignResources,
    AddResource(ResourceType),
    ResourceInfo,
    /// Resource > Notes: Resource Information on its Notes tab
    ResourceNotes,
    /// Resource > Leveling Options
    LevelingOptions,
    /// Task > Format Painter
    FormatPainter,
    /// Report tab: show a report
    Report(crate::reports::ReportKind),
    /// Report > Compare Projects
    CompareProjects,
    /// Project > WBS > Define Code / Renumber (true = selected tasks only)
    WbsDefine,
    WbsRenumber(bool),
    /// Project > Insert > Subproject
    InsertSubproject,
    /// Project > Custom Fields (Format > Custom Fields, column menu)
    CustomFields,
    /// Format > Gridlines > Progress Lines (at the status date)
    ProgressLines,
    /// View > Details (the Task Form under the chart)
    ToggleDetails,
    /// Project > Manage Sprints
    ManageSprints,
    /// Task > Move > Reschedule (0 when resources are available, 1 incomplete parts, 2 completed parts)
    Reschedule(u8),
    /// View > Macros: the list / record / stop recording
    Macros,
    MacroRecord,
    MacroStop,
    /// Project > Links Between Projects
    ProjectLinks,
    /// Project > Spelling (F7)
    Spelling,
    /// Resource > Resource Pool > Share Resources: this project takes the resources of another file
    ShareResources,
    /// Resource > Resource Pool > Refresh Resource Pool / Update Resource Pool
    RefreshPool,
    UpdatePool,
    /// View > Window > Arrange All (the one window fills the screen)
    ArrangeAll,
    /// Format > Drawing: draw a shape (0 text box .. 4 oval); edit a text box; delete the selected one
    DrawingTool(u8),
    DrawingText,
    DrawingDelete,
    /// View > Window: New Window (this file in a second window), Hide (minimise)
    NewWindow,
    HideWindow,
    /// Window > Switch Windows: window number
    SwitchWindow(u32),
    /// View > Unhide: window number
    UnhideWindow(u32),
    /// close the active window (the project stays open in its others)
    CloseWindow,
    /// Format > Task Path (0 = Remove Highlighting)
    TaskPath(u8),
    /// Format > Baseline / Slippage: which baseline (None = off)
    BaselineBars(Option<u8>),
    Slippage(Option<u8>),
    /// Resource > Next Overallocation
    NextOverallocation,
    DeleteResource,
    Overview,
    ProjectInfo,
    WorkingTime,
    Calculate,
    StatusDate,
    ZoomIn,
    ZoomOut,
    Timescale(f32),
    EntireProject,
    ZoomDialog,
    SelectedTasks,
    ToggleTimeline,
    ToggleCritical,
    ToggleOutlineNumber,
    ToggleSummary,
    /// Format > Project Summary Task
    ToggleProjectSummary,
    ShowOutline(Option<u8>),
    InsertColumn(TCol),
    About,
    Shortcuts,
    /// Help > Check for Updates
    CheckUpdates,
    /// Home / New page template (0 blank, 1 construction sample, 2 copy of existing)
    NewTemplate(u8),
    ShowBackstage(Backstage),
    ToggleFavorite(std::path::PathBuf),
    CollapseRibbon,
    // ---- Format tab / dialogs
    TextStyles,
    Gridlines,
    Layout,
    BarStyles,
    FormatBar,
    TimescaleDialog(u8),
    FieldSettings,
    FontDialog,
    Options(u8),
    WrapText,
    AlignCol(u8),
    FontName(String),
    FontSize(f32),
    FontColor(Option<crate::format::Rgb>),
    CellBackground(Option<crate::format::Rgb>),
    ToggleSlack,
    ToggleLate,
    Gallery(u8),
    HideColumn,
    SetLanguage(u8),
}

pub fn run(app: &mut App, ctx: &egui::Context, cmd: Cmd) {
    if !matches!(cmd, Cmd::Undo | Cmd::Redo) {
        app.commit_edit();
    }
    // View > Macros > Record Macro: the commands used are kept (not the macro commands themselves,
    // nor files and windows)
    if let Some(rec) = app.recording.as_mut() {
        if !matches!(cmd, Cmd::MacroRecord | Cmd::MacroStop | Cmd::Macros | Cmd::New | Cmd::Open | Cmd::OpenRecent(_) | Cmd::Save | Cmd::SaveAs | Cmd::Exit | Cmd::CloseDoc | Cmd::NewWindow | Cmd::HideWindow | Cmd::Export(_) | Cmd::Picture(_)) {
            rec.1.push(cmd.clone());
        }
    }
    match cmd {
        Cmd::New => app.cmd_new(),
        Cmd::Open => app.cmd_open(),
        Cmd::OpenRecent(p) => app.cmd_open_path(p),
        Cmd::Save => {
            app.cmd_save();
        }
        Cmd::SaveAs => {
            app.cmd_save_as();
        }
        Cmd::Export(e) => app.cmd_export(e),
        Cmd::Picture(save) => crate::picture::request(app, save),
        Cmd::ImportBoq => {
            if let Some(p) = rfd::FileDialog::new().set_title(crate::i18n::tr("Import BOQ").to_string()).add_filter("BOQ (*.xlsx; *.csv)", &["xlsx", "xlsm", "csv"]).pick_file() {
                crate::boq::open(app, &p);
            }
        }
        Cmd::Payments => crate::payments::open(app),
        Cmd::CloseDoc => app.cmd_new(),
        Cmd::Exit => app.cmd_exit(),
        Cmd::Undo => {
            app.editing = None;
            app.undo()
        }
        Cmd::Redo => {
            app.editing = None;
            app.redo()
        }
        Cmd::View(v) => {
            app.view = v;
            app.backstage = None;
        }
        Cmd::Paste => app.paste(),
        Cmd::Cut => app.copy_selected(true),
        Cmd::Copy => {
            // MPP: the selected cells go to the Windows clipboard as a table (Excel, Word ...)
            // and the tasks to its own clipboard for Paste
            let (text, n) = app.selection_text();
            app.copy_selected(false);
            if n > 0 {
                ctx.copy_text(text);
                app.status = crate::i18n::trf("Copied {} cell(s)", &[&n.to_string()]);
            }
        }
        Cmd::Bold => app.toggle_cell_attr(0),
        Cmd::Italic => app.toggle_cell_attr(1),
        Cmd::Underline => app.toggle_cell_attr(2),
        Cmd::FontName(name) => {
            theme::ensure_font(ctx, &name);
            app.format_selected_cells(move |c| c.font = if name == crate::format::DEFAULT_FONT { None } else { Some(name.clone()) });
        }
        Cmd::SetLanguage(l) => {
            app.settings.language = l;
            app.settings.language_chosen = true;
            app.apply_settings();
            // the last status message was written in the old language
            app.status = "Ready".into();
        }
        Cmd::HideColumn => {
            if app.sel_col < app.task_cols.len() && app.task_cols[app.sel_col].0 != TCol::Name {
                app.task_cols.remove(app.sel_col);
                app.sel_col = app.sel_col.min(app.task_cols.len().saturating_sub(1));
            }
        }
        Cmd::FontSize(size) => app.format_selected_cells(move |c| c.size = Some(size)),
        Cmd::FontColor(col) => app.format_selected_cells(move |c| c.color = col),
        Cmd::CellBackground(col) => app.format_selected_cells(move |c| c.background = col),
        Cmd::WrapText => {
            if let Some(col) = app.task_cols.get(app.sel_col).map(|c| c.0) {
                if app.wrap_cols.contains(&col) {
                    app.wrap_cols.retain(|c| *c != col);
                } else {
                    app.wrap_cols.push(col);
                }
            }
        }
        Cmd::AlignCol(a) => {
            if let Some(col) = app.task_cols.get(app.sel_col).map(|c| c.0) {
                app.col_align.retain(|(c, _)| *c != col);
                app.col_align.push((col, a));
            }
        }
        Cmd::ToggleSlack => app.doc.formatting.show_slack = !app.doc.formatting.show_slack,
        Cmd::ToggleLate => app.doc.formatting.show_late = !app.doc.formatting.show_late,
        Cmd::Gallery(g) => crate::fmt_dialogs::apply_gallery(app, g),
        Cmd::TextStyles => crate::fmt_dialogs::open_text_styles(app),
        Cmd::Gridlines => crate::fmt_dialogs::open_gridlines(app),
        Cmd::Layout => crate::fmt_dialogs::open_layout(app),
        Cmd::BarStyles => crate::fmt_dialogs::open_bar_styles(app),
        Cmd::FormatBar => crate::fmt_dialogs::open_format_bar(app),
        Cmd::TimescaleDialog(tab) => crate::fmt_dialogs::open_timescale(app, tab),
        Cmd::FieldSettings => crate::fmt_dialogs::open_field_settings(app),
        Cmd::FontDialog => crate::fmt_dialogs::open_font(app),
        Cmd::Options(page) => crate::fmt_dialogs::open_options(app, page),
        Cmd::Percent(p) => app.set_percent(p),
        Cmd::MarkOnTrack => app.mark_on_track(),
        Cmd::RespectLinks => {
            let sel = app.selected_tasks();
            app.checkpoint();
            for i in sel {
                app.doc.tasks[i].clear_constraint();
            }
            app.recalc();
        }
        Cmd::Inactivate => app.toggle_inactive(),
        Cmd::Link => app.link_selected(),
        Cmd::Unlink => app.unlink_selected(),
        Cmd::Indent => app.indent(1),
        Cmd::Outdent => app.indent(-1),
        Cmd::Manual => app.set_mode(true),
        Cmd::Auto => app.set_mode(false),
        Cmd::Inspect => {
            if let Some(i) = app.current_task() {
                app.dialog = Some(Dialog::Inspect(i));
            }
        }
        Cmd::MoveUp => app.move_selected(-1),
        Cmd::MoveTask(k) => {
            let du = app.dur_units();
            let m = match k {
                0 => du.mpd,
                1 => du.mpw,
                _ => 4 * du.mpw,
            };
            app.move_tasks_by(m);
        }
        Cmd::Table(k) => app.apply_table(k),
        Cmd::LevelAll => app.level_resources(None),
        Cmd::PrintNow => app.cmd_print(),
        Cmd::SplitTask => app.split_mode = !app.split_mode,
        Cmd::Group(g) => {
            app.commit_edit();
            app.group = g;
            app.sel_row = 0;
            app.sel_anchor = 0;
            app.scroll_y = 0.0;
        }
        Cmd::PrintPage => {
            app.commit_edit();
            app.backstage = Some(Backstage::Print);
        }
        Cmd::Filter(f, hl) => app.set_filter(f, hl),
        Cmd::RelatedSummaryRows => app.related_summary = !app.related_summary,
        Cmd::Sort(k) => {
            app.commit_edit();
            app.sort = k;
        }
        Cmd::LevelResource => {
            if let Some(r) = app.doc.resources.get(app.res_sel_row) {
                let u = r.uid;
                app.level_resources(Some(u));
            }
        }
        Cmd::ClearLeveling => app.clear_leveling(),
        Cmd::SetBaseline => crate::dialogs::open_baseline(app, false),
        Cmd::ClearBaseline => crate::dialogs::open_baseline(app, true),
        Cmd::UpdateTasks => crate::dialogs::open_update_tasks(app),
        Cmd::UpdateProject => crate::dialogs::open_update_project(app),
        Cmd::MoveTaskCustom => {
            if !app.selected_tasks().is_empty() {
                let du = app.dur_units();
                let txt = crate::util::fmt_dur(du.mpd, crate::model::DurUnit::Days, false, du, &app.doc.options, crate::i18n::is_thai());
                app.dialog = Some(Dialog::MoveTasks(txt, None));
            }
        }
        Cmd::MoveProject => {
            let txt = app.fmt_date(app.doc.start);
            app.dialog = Some(Dialog::MoveProject(txt, true, None));
        }
        Cmd::MoveDown => app.move_selected(1),
        Cmd::InsertTask => app.insert_task("task"),
        Cmd::InsertSummary => app.insert_task("summary"),
        Cmd::InsertMilestone => app.insert_task("milestone"),
        Cmd::TaskInfo => dialogs::open_task_info(app, 0),
        Cmd::Notes => dialogs::open_task_info(app, 4),
        Cmd::AddToTimeline => app.toggle_timeline_for_selection(),
        Cmd::ScrollToTask => app.scroll_to_task(),
        Cmd::Find => app.dialog = Some(Dialog::Find),
        Cmd::Clear => app.clear_selection_cells(),
        Cmd::FillDown => app.fill_down(),
        Cmd::AssignResources => {
            if app.current_task().is_some() {
                app.dialog = Some(Dialog::AssignResources);
            }
        }
        Cmd::AddResource(t) => app.add_resource(t),
        Cmd::ResourceInfo => {
            if app.res_sel_row < app.doc.resources.len() {
                dialogs::open_resource_info(app, app.res_sel_row);
            }
        }
        Cmd::ResourceNotes => {
            if app.res_sel_row < app.doc.resources.len() {
                dialogs::open_resource_info_tab(app, app.res_sel_row, 2);
            }
        }
        Cmd::LevelingOptions => {
            let o = app.doc.options.leveling.clone();
            let (from, to) = match o.range {
                Some((a, b)) => (app.fmt_date(a), app.fmt_date(b)),
                None => (String::new(), String::new()),
            };
            app.dialog = Some(Dialog::LevelingOptions(o, from, to, None));
        }
        Cmd::NextOverallocation => app.next_overallocation(),
        Cmd::Report(k) => {
            app.report = k;
            app.recent_reports.retain(|r| *r != k);
            app.recent_reports.insert(0, k);
            app.recent_reports.truncate(8);
            app.view = View::Report;
            app.backstage = None;
        }
        Cmd::CompareProjects => crate::compare::open_compare(app),
        Cmd::WbsDefine => {
            let m = app.doc.wbs_mask.clone().unwrap_or_default();
            app.dialog = Some(Dialog::WbsCode(m));
        }
        Cmd::WbsRenumber(sel) => app.renumber_wbs(sel),
        Cmd::ProgressLines => {
            app.doc.formatting.progress_lines = !app.doc.formatting.progress_lines;
            app.dirty = true;
        }
        Cmd::NewWindow => app.dialog = Some(Dialog::NewWindow(app.view)),
        Cmd::HideWindow => app.hide_window(),
        Cmd::SwitchWindow(n) | Cmd::UnhideWindow(n) => app.switch_window(n),
        Cmd::CloseWindow => app.close_window(),
        Cmd::CustomFields => {
            let f = match app.task_cols.get(app.sel_col).map(|c| c.0) {
                Some(crate::app::TCol::Custom(f)) => Some(f),
                _ => None,
            };
            dialogs::open_custom_fields(app, f);
        }
        Cmd::InsertSubproject => {
            if let Some(p) = rfd::FileDialog::new().set_title(crate::i18n::tr("Insert Project").to_string()).add_filter("Project (*.mpp; *.xml; *.neramit)", &["mpp", "xml", "neramit"]).pick_file() {
                app.insert_subproject(&p);
            }
        }
        Cmd::FormatPainter => {
            if app.format_painter.is_some() {
                app.format_painter = None;
            } else {
                app.pick_format();
            }
        }
        Cmd::TaskPath(k) => {
            app.task_path = if k == 0 { 0 } else { app.task_path ^ k };
        }
        Cmd::BaselineBars(b) => {
            app.doc.formatting.baseline_bars = if app.doc.formatting.baseline_bars == b { None } else { b };
            app.dirty = true;
        }
        Cmd::Slippage(b) => {
            app.doc.formatting.slippage = if app.doc.formatting.slippage == b { None } else { b };
            app.dirty = true;
        }
        Cmd::DeleteResource => app.delete_resource(),
        Cmd::Overview => app.dialog = Some(Dialog::Overview),
        Cmd::ProjectInfo => dialogs::open_project_info(app),
        Cmd::WorkingTime => dialogs::open_working_time(app),
        Cmd::Calculate => {
            app.recalc();
            app.status = "Project recalculated".into();
        }
        Cmd::StatusDate => dialogs::open_project_info(app),
        Cmd::ZoomIn => app.zoom(1),
        Cmd::ZoomOut => app.zoom(-1),
        Cmd::Timescale(z) => app.set_zoom(z, None),
        Cmd::ZoomDialog => dialogs::open_zoom(app),
        Cmd::EntireProject => {
            let w = app.chart_view.1;
            let (a, b) = (app.doc.tasks.iter().map(|t| t.start).min(), app.doc.tasks.iter().map(|t| t.finish).max());
            if let (Some(a), Some(b)) = (a, b) {
                app.zoom_to_range(a, b, w);
            }
        }
        Cmd::SelectedTasks => {
            let w = app.chart_view.1;
            let sel = app.selected_tasks();
            let a = sel.iter().map(|&i| app.doc.tasks[i].start).min();
            let b = sel.iter().map(|&i| app.doc.tasks[i].finish).max();
            if let (Some(a), Some(b)) = (a, b) {
                app.zoom_to_range(a, b, w);
            }
        }
        Cmd::ToggleTimeline => app.show_timeline = !app.show_timeline,
        Cmd::Reschedule(k) => app.reschedule(k),
        Cmd::Macros => app.dialog = Some(Dialog::Macros(0)),
        Cmd::MacroRecord => app.dialog = Some(Dialog::RecordMacro(format!("Macro{}", app.macros.len() + 1))),
        Cmd::MacroStop => {
            if let Some(m) = app.recording.take() {
                app.macros.push(m);
                crate::app::save_macros(&app.macros);
                app.status = "Ready".into();
            }
        }
        Cmd::ProjectLinks => app.dialog = Some(Dialog::ProjectLinks(0, 0)),
        Cmd::Spelling => dialogs::open_spelling(app),
        Cmd::ShareResources => {
            let rp = app.doc.resource_pool.clone();
            app.dialog = Some(Dialog::ShareResources(rp.is_none(), rp.as_ref().map(|r| r.path.clone()).unwrap_or_default(), rp.map(|r| r.sharer_first).unwrap_or(false)));
        }
        Cmd::RefreshPool => match app.refresh_pool() {
            Ok((a, u)) => app.status = crate::i18n::trf("Resources refreshed from the pool ({} added, {} updated)", &[&a.to_string(), &u.to_string()]),
            Err(e) => app.show_error(e),
        },
        Cmd::UpdatePool => match app.update_pool() {
            Ok(p) => app.status = crate::i18n::trf("The resource pool {} was updated", &[&p.display().to_string()]),
            Err(e) => app.show_error(e),
        },
        Cmd::ArrangeAll => {
            // the project's windows tiled in the frame (one window: it fills the frame)
            app.arranged = app.window_list().iter().filter(|w| !w.1).count() > 1;
            if !app.arranged {
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
            }
        }
        Cmd::DrawingTool(k) => {
            app.drawing_tool = Some(k);
            if !app.view.is_gantt() {
                app.view = View::Gantt;
            }
        }
        Cmd::DrawingText => {
            if let Some(k) = app.drawing_sel.filter(|k| *k < app.doc.drawings.len()) {
                let t = app.doc.drawings[k].text.clone();
                app.dialog = Some(Dialog::DrawingText(k, t));
            }
        }
        Cmd::DrawingDelete => {
            if let Some(k) = app.drawing_sel.filter(|k| *k < app.doc.drawings.len()) {
                app.checkpoint();
                app.doc.drawings.remove(k);
                app.drawing_sel = None;
                app.dirty = true;
            }
        }
        Cmd::ManageSprints => {
            let rows = app.doc.sprints.iter().map(|s| (s.uid, s.name.clone(), app.fmt_date(s.start), app.fmt_finish(s.finish))).collect();
            app.dialog = Some(Dialog::Sprints(rows, None));
        }
        Cmd::ToggleDetails => {
            app.show_details = !app.show_details;
            app.details = None;
        }
        Cmd::ToggleCritical => app.doc.formatting.show_critical = !app.doc.formatting.show_critical,
        Cmd::ToggleOutlineNumber => app.show_outline_number = !app.show_outline_number,
        Cmd::ToggleProjectSummary => {
            app.doc.show_project_summary = !app.doc.show_project_summary;
            app.dirty = true;
        }
        Cmd::ToggleSummary => {
            app.show_summary_tasks = !app.show_summary_tasks;
            app.sel_row = 0;
            app.sel_anchor = 0;
        }
        Cmd::ShowOutline(l) => app.set_all_collapsed(l),
        Cmd::InsertColumn(c) => {
            if !app.task_cols.iter().any(|x| x.0 == c) {
                let at = (app.sel_col + 1).min(app.task_cols.len());
                app.task_cols.insert(at, (c, c.default_width()));
            }
        }
        Cmd::About => app.dialog = Some(Dialog::About),
        Cmd::Shortcuts => app.dialog = Some(Dialog::Shortcuts),
        Cmd::CheckUpdates => {
            crate::update::check(ctx);
            app.dialog = Some(Dialog::Update);
        }
        Cmd::NewTemplate(k) => {
            if app.dirty {
                app.pending = Some(crate::app::PendingAction::New);
                app.dialog = Some(Dialog::ConfirmSave);
                if k == 0 {
                    app.backstage = None;
                }
            } else {
                app.new_from_template(k);
            }
        }
        Cmd::ShowBackstage(b) => app.backstage = Some(b),
        Cmd::ToggleFavorite(p) => app.toggle_favorite(&p),
        Cmd::CollapseRibbon => app.ribbon_collapsed = !app.ribbon_collapsed,
    }
}


// ================================================================ modern chrome
//
// Layout and labels of the modern window
// (title bar 48 px, tab row 30 px, ribbon 100 px; every ribbon group and control name in order).

pub const TITLE_H: f32 = 40.0;
pub const TABS_H: f32 = 30.0;

/// Height of the tab row.
pub fn tabs_h() -> f32 {
    TABS_H
}
pub const RIBBON_H: f32 = 98.0;

/// Title-bar language switch (globe + current language), left of the window buttons.
fn language_button(ui: &mut Ui, rect: Rect, cmds: &mut Vec<Cmd>) {
    let thai = crate::i18n::is_thai();
    let label = if thai { crate::thai::shape("ไทย").to_string() } else { "EN".to_string() };
    let r = Rect::from_center_size(egui::pos2(rect.right() - 46.0 * 3.0 - 44.0, rect.center().y), Vec2::new(70.0, 24.0));
    let resp = ui.interact(r, ui.scope_id().with("lang_btn"), Sense::click()).on_hover_text(if thai { tr("Language") } else { "Language / ภาษา" });
    let p = ui.painter();
    let fill = if resp.is_pointer_button_down_on() { 64 } else if resp.hovered() { 46 } else { 24 };
    p.rect_filled(r, 12.0, theme::white(fill));
    p.rect_stroke(r, 12.0, Stroke::new(1.0_f32, theme::white(60)), egui::StrokeKind::Inside);
    // globe
    let c = egui::pos2(r.left() + 15.0, r.center().y);
    let s = Stroke::new(1.1_f32, Color32::WHITE);
    p.circle_stroke(c, 6.0, s);
    p.line_segment([c + Vec2::new(-6.0, 0.0), c + Vec2::new(6.0, 0.0)], s);
    p.add(egui::Shape::ellipse_stroke(c, Vec2::new(2.6, 6.0), s));
    p.text(egui::pos2(r.left() + 26.0, r.center().y), Align2::LEFT_CENTER, label, FontId::proportional(12.0), Color32::WHITE);
    white_caret(p, egui::pos2(r.right() - 11.0, r.center().y + 1.0));
    egui::Popup::menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClick).show(|ui| {
        ui.set_min_width(140.0);
        if ui.radio(!thai, "English").clicked() {
            cmds.push(Cmd::SetLanguage(0));
        }
        if ui.radio(thai, crate::thai::shape("ไทย (Thai)").to_string()).clicked() {
            cmds.push(Cmd::SetLanguage(1));
        }
    });
}

pub(crate) fn caption_buttons(ui: &mut Ui, rect: Rect, cmds: &mut Vec<Cmd>) {
    let ctx = ui.ctx().clone();
    let p = ui.painter().clone();
    let bw = 46.0;
    let maxed = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    for (k, which) in ["close", "max", "min"].iter().enumerate() {
        let r = Rect::from_min_size(egui::pos2(rect.right() - bw * (k as f32 + 1.0), rect.top()), Vec2::new(bw, rect.height()));
        let resp = ui.interact(r, ui.scope_id().with(("cap", which)), Sense::click_and_drag());
        // like native caption buttons: act when the button is released over it, even if the mouse
        // moved a few pixels while pressed (egui would otherwise turn that into a drag, not a click).
        // Sensing drag here also keeps a press on the buttons from dragging the window.
        let release_pos = ctx.input(|i| i.pointer.interact_pos());
        let activated = resp.clicked() || (resp.drag_stopped() && release_pos.is_some_and(|p| r.contains(p)));
        // transparent over the header gradient; Windows 11 hover / pressed states
        let down = resp.is_pointer_button_down_on();
        if *which == "close" && (resp.hovered() || down) {
            p.rect_filled(r, 0.0, if down { Color32::from_rgb(0xA5, 0x23, 0x13) } else { Color32::from_rgb(0xC4, 0x2B, 0x1C) });
        } else if down {
            p.rect_filled(r, 0.0, theme::white(52));
        } else if resp.hovered() {
            p.rect_filled(r, 0.0, theme::white(34));
        }
        let c = r.center();
        let s = Stroke::new(1.0_f32, Color32::WHITE);
        match *which {
            "close" => {
                p.line_segment([c + Vec2::new(-5.0, -5.0), c + Vec2::new(5.0, 5.0)], s);
                p.line_segment([c + Vec2::new(5.0, -5.0), c + Vec2::new(-5.0, 5.0)], s);
                if activated {
                    cmds.push(Cmd::Exit);
                }
            }
            "max" => {
                if maxed {
                    p.rect_stroke(Rect::from_center_size(c + Vec2::new(-1.5, 1.5), Vec2::splat(8.0)), 0.0, s, egui::StrokeKind::Inside);
                    p.line_segment([c + Vec2::new(-3.0, -5.0), c + Vec2::new(5.0, -5.0)], s);
                    p.line_segment([c + Vec2::new(5.0, -5.0), c + Vec2::new(5.0, 3.0)], s);
                } else {
                    p.rect_stroke(Rect::from_center_size(c, Vec2::splat(10.0)), 0.0, s, egui::StrokeKind::Inside);
                }
                if activated {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maxed));
                }
            }
            _ => {
                p.line_segment([c + Vec2::new(-5.0, 0.0), c + Vec2::new(5.0, 0.0)], s);
                if activated {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
            }
        }
    }
}

fn white_caret(p: &egui::Painter, c: egui::Pos2) {
    p.add(egui::Shape::convex_polygon(vec![c + Vec2::new(-3.0, -1.5), c + Vec2::new(3.0, -1.5), c + Vec2::new(0.0, 1.5)], Color32::WHITE, Stroke::NONE));
}

pub fn title_bar(app: &mut App, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
    let ctx = ui.ctx().clone();
    let rect = ui.max_rect();
    let p = ui.painter().clone();
    let classic = app.settings.classic_ui;
    if classic {
        theme::vgradient(&p, rect, theme::HEADER_TOP, theme::HEADER_MID);
        // soft light falling from the brand corner
        theme::hgradient(&p, Rect::from_min_size(rect.min, Vec2::new((rect.width() * 0.5).min(700.0), rect.height())), theme::white(18), Color32::TRANSPARENT);
    } else {
        p.rect_filled(rect, 0.0, theme::GREEN);
    }
    let drag = ui.interact(rect, ui.scope_id().with("titlebar"), Sense::click_and_drag());
    if drag.double_clicked() {
        let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
    } else if drag.drag_started_by(egui::PointerButton::Primary) {
        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }

    // Brand: selected application icon + "Neramit Planner" wordmark.
    let cy = rect.center().y;
    theme::paint_title_mark(&ctx, &p, Rect::from_center_size(egui::pos2(rect.left() + 22.0, cy), Vec2::splat(18.0)));
    let mut x = rect.left() + 40.0;
    let brand = p.layout_no_wrap("Neramit Planner".to_string(), theme::bold(13.5), Color32::WHITE);
    let w = brand.size().x;
    p.galley(egui::pos2(x, cy - brand.size().y / 2.0), brand, Color32::WHITE);
    x += w;
    x += 16.0;
    let start_screen = app.backstage.is_some();
    if !start_screen {
        // Quick Access Toolbar: Save, Undo ▾, Redo ▾, Customize ▾
        p.line_segment([egui::pos2(x - 8.0, rect.top() + 12.0), egui::pos2(x - 8.0, rect.bottom() - 12.0)], Stroke::new(1.0_f32, theme::white(70)));
        x += 2.0;
        let items: [(&str, Option<Cmd>, bool, bool); 4] = [
            ("Save (Ctrl+S)", Some(Cmd::Save), true, false),
            ("Undo (Ctrl+Z)", Some(Cmd::Undo), app.can_undo(), true),
            ("Redo (Ctrl+Y)", Some(Cmd::Redo), app.can_redo(), true),
            ("Customize Quick Access Toolbar", None, true, false),
        ];
        for (k, (tip, cmd, enabled, caret)) in items.into_iter().enumerate() {
            let w = if caret { 36.0 } else { 28.0 };
            let r = Rect::from_min_size(egui::pos2(x, rect.top() + 7.0), Vec2::new(w, rect.height() - 14.0));
            let resp = ui.interact(r, ui.scope_id().with(("qat", k)), Sense::click()).on_hover_text(tr(tip));
            if resp.hovered() && enabled {
                p.rect_filled(r, 5.0, theme::white(if resp.is_pointer_button_down_on() { 60 } else { 40 }));
            }
            let ir = Rect::from_center_size(egui::pos2(r.left() + 14.0, r.center().y), Vec2::splat(16.0));
            match k {
                0 => paint_save_white(&p, ir),
                1 => paint_icon(&p, ir, Icon::Undo, enabled),
                2 => paint_icon(&p, ir, Icon::Redo, enabled),
                _ => {
                    p.line_segment([ir.left_center() + Vec2::new(4.0, -3.0), ir.right_center() + Vec2::new(-4.0, -3.0)], Stroke::new(1.0_f32, Color32::WHITE));
                    white_caret(&p, ir.center() + Vec2::new(0.0, 3.0));
                }
            }
            if caret {
                white_caret(&p, egui::pos2(r.right() - 7.0, r.center().y));
            }
            if resp.clicked() && enabled {
                if let Some(c) = cmd {
                    cmds.push(c);
                }
            }
            x += w + 2.0;
        }
        x += 12.0;
    }

    // document title, centred like Office, with state badges
    if !(start_screen && app.path.is_none() && app.doc.tasks.is_empty()) {
        let mut g = p.layout_no_wrap(crate::thai::shape(&app.window_caption(app.win_num)).to_string(), FontId::proportional(12.5), Color32::WHITE);
        let mut badges: Vec<(std::sync::Arc<egui::Galley>, bool)> = Vec::new();
        if app.compat_mode() {
            badges.push((p.layout_no_wrap(tr("Compatibility Mode").to_string(), FontId::proportional(11.0), Color32::WHITE), false));
        }
        let badge_w = |b: &(std::sync::Arc<egui::Galley>, bool)| b.0.size().x + 20.0 + if b.1 { 17.0 } else { 0.0 };
        let dirty = app.dirty;
        let left_limit = x;
        let right_limit = rect.right() - 46.0 * 3.0 - 96.0;
        let extras = if dirty { 14.0 } else { 0.0 } + badges.iter().map(|b| badge_w(b) + 8.0).sum::<f32>();
        if g.size().x + extras > right_limit - left_limit {
            // shorten a long file name ("Master Schedule HMT_Re...mpp") instead of clipping it
            // mid-glyph, so the badges stay visible
            let name = app.window_caption(app.win_num);
            let (stem, ext) = match name.rfind('.') {
                Some(i) if i > 0 => (name[..i].to_string(), name[i..].to_string()),
                _ => (name.clone(), String::new()),
            };
            let chars: Vec<char> = stem.chars().collect();
            let mut n = chars.len();
            while n > 4 {
                n -= 1;
                let short = format!("{}...{}", chars[..n].iter().collect::<String>().trim_end(), ext.trim_start_matches('.'));
                g = p.layout_no_wrap(crate::thai::shape(&short).to_string(), FontId::proportional(12.5), Color32::WHITE);
                if g.size().x + extras <= right_limit - left_limit {
                    break;
                }
            }
        }
        let total = g.size().x + extras;
        let mut tx = (rect.center().x - total / 2.0).max(left_limit);
        if tx + total > right_limit {
            tx = left_limit;
        }
        let clip = p.with_clip_rect(Rect::from_min_max(egui::pos2(left_limit, rect.top()), egui::pos2(right_limit.max(left_limit), rect.bottom())));
        let gw = g.size().x;
        clip.galley(egui::pos2(tx, cy - g.size().y / 2.0), g, Color32::WHITE);
        tx += gw;
        if dirty {
            // unsaved changes
            clip.circle_filled(egui::pos2(tx + 8.0, cy + 0.5), 3.0, theme::GOLD);
            tx += 14.0;
        }
        for b in badges {
            tx += 8.0;
            let w = badge_w(&b);
            let br = Rect::from_min_size(egui::pos2(tx, cy - 10.0), Vec2::new(w, 20.0));
            clip.rect_filled(br, 10.0, theme::white(36));
            clip.rect_stroke(br, 10.0, Stroke::new(1.0_f32, theme::white(40)), egui::StrokeKind::Inside);
            let mut lx = br.left() + 10.0;
            if b.1 {
                // eye: viewing only
                let c = egui::pos2(lx + 6.0, cy);
                clip.add(egui::Shape::ellipse_stroke(c, Vec2::new(6.0, 3.6), Stroke::new(1.2_f32, Color32::WHITE)));
                clip.circle_filled(c, 1.8, Color32::WHITE);
                lx += 17.0;
            }
            let h = b.0.size().y;
            clip.galley(egui::pos2(lx, cy - h / 2.0), b.0, Color32::WHITE);
            tx += w;
        }
    }
    app.title_rect = rect;
}

/// Language switch and the minimize / maximize / close buttons. They live on the top-most layer so
/// that an open dialog (egui modal) can never block them, as with a native window frame. While a
/// dialog is open the title bar can still be dragged.
pub fn window_controls(app: &mut App, ctx: &egui::Context, cmds: &mut Vec<Cmd>) {
    let rect = app.title_rect;
    if rect.width() <= 0.0 {
        return;
    }
    let modal_open = app.dialog.is_some() || app.message.is_some();
    egui::Area::new(egui::Id::unique("window_controls"))
        .order(egui::Order::Tooltip)
        .fixed_pos(rect.min)
        .interactable(true)
        .show(ctx, |ui| {
            if modal_open {
                // keep the frame usable: drag / double-click the title bar
                let drag_r = Rect::from_min_max(rect.min, egui::pos2(rect.right() - 46.0 * 3.0 - 84.0, rect.bottom()));
                let drag = ui.interact(drag_r, egui::Id::unique("title_drag_modal"), Sense::click_and_drag());
                if drag.double_clicked() {
                    let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
                } else if drag.drag_started_by(egui::PointerButton::Primary) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
            }
            language_button(ui, rect, cmds);
            caption_buttons(ui, rect, cmds);
        });
}

fn paint_save_white(p: &egui::Painter, r: Rect) {
    let w = Color32::WHITE;
    p.rect_stroke(r.shrink(1.0), 1.0, Stroke::new(1.2_f32, w), egui::StrokeKind::Inside);
    p.rect_stroke(Rect::from_min_max(r.min + Vec2::new(4.0, 1.0), egui::pos2(r.right() - 4.0, r.top() + 6.0)), 0.0, Stroke::new(1.0_f32, w), egui::StrokeKind::Inside);
    p.rect_stroke(
        Rect::from_min_max(egui::pos2(r.left() + 4.0, r.bottom() - 7.0), egui::pos2(r.right() - 4.0, r.bottom() - 1.0)),
        0.0,
        Stroke::new(1.0_f32, w),
        egui::StrokeKind::Inside,
    );
}

/// Invisible resize borders for the undecorated window.
pub fn resize_borders(ctx: &egui::Context) {
    use egui::viewport::ResizeDirection as D;
    if ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
        return;
    }
    let screen = ctx.content_rect();
    let t = 4.0;
    let zones = [
        (Rect::from_min_max(screen.min, egui::pos2(screen.right(), screen.top() + t)), D::North, egui::CursorIcon::ResizeVertical),
        (Rect::from_min_max(egui::pos2(screen.left(), screen.bottom() - t), screen.max), D::South, egui::CursorIcon::ResizeVertical),
        (Rect::from_min_max(screen.min, egui::pos2(screen.left() + t, screen.bottom())), D::West, egui::CursorIcon::ResizeHorizontal),
        (Rect::from_min_max(egui::pos2(screen.right() - t, screen.top()), screen.max), D::East, egui::CursorIcon::ResizeHorizontal),
        (Rect::from_min_size(egui::pos2(screen.right() - 8.0, screen.bottom() - 8.0), Vec2::splat(8.0)), D::SouthEast, egui::CursorIcon::ResizeNwSe),
        (Rect::from_min_size(screen.min, Vec2::splat(8.0)), D::NorthWest, egui::CursorIcon::ResizeNwSe),
        (Rect::from_min_size(egui::pos2(screen.right() - 8.0, screen.top()), Vec2::splat(8.0)), D::NorthEast, egui::CursorIcon::ResizeNeSw),
        (Rect::from_min_size(egui::pos2(screen.left(), screen.bottom() - 8.0), Vec2::splat(8.0)), D::SouthWest, egui::CursorIcon::ResizeNeSw),
    ];
    egui::Area::new(egui::Id::unique("resize_borders")).order(egui::Order::Foreground).fixed_pos(screen.min).interactable(true).show(ctx, |ui| {
        for (k, (r, dir, cursor)) in zones.into_iter().enumerate() {
            let resp = ui.interact(r, egui::Id::unique(("rz", k)), Sense::drag());
            if resp.hovered() || resp.dragged() {
                ctx.set_cursor_icon(cursor);
            }
            if resp.drag_started() {
                ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
            }
        }
    });
}

// ---------------------------------------------------------------- tab row

const TABS: [(RibbonTab, &str); 7] = [
    (RibbonTab::Task, "Task"),
    (RibbonTab::Resource, "Resource"),
    (RibbonTab::Report, "Report"),
    (RibbonTab::Project, "Project"),
    (RibbonTab::View, "View"),
    (RibbonTab::Help, "Help"),
    (RibbonTab::Format, "Chart Format"),
];

pub fn tab_row(app: &mut App, ui: &mut Ui) {
    let rect = ui.max_rect();
    let p = ui.painter().clone();
    let classic = app.settings.classic_ui;
    if classic {
        theme::vgradient(&p, rect, theme::HEADER_MID, theme::HEADER_BOTTOM);
    } else {
        p.rect_filled(rect, 0.0, theme::TAB_ROW);
    }
    let font = FontId::proportional(12.5);
    let mut x = rect.left() + 8.0;
    // "File" is an ordinary tab label in the modern look
    let mut tabs: Vec<(Option<RibbonTab>, &str)> = vec![(None, "File")];
    for (t, l) in TABS {
        // the contextual "Format" tab only exists for the Gantt view
        if t == RibbonTab::Format && !app.view.is_gantt() {
            continue;
        }
        tabs.push((Some(t), l));
    }
    for (tab, label) in tabs {
        let contextual = tab == Some(RibbonTab::Format);
        let shown = tr(label);
        let g = ui.painter().layout_no_wrap(shown.to_string(), font.clone(), theme::TEXT);
        let w = g.size().x + 20.0;
        let r = Rect::from_min_size(egui::pos2(x, rect.top() + 1.0), Vec2::new(w, rect.height() - 2.0));
        let ty = 1.0;
        let resp = ui.interact(r, ui.scope_id().with(("tab", label)), Sense::click());
        let active = tab.is_some() && tab == Some(app.tab) && !app.ribbon_collapsed;
        if classic {
            // white tab labels on the green header; the active tab is a light sheet with rounded
            // top corners joined to the ribbon
            let sheet = Rect::from_min_max(egui::pos2(r.left(), rect.top() + 3.0), egui::pos2(r.right(), rect.bottom() + 1.0));
            if active {
                p.rect_filled(sheet, theme::TAB_TOP, theme::CLASSIC_RIBBON_BG);
                p.text(r.center() + Vec2::new(0.0, ty), Align2::CENTER_CENTER, shown, theme::bold(12.5), theme::HEADER_INK);
            } else {
                if resp.hovered() {
                    p.rect_filled(sheet, theme::TAB_TOP, theme::white(if resp.is_pointer_button_down_on() { 54 } else { 36 }));
                }
                p.text(r.center() + Vec2::new(0.0, ty), Align2::CENTER_CENTER, shown, font.clone(), Color32::WHITE);
            }
        } else {
            if resp.hovered() {
                p.rect_filled(r.shrink2(Vec2::new(2.0, 2.0)), 3.0, theme::HOVER_BG);
            }
            let color = if contextual { theme::ACCENT } else { theme::TEXT };
            p.text(r.center(), Align2::CENTER_CENTER, shown, if active { theme::bold(12.5) } else { font.clone() }, color);
            if active {
                let ul = Rect::from_center_size(egui::pos2(r.center().x, r.bottom() - 2.0), Vec2::new(g.size().x.min(w - 12.0), 3.0));
                p.rect_filled(ul, 1.5, theme::ACCENT);
            }
        }
        if resp.clicked() {
            match tab {
                None => {
                    app.commit_edit();
                    app.backstage = Some(Backstage::Info);
                }
                Some(t) => {
                    if app.tab == t {
                        app.ribbon_collapsed = !app.ribbon_collapsed;
                    } else {
                        app.tab = t;
                        app.ribbon_collapsed = false;
                    }
                }
            }
        }
        x += w + 2.0;
    }
}

// ---------------------------------------------------------------- ribbon model

/// One ribbon control. `cmd = None` with `menu = None` means "present but not available in Neramit Planner".
#[derive(Clone)]
enum It {
    /// large button: label (\n for two lines), dropdown
    L { label: &'static str, cmd: Option<Cmd>, menu: Option<&'static str>, on: bool, enabled: bool },
    /// small button with icon + label
    S { label: &'static str, cmd: Option<Cmd>, menu: Option<&'static str>, on: bool, enabled: bool },
    /// icon-only small button (tooltip = label)
    I { label: &'static str, cmd: Option<Cmd>, menu: Option<&'static str>, on: bool, enabled: bool },
    /// check box
    C { label: &'static str, cmd: Option<Cmd>, on: bool, enabled: bool },
    /// combo box: prefix label, current value, width, menu key
    Combo { prefix: &'static str, value: String, width: f32, menu: Option<&'static str> },
    Col(Vec<It>),
    Row(Vec<It>),
    /// Gantt chart style gallery swatches
    Gallery,
    /// whole group folded into one dropdown button (narrow windows)
    Folded { title: &'static str, items: Vec<It> },
}

fn l(label: &'static str, cmd: Option<Cmd>, enabled: bool) -> It {
    It::L { label, cmd, menu: None, on: false, enabled }
}
fn lm(label: &'static str, menu: &'static str, enabled: bool) -> It {
    It::L { label, cmd: None, menu: Some(menu), on: false, enabled }
}
fn s(label: &'static str, cmd: Option<Cmd>, enabled: bool) -> It {
    It::S { label, cmd, menu: None, on: false, enabled }
}
fn sm(label: &'static str, menu: &'static str, enabled: bool) -> It {
    It::S { label, cmd: None, menu: Some(menu), on: false, enabled }
}
fn i(label: &'static str, cmd: Option<Cmd>, enabled: bool) -> It {
    It::I { label, cmd, menu: None, on: false, enabled }
}

fn tab_spec(app: &App) -> Vec<(&'static str, Vec<It>)> {
    let has = app.current_task().is_some() && app.view != View::ResourceSheet;
    let multi = app.selected_tasks().len() >= 2;
    let t = app.current_task().map(|i| app.doc.tasks[i].clone());
    let manual = t.as_ref().map(|t| t.manual);
    let gantt = app.view.is_gantt();
    let res_ok = app.view == View::ResourceSheet && app.res_sel_row < app.doc.resources.len();
    let ts = crate::gantt::timescale_name(app.px_per_day);
    let cell_fmt = app.selected_format();
    let cell_style = app.selected_text_style();
    let cur_col = app.task_cols.get(app.sel_col).map(|c| c.0);
    let col_align = cur_col.map(|c| app.col_alignment(c)).unwrap_or(0);
    let wrapped = cur_col.map(|c| app.wrap_cols.contains(&c)).unwrap_or(false);
    match app.tab {
        RibbonTab::Task => vec![
            ("View", vec![It::L { label: "Gantt\nChart", cmd: None, menu: Some("views"), on: false, enabled: true }]),
            (
                "Clipboard",
                vec![
                    It::L { label: "Paste", cmd: Some(Cmd::Paste), menu: None, on: false, enabled: !app.clipboard.is_empty() },
                    It::Col(vec![
                        s("Cut", Some(Cmd::Cut), has),
                        s("Copy", Some(Cmd::Copy), has),
                        It::S { label: "Format Painter", cmd: Some(Cmd::FormatPainter), menu: None, on: app.format_painter.is_some(), enabled: has },
                    ]),
                ],
            ),
            (
                "Font",
                vec![It::Col(vec![
                    It::Row(vec![
                        It::Combo { prefix: "", value: cell_style.font.clone(), width: 96.0, menu: if has { Some("fontname") } else { None } },
                        It::Combo { prefix: "", value: format!("{}", cell_style.size), width: 44.0, menu: if has { Some("fontsize") } else { None } },
                    ]),
                    It::Row(vec![
                        It::I { label: "Bold", cmd: Some(Cmd::Bold), menu: None, on: cell_fmt.bold, enabled: has },
                        It::I { label: "Italic", cmd: Some(Cmd::Italic), menu: None, on: cell_fmt.italic, enabled: has },
                        It::I { label: "Underline", cmd: Some(Cmd::Underline), menu: None, on: cell_fmt.underline, enabled: has },
                        It::I { label: "Background Color", cmd: None, menu: Some("bgcolor"), on: false, enabled: has },
                        It::I { label: "Font Color", cmd: None, menu: Some("fontcolor"), on: false, enabled: has },
                    ]),
                ])],
            ),
            (
                "Schedule",
                vec![
                    It::Col(vec![
                        It::Row(vec![
                            i("0% Complete", Some(Cmd::Percent(0)), has),
                            i("25% Complete", Some(Cmd::Percent(25)), has),
                            i("50% Complete", Some(Cmd::Percent(50)), has),
                            i("75% Complete", Some(Cmd::Percent(75)), has),
                            i("100% Complete", Some(Cmd::Percent(100)), has),
                        ]),
                        It::Row(vec![
                            i("Outdent", Some(Cmd::Outdent), has),
                            i("Indent", Some(Cmd::Indent), has),
                            It::I { label: "Split Task", cmd: Some(Cmd::SplitTask), menu: None, on: app.split_mode, enabled: has },
                            i("Link Tasks", Some(Cmd::Link), multi),
                            i("Unlink Tasks", Some(Cmd::Unlink), has),
                        ]),
                    ]),
                    It::Col(vec![
                        It::S { label: "Update to Plan", cmd: None, menu: Some("markontrack"), on: false, enabled: has },
                        s("Reapply Links", Some(Cmd::RespectLinks), has),
                    ]),
                ],
            ),
            (
                "Tasks",
                vec![
                    It::L { label: "Manually\nSchedule", cmd: Some(Cmd::Manual), menu: None, on: manual == Some(true), enabled: has },
                    It::L { label: "Auto\nSchedule", cmd: Some(Cmd::Auto), menu: None, on: manual == Some(false), enabled: has },
                    It::L { label: "Trace", cmd: Some(Cmd::Inspect), menu: None, on: false, enabled: has },
                    lm("Move", "move", has),
                    lm("Mode", "mode", true),
                ],
            ),
            (
                "Insert",
                vec![lm("Task", "inserttask", true), l("Summary", Some(Cmd::InsertSummary), true), l("Milestone", Some(Cmd::InsertMilestone), true)],
            ),
            (
                "Properties",
                vec![
                    l("Information", Some(Cmd::TaskInfo), has),
                    It::Col(vec![s("Notes", Some(Cmd::Notes), has), s("Details", Some(Cmd::TaskInfo), has), s("Show on Timeline", Some(Cmd::AddToTimeline), has)]),
                ],
            ),
            (
                "Editing",
                vec![
                    l("Go to\nTask", Some(Cmd::ScrollToTask), has && gantt),
                    It::Col(vec![s("Find...", Some(Cmd::Find), true), sm("Clear", "clear", has), sm("Fill", "fill", multi)]),
                ],
            ),
        ],
        RibbonTab::Resource => vec![
            ("View", vec![It::L { label: "Resource\nSheet", cmd: None, menu: Some("views"), on: app.view == View::ResourceSheet, enabled: true }]),
            ("Assignments", vec![l("Assign\nResources", Some(Cmd::AssignResources), has), lm("Resource\nPool", "pool", true)]),
            ("Insert", vec![lm("Add\nResources", "addres", true)]),
            (
                "Properties",
                vec![l("Information", Some(Cmd::ResourceInfo), res_ok), It::Col(vec![s("Notes...", Some(Cmd::ResourceNotes), res_ok), s("Details", Some(Cmd::ResourceInfo), res_ok)])],
            ),
            (
                "Level",
                vec![
                    l("Level\nSelection", Some(Cmd::LevelAll), true),
                    l("Level\nResource", Some(Cmd::LevelResource), res_ok),
                    l("Level\nAll", Some(Cmd::LevelAll), true),
                    It::Col(vec![s("Leveling Options", Some(Cmd::LevelingOptions), true), s("Clear Leveling", Some(Cmd::ClearLeveling), true), s("Next Overallocation", Some(Cmd::NextOverallocation), app.view != View::ResourceSheet)]),
                ],
            ),
        ],
        RibbonTab::Report => vec![
            ("Project", vec![l("Compare\nProjects", Some(Cmd::CompareProjects), true)]),
            (
                "View Reports",
                vec![
                    lm("New\nReport", "rep_new", true),
                    lm("Dashboards", "rep_dash", true),
                    lm("Resources", "rep_res", true),
                    lm("Costs", "rep_cost", true),
                    lm("In\nProgress", "rep_prog", true),
                    l("Getting\nStarted", Some(Cmd::Report(crate::reports::ReportKind::GettingStarted)), true),
                    l("Custom", Some(Cmd::Report(crate::reports::ReportKind::CustomTable)), true),
                    lm("Recent", "rep_recent", !app.recent_reports.is_empty()),
                    l("Task\nBoards", Some(Cmd::Report(crate::reports::ReportKind::BoardStatus)), true),
                ],
            ),
            ("Export", vec![It::L { label: "Export\nData", cmd: Some(Cmd::Export("csv")), menu: None, on: false, enabled: true }]),
            ("Share", vec![l("Copy\nPicture", Some(Cmd::Picture(false)), true), l("Save\nPicture", Some(Cmd::Picture(true)), true)]),
        ],
        RibbonTab::Project => vec![
            ("Insert", vec![l("Subproject", Some(Cmd::InsertSubproject), true), l("Import\nBOQ", Some(Cmd::ImportBoq), true)]),
            (
                "Properties",
                vec![
                    l("Project\nInformation", Some(Cmd::ProjectInfo), true),
                    l("Custom\nFields", Some(Cmd::CustomFields), true),
                    l("Manage\nSprints", Some(Cmd::ManageSprints), true),
                    l("Payment\nSchedule", Some(Cmd::Payments), true),
                    l("Project\nControls", Some(Cmd::Report(crate::reports::ReportKind::ProjectControls)), true),
                    It::Col(vec![s("Links Between Projects...", Some(Cmd::ProjectLinks), true), sm("WBS", "wbs", true), s("Change Working Time...", Some(Cmd::WorkingTime), true)]),
                ],
            ),
            ("Schedule", vec![l("Calculate\nProject", Some(Cmd::Calculate), true), It::Col(vec![sm("Set Baseline", "baseline", true), s("Move Project", Some(Cmd::MoveProject), true)])]),
            (
                "Status",
                vec![It::Col(vec![
                    It::Combo {
                        prefix: "Status Date:",
                        value: app.doc.status_date.map(|d| app.fmt_date(d)).unwrap_or_else(|| "NA".into()),
                        width: 104.0,
                        menu: Some("statusdate"),
                    },
                    s("Update Project...", Some(Cmd::UpdateProject), true),
                ])],
            ),
            ("Proofing", vec![l("Spelling...", Some(Cmd::Spelling), true)]),
        ],
        RibbonTab::View => vec![
            (
                "Task Views",
                vec![
                    It::L { label: "Gantt\nChart", cmd: Some(Cmd::View(View::Gantt)), menu: None, on: app.view == View::Gantt, enabled: true },
                    It::L { label: "Task\nUsage", cmd: Some(Cmd::View(View::TaskUsage)), menu: None, on: app.view == View::TaskUsage, enabled: true },
                    It::L { label: "Task\nBoard", cmd: Some(Cmd::View(View::TaskBoard)), menu: None, on: app.view == View::TaskBoard, enabled: true },
                    It::Col(vec![
                        It::S { label: "Network Diagram", cmd: Some(Cmd::View(View::NetworkDiagram)), menu: None, on: app.view == View::NetworkDiagram, enabled: true },
                        It::S { label: "Calendar", cmd: Some(Cmd::View(View::Calendar)), menu: None, on: app.view == View::Calendar, enabled: true },
                        sm("Other Views", "taskviews", true),
                    ]),
                ],
            ),
            (
                "Resource Views",
                vec![
                    It::L { label: "Resource\nSheet", cmd: Some(Cmd::View(View::ResourceSheet)), menu: None, on: app.view == View::ResourceSheet, enabled: true },
                    It::Col(vec![It::S { label: "Resource Usage", cmd: Some(Cmd::View(View::ResourceUsage)), menu: None, on: app.view == View::ResourceUsage, enabled: true }, sm("Other Views", "resviews", true)]),
                ],
            ),
            (
                "Data",
                vec![
                    lm("Sort", "sort", true),
                    lm("Outline", "outline", true),
                    lm("Tables", "tables", true),
                    It::Col(vec![
                        It::Combo { prefix: "Highlight:", value: if app.highlight.kind == TaskFilter::All { "[No Highlight]".into() } else { app.highlight.kind.label().trim_end_matches("...").into() }, width: 104.0, menu: Some("highlight") },
                        It::Combo { prefix: "Filter:", value: if app.filter.kind == TaskFilter::All { "[No Filter]".into() } else { app.filter.kind.label().trim_end_matches("...").into() }, width: 104.0, menu: Some("filter") },
                        It::Combo { prefix: "Group by:", value: if app.group == TaskGroup::None { "[No Group]".into() } else { app.group.label().into() }, width: 104.0, menu: Some("group") },
                    ]),
                ],
            ),
            (
                "Zoom",
                vec![
                    It::Col(vec![It::Combo { prefix: "Timescale:", value: ts.into(), width: 112.0, menu: Some("timescale") }]),
                    lm("Zoom", "zoom", gantt),
                    l("Entire\nProject", Some(Cmd::EntireProject), gantt),
                    l("Selected\nTasks", Some(Cmd::SelectedTasks), gantt && has),
                ],
            ),
            (
                "Split View",
                vec![It::Col(vec![
                    It::C { label: "Timeline", cmd: Some(Cmd::ToggleTimeline), on: app.show_timeline, enabled: true },
                    It::C { label: "Details", cmd: Some(Cmd::ToggleDetails), on: app.show_details, enabled: true },
                ])],
            ),
            ("Window", vec![l("New\nWindow", Some(Cmd::NewWindow), true), It::Col(vec![sm("Switch Windows", "windows", true), s("Arrange All", Some(Cmd::ArrangeAll), true), s("Hide", Some(Cmd::HideWindow), true)])]),
            ("Share", vec![l("Copy\nPicture", Some(Cmd::Picture(false)), true), l("Save\nPicture", Some(Cmd::Picture(true)), true)]),
            ("Macros", vec![It::L { label: "Macros", cmd: None, menu: Some("macros"), on: app.recording.is_some(), enabled: true }]),
        ],
        RibbonTab::Help => vec![
            ("Help", vec![l("Help", Some(Cmd::Shortcuts), true), l("About", Some(Cmd::About), true), l("Check for\nUpdates", Some(Cmd::CheckUpdates), true), l("AI\nAssistants", Some(Cmd::Options(crate::fmt_dialogs::AI_PAGE)), true)]),
            ("Language", vec![lm("Language", "language", true)]),
        ],
        RibbonTab::Format => vec![
            ("Format", vec![l("Text\nStyles...", Some(Cmd::TextStyles), true), lm("Gridlines", "gridlines", gantt), l("Layout", Some(Cmd::Layout), gantt)]),
            (
                "Columns",
                vec![
                    lm("Insert\nColumn", "insertcolumn", true),
                    It::Col(vec![
                        It::Row(vec![
                            It::I { label: "Align Left", cmd: Some(Cmd::AlignCol(0)), menu: None, on: col_align == 0, enabled: cur_col.is_some() },
                            It::I { label: "Center", cmd: Some(Cmd::AlignCol(1)), menu: None, on: col_align == 1, enabled: cur_col.is_some() },
                            It::I { label: "Align Right", cmd: Some(Cmd::AlignCol(2)), menu: None, on: col_align == 2, enabled: cur_col.is_some() },
                        ]),
                        It::S { label: "Wrap Text", cmd: Some(Cmd::WrapText), menu: None, on: wrapped, enabled: cur_col.is_some() },
                        sm("Column Settings", "colsettings", cur_col.is_some()),
                    ]),
                    l("Custom\nFields", Some(Cmd::CustomFields), true),
                ],
            ),
            (
                "Bar Styles",
                vec![
                    lm("Format", "format", gantt),
                    It::Col(vec![
                        It::C { label: "Critical Tasks", cmd: Some(Cmd::ToggleCritical), on: app.doc.formatting.show_critical, enabled: true },
                        It::C { label: "Slack", cmd: Some(Cmd::ToggleSlack), on: app.doc.formatting.show_slack, enabled: true },
                        It::C { label: "Late Tasks", cmd: Some(Cmd::ToggleLate), on: app.doc.formatting.show_late, enabled: true },
                    ]),
                    It::L { label: "Task\nPath", cmd: None, menu: Some("taskpath"), on: app.task_path != 0, enabled: gantt },
                    It::L { label: "Baseline", cmd: None, menu: Some("baselinebars"), on: app.doc.formatting.baseline_bars.is_some(), enabled: gantt },
                    It::L { label: "Slippage", cmd: None, menu: Some("slippage"), on: app.doc.formatting.slippage.is_some(), enabled: gantt },
                ],
            ),
            ("Gantt Chart Style", vec![It::Gallery]),
            (
                "Show/Hide",
                vec![It::Col(vec![
                    It::C { label: "Outline Number", cmd: Some(Cmd::ToggleOutlineNumber), on: app.show_outline_number, enabled: true },
                    It::C { label: "Project Summary Task", cmd: Some(Cmd::ToggleProjectSummary), on: app.doc.show_project_summary, enabled: true },
                    It::C { label: "Summary Tasks", cmd: Some(Cmd::ToggleSummary), on: app.show_summary_tasks, enabled: true },
                ])],
            ),
            ("Drawings", vec![It::L { label: "Drawing", cmd: None, menu: Some("drawing"), on: app.drawing_tool.is_some(), enabled: gantt }]),
        ],
    }
}

/// Map a ribbon label to an icon (classic icons where they exist, Fluent outline icons otherwise).
fn draw_icon(p: &egui::Painter, r: Rect, label: &str, enabled: bool) {
    let name = label.replace('\n', " ");
    let icon = match name.as_str() {
        "Gantt Chart" => Some(Icon::Gantt),
        "Paste" => Some(Icon::Paste),
        "Cut" => Some(Icon::Cut),
        "Copy" => Some(Icon::Copy),
        "Format Painter" => Some(Icon::Painter),
        "0% Complete" => Some(Icon::Percent(0)),
        "25% Complete" => Some(Icon::Percent(25)),
        "50% Complete" => Some(Icon::Percent(50)),
        "75% Complete" => Some(Icon::Percent(75)),
        "100% Complete" | "Update to Plan" => Some(Icon::Percent(100)),
        "Outdent" => Some(Icon::Outdent),
        "Indent" => Some(Icon::Indent),
        "Link Tasks" | "Reapply Links" => Some(Icon::Link),
        "Unlink Tasks" => Some(Icon::Unlink),
        "Manually Schedule" => Some(Icon::Manual),
        "Auto Schedule" | "Mode" => Some(Icon::Auto),
        "Trace" => Some(Icon::Inspect),
        "Move" => Some(Icon::Move),
        "Go to Task" => Some(Icon::Scroll),
        "Task" => Some(Icon::Task),
        "Summary" => Some(Icon::Summary),
        "Milestone" => Some(Icon::Milestone),
        "Information" | "About" => Some(Icon::Info),
        "Check for Updates" => Some(Icon::Update),
        "AI Assistants" => Some(Icon::Ai),
        "Notes" => Some(Icon::Notes),
        "Show on Timeline" => Some(Icon::Timeline),
        "Find..." => Some(Icon::Find),
        "Clear" => Some(Icon::Delete),
        "Fill" => Some(Icon::Outdent),
        "Resource Sheet" => Some(Icon::ResourceSheet),
        "Assign Resources" => Some(Icon::AssignRes),
        "Add Resources" => Some(Icon::AddRes),
        "Project Information" => Some(Icon::ProjectInfo),
        "Change Working Time..." => Some(Icon::Calendar),
        "Calculate Project" => Some(Icon::Calculate),
        "Entire Project" => Some(Icon::Entire),
        "Selected Tasks" => Some(Icon::Selected),
        "Insert Column" => Some(Icon::Sheet),
        "Dashboards" => Some(Icon::Report),
        "Zoom" | "Zoom In" | "Zoom..." => Some(Icon::ZoomIn),
        "Zoom Out" => Some(Icon::ZoomOut),
        "Change Working Time" => Some(Icon::Calendar),
        "Export Data" => Some(Icon::Export),
        _ => None,
    };
    match icon {
        Some(ic) => paint_icon(p, r, ic, enabled),
        None => theme::paint_named(p, r, &name, enabled),
    }
}

const BODY_H: f32 = 72.0;
/// Large buttons: room either side of the label, and the narrowest button (so short labels
/// such as Task, Move or Mode do not crowd their neighbours' icons).
const LARGE_PAD: f32 = 10.0;
const LARGE_MIN_W: f32 = 46.0;

/// Render one control; returns its response when interactive.
fn item(ui: &mut Ui, it: &It, cmds: &mut Vec<Cmd>, app: &mut App) {
    match it {
        It::L { label, cmd, menu, on, enabled } => {
            let font = FontId::proportional(11.5);
            let lines: Vec<String> = tr(label).split('\n').map(|s| s.to_string()).collect();
            let mut shown = lines.clone();
            if menu.is_some() {
                let last = shown.len() - 1;
                shown[last] = format!("{} ▾", shown[last]);
            }
            let (pad, min_w) = (LARGE_PAD, LARGE_MIN_W);
            let tw = shown.iter().map(|l| ui.painter().layout_no_wrap(l.clone(), font.clone(), theme::TEXT).size().x + pad).fold(0.0f32, f32::max);
            let w = tw.max(min_w);
            let (r, resp) = ui.allocate_exact_size(Vec2::new(w, BODY_H), if *enabled { Sense::click() } else { Sense::hover() });
            let resp = resp.on_hover_text(tr(label).replace('\n', " "));
            let p = ui.painter();
            if *on {
                p.rect_filled(r.shrink(1.0), 5.0, theme::RIBBON_CHECKED);
                p.rect_stroke(r.shrink(1.0), 5.0, Stroke::new(1.0_f32, theme::RIBBON_CHECKED_EDGE), egui::StrokeKind::Inside);
            } else if resp.hovered() && *enabled {
                p.rect_filled(r.shrink(1.0), 5.0, if resp.is_pointer_button_down_on() { theme::RIBBON_CHECKED } else { theme::RIBBON_HOVER });
            }
            draw_icon(p, Rect::from_center_size(egui::pos2(r.center().x, r.top() + 19.0), Vec2::splat(28.0)), label, *enabled);
            for (k, line) in shown.iter().enumerate() {
                theme::text(
                    p,
                    egui::pos2(r.center().x, r.top() + 44.0 + k as f32 * 13.0),
                    Align2::CENTER_CENTER,
                    line,
                    font.clone(),
                    if *enabled { theme::TEXT } else { theme::DISABLED },
                );
            }
            handle(ui, &resp, cmd, menu, cmds, app);
        }
        It::S { label, cmd, menu, on, enabled } | It::I { label, cmd, menu, on, enabled } => {
            let icon_only = matches!(it, It::I { .. });
            let font = FontId::proportional(12.0);
            let text = if menu.is_some() { format!("{} ▾", tr(label)) } else { tr(label).to_string() };
            let tw = if icon_only { 0.0 } else { ui.painter().layout_no_wrap(text.clone(), font.clone(), theme::TEXT).size().x + 12.0 };
            let (r, resp) = ui.allocate_exact_size(Vec2::new(24.0 + tw, 22.0), if *enabled { Sense::click() } else { Sense::hover() });
            let resp = resp.on_hover_text(tr(label));
            let p = ui.painter();
            if *on {
                p.rect_filled(r, 4.0, theme::RIBBON_CHECKED);
                p.rect_stroke(r, 4.0, Stroke::new(1.0_f32, theme::RIBBON_CHECKED_EDGE), egui::StrokeKind::Inside);
            } else if resp.hovered() && *enabled {
                p.rect_filled(r, 4.0, if resp.is_pointer_button_down_on() { theme::RIBBON_CHECKED } else { theme::RIBBON_HOVER });
            }
            let ir = Rect::from_center_size(egui::pos2(r.left() + 12.0, r.center().y), Vec2::splat(16.0));
            match *label {
                "Bold" | "Italic" | "Underline" => {
                    let f = if *label == "Bold" { theme::bold(13.0) } else { FontId::proportional(13.0) };
                    let g = &label[..1];
                    p.text(ir.center(), Align2::CENTER_CENTER, g, f, if *enabled { theme::TEXT } else { theme::DISABLED });
                    if *label == "Underline" {
                        p.line_segment([ir.center() + Vec2::new(-4.0, 7.0), ir.center() + Vec2::new(4.0, 7.0)], Stroke::new(1.0_f32, theme::TEXT));
                    }
                }
                _ => draw_icon(p, ir, label, *enabled),
            }
            if !icon_only {
                theme::text(p, egui::pos2(r.left() + 26.0, r.center().y), Align2::LEFT_CENTER, &text, font, if *enabled { theme::TEXT } else { theme::DISABLED });
            }
            handle(ui, &resp, cmd, menu, cmds, app);
        }
        It::C { label, cmd, on, enabled } => {
            let font = FontId::proportional(12.0);
            let tw = ui.painter().layout_no_wrap(tr(label).to_string(), font.clone(), theme::TEXT).size().x;
            let (r, resp) = ui.allocate_exact_size(Vec2::new(tw + 26.0, 22.0), if *enabled { Sense::click() } else { Sense::hover() });
            let p = ui.painter();
            if resp.hovered() && *enabled {
                p.rect_filled(r, 4.0, theme::RIBBON_HOVER);
            }
            let b = Rect::from_center_size(egui::pos2(r.left() + 10.0, r.center().y), Vec2::splat(13.0));
            if *on {
                p.rect_filled(b, 3.0, if *enabled { theme::BRAND_GREEN } else { theme::DISABLED });
                p.add(egui::Shape::line(
                    vec![b.left_center() + Vec2::new(2.8, 0.2), egui::pos2(b.center().x - 0.8, b.bottom() - 3.2), b.right_top() + Vec2::new(-2.6, 3.2)],
                    Stroke::new(1.6_f32, Color32::WHITE),
                ));
            } else {
                p.rect_filled(b, 3.0, Color32::WHITE);
                p.rect_stroke(b, 3.0, Stroke::new(1.0_f32, if !*enabled { theme::DISABLED } else if resp.hovered() { theme::BRAND_GREEN } else { theme::TEXT_DIM }), egui::StrokeKind::Inside);
            }
            p.text(egui::pos2(r.left() + 22.0, r.center().y), Align2::LEFT_CENTER, tr(label), font, if *enabled { theme::TEXT } else { theme::DISABLED });
            if resp.clicked() {
                if let Some(c) = cmd {
                    cmds.push(c.clone());
                }
            }
        }
        It::Combo { prefix, value, width, menu } => {
            let font = FontId::proportional(12.0);
            let pw = if prefix.is_empty() { 0.0 } else { ui.painter().layout_no_wrap(tr(prefix).to_string(), font.clone(), theme::TEXT).size().x + 6.0 };
            let enabled = menu.is_some();
            let (r, resp) = ui.allocate_exact_size(Vec2::new(pw + width, 22.0), if enabled { Sense::click() } else { Sense::hover() });
            let p = ui.painter();
            if !prefix.is_empty() {
                p.text(egui::pos2(r.left(), r.center().y), Align2::LEFT_CENTER, tr(prefix), font.clone(), theme::TEXT);
            }
            let br = Rect::from_min_max(egui::pos2(r.left() + pw, r.top() + 1.0), egui::pos2(r.right(), r.bottom() - 1.0));
            p.rect_filled(br, 4.0, Color32::WHITE);
            p.rect_stroke(br, 4.0, Stroke::new(1.0_f32, if resp.hovered() && enabled { theme::BRAND_GREEN } else { Color32::from_rgb(0xC8, 0xCC, 0xC8) }), egui::StrokeKind::Inside);
            // a long value (a font name ...) ends in "…" before the arrow instead of running under it
            let mut job = egui::text::LayoutJob::simple_singleline(tr(value).to_string(), font, if enabled { theme::TEXT } else { theme::TEXT_DIM });
            job.wrap = egui::text::TextWrapping::truncate_at_width(br.width() - 4.0 - 16.0);
            let galley = ui.fonts_mut(|f| f.layout_job(job));
            p.galley(egui::pos2(br.left() + 4.0, br.center().y - galley.size().y / 2.0), galley, theme::TEXT);
            let c = egui::pos2(br.right() - 8.0, br.center().y);
            p.add(egui::Shape::convex_polygon(vec![c + Vec2::new(-3.0, -1.5), c + Vec2::new(3.0, -1.5), c + Vec2::new(0.0, 2.0)], theme::TEXT_DIM, Stroke::NONE));
            handle(ui, &resp, &None, menu, cmds, app);
        }
        It::Col(items) => {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(0.0, 1.0);
                for it in items {
                    item(ui, it, cmds, app);
                }
            });
        }
        It::Row(items) => {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(1.0, 0.0);
                for it in items {
                    item(ui, it, cmds, app);
                }
            });
        }
        It::Folded { title, items } => {
            let font = FontId::proportional(11.5);
            let text = format!("{} ▾", tr(title));
            let w = ui.painter().layout_no_wrap(text.clone(), font.clone(), theme::TEXT).size().x + 20.0;
            let (r, resp) = ui.allocate_exact_size(Vec2::new(w.max(46.0), BODY_H), Sense::click());
            let p = ui.painter();
            if resp.hovered() {
                p.rect_filled(r.shrink(1.0), 5.0, theme::RIBBON_HOVER);
            }
            let first = items.iter().find_map(|i| match i {
                It::L { label, .. } | It::S { label, .. } | It::I { label, .. } => Some(*label),
                It::Col(v) | It::Row(v) => v.iter().find_map(|j| match j {
                    It::L { label, .. } | It::S { label, .. } | It::I { label, .. } => Some(*label),
                    _ => None,
                }),
                _ => None,
            });
            draw_icon(p, Rect::from_center_size(egui::pos2(r.center().x, r.top() + 19.0), Vec2::splat(28.0)), first.unwrap_or(title), true);
            theme::text(p, egui::pos2(r.center().x, r.top() + 50.0), Align2::CENTER_CENTER, &text, font, theme::TEXT);
            egui::Popup::menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                ui.horizontal_top(|ui| {
                    ui.set_min_height(BODY_H);
                    for it in items {
                        item(ui, it, cmds, app);
                    }
                });
            });
        }
        It::Gallery => {
            let (r, gresp) = ui.allocate_exact_size(Vec2::new(176.0, BODY_H), Sense::click());
            let p = ui.painter();
            if gresp.clicked() {
                if let Some(pos) = gresp.interact_pointer_pos() {
                    let kx = ((pos.x - r.left() - 8.0) / 56.0).floor().clamp(0.0, 2.0) as u8;
                    let ky = ((pos.y - r.top() - 12.0) / 24.0).floor().clamp(0.0, 1.0) as u8;
                    cmds.push(Cmd::Gallery(ky * 3 + kx));
                }
            }
            let sel = app.doc.formatting.gallery as usize;
            p.rect_stroke(r.shrink2(Vec2::new(0.0, 6.0)), 2.0, Stroke::new(1.0_f32, Color32::from_rgb(0xD8, 0xD8, 0xD8)), egui::StrokeKind::Inside);
            let colors = [
                (Color32::from_rgb(0x8C, 0xB4, 0xE6), Color32::from_rgb(0x41, 0x7A, 0xC0)),
                (Color32::from_rgb(0x9C, 0xD0, 0x9A), Color32::from_rgb(0x2E, 0x77, 0x32)),
                (Color32::from_rgb(0xF4, 0xB1, 0x83), Color32::from_rgb(0xC5, 0x5A, 0x11)),
                (Color32::from_rgb(0xC9, 0xC9, 0xC9), Color32::from_rgb(0x59, 0x59, 0x59)),
                (Color32::from_rgb(0xB4, 0xA7, 0xD6), Color32::from_rgb(0x67, 0x4E, 0xA7)),
                (Color32::from_rgb(0x9F, 0xD5, 0xE6), Color32::from_rgb(0x1F, 0x86, 0xA8)),
            ];
            for (k, (fill, edge)) in colors.iter().enumerate() {
                let x = r.left() + 8.0 + (k % 3) as f32 * 56.0;
                let y = r.top() + 14.0 + (k / 3) as f32 * 24.0;
                let a = Rect::from_min_size(egui::pos2(x, y), Vec2::new(30.0, 6.0));
                let b = Rect::from_min_size(egui::pos2(x + 12.0, y + 9.0), Vec2::new(32.0, 6.0));
                for bar in [a, b] {
                    p.rect_filled(bar, 1.0, *fill);
                    p.rect_stroke(bar, 1.0, Stroke::new(1.0_f32, *edge), egui::StrokeKind::Inside);
                }
                if k == sel {
                    p.rect_stroke(Rect::from_min_size(egui::pos2(x - 4.0, y - 4.0), Vec2::new(52.0, 23.0)), 2.0, Stroke::new(1.0_f32, theme::ACCENT), egui::StrokeKind::Inside);
                }
            }
        }
    }
}

fn handle(_ui: &mut Ui, resp: &egui::Response, cmd: &Option<Cmd>, menu: &Option<&'static str>, cmds: &mut Vec<Cmd>, app: &mut App) {
    if let Some(key) = menu {
        // the font list has a search box: clicks inside it must not close the menu
        let close = if *key == "fontname" { egui::PopupCloseBehavior::CloseOnClickOutside } else { egui::PopupCloseBehavior::CloseOnClick };
        egui::Popup::menu(resp).close_behavior(close).show(|ui| {
            ui.set_min_width(200.0);
            fill_menu(key, ui, cmds, app);
        });
    } else if resp.clicked() {
        if let Some(c) = cmd {
            cmds.push(c.clone());
        }
    }
}

fn fill_menu(key: &str, ui: &mut Ui, cmds: &mut Vec<Cmd>, app: &mut App) {
    // commands from items that are not plain buttons (check boxes, radio buttons)
    let mut pending: Vec<Cmd> = Vec::new();
    fill_menu_items(key, ui, cmds, app, &mut pending);
    cmds.extend(pending);
}

fn fill_menu_items(key: &str, ui: &mut Ui, cmds: &mut Vec<Cmd>, app: &mut App, pending: &mut Vec<Cmd>) {
    let mut b = |ui: &mut Ui, label: &str, cmd: Cmd| {
        if ui.button(tr(label)).clicked() {
            cmds.push(cmd);
        }
    };
    match key {
        "views" | "taskviews" | "resviews" => {
            ui.label(egui::RichText::new(tr("Built-In")).strong());
            if key != "resviews" {
                b(ui, "Gantt Chart", Cmd::View(View::Gantt));
                b(ui, "Task Sheet", Cmd::View(View::TaskSheet));
                b(ui, "Tracking Gantt", Cmd::View(View::TrackingGantt));
                b(ui, "Task Usage", Cmd::View(View::TaskUsage));
                b(ui, "Network Diagram", Cmd::View(View::NetworkDiagram));
                b(ui, "Calendar", Cmd::View(View::Calendar));
                b(ui, "Task Board", Cmd::View(View::TaskBoard));
                b(ui, "Sprint Planning Board", Cmd::View(View::SprintBoard));
            }
            b(ui, "Resource Sheet", Cmd::View(View::ResourceSheet));
            b(ui, "Resource Usage", Cmd::View(View::ResourceUsage));
        }
        "markontrack" => {
            b(ui, "Update to Plan", Cmd::MarkOnTrack);
            b(ui, "Update Tasks", Cmd::UpdateTasks);
        }
        "move" => {
            // the standard Move menu: move the task forward, reschedule parts of it
            ui.label(egui::RichText::new(tr("Move Task")).strong());
            b(ui, "1 Day", Cmd::MoveTask(0));
            b(ui, "1 Week", Cmd::MoveTask(1));
            b(ui, "4 Weeks", Cmd::MoveTask(2));
            b(ui, "Custom...", Cmd::MoveTaskCustom);
            ui.separator();
            ui.label(egui::RichText::new(tr("Reschedule")).strong());
            for (k, l) in ["When Resources Are Available", "Incomplete Parts to Status Date", "Completed Parts to Status Date"].iter().enumerate() {
                b(ui, l, Cmd::Reschedule(k as u8));
            }
            ui.separator();
            b(ui, "Move Task Up", Cmd::MoveUp);
            b(ui, "Move Task Down", Cmd::MoveDown);
        }
        "mode" => {
            if ui.radio(!app.doc.new_tasks_auto, tr("Manually Schedule New Tasks")).clicked() {
                app.doc.new_tasks_auto = false;
            }
            if ui.radio(app.doc.new_tasks_auto, tr("Auto Schedule New Tasks")).clicked() {
                app.doc.new_tasks_auto = true;
            }
        }
        "inserttask" => {
            b(ui, "Task", Cmd::InsertTask);
            b(ui, "Blank Row", Cmd::InsertTask);
        }
        "clear" => {
            b(ui, "Clear Contents", Cmd::Clear);
        }
        "rep_dash" | "rep_res" | "rep_cost" | "rep_prog" => {
            for k in crate::reports::ReportKind::menu(key) {
                b(ui, k.label(), Cmd::Report(*k));
            }
            if key == "rep_dash" {
                ui.separator();
                b(ui, "Project Statistics...", Cmd::Overview);
            }
        }
        "macros" => {
            b(ui, "View Macros", Cmd::Macros);
            if app.recording.is_some() {
                b(ui, "Stop Recording", Cmd::MacroStop);
            } else {
                b(ui, "Record Macro...", Cmd::MacroRecord);
            }
        }
        "pool" => {
            b(ui, "Share Resources...", Cmd::ShareResources);
            let shared = app.doc.resource_pool.is_some();
            if ui.add_enabled(shared, egui::Button::new(tr("Refresh Resource Pool"))).clicked() {
                cmds.push(Cmd::RefreshPool);
                ui.close();
            }
            if ui.add_enabled(shared, egui::Button::new(tr("Update Resource Pool"))).clicked() {
                cmds.push(Cmd::UpdatePool);
                ui.close();
            }
        }
        "drawing" => {
            for (k, name) in crate::drawing::SHAPES.iter().enumerate() {
                b(ui, name, Cmd::DrawingTool(k as u8));
            }
            ui.separator();
            if app.drawing_sel.is_some() {
                b(ui, "Edit Text...", Cmd::DrawingText);
                b(ui, "Delete Drawing", Cmd::DrawingDelete);
            } else {
                let _ = ui.add_enabled(false, egui::Button::new(tr("Delete Drawing")));
            }
        }
        "windows" => {
            // the project's windows, the active one checked, as MPP lists them
            let list = app.window_list();
            let mut k = 0;
            for (num, hidden) in list.iter().copied() {
                if hidden {
                    continue;
                }
                k += 1;
                let cap = app.window_caption(num);
                if ui.radio(num == app.win_num && !app.win_hidden, format!("{k} {}", crate::thai::shape(&cap))).clicked() {
                    cmds.push(Cmd::SwitchWindow(num));
                    ui.close();
                }
            }
            let hidden: Vec<u32> = list.iter().filter(|w| w.1).map(|w| w.0).collect();
            if !hidden.is_empty() {
                ui.separator();
                for num in hidden {
                    let cap = app.window_caption(num);
                    if ui.button(format!("{} {}", tr("Unhide"), crate::thai::shape(&cap))).clicked() {
                        cmds.push(Cmd::UnhideWindow(num));
                        ui.close();
                    }
                }
            }
            if list.len() > 1 {
                ui.separator();
                if ui.button(tr("Close Window")).clicked() {
                    cmds.push(Cmd::CloseWindow);
                    ui.close();
                }
            }
        }
        "wbs" => {
            b(ui, "Define Code...", Cmd::WbsDefine);
            b(ui, "Renumber...", Cmd::WbsRenumber(false));
            b(ui, "Renumber Selected Tasks", Cmd::WbsRenumber(true));
        }
        "rep_new" => {
            b(ui, "Table", Cmd::Report(crate::reports::ReportKind::CustomTable));
            b(ui, "Chart", Cmd::Report(crate::reports::ReportKind::WorkOverview));
            b(ui, "Comparison", Cmd::Report(crate::reports::ReportKind::CostOverview));
            ui.separator();
            b(ui, "Dashboard", Cmd::Report(crate::reports::ReportKind::ProjectOverview));
        }
        "rep_recent" => {
            for k in app.recent_reports.clone() {
                b(ui, k.label(), Cmd::Report(k));
            }
        }
        "taskpath" => {
            for (k, l) in [(1u8, "Predecessors"), (2, "Driving Predecessors"), (4, "Successors"), (8, "Driven Successors")] {
                if ui.checkbox(&mut (app.task_path & k != 0), tr(l)).clicked() {
                    pending.push(Cmd::TaskPath(k));
                }
            }
            ui.separator();
            b(ui, "Remove Highlighting", Cmd::TaskPath(0));
        }
        "baselinebars" | "slippage" => {
            let cur = if key == "baselinebars" { app.doc.formatting.baseline_bars } else { app.doc.formatting.slippage };
            for n in 0..=10u8 {
                let label = if n == 0 { tr("Baseline").to_string() } else { format!("{} {n}", tr("Baseline")) };
                let has_data = app.doc.tasks.iter().any(|t| t.baseline(n).is_some());
                let text = if has_data { label.clone() } else { format!("{label}  ({})", tr("not set")) };
                if ui.radio(cur == Some(n), text).clicked() {
                    pending.push(if key == "baselinebars" { Cmd::BaselineBars(Some(n)) } else { Cmd::Slippage(Some(n)) });
                }
            }
            ui.separator();
            if ui.button(tr("Hide")).clicked() {
                pending.push(if key == "baselinebars" { Cmd::BaselineBars(None) } else { Cmd::Slippage(None) });
            }
        }
        "fill" => {
            b(ui, "Down", Cmd::FillDown);
        }
        "addres" => {
            b(ui, "Work Resource", Cmd::AddResource(ResourceType::Work));
            b(ui, "Material Resource", Cmd::AddResource(ResourceType::Material));
            b(ui, "Cost Resource", Cmd::AddResource(ResourceType::Cost));
        }
        "outline" => {
            b(ui, "All Subtasks", Cmd::ShowOutline(None));
            for lv in 1..=5u8 {
                b(ui, &format!("Level {lv}"), Cmd::ShowOutline(Some(lv)));
            }
        }
        "timescale" => {
            for (lbl, z) in crate::gantt::TIMESCALES {
                b(ui, lbl, Cmd::Timescale(z));
            }
            ui.separator();
            b(ui, "Timescale...", Cmd::TimescaleDialog(0));
        }
        "language" => {
            if ui.radio(!crate::i18n::is_thai(), "English").clicked() {
                cmds.push(Cmd::SetLanguage(0));
            }
            if ui.radio(crate::i18n::is_thai(), crate::thai::shape("ไทย (Thai)").to_string()).clicked() {
                cmds.push(Cmd::SetLanguage(1));
            }
        }
        "fontname" => {
            // every font on the PC, found by typing part of its name. Names are drawn in the
            // program's font: drawing each in its own face would read hundreds of font files.
            let id = egui::Id::unique("font_menu_filter");
            let pass = ui.ctx().cumulative_pass_nr();
            let (mut q, last): (String, u64) = ui.data(|d| d.get_temp(id)).unwrap_or_default();
            let opened = last + 1 < pass;
            if opened {
                q.clear();
            }
            const W: f32 = 230.0;
            ui.set_width(W);
            let edit = ui.add(egui::TextEdit::singleline(&mut q).hint_text(tr("Search fonts")).desired_width(W));
            if opened {
                edit.request_focus();
            }
            let current = app.selected_text_style().font;
            let current = if current.is_empty() { crate::format::DEFAULT_FONT.to_string() } else { current };
            let fonts = theme::find_fonts(&q);
            let mut chosen = None;
            if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                chosen = fonts.first().map(|f| (*f).clone());
            }
            ui.separator();
            if fonts.is_empty() {
                ui.label(egui::RichText::new(tr("No matching font")).weak());
            }
            let row_h = ui.spacing().interact_size.y + ui.spacing().item_spacing.y;
            let mut area = egui::ScrollArea::vertical().max_height(360.0).auto_shrink([false, true]);
            if opened {
                // open at the font in use, a few rows from the top
                if let Some(k) = fonts.iter().position(|f| f.eq_ignore_ascii_case(&current)) {
                    area = area.vertical_scroll_offset((k as f32 - 3.0).max(0.0) * row_h);
                }
            }
            area.show_rows(ui, ui.spacing().interact_size.y, fonts.len(), |ui, rows| {
                ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                    for f in &fonts[rows] {
                        if ui.selectable_label(f.eq_ignore_ascii_case(&current), f.as_str()).clicked() {
                            chosen = Some((*f).clone());
                        }
                    }
                });
            });
            ui.data_mut(|d| d.insert_temp(id, (q, pass)));
            if let Some(f) = chosen {
                cmds.push(Cmd::FontName(f));
                ui.close();
            }
        }
        "fontsize" => {
            for s in [8.0f32, 9.0, 10.0, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 24.0, 28.0, 36.0] {
                b(ui, &format!("{s}"), Cmd::FontSize(s));
            }
        }
        "fontcolor" | "bgcolor" => {
            let bg = key == "bgcolor";
            let mk = |c: Option<crate::format::Rgb>| if bg { Cmd::CellBackground(c) } else { Cmd::FontColor(c) };
            b(ui, if bg { "No Color" } else { "Automatic" }, mk(None));
            ui.label(egui::RichText::new(tr("Theme Colors")).small());
            ui.horizontal(|ui| {
                for c in crate::format::THEME_COLORS {
                    let (r, resp) = ui.allocate_exact_size(Vec2::new(16.0, 16.0), Sense::click());
                    ui.painter().rect_filled(r, 0.0, theme::rgb(c));
                    ui.painter().rect_stroke(r, 0.0, Stroke::new(1.0_f32, Color32::from_rgb(0x90, 0x90, 0x90)), egui::StrokeKind::Inside);
                    if resp.clicked() {
                        cmds.push(mk(Some(c)));
                    }
                }
            });
            ui.label(egui::RichText::new(tr("Standard Colors")).small());
            ui.horizontal_wrapped(|ui| {
                for (name, c) in crate::format::NAMED_COLORS {
                    let (r, resp) = ui.allocate_exact_size(Vec2::new(16.0, 16.0), Sense::click());
                    ui.painter().rect_filled(r, 0.0, theme::rgb(c));
                    ui.painter().rect_stroke(r, 0.0, Stroke::new(1.0_f32, Color32::from_rgb(0x90, 0x90, 0x90)), egui::StrokeKind::Inside);
                    if resp.on_hover_text(name).clicked() {
                        cmds.push(mk(Some(c)));
                    }
                }
            });
        }
        "gridlines" => {
            b(ui, "Gridlines...", Cmd::Gridlines);
            if ui.checkbox(&mut app.doc.formatting.progress_lines.clone(), tr("Progress Lines...")).clicked() {
                pending.push(Cmd::ProgressLines);
            }
        }
        "format" => {
            if app.current_task().is_some() {
                b(ui, "Bar...", Cmd::FormatBar);
            } else {
                let _ = ui.add_enabled(false, egui::Button::new(tr("Bar...")));
            }
            b(ui, "Bar Styles...", Cmd::BarStyles);
            ui.separator();
            b(ui, "Gridlines...", Cmd::Gridlines);
            b(ui, "Layout...", Cmd::Layout);
        }
        "colsettings" => {
            b(ui, "Field Settings...", Cmd::FieldSettings);
            b(ui, "Hide Column", Cmd::HideColumn);
            b(ui, "Wrap Text", Cmd::WrapText);
            b(ui, "Data Type...", Cmd::CustomFields);
            b(ui, "Custom Fields...", Cmd::CustomFields);
        }
        "zoom" => {
            b(ui, "Zoom In", Cmd::ZoomIn);
            b(ui, "Zoom Out", Cmd::ZoomOut);
            b(ui, "Zoom...", Cmd::ZoomDialog);
            ui.separator();
            b(ui, "Entire Project", Cmd::EntireProject);
            b(ui, "Selected Tasks", Cmd::SelectedTasks);
        }
        "filter" | "highlight" => {
            let hl = key == "highlight";
            if ui.button(tr(if hl { "Clear Highlight" } else { "Clear Filter" })).clicked() {
                cmds.push(Cmd::Filter(TaskFilter::All, hl));
            }
            ui.separator();
            ui.label(egui::RichText::new(tr("Built-In")).strong());
            let cur = if hl { app.highlight.kind } else { app.filter.kind };
            for f in TaskFilter::ALL {
                if f == TaskFilter::All && hl {
                    continue;
                }
                if ui.add(egui::Button::new(tr(f.label())).selected(cur == f && f != TaskFilter::All)).clicked() {
                    cmds.push(Cmd::Filter(f, hl));
                }
            }
            if !hl {
                ui.separator();
                let mut on = app.related_summary;
                if ui.checkbox(&mut on, tr("Show Related Summary Rows")).clicked() {
                    cmds.push(Cmd::RelatedSummaryRows);
                }
            }
        }
        "group" => {
            if ui.button(tr("Clear Group")).clicked() {
                cmds.push(Cmd::Group(TaskGroup::None));
            }
            ui.separator();
            ui.label(egui::RichText::new(tr("Built-In")).strong());
            for g in TaskGroup::ALL {
                if ui.add(egui::Button::new(tr(g.label())).selected(app.group == g)).clicked() {
                    cmds.push(Cmd::Group(g));
                }
            }
        }
        "sort" => {
            for k in SortKey::ALL {
                if ui.add(egui::Button::new(tr(k.label())).selected(app.sort == k)).clicked() {
                    cmds.push(Cmd::Sort(k));
                }
            }
        }
        "baseline" => {
            b(ui, "Set Baseline...", Cmd::SetBaseline);
            b(ui, "Clear Baseline...", Cmd::ClearBaseline);
        }
        "tables" => {
            // the standard Tables list (the ones without their fields yet are shown disabled)
            for (k, name) in [(5u8, "Cost"), (0, "Entry"), (8, "Hyperlink"), (1, "Schedule"), (2, "Summary"), (3, "Tracking"), (7, "Usage"), (4, "Variance"), (6, "Work")] {
                if k == 255 {
                    ui.add_enabled(false, egui::Button::new(tr(name)));
                } else if ui.add(egui::Button::new(tr(name)).selected(app.table == k && app.view != View::ResourceSheet)).clicked() {
                    cmds.push(Cmd::Table(k));
                }
            }
            ui.separator();
            if ui.button(tr("Reset to Default")).clicked() {
                cmds.push(Cmd::Table(app.table));
            }
        }
        "statusdate" => {
            b(ui, "Status Date...", Cmd::StatusDate);
        }
        "insertcolumn" => {
            let present: Vec<TCol> = app.task_cols.iter().map(|c| c.0).collect();
            for c in TCol::ALL {
                if !present.contains(&c) && ui.button(tr(c.title())).clicked() {
                    cmds.push(Cmd::InsertColumn(c));
                }
            }
        }
        _ => {}
    }
}

fn text_w(ctx: &egui::Context, s: &str, size: f32) -> f32 {
    ctx.fonts_mut(|f| f.layout_no_wrap(s.to_string(), FontId::proportional(size), theme::TEXT).size().x)
}

/// Natural width of a control (mirrors the sizes used by `item`).
fn item_width(ctx: &egui::Context, it: &It) -> f32 {
    match it {
        It::L { label, menu, .. } => tr(label)
            .split('\n')
            .enumerate()
            .map(|(k, l)| text_w(ctx, &if menu.is_some() && k == tr(label).split('\n').count() - 1 { format!("{l} ▾") } else { l.to_string() }, 11.5) + LARGE_PAD)
            .fold(LARGE_MIN_W, f32::max),
        It::S { label, menu, .. } => 24.0 + text_w(ctx, &if menu.is_some() { format!("{} ▾", tr(label)) } else { tr(label).to_string() }, 12.0) + 12.0,
        It::I { .. } => 24.0,
        It::C { label, .. } => text_w(ctx, tr(label), 12.0) + 26.0,
        It::Combo { prefix, width, .. } => if prefix.is_empty() { *width } else { text_w(ctx, tr(prefix), 12.0) + 6.0 + width },
        It::Col(items) => items.iter().map(|i| item_width(ctx, i)).fold(0.0, f32::max),
        It::Row(items) => items.iter().map(|i| item_width(ctx, i) + 1.0).sum(),
        It::Gallery => 176.0,
        It::Folded { title, .. } => text_w(ctx, &format!("{} ▾", tr(title)), 11.5) + 10.0f32.max(0.0) + 10.0,
    }
}

fn group_width(ctx: &egui::Context, title: &str, items: &[It]) -> f32 {
    let content: f32 = items.iter().map(|i| item_width(ctx, i) + 3.0).sum();
    content.max(text_w(ctx, tr(title), 11.0) + 8.0) + 16.0
}

/// Convert runs of large buttons into columns of (up to three) small buttons.
fn compact(items: Vec<It>) -> Vec<It> {
    let mut out = Vec::new();
    let mut run: Vec<It> = Vec::new();
    let flush = |run: &mut Vec<It>, out: &mut Vec<It>| {
        for chunk in run.chunks(3) {
            out.push(It::Col(chunk.to_vec()));
        }
        run.clear();
    };
    for it in items {
        match it {
            It::L { label, cmd, menu, on, enabled } => {
                // single-line label for the small form
                let short: &'static str = Box::leak(label.replace('\n', " ").into_boxed_str());
                run.push(It::S { label: short, cmd, menu, on, enabled });
            }
            It::Gallery => {
                flush(&mut run, &mut out);
            }
            other => {
                flush(&mut run, &mut out);
                out.push(other);
            }
        }
    }
    flush(&mut run, &mut out);
    out
}

/// Small buttons lose their labels (icon only), as Office does before it compacts large buttons.
fn icon_only(items: Vec<It>) -> Vec<It> {
    items
        .into_iter()
        .map(|it| match it {
            It::S { label, cmd, menu, on, enabled } => It::I { label, cmd, menu, on, enabled },
            It::Col(v) => It::Col(icon_only(v)),
            It::Row(v) => It::Row(icon_only(v)),
            other => other,
        })
        .collect()
}

pub fn ribbon_body(app: &mut App, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
    let rect = ui.max_rect();
    let classic = app.settings.classic_ui;
    let panel;
    if classic {
        theme::vgradient(ui.painter(), rect, theme::CLASSIC_RIBBON_BG, theme::CLASSIC_RIBBON_BOTTOM);
        ui.painter().line_segment([rect.left_bottom() - Vec2::new(0.0, 1.0), rect.right_bottom() - Vec2::new(0.0, 1.0)], Stroke::new(1.0_f32, theme::CLASSIC_RIBBON_EDGE));
        panel = Rect::from_min_max(rect.min, egui::pos2(rect.right(), rect.bottom() - 1.0));
    } else {
        ui.painter().rect_filled(rect, 0.0, theme::TAB_ROW);
        let pn = rect.shrink2(Vec2::new(4.0, 0.0));
        panel = Rect::from_min_max(pn.min, egui::pos2(pn.right(), pn.bottom() - 3.0));
        ui.painter().rect_filled(panel, 6.0, theme::RIBBON_BG);
        ui.painter().rect_stroke(panel, 6.0, Stroke::new(1.0_f32, theme::RIBBON_BORDER), egui::StrokeKind::Inside);
    }
    let mut spec = tab_spec(app);
    // Office-style adaptive ribbon: when the window is too narrow, first drop the labels of small
    // buttons (the standard order for the Task tab: Editing, Properties, Clipboard, ...), then turn
    // large buttons into stacks of small buttons, starting with the right-most group, until it fits.
    // (the collapse chevron sits in the last 25 px, below the group content)
    let avail = panel.width() - 16.0 - 20.0;
    let fonts_ctx = ui.ctx().clone();
    let width_of = |spec: &Vec<(&'static str, Vec<It>)>| -> f32 { spec.iter().map(|(t, items)| group_width(&fonts_ctx, t, items)).sum() };
    let order: Vec<usize> = if app.tab == RibbonTab::Task {
        ["Editing", "Properties", "Clipboard", "Insert", "Tasks", "Schedule", "View"].iter().filter_map(|n| spec.iter().position(|(t, _)| t == n)).collect()
    } else {
        (0..spec.len()).rev().collect()
    };
    for gi in order {
        if width_of(&spec) <= avail {
            break;
        }
        let items = std::mem::take(&mut spec[gi].1);
        spec[gi].1 = icon_only(items);
    }
    let mut gi = spec.len();
    while width_of(&spec) > avail && gi > 0 {
        gi -= 1;
        let items = std::mem::take(&mut spec[gi].1);
        spec[gi].1 = compact(items);
    }
    // still too wide: fold whole groups (right-most first) into a single dropdown button
    let mut gi = spec.len();
    while width_of(&spec) > avail && gi > 1 {
        gi -= 1;
        let title = spec[gi].0;
        let items = std::mem::take(&mut spec[gi].1);
        spec[gi].1 = vec![It::Folded { title, items }];
    }
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(panel.shrink2(Vec2::new(8.0, 3.0))).layout(egui::Layout::left_to_right(egui::Align::Min)));
    let ui2 = &mut child;
    ui2.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
    for (title, items) in &spec {
        let top = ui2.cursor().top();
        let start_x = ui2.cursor().left();
        ui2.horizontal_top(|ui| {
            ui.set_min_height(BODY_H);
            ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);
            for it in items {
                item(ui, it, cmds, app);
            }
        });
        let end_x = ui2.min_rect().right().max(start_x + 30.0);
        let p = ui2.painter().clone();
        let lr = Rect::from_min_max(egui::pos2(start_x, top + BODY_H + 2.0), egui::pos2(end_x, top + BODY_H + 18.0));
        p.text(lr.center(), Align2::CENTER_CENTER, tr(title), FontId::proportional(11.0), Color32::from_rgb(0x6A, 0x70, 0x6B));
        // dialog launcher (Font group opens the Font dialog, like MPP)
        if *title == "Font" {
            let lb = Rect::from_min_size(egui::pos2(end_x + 2.0, lr.top() + 2.0), Vec2::splat(12.0));
            let lresp = ui2.interact(lb, ui2.scope_id().with("font_launcher"), Sense::click()).on_hover_text(tr("Font"));
            let c = if lresp.hovered() { theme::TEXT } else { Color32::from_rgb(0x61, 0x61, 0x61) };
            p.line_segment([lb.left_top() + Vec2::new(2.0, 2.0), lb.right_bottom() - Vec2::new(3.0, 3.0)], Stroke::new(1.0_f32, c));
            p.line_segment([lb.right_bottom() - Vec2::new(3.0, 3.0), lb.right_bottom() - Vec2::new(3.0, 7.0)], Stroke::new(1.0_f32, c));
            p.line_segment([lb.right_bottom() - Vec2::new(3.0, 3.0), lb.right_bottom() - Vec2::new(7.0, 3.0)], Stroke::new(1.0_f32, c));
            p.rect_stroke(Rect::from_min_size(lb.min, Vec2::new(7.0, 7.0)), 0.0, Stroke::new(1.0_f32, c), egui::StrokeKind::Inside);
            if lresp.clicked() {
                cmds.push(Cmd::FontDialog);
            }
            ui2.add_space(12.0);
        }
        let gap = 7.0;
        ui2.add_space(gap);
        let sx = ui2.cursor().left();
        p.line_segment([egui::pos2(sx, top + 8.0), egui::pos2(sx, top + BODY_H + 12.0)], Stroke::new(1.0_f32, Color32::from_rgb(0xDC, 0xDE, 0xDC)));
        ui2.add_space(gap);
    }
    // collapse-the-ribbon chevron (bottom right)
    let cr = Rect::from_center_size(egui::pos2(panel.right() - 16.0, panel.bottom() - 12.0), Vec2::splat(18.0));
    let resp = ui.interact(cr, ui.scope_id().with("collapse_ribbon"), Sense::click()).on_hover_text(tr("Collapse the Ribbon"));
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(cr, 4.0, theme::RIBBON_HOVER);
    }
    let c = cr.center();
    p.add(egui::Shape::line(vec![c + Vec2::new(-5.0, -2.0), c + Vec2::new(0.0, 3.0), c + Vec2::new(5.0, -2.0)], Stroke::new(1.3_f32, theme::TEXT)));
    if resp.clicked() {
        cmds.push(Cmd::CollapseRibbon);
    }
}

// ================================================================ backstage (Home + File)

fn nav_icon(p: &egui::Painter, r: Rect, name: &str, color: Color32) {
    let s = Stroke::new(1.2_f32, color);
    let at = |fx: f32, fy: f32| egui::pos2(r.left() + fx * r.width(), r.top() + fy * r.height());
    match name {
        "Home" => {
            p.add(egui::Shape::line(vec![at(0.1, 0.5), at(0.5, 0.1), at(0.9, 0.5)], s));
            p.add(egui::Shape::line(vec![at(0.22, 0.4), at(0.22, 0.9), at(0.78, 0.9), at(0.78, 0.4)], s));
            p.add(egui::Shape::line(vec![at(0.42, 0.9), at(0.42, 0.62), at(0.58, 0.62), at(0.58, 0.9)], s));
        }
        "New" => {
            p.add(egui::Shape::closed_line(vec![at(0.2, 0.06), at(0.62, 0.06), at(0.82, 0.26), at(0.82, 0.94), at(0.2, 0.94)], s));
            p.add(egui::Shape::line(vec![at(0.62, 0.06), at(0.62, 0.26), at(0.82, 0.26)], s));
        }
        "Open" => {
            p.add(egui::Shape::closed_line(vec![at(0.06, 0.2), at(0.38, 0.2), at(0.48, 0.32), at(0.88, 0.32), at(0.88, 0.86), at(0.06, 0.86)], s));
            p.add(egui::Shape::line(vec![at(0.06, 0.86), at(0.24, 0.5), at(0.98, 0.5), at(0.88, 0.86)], s));
        }
        "Info" | "About" => {
            p.circle_stroke(r.center(), r.width() * 0.42, s);
            p.line_segment([at(0.5, 0.45), at(0.5, 0.75)], s);
            p.circle_filled(at(0.5, 0.3), 1.2, color);
        }
        "Save" | "Save As" => {
            p.rect_stroke(Rect::from_min_max(at(0.1, 0.1), at(0.9, 0.9)), 1.0, s, egui::StrokeKind::Inside);
            p.rect_stroke(Rect::from_min_max(at(0.28, 0.1), at(0.72, 0.36)), 0.0, s, egui::StrokeKind::Inside);
            p.rect_stroke(Rect::from_min_max(at(0.24, 0.56), at(0.76, 0.9)), 0.0, s, egui::StrokeKind::Inside);
            if name == "Save As" {
                p.line_segment([at(0.6, 0.98), at(0.98, 0.6)], Stroke::new(1.6_f32, color));
            }
        }
        "Print" => {
            p.rect_stroke(Rect::from_min_max(at(0.06, 0.36), at(0.94, 0.76)), 1.0, s, egui::StrokeKind::Inside);
            p.rect_stroke(Rect::from_min_max(at(0.24, 0.08), at(0.76, 0.36)), 0.0, s, egui::StrokeKind::Inside);
            p.rect_stroke(Rect::from_min_max(at(0.24, 0.6), at(0.76, 0.94)), 0.0, s, egui::StrokeKind::Inside);
        }
        "Share" => {
            p.add(egui::Shape::line(vec![at(0.1, 0.5), at(0.1, 0.9), at(0.9, 0.9), at(0.9, 0.5)], s));
            p.line_segment([at(0.5, 0.1), at(0.5, 0.66)], s);
            p.add(egui::Shape::line(vec![at(0.3, 0.3), at(0.5, 0.1), at(0.7, 0.3)], s));
        }
        "Export" => {
            p.add(egui::Shape::line(vec![at(0.7, 0.3), at(0.7, 0.06), at(0.14, 0.06), at(0.14, 0.94), at(0.7, 0.94), at(0.7, 0.7)], s));
            p.line_segment([at(0.4, 0.5), at(0.96, 0.5)], s);
            p.add(egui::Shape::line(vec![at(0.8, 0.34), at(0.96, 0.5), at(0.8, 0.66)], s));
        }
        "Close" => {
            p.rect_stroke(Rect::from_min_max(at(0.08, 0.14), at(0.92, 0.86)), 1.0, s, egui::StrokeKind::Inside);
            p.line_segment([at(0.08, 0.32), at(0.92, 0.32)], s);
        }
        _ => {
            // Options
            p.rect_stroke(Rect::from_min_max(at(0.14, 0.06), at(0.86, 0.94)), 1.0, s, egui::StrokeKind::Inside);
            for fy in [0.3f32, 0.5, 0.7] {
                p.circle_filled(at(0.32, fy), 1.4, color);
                p.line_segment([at(0.44, fy), at(0.74, fy)], s);
            }
        }
    }
}

/// File > Print: settings on the left, the pages on the right (as they will print).
fn print_page(app: &mut App, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
    ui.label(egui::RichText::new(tr("Print")).size(22.0).color(theme::TEXT));
    ui.add_space(14.0);
    let full = ui.available_rect_before_wrap();
    let left = Rect::from_min_size(full.min, Vec2::new(300.0, full.height()));
    let right = Rect::from_min_max(egui::pos2(left.right() + 24.0, full.top()), full.max);
    ui.scope_builder(egui::UiBuilder::new().max_rect(left), |ui| {
        ui.horizontal(|ui| {
            let big = egui::Button::new(egui::RichText::new(tr("Print")).size(15.0).strong()).min_size(Vec2::new(96.0, 64.0));
            if ui.add(big).clicked() {
                cmds.push(Cmd::PrintNow);
            }
            ui.vertical(|ui| {
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.label(tr("Copies:"));
                    ui.add(egui::DragValue::new(&mut app.print.copies).range(1..=99));
                });
            });
        });
        ui.add_space(12.0);
        ui.label(egui::RichText::new(tr("Printer")).strong());
        ui.label(tr("Default printer (Windows)"));
        ui.add_space(12.0);
        ui.label(egui::RichText::new(tr("Settings")).strong());
        egui::ComboBox::from_id_salt("print_range").width(280.0).selected_text(tr(if app.print.specific_dates { "Print Specific Dates" } else { "Print Entire Project" })).show_ui(ui, |ui| {
            ui.selectable_value(&mut app.print.specific_dates, false, tr("Print Entire Project"));
            ui.selectable_value(&mut app.print.specific_dates, true, tr("Print Specific Dates"));
        });
        if app.print.specific_dates {
            if app.print.from.is_empty() {
                app.print.from = app.fmt_date(app.doc.start);
                app.print.to = app.fmt_date(app.doc.finish_date());
            }
            ui.horizontal(|ui| {
                ui.label(tr("Dates:"));
                ui.add(egui::TextEdit::singleline(&mut app.print.from).desired_width(100.0));
                ui.label(tr("To"));
                ui.add(egui::TextEdit::singleline(&mut app.print.to).desired_width(100.0));
            });
        }
        ui.add_space(6.0);
        egui::ComboBox::from_id_salt("print_orient").width(280.0).selected_text(tr(if app.print.landscape { "Landscape Orientation" } else { "Portrait Orientation" })).show_ui(ui, |ui| {
            ui.selectable_value(&mut app.print.landscape, true, tr("Landscape Orientation"));
            ui.selectable_value(&mut app.print.landscape, false, tr("Portrait Orientation"));
        });
        ui.add_space(6.0);
        let papers = crate::app::PAPERS;
        egui::ComboBox::from_id_salt("print_paper").width(280.0).selected_text(papers[app.print.paper as usize].0).show_ui(ui, |ui| {
            for (k, (name, w, h)) in papers.iter().enumerate() {
                ui.selectable_value(&mut app.print.paper, k as u8, format!("{name}  ({:.0} x {:.0} mm)", w / 72.0 * 25.4, h / 72.0 * 25.4));
            }
        });
        ui.add_space(6.0);
        ui.checkbox(&mut app.print.signoff, tr("Signature boxes (Prepared / Checked / Approved)"));
        ui.add_space(14.0);
        ui.label(egui::RichText::new(tr("The view's rows print with the Gantt Chart fitted to the page width.")).color(theme::TEXT_DIM));
    });
    // preview of the pages
    let d = app.print_data();
    let pages = crate::export::preview(&d);
    let n = pages.len().max(1);
    app.print.page = app.print.page.min(n - 1);
    let nav_h = 30.0;
    let avail = Rect::from_min_max(right.min, egui::pos2(right.right(), right.bottom() - nav_h));
    let scale = (avail.width() / d.page_w).min(avail.height() / d.page_h).max(0.05);
    let page = Rect::from_center_size(avail.center(), Vec2::new(d.page_w * scale, d.page_h * scale));
    let p = ui.painter().clone();
    p.rect_filled(page.translate(Vec2::new(3.0, 3.0)), 0.0, Color32::from_rgb(0xC8, 0xC8, 0xC8));
    p.rect_filled(page, 0.0, Color32::WHITE);
    let pp = p.with_clip_rect(page);
    let at = |x: f32, y: f32| egui::pos2(page.left() + x * scale, page.top() + y * scale);
    let col = |c: crate::export::Rgb| Color32::from_rgb((c.0 * 255.0) as u8, (c.1 * 255.0) as u8, (c.2 * 255.0) as u8);
    if let Some(ops) = pages.get(app.print.page) {
        for op in ops {
            match op {
                crate::export::PrevOp::Fill(x, y, w, h, c) => {
                    pp.rect_filled(Rect::from_min_max(at(*x, *y), at(x + w, y + h)), 0.0, col(*c));
                }
                crate::export::PrevOp::Stroke(x, y, w, h, c, lw) => {
                    pp.rect_stroke(Rect::from_min_max(at(*x, *y), at(x + w, y + h)), 0.0, Stroke::new((lw * scale).max(0.5), col(*c)), egui::StrokeKind::Middle);
                }
                crate::export::PrevOp::Line(pts, c, lw) => {
                    pp.add(egui::Shape::line(pts.iter().map(|(x, y)| at(*x, *y)).collect(), Stroke::new((lw * scale).max(0.5), col(*c))));
                }
                crate::export::PrevOp::Poly(pts, c) => {
                    pp.add(egui::Shape::convex_polygon(pts.iter().map(|(x, y)| at(*x, *y)).collect(), col(*c), Stroke::NONE));
                }
                crate::export::PrevOp::Text(x, base, size, bold, s, c) => {
                    let fs = (size * scale).max(3.0);
                    let f = if *bold { theme::bold(fs) } else { FontId::proportional(fs) };
                    pp.text(at(*x, base + size * 0.22), Align2::LEFT_BOTTOM, s, f, col(*c));
                }
            }
        }
    }
    // page navigation
    let nav = Rect::from_min_max(egui::pos2(right.left(), right.bottom() - nav_h), right.max);
    ui.scope_builder(egui::UiBuilder::new().max_rect(nav), |ui| {
        ui.horizontal_centered(|ui| {
            if ui.add_enabled(app.print.page > 0, egui::Button::new("<")).clicked() {
                app.print.page -= 1;
            }
            ui.label(crate::i18n::trf("Page {} of {}", &[&(app.print.page + 1).to_string(), &n.to_string()]));
            if ui.add_enabled(app.print.page + 1 < n, egui::Button::new(">")).clicked() {
                app.print.page += 1;
            }
        });
    });
}

fn greeting() -> &'static str {
    use chrono::Timelike;
    match chrono::Local::now().hour() {
        5..=11 => "Good morning",
        12..=17 => "Good afternoon",
        _ => "Good evening",
    }
}

fn link(ui: &mut Ui, pos: egui::Pos2, text: &str, align: Align2) -> egui::Response {
    let font = theme::bold(12.0);
    let g = ui.painter().layout_no_wrap(tr(text).to_string(), font.clone(), theme::ACCENT);
    let r = align.anchor_size(pos, Vec2::new(g.size().x + 22.0, 20.0));
    let resp = ui.interact(r, ui.scope_id().with(("link", text)), Sense::click());
    let p = ui.painter();
    p.text(egui::pos2(r.left(), r.center().y), Align2::LEFT_CENTER, tr(text), font, theme::ACCENT);
    let c = egui::pos2(r.right() - 8.0, r.center().y);
    let s = Stroke::new(1.3_f32, theme::ACCENT);
    p.line_segment([c + Vec2::new(-7.0, 0.0), c + Vec2::new(6.0, 0.0)], s);
    p.add(egui::Shape::line(vec![c + Vec2::new(2.0, -4.0), c + Vec2::new(6.0, 0.0), c + Vec2::new(2.0, 4.0)], s));
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        p.line_segment([egui::pos2(r.left(), r.bottom() - 2.0), egui::pos2(r.left() + g.size().x, r.bottom() - 2.0)], Stroke::new(1.0_f32, theme::ACCENT));
    }
    resp
}

/// Template thumbnail drawn in the style of start screen cards.
fn template_card(ui: &mut Ui, pos: egui::Pos2, which: u8, label: &str, selected: bool, cmds: &mut Vec<Cmd>) {
    let card = Rect::from_min_size(pos, Vec2::new(176.0, 150.0));
    let resp = ui.interact(card, ui.scope_id().with(("tpl", which)), Sense::click());
    let p = ui.painter();
    if selected || resp.hovered() {
        p.rect_filled(card, 4.0, Color32::from_rgb(0xEB, 0xEB, 0xEB));
    }
    if selected {
        p.rect_stroke(card, 4.0, Stroke::new(2.0_f32, Color32::BLACK), egui::StrokeKind::Inside);
    }
    let th = Rect::from_min_size(egui::pos2(card.center().x - 56.0, card.top() + 14.0), Vec2::new(112.0, 86.0));
    p.rect_filled(th, 0.0, Color32::WHITE);
    p.rect_stroke(th, 0.0, Stroke::new(1.0_f32, Color32::from_rgb(0xD0, 0xD0, 0xD0)), egui::StrokeKind::Inside);
    let blue = Color32::from_rgb(0x2B, 0x6C, 0xB5);
    let lblue = Color32::from_rgb(0x8F, 0xB4, 0xDE);
    match which {
        1 => {
            // waterfall: stepping bars with link arrows and a milestone
            for (k, (x, w)) in [(10.0, 34.0), (34.0, 30.0), (56.0, 30.0)].iter().enumerate() {
                let y = th.top() + 14.0 + k as f32 * 18.0;
                p.rect_filled(Rect::from_min_size(egui::pos2(th.left() + x, y), Vec2::new(*w, 10.0)), 1.0, lblue);
                p.line_segment([egui::pos2(th.left() + x + w - 6.0, y + 10.0), egui::pos2(th.left() + x + w - 6.0, y + 17.0)], Stroke::new(1.5_f32, blue));
            }
            let c = egui::pos2(th.left() + 92.0, th.top() + 72.0);
            p.add(egui::Shape::convex_polygon(vec![c + Vec2::new(0.0, -7.0), c + Vec2::new(7.0, 0.0), c + Vec2::new(0.0, 7.0), c + Vec2::new(-7.0, 0.0)], Color32::from_rgb(0x70, 0x70, 0x70), Stroke::NONE));
        }
        2 => {
            for k in 0..8 {
                let y = th.top() + 10.0 + k as f32 * 9.0;
                p.line_segment([egui::pos2(th.left() + 12.0, y), egui::pos2(th.right() - 12.0, y)], Stroke::new(if k == 0 { 2.0_f32 } else { 1.0_f32 }, Color32::from_rgb(0xB0, 0xB0, 0xB0)));
            }
        }
        3 => {
            // a .mpp file opened read-only
            p.rect_filled(Rect::from_min_size(th.min + Vec2::new(30.0, 10.0), Vec2::new(52.0, 66.0)), 2.0, theme::GREEN);
            p.text(th.center() + Vec2::new(0.0, -2.0), Align2::CENTER_CENTER, tr(".mpp"), theme::bold(14.0), Color32::WHITE);
        }
        _ => {}
    }
    p.text(egui::pos2(card.center().x, card.bottom() - 26.0), Align2::CENTER_CENTER, tr(label), FontId::proportional(12.0), theme::TEXT);
    if resp.clicked() {
        cmds.push(match which {
            3 => Cmd::Open,
            k => Cmd::NewTemplate(k),
        });
    }
}

/// The Recent / Favorites list; returns the number of rows shown.
fn recent_list(app: &mut App, ui: &mut Ui, area: Rect, cmds: &mut Vec<Cmd>) -> usize {
    let list: Vec<std::path::PathBuf> = if app.home_favorites { app.favorites.clone() } else { app.recent.clone() };
    let needle = app.home_search.to_lowercase();
    let list: Vec<_> = list.into_iter().filter(|p| needle.is_empty() || p.display().to_string().to_lowercase().contains(&needle)).collect();
    let p = ui.painter().clone();
    if list.is_empty() {
        let msg = if app.home_favorites {
            "You haven't added any projects to Favorites. Select the star next to a recent project to add it."
        } else {
            "You haven't opened any projects recently. Click Open to browse for a project."
        };
        p.text(area.min, Align2::LEFT_TOP, tr(msg), FontId::proportional(12.0), theme::TEXT);
        return 0;
    }
    // header
    let hdr = FontId::proportional(11.5);
    p.text(egui::pos2(area.left() + 34.0, area.top()), Align2::LEFT_TOP, tr("Name"), hdr.clone(), theme::TEXT_DIM);
    p.text(egui::pos2(area.right() - 180.0, area.top()), Align2::LEFT_TOP, tr("Date modified"), hdr, theme::TEXT_DIM);
    let mut y = area.top() + 22.0;
    let shown = list.len().min(((area.height() - 22.0) / 44.0).max(1.0) as usize);
    for path in list.iter().take(shown) {
        let row = Rect::from_min_size(egui::pos2(area.left(), y), Vec2::new(area.width(), 42.0));
        let resp = ui.interact(row, ui.scope_id().with(("recent", path.display().to_string())), Sense::click());
        if resp.hovered() {
            p.rect_filled(row, 3.0, Color32::from_rgb(0xEB, 0xEB, 0xEB));
        }
        // file icon
        let ir = Rect::from_min_size(egui::pos2(row.left() + 6.0, row.top() + 9.0), Vec2::new(18.0, 22.0));
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        p.rect_filled(ir, 2.0, if ext == "mpp" || ext == "mpt" { theme::GREEN } else { Color32::from_rgb(0x2B, 0x6C, 0xB5) });
        for k in 0..3 {
            p.line_segment([egui::pos2(ir.left() + 4.0, ir.top() + 7.0 + k as f32 * 4.0), egui::pos2(ir.right() - 4.0, ir.top() + 7.0 + k as f32 * 4.0)], Stroke::new(1.0_f32, Color32::WHITE));
        }
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let dir = path.parent().map(|d| d.display().to_string()).unwrap_or_default();
        p.text(egui::pos2(row.left() + 34.0, row.top() + 12.0), Align2::LEFT_CENTER, crate::thai::shape(&name), FontId::proportional(12.5), theme::TEXT);
        let cp = p.with_clip_rect(Rect::from_min_max(row.min, egui::pos2(row.right() - 200.0, row.bottom())));
        cp.text(egui::pos2(row.left() + 34.0, row.top() + 29.0), Align2::LEFT_CENTER, crate::thai::shape(&dir), FontId::proportional(11.0), theme::TEXT_DIM);
        let modified = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .ok()
            .map(|t| chrono::DateTime::<chrono::Local>::from(t).format("%d/%m/%Y %H:%M").to_string())
            .unwrap_or_default();
        p.text(egui::pos2(row.right() - 180.0, row.center().y), Align2::LEFT_CENTER, modified, FontId::proportional(12.0), theme::TEXT);
        // favorite star (outline / filled)
        let fav = app.favorites.contains(path);
        let sc = egui::pos2(row.right() - 20.0, row.center().y);
        let sr = Rect::from_center_size(sc, Vec2::splat(20.0));
        let sresp = ui.interact(sr, ui.scope_id().with(("star", path.display().to_string())), Sense::click()).on_hover_text(if fav { "Remove from Favorites" } else { "Add to Favorites" });
        if fav || resp.hovered() || sresp.hovered() {
            let pts: Vec<egui::Pos2> = (0..10)
                .map(|k| {
                    let a = -std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::PI / 5.0;
                    let rad = if k % 2 == 0 { 8.0 } else { 3.5 };
                    sc + Vec2::new(a.cos() * rad, a.sin() * rad)
                })
                .collect();
            if fav {
                p.add(egui::Shape::Path(egui::epaint::PathShape { points: pts, closed: true, fill: theme::ACCENT, stroke: egui::epaint::PathStroke::NONE }));
            } else {
                p.add(egui::Shape::closed_line(pts, Stroke::new(1.0_f32, theme::TEXT_DIM)));
            }
        }
        if sresp.clicked() {
            cmds.push(Cmd::ToggleFavorite(path.clone()));
        } else if resp.clicked() {
            cmds.push(Cmd::OpenRecent(path.clone()));
        }
        y += 44.0;
    }
    shown
}

fn pill(ui: &mut Ui, r: Rect, text: &str, on: bool) -> bool {
    let resp = ui.interact(r, ui.scope_id().with(("pill", text)), Sense::click());
    let p = ui.painter();
    if on {
        p.rect_filled(r, r.height() / 2.0, theme::GREEN);
        p.text(r.center(), Align2::CENTER_CENTER, tr(text), FontId::proportional(13.0), Color32::WHITE);
    } else {
        p.rect_filled(r, r.height() / 2.0, if resp.hovered() { Color32::from_rgb(0xEB, 0xEB, 0xEB) } else { Color32::WHITE });
        p.rect_stroke(r, r.height() / 2.0, Stroke::new(1.0_f32, Color32::from_rgb(0x91, 0x91, 0x91)), egui::StrokeKind::Inside);
        p.text(r.center(), Align2::CENTER_CENTER, tr(text), FontId::proportional(13.0), theme::TEXT);
    }
    resp.clicked()
}

/// Maker credit for the About pages.
pub fn credit_lines(ui: &mut Ui) {
    ui.add_space(4.0);
    ui.label(egui::RichText::new(crate::app::CREDIT).color(theme::HEADER_INK).strong());
    ui.label(egui::RichText::new(format!("© 2026 {}", crate::thai::shape(crate::app::DESIGNER))).color(theme::TEXT_DIM));
}

fn home_page(app: &mut App, ui: &mut Ui, content: Rect, cmds: &mut Vec<Cmd>, title: &str) {
    let p = ui.painter().clone();
    // footer: product, version and maker credit
    p.text(
        egui::pos2(content.left() + 48.0, content.bottom() - 18.0),
        Align2::LEFT_CENTER,
        format!(
            "{} {}   ·   {}",
            crate::app::product(),
            env!("CARGO_PKG_VERSION"),
            crate::app::CREDIT
        ),
        FontId::proportional(11.5),
        theme::TEXT_DIM,
    );
    let left = content.left() + 48.0;
    let right = (content.right() - 48.0).min(left + 1500.0);
    p.text(egui::pos2(left, content.top() + 26.0), Align2::LEFT_CENTER, tr(title), FontId::proportional(22.0), theme::TEXT);
    // "New" section with chevron
    let ny = content.top() + 72.0;
    let c = egui::pos2(left - 16.0, ny);
    p.add(egui::Shape::line(vec![c + Vec2::new(-4.0, -2.0), c + Vec2::new(0.0, 2.0), c + Vec2::new(4.0, -2.0)], Stroke::new(1.2_f32, theme::TEXT)));
    p.text(egui::pos2(left, ny), Align2::LEFT_CENTER, tr("New"), FontId::proportional(17.0), theme::TEXT);
    if link(ui, egui::pos2(right, ny), "More templates", Align2::RIGHT_CENTER).clicked() {
        cmds.push(Cmd::ShowBackstage(Backstage::New));
    }
    let all_cards = [(0u8, "Blank Project"), (1, "Construction Project"), (2, "New from existing project"), (3, "Open .mpp / XML file")];
    let cards: &[(u8, &str)] = &all_cards;
    for (k, (which, label)) in cards.iter().enumerate() {
        template_card(ui, egui::pos2(left + k as f32 * 196.0, ny + 22.0), *which, label, k == 0, cmds);
    }
    // Recent / Favorites + search
    let py = ny + 206.0;
    if pill(ui, Rect::from_min_size(egui::pos2(left, py), Vec2::new(76.0, 30.0)), "Recent", !app.home_favorites) {
        app.home_favorites = false;
    }
    if pill(ui, Rect::from_min_size(egui::pos2(left + 86.0, py), Vec2::new(86.0, 30.0)), "Favorites", app.home_favorites) {
        app.home_favorites = true;
    }
    let sr = Rect::from_min_max(egui::pos2((right - 440.0).max(left + 200.0), py), egui::pos2(right, py + 30.0));
    p.rect_filled(sr, 3.0, Color32::WHITE);
    p.rect_stroke(sr, 3.0, Stroke::new(1.0_f32, Color32::from_rgb(0x8A, 0x8A, 0x8A)), egui::StrokeKind::Inside);
    let mc = egui::pos2(sr.left() + 18.0, sr.center().y);
    p.circle_stroke(mc + Vec2::new(1.5, -1.5), 5.0, Stroke::new(1.2_f32, theme::TEXT));
    p.line_segment([mc + Vec2::new(-2.0, 2.0), mc + Vec2::new(-6.0, 6.0)], Stroke::new(1.4_f32, theme::TEXT));
    let text_r = Rect::from_min_max(egui::pos2(sr.left() + 36.0, sr.top() + 6.0), egui::pos2(sr.right() - 8.0, sr.bottom() - 4.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(text_r));
    child.add(egui::TextEdit::singleline(&mut app.home_search).hint_text(tr("Search")).frame(egui::Frame::NONE).desired_width(text_r.width()));
    let list_area = Rect::from_min_max(egui::pos2(left, py + 42.0), egui::pos2(right, content.bottom() - 60.0));
    let shown = recent_list(app, ui, list_area, cmds);
    // under the last row shown (the list leaves room for it above the footer)
    let more_y = (py + 42.0 + 22.0 + 44.0 * shown as f32 + 16.0).max(py + 72.0);
    if link(ui, egui::pos2(right, more_y), "More projects", Align2::RIGHT_CENTER).clicked() {
        cmds.push(Cmd::ShowBackstage(Backstage::Open));
    }
}

/// Start screen (no document) uses a narrow icon rail; the File tab uses the wide menu.
pub fn backstage(app: &mut App, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
    let rect = ui.max_rect();
    let p = ui.painter().clone();
    p.rect_filled(rect, 0.0, theme::CONTENT_BG);
    let cur = app.backstage.unwrap_or(Backstage::Home);
    let start_screen = app.path.is_none() && app.doc.tasks.is_empty() && !app.dirty;

    let content;
    if start_screen {
        // ------------- narrow rail: Home / New / Open ... About / Options
        let rail = Rect::from_min_size(rect.min, Vec2::new(68.0, rect.height()));
        p.rect_filled(rail, 0.0, theme::NAV_BG);
        p.line_segment([rail.right_top(), rail.right_bottom()], Stroke::new(1.0_f32, Color32::from_rgb(0xDB, 0xDD, 0xE2)));
        let mut items: Vec<(&str, Option<Backstage>, f32)> =
            vec![("Home", Some(Backstage::Home), rail.top() + 8.0), ("New", Some(Backstage::New), rail.top() + 68.0), ("Open", Some(Backstage::Open), rail.top() + 128.0)];
        items.push(("About", Some(Backstage::About), rail.bottom() - 126.0));
        items.push(("Options", Some(Backstage::Options), rail.bottom() - 66.0));
        p.line_segment([egui::pos2(rail.left() + 14.0, rail.bottom() - 134.0), egui::pos2(rail.right() - 14.0, rail.bottom() - 134.0)], Stroke::new(1.0_f32, Color32::from_rgb(0xD0, 0xD0, 0xD0)));
        for (label, page, y) in items {
            let r = Rect::from_min_size(egui::pos2(rail.left() + 4.0, y), Vec2::new(rail.width() - 8.0, 56.0));
            let resp = ui.interact(r, ui.scope_id().with(("rail", label)), Sense::click());
            let sel = page == Some(cur);
            if resp.hovered() {
                p.rect_filled(r, 4.0, Color32::from_rgb(0xE3, 0xE3, 0xE3));
            }
            if sel {
                p.rect_filled(Rect::from_min_size(egui::pos2(rail.left() + 2.0, r.top() + 8.0), Vec2::new(3.0, r.height() - 16.0)), 1.5, theme::ACCENT);
            }
            let color = if sel { theme::ACCENT } else { theme::TEXT };
            nav_icon(&p, Rect::from_center_size(egui::pos2(r.center().x, r.top() + 20.0), Vec2::splat(20.0)), label, color);
            p.text(egui::pos2(r.center().x, r.top() + 43.0), Align2::CENTER_CENTER, tr(label), FontId::proportional(11.5), color);
            if resp.clicked() {
                if label == "Options" {
                    cmds.push(Cmd::Options(0));
                } else {
                    app.backstage = page;
                }
            }
        }
        content = Rect::from_min_max(egui::pos2(rail.right(), rect.top()), rect.max);
    } else {
        // ------------- wide File menu with back arrow
        let nav = Rect::from_min_size(rect.min, Vec2::new(160.0, rect.height()));
        p.rect_filled(nav, 0.0, theme::BACKSTAGE_NAV);
        let back = Rect::from_min_size(nav.min + Vec2::new(10.0, 8.0), Vec2::splat(30.0));
        let bresp = ui.interact(back, ui.scope_id().with("bs_back"), Sense::click()).on_hover_text(tr("Back"));
        if bresp.hovered() {
            p.circle_filled(back.center(), 15.0, Color32::from_rgb(0xD6, 0xD6, 0xDE));
        }
        p.circle_stroke(back.center(), 11.0, Stroke::new(1.2_f32, theme::TEXT));
        let c = back.center();
        let ws = Stroke::new(1.3_f32, theme::TEXT);
        p.line_segment([c + Vec2::new(-5.0, 0.0), c + Vec2::new(5.0, 0.0)], ws);
        p.line_segment([c + Vec2::new(-5.0, 0.0), c + Vec2::new(-1.0, -4.0)], ws);
        p.line_segment([c + Vec2::new(-5.0, 0.0), c + Vec2::new(-1.0, 4.0)], ws);
        // Esc leaves the File menu too (unless a dialog or a text field has the keyboard)
        let esc = app.dialog.is_none() && app.message.is_none() && ui.ctx().memory(|m| m.focused().is_none()) && !egui::Popup::is_any_open(ui.ctx()) && ui.input(|i| i.key_pressed(egui::Key::Escape));
        if bresp.clicked() || esc {
            app.backstage = None;
        }
        let full: [&[(&str, Option<Backstage>, Option<Cmd>, bool)]; 2] = [
            &[("Home", Some(Backstage::Home), None, true), ("New", Some(Backstage::New), None, true), ("Open", Some(Backstage::Open), None, true)],
            &[
                ("Info", Some(Backstage::Info), None, true),
                ("Save", None, Some(Cmd::Save), true),
                ("Save As", Some(Backstage::SaveAs), None, true),
                ("Print", Some(Backstage::Print), None, true),
                ("Export", Some(Backstage::Export), None, true),
                ("Close", None, Some(Cmd::CloseDoc), true),
            ],
        ];
        let groups = full;
        let mut y = nav.top() + 48.0;
        for (gi, g) in groups.iter().enumerate() {
            if gi > 0 {
                p.line_segment([egui::pos2(nav.left() + 16.0, y + 5.0), egui::pos2(nav.right() - 16.0, y + 5.0)], Stroke::new(1.0_f32, Color32::from_rgb(0xC8, 0xC8, 0xD0)));
                y += 12.0;
            }
            for (label, page, cmd, enabled) in g.iter() {
                let r = Rect::from_min_size(egui::pos2(nav.left(), y), Vec2::new(nav.width(), 38.0));
                let resp = ui.interact(r, ui.scope_id().with(("bs", *label)), if *enabled { Sense::click() } else { Sense::hover() });
                let sel = *page == Some(cur);
                if sel {
                    p.rect_filled(r.shrink2(Vec2::new(2.0, 1.0)), 0.0, Color32::from_rgb(0xE3, 0xE7, 0xEC));
                    p.rect_stroke(r.shrink2(Vec2::new(2.0, 1.0)), 0.0, Stroke::new(1.5_f32, Color32::BLACK), egui::StrokeKind::Inside);
                } else if resp.hovered() && *enabled {
                    p.rect_filled(r, 0.0, Color32::from_rgb(0xD6, 0xD6, 0xDE));
                }
                let color = if !*enabled { theme::DISABLED } else if sel { theme::ACCENT } else { theme::TEXT };
                nav_icon(&p, Rect::from_center_size(egui::pos2(r.left() + 26.0, r.center().y), Vec2::splat(15.0)), label, color);
                p.text(egui::pos2(r.left() + 44.0, r.center().y), Align2::LEFT_CENTER, tr(label), FontId::proportional(12.5), color);
                if resp.clicked() && *enabled {
                    if let Some(pg) = page {
                        app.backstage = Some(*pg);
                    }
                    if let Some(c) = cmd {
                        cmds.push(c.clone());
                    }
                }
                y += 38.0;
            }
        }
        for (k, (label, page)) in [("About", Backstage::About), ("Options", Backstage::Options)].iter().enumerate() {
            let r = Rect::from_min_size(egui::pos2(nav.left(), nav.bottom() - 90.0 + k as f32 * 38.0), Vec2::new(nav.width(), 38.0));
            let resp = ui.interact(r, ui.scope_id().with(("bs2", *label)), Sense::click());
            let sel = *page == cur;
            if sel {
                p.rect_stroke(r.shrink2(Vec2::new(2.0, 1.0)), 0.0, Stroke::new(1.5_f32, Color32::BLACK), egui::StrokeKind::Inside);
            } else if resp.hovered() {
                p.rect_filled(r, 0.0, Color32::from_rgb(0xD6, 0xD6, 0xDE));
            }
            nav_icon(&p, Rect::from_center_size(egui::pos2(r.left() + 26.0, r.center().y), Vec2::splat(15.0)), label, theme::TEXT);
            p.text(egui::pos2(r.left() + 44.0, r.center().y), Align2::LEFT_CENTER, tr(label), FontId::proportional(12.5), theme::TEXT);
            if resp.clicked() {
                if *label == "Options" {
                    cmds.push(Cmd::Options(0));
                } else {
                    app.backstage = Some(*page);
                }
            }
        }
        p.line_segment([egui::pos2(nav.left() + 16.0, nav.bottom() - 98.0), egui::pos2(nav.right() - 16.0, nav.bottom() - 98.0)], Stroke::new(1.0_f32, Color32::from_rgb(0xC8, 0xC8, 0xD0)));
        content = Rect::from_min_max(egui::pos2(nav.right(), rect.top()), rect.max);
    }

    if cur == Backstage::Home {
        home_page(app, ui, content, cmds, greeting());
        return;
    }
    let inner = Rect::from_min_max(content.min + Vec2::new(40.0, 20.0), content.max - Vec2::new(30.0, 20.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)));
    let ui = &mut child;
    let heading = |ui: &mut Ui, t: &str| {
        ui.label(egui::RichText::new(tr(t)).size(22.0).color(theme::TEXT));
        ui.add_space(14.0);
    };
    match cur {
        Backstage::Home => {}
        Backstage::New => {
            heading(ui, "New");
            let origin = ui.cursor().min;
            let cards = [(0u8, "Blank Project"), (1, "Construction Project"), (2, "New from existing project"), (3, "Open .mpp / XML file")];
            for (k, (which, label)) in cards.iter().enumerate() {
                template_card(ui, origin + Vec2::new(k as f32 * 196.0, 0.0), *which, label, k == 0, cmds);
            }
        }
        Backstage::Open => {
            heading(ui, "Open");
            let origin = ui.cursor().min;
            let lr = Rect::from_min_size(origin, Vec2::new(260.0, 44.0));
            let p = ui.painter().clone();
            p.rect_filled(lr, 0.0, Color32::from_rgb(0xE0, 0xE0, 0xE0));
            p.rect_stroke(lr, 0.0, Stroke::new(1.0_f32, Color32::from_rgb(0x8A, 0x8A, 0x8A)), egui::StrokeKind::Inside);
            p.circle_stroke(egui::pos2(lr.left() + 22.0, lr.center().y), 9.0, Stroke::new(1.2_f32, theme::TEXT));
            p.line_segment([egui::pos2(lr.left() + 22.0, lr.center().y - 5.0), egui::pos2(lr.left() + 22.0, lr.center().y)], Stroke::new(1.2_f32, theme::TEXT));
            p.line_segment([egui::pos2(lr.left() + 22.0, lr.center().y), egui::pos2(lr.left() + 26.0, lr.center().y + 2.0)], Stroke::new(1.2_f32, theme::TEXT));
            p.text(egui::pos2(lr.left() + 46.0, lr.center().y), Align2::LEFT_CENTER, tr("Recent"), FontId::proportional(14.0), theme::TEXT);
            p.text(egui::pos2(lr.left() + 6.0, lr.bottom() + 24.0), Align2::LEFT_CENTER, tr("Other locations"), FontId::proportional(13.0), theme::TEXT);
            for (k, (label, cmd)) in [("This PC", Cmd::Open), ("Browse", Cmd::Open)].iter().enumerate() {
                let r = Rect::from_min_size(egui::pos2(lr.left(), lr.bottom() + 42.0 + k as f32 * 44.0), Vec2::new(260.0, 42.0));
                let resp = ui.interact(r, ui.scope_id().with(("openloc", *label)), Sense::click());
                if resp.hovered() {
                    p.rect_filled(r, 0.0, Color32::from_rgb(0xE6, 0xE6, 0xE6));
                }
                nav_icon(&p, Rect::from_center_size(egui::pos2(r.left() + 22.0, r.center().y), Vec2::splat(22.0)), "Open", theme::TEXT);
                p.text(egui::pos2(r.left() + 46.0, r.center().y), Align2::LEFT_CENTER, tr(label), FontId::proportional(14.0), theme::TEXT);
                if resp.clicked() {
                    cmds.push(cmd.clone());
                }
            }
            p.line_segment([egui::pos2(lr.right() + 2.0, origin.y), egui::pos2(lr.right() + 2.0, inner.bottom())], Stroke::new(1.0_f32, Color32::from_rgb(0xD8, 0xD8, 0xD8)));
            let tabs_y = origin.y + 10.0;
            p.text(egui::pos2(lr.right() + 36.0, tabs_y), Align2::LEFT_CENTER, tr("Projects"), FontId::proportional(14.0), theme::TEXT);
            p.line_segment([egui::pos2(lr.right() + 36.0, tabs_y + 13.0), egui::pos2(lr.right() + 90.0, tabs_y + 13.0)], Stroke::new(2.0_f32, theme::ACCENT));
            let area = Rect::from_min_max(egui::pos2(lr.right() + 36.0, tabs_y + 30.0), egui::pos2(inner.right(), inner.bottom()));
            let saved = app.home_favorites;
            app.home_favorites = false;
            recent_list(app, ui, area, cmds);
            app.home_favorites = saved;
        }
        Backstage::Info => {
            heading(ui, "Info");
            let name = app.path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "Project1 (not saved)".into());
            ui.label(egui::RichText::new(name).color(theme::TEXT_DIM));
            ui.add_space(14.0);
            egui::Grid::new("bs_info").num_columns(2).spacing([24.0, 8.0]).show(ui, |ui| {
                let mut changed = false;
                for (label, field) in [("Title", 0), ("Manager", 1), ("Company", 2), ("Author", 3)] {
                    ui.label(tr(label));
                    let v = match field {
                        0 => &mut app.doc.title,
                        1 => &mut app.doc.manager,
                        2 => &mut app.doc.company,
                        _ => &mut app.doc.author,
                    };
                    changed |= ui.add(egui::TextEdit::singleline(v).desired_width(320.0)).changed();
                    ui.end_row();
                }
                ui.label(tr("Start Date"));
                ui.label(app.fmt_date(app.doc.start));
                ui.end_row();
                ui.label(tr("Finish Date"));
                ui.label(app.fmt_date(app.doc.finish_date()));
                ui.end_row();
                ui.label(tr("Tasks"));
                ui.label(app.doc.tasks.len().to_string());
                ui.end_row();
                ui.label(tr("Resources"));
                ui.label(app.doc.resources.len().to_string());
                ui.end_row();
                if changed {
                    app.dirty = true;
                }
            });
            ui.add_space(16.0);
            if ui.button(tr("Project Information...")).clicked() {
                app.backstage = None;
                cmds.push(Cmd::ProjectInfo);
            }
        }
        Backstage::SaveAs => {
            heading(ui, "Save As");
            if ui.add(egui::Button::new(tr("Browse...")).min_size(Vec2::new(160.0, 34.0))).clicked() {
                cmds.push(Cmd::SaveAs);
            }
            ui.add_space(10.0);
            ui.label(tr("Supported formats: Neramit Planner (*.neramit), XML (*.xml), CSV (*.csv)."));
        }
        Backstage::Print => {
            print_page(app, ui, cmds);
        }
        Backstage::Export => {
            heading(ui, "Export");
            if ui.add(egui::Button::new(tr("Create PDF/XPS Document")).min_size(Vec2::new(360.0, 34.0))).clicked() {
                cmds.push(Cmd::Export("pdf"));
            }
            ui.label(egui::RichText::new(tr("PDF keeps the Gantt Chart layout for printing and sharing.")).color(theme::TEXT_DIM));
            ui.add_space(6.0);
            if ui.add(egui::Button::new(tr("Save Project as Excel Workbook (*.xlsx)")).min_size(Vec2::new(360.0, 34.0))).clicked() {
                cmds.push(Cmd::Export("xlsx"));
            }
            ui.label(egui::RichText::new(tr("Excel workbooks contain the task list (with real dates and numbers) and the resource sheet.")).color(theme::TEXT_DIM));
            ui.add_space(6.0);
            if ui.add(egui::Button::new(tr("Save Project as XML (*.xml)")).min_size(Vec2::new(360.0, 34.0))).clicked() {
                cmds.push(Cmd::Export("xml"));
            }
            ui.add_space(6.0);
            if ui.add(egui::Button::new(tr("Save Task List as CSV (*.csv) for Excel")).min_size(Vec2::new(360.0, 34.0))).clicked() {
                cmds.push(Cmd::Export("csv"));
            }
            ui.add_space(14.0);
            ui.label(egui::RichText::new(tr("Files saved as XML can be opened by other project scheduling programs.")).color(theme::TEXT_DIM));
        }
        Backstage::Options => {
            heading(ui, "Options");
            ui.label(egui::RichText::new(tr("Display")).strong());
            ui.add_space(10.0);
            ui.label(egui::RichText::new(tr("Schedule")).strong());
            let mut auto = app.doc.new_tasks_auto;
            ui.horizontal(|ui| {
                ui.label(tr("New tasks created:"));
                ui.radio_value(&mut auto, true, tr("Auto Scheduled"));
                ui.radio_value(&mut auto, false, tr("Manually Scheduled"));
            });
            if auto != app.doc.new_tasks_auto {
                app.doc.new_tasks_auto = auto;
                app.dirty = true;
            }
            let mut hpd = app.doc.calendar.hours_per_day;
            ui.horizontal(|ui| {
                ui.label(tr("Hours per day:"));
                ui.add(egui::DragValue::new(&mut hpd).range(1.0..=24.0).speed(0.5));
            });
            if (hpd - app.doc.calendar.hours_per_day).abs() > f64::EPSILON {
                app.checkpoint();
                app.doc.calendar.hours_per_day = hpd;
                app.recalc();
            }
        }
        Backstage::About => {
            let about = crate::i18n::trp("About Neramit Planner");
            ui.label(egui::RichText::new(about).size(22.0).color(theme::TEXT));
            ui.add_space(14.0);
            let tex = theme::logo_texture(ui.ctx(), true);
            ui.add(egui::Image::new(&tex).fit_to_exact_size(Vec2::splat(160.0)));
            ui.add_space(10.0);
            ui.label(egui::RichText::new(format!("{} {}", crate::app::product(), env!("CARGO_PKG_VERSION"))).strong());
            credit_lines(ui);
            ui.add_space(6.0);
            if ui.button(tr("Open-source licenses...")).clicked() {
                app.dialog = Some(crate::dialogs::Dialog::Licenses);
            }
            ui.add_space(6.0);
            if crate::dialogs::update_button(ui) {
                crate::update::check(ui.ctx());
                app.dialog = Some(crate::dialogs::Dialog::Update);
            }
            ui.add_space(6.0);
            ui.label(tr("Offline project scheduling and Gantt chart tool written in Rust."));
            ui.label(tr("No sign-in required. Opens .neramit, .mpp and XML files; saves .neramit, .mpp, XML, CSV, PDF and Excel."));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Press on the close button, move 9 px while holding (beyond egui's click distance), release
    /// over the button: like a native window, that still closes the program.
    #[test]
    fn close_button_tolerates_mouse_movement() {
        let ctx = egui::Context::default();
        let screen = Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(800.0, 600.0));
        let title = Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(800.0, 40.0));
        let p0 = egui::pos2(800.0 - 23.0, 18.0);
        let p1 = p0 + Vec2::new(9.0, -2.0);
        let btn = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        // the first frames only fade the new area in (not interactive yet)
        let mut frames: Vec<Vec<egui::Event>> = (0..8).map(|_| vec![egui::Event::PointerMoved(p0)]).collect();
        frames.extend(vec![
            vec![egui::Event::PointerMoved(p0)],
            vec![btn(p0, true)],
            vec![egui::Event::PointerMoved(p0 + Vec2::new(4.0, -1.0))],
            vec![egui::Event::PointerMoved(p1)],
            vec![btn(p1, false)],
            vec![],
        ]);
        let mut exit = false;
        for (k, events) in frames.into_iter().enumerate() {
            let input = egui::RawInput { screen_rect: Some(screen), time: Some(k as f64 * 0.03), events, ..Default::default() };
            let mut cmds = Vec::new();
            let _ = ctx.run_pass(input, |ctx| {
                egui::Area::new(egui::Id::unique("caption_test")).fixed_pos(egui::Pos2::ZERO).show(ctx, |ui| caption_buttons(ui, title, &mut cmds));
            });
            exit |= cmds.iter().any(|c| matches!(c, Cmd::Exit));
        }
        assert!(exit, "close button did not fire after a small mouse movement");
    }

    /// Press on the close button, move off it and release elsewhere: nothing happens (native behaviour).
    #[test]
    fn close_button_cancelled_when_released_outside() {
        let ctx = egui::Context::default();
        let screen = Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(800.0, 600.0));
        let title = Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(800.0, 40.0));
        let p0 = egui::pos2(800.0 - 23.0, 18.0);
        let p1 = egui::pos2(400.0, 300.0);
        let btn = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        // the first frames only fade the new area in (not interactive yet)
        let mut frames: Vec<Vec<egui::Event>> = (0..8).map(|_| vec![egui::Event::PointerMoved(p0)]).collect();
        frames.extend(vec![vec![egui::Event::PointerMoved(p0)], vec![btn(p0, true)], vec![egui::Event::PointerMoved(p1)], vec![btn(p1, false)], vec![]]);
        let mut exit = false;
        for (k, events) in frames.into_iter().enumerate() {
            let input = egui::RawInput { screen_rect: Some(screen), time: Some(k as f64 * 0.03), events, ..Default::default() };
            let mut cmds = Vec::new();
            let _ = ctx.run_pass(input, |ctx| {
                egui::Area::new(egui::Id::unique("caption_test")).fixed_pos(egui::Pos2::ZERO).show(ctx, |ui| caption_buttons(ui, title, &mut cmds));
            });
            exit |= cmds.iter().any(|c| matches!(c, Cmd::Exit));
        }
        assert!(!exit, "close fired although the button was released elsewhere");
    }

}
