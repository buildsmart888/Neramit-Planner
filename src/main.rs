#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Neramit Planner — offline desktop project scheduling with a Gantt chart.
//! Neramit Planner Made by BiGByte Studio.

mod aiconnect;
mod app;
mod dialogs;
mod fileio;
mod fmt_dialogs;
mod format;
mod gantt;
mod i18n;
mod export;
mod locale;
mod mcp;
mod grid;
mod model;
mod mpp;
mod mppw;
mod mppt;
mod board;
mod boq;
mod payments;
mod picture;
mod reports;
mod scurve;
mod compare;
mod wbs;
mod custom;
mod controls;
mod control_ui;
mod custom_tables;
mod cfdefs;
mod sprintio;
mod extlinks;
mod field_ids;
mod drawing;
mod spell;
mod qa;
mod ribbon;
mod schedule;
mod setup;
mod update;
mod taskinfo;
mod sysfonts;
mod thai;
mod thai_holidays;
mod win11;
mod theme;
mod usage;
mod network;
mod calendar;
mod undo;
mod util;

use crate::i18n::{tr, trf};
use app::{App, View};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use ribbon::Cmd;
use std::path::PathBuf;

fn app_icon() -> egui::IconData {
    let rgba: &[u8] = include_bytes!("../assets/icon-64.rgba");
    egui::IconData { rgba: rgba.to_vec(), width: 64, height: 64 }
}

/// `NeramitPlanner --convert <input> <output>`: convert between .neramit / .xml / .csv without opening the UI.
fn convert(args: &[std::ffi::OsString]) -> i32 {
    let (Some(input), Some(output)) = (args.get(2), args.get(3)) else {
        eprintln!("usage: NeramitPlanner --convert <input.neramit|input.mpp|input.xml> <output.neramit|output.xml|output.csv>");
        return 2;
    };
    let (inp, out) = (std::path::Path::new(input), std::path::Path::new(output));
    let is_mpp = |p: &std::path::Path| p.extension().map(|e| e.eq_ignore_ascii_case("mpp")).unwrap_or(false);
    let result = fileio::load(inp).and_then(|mut p| {
        if let Err(e) = schedule::schedule(&mut p) {
            eprintln!("warning: {e}");
        }
        if is_mpp(out) {
            if !p.controls.is_empty() {
                return Err("Structured BOQ, progress and posted costs cannot be retained in MPP. Convert to .neramit instead.".into());
            }
            // a .mpp is written from the input .mpp (never over it), or as a new .mpp
            if !is_mpp(inp) {
                return crate::mppt::save_new_mpp(&p).and_then(|b| std::fs::write(out, b).map_err(|e| e.to_string()));
            }
            if inp.canonicalize().ok() == out.canonicalize().ok() && out.exists() {
                return Err("The output must be a new file.".into());
            }
            let src = std::fs::read(inp).map_err(|e| e.to_string())?;
            return crate::mppt::save_mpp(&src, &p).and_then(|b| std::fs::write(out, b).map_err(|e| e.to_string()));
        }
        fileio::save(out, &p)
    });
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    }
}

fn main() -> eframe::Result<()> {
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    if args.get(1).map(|a| a == "--finish-portable-update").unwrap_or(false) {
        std::process::exit(update::finish_portable_update(&args[2..]));
    }
    // Read-only diagnostics for release verification; downloads are verified in memory only.
    if args.get(1).map(|a| a == "--check-update" || a == "--verify-update-download").unwrap_or(false) {
        std::process::exit(update::cli_check(args[1] == "--verify-update-download"));
    }
    // `--register <tasks>` / `--choose-mpp-default` / `--uninstall [--quiet]`: per-user install, run by
    // NeramitPlanner-Setup.exe and Settings > Apps
    if args.get(1).map(|a| a == "--register").unwrap_or(false) {
        let opts = args.get(2).map(|a| a.to_string_lossy().to_string()).unwrap_or_else(|| "desktop,startmenu".into());
        std::process::exit(setup::register(setup::Options::parse(&opts)));
    }
    if args.get(1).map(|a| a == "--choose-mpp-default").unwrap_or(false) {
        setup::choose_mpp_default();
        std::process::exit(0);
    }
    if args.get(1).map(|a| a == "--uninstall-finish").unwrap_or(false) {
        let dir = args.get(2).map(PathBuf::from).unwrap_or_default();
        let pid = args.get(3).and_then(|p| p.to_string_lossy().parse().ok()).unwrap_or(0);
        std::process::exit(setup::uninstall_finish(&dir, pid));
    }
    if args.get(1).map(|a| a == "--uninstall").unwrap_or(false) {
        std::process::exit(setup::uninstall(args.iter().any(|a| a == "--quiet")));
    }
    if args.get(1).map(|a| a == "--convert").unwrap_or(false) {
        std::process::exit(convert(&args));
    }
    // `--connect-ai` / `--disconnect-ai`: add Neramit Planner to the AI assistants on this PC, or take
    // it out (aiconnect.rs); exit code 0 = at least one connected / done
    if args.get(1).map(|a| a == "--connect-ai").unwrap_or(false) {
        let exe = std::env::current_exe().unwrap_or_default();
        std::process::exit(if aiconnect::connect(&exe).is_empty() { 1 } else { 0 });
    }
    if args.get(1).map(|a| a == "--disconnect-ai").unwrap_or(false) {
        aiconnect::disconnect();
        std::process::exit(0);
    }
    // `--mcp`: serve AI assistants over stdin / stdout (see mcp.rs)
    if args.get(1).map(|a| a == "--mcp").unwrap_or(false) {
        std::process::exit(mcp::run());
    }
    // `--qa-script steps.txt`: run the program headless from a script (see qa.rs)
    if args.get(1).map(|a| a == "--qa-script").unwrap_or(false) {
        let path = args.get(2).map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        std::process::exit(qa::run(&path));
    }
    // `--ui <tab|file>` opens a given ribbon tab or the File page (used for UI screenshots / QA)
    let ui_start = args.iter().position(|a| a == "--ui").and_then(|i| args.get(i + 1)).map(|s| s.to_string_lossy().to_string());
    let open = args.get(1).filter(|a| !a.to_string_lossy().starts_with("--")).map(PathBuf::from);
    // `--restored` starts in a normal (not maximized) window, e.g. to check the rounded Windows 11 frame
    let restored = args.iter().any(|a| a == "--restored");
    // `--size WxH` sets the restored window size (QA screenshots at a given width)
    let size: Option<(f32, f32)> = args.iter().position(|a| a == "--size").and_then(|i| args.get(i + 1)).and_then(|s| {
        let s = s.to_string_lossy();
        let (w, h) = s.split_once('x')?;
        Some((w.parse().ok()?, h.parse().ok()?))
    });
    // `--zoom <px per day>` starts at a given timescale zoom (QA screenshots of each timescale)
    // `--screenshot <out.png>`: render a few frames, save the window's own framebuffer and quit (QA;
    // works even when the desktop is locked and screen capture returns nothing)
    let screenshot_to: Option<PathBuf> = args.iter().position(|a| a == "--screenshot").and_then(|i| args.get(i + 1)).map(PathBuf::from);
    // `--export <out.pdf|out.xlsx> [--lang th]`: open the file, write the export and quit
    let export_to: Option<PathBuf> = args.iter().position(|a| a == "--export").and_then(|i| args.get(i + 1)).map(PathBuf::from);
    // QA: `--qa-back` presses Back once after the sample highlights; `--qa-select` drags a cell
    // range (rows 3-6, columns 3-5); `--qa-copy out.txt` writes what Copy would put on the clipboard
    let qa_select = args.iter().any(|a| a == "--qa-select");
    // QA: `--qa-bestfit` double-clicks every column border once the fonts exist (frame 3)
    QA_BESTFIT.store(args.iter().any(|a| a == "--qa-bestfit"), std::sync::atomic::Ordering::Relaxed);
    let qa_copy: Option<PathBuf> = args.iter().position(|a| a == "--qa-copy").and_then(|i| args.get(i + 1)).map(PathBuf::from);
    let lang_th = args.iter().position(|a| a == "--lang").and_then(|i| args.get(i + 1)).map(|s| s == "th");
    let zoom_start: Option<f32> = args.iter().position(|a| a == "--zoom").and_then(|i| args.get(i + 1)).and_then(|s| s.to_string_lossy().parse().ok());
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(app::product())
            .with_inner_size(size.map(|(w, h)| [w, h]).unwrap_or([1366.0, 820.0]))
            .with_min_inner_size([900.0, 560.0])
            .with_maximized(!restored)
            .with_decorations(false)
            .with_drag_and_drop(true)
            .with_icon(app_icon()),
        ..Default::default()
    };
    let result = eframe::run_native("Neramit Planner", options, Box::new(move |cc| {
        win11::apply(cc);
        let mut app = App::new(cc, open);
        if let Some(th) = lang_th {
            i18n::set_lang(if th { i18n::Lang::Thai } else { i18n::Lang::English });
            if let Some(name) = app.path.as_ref().and_then(|p| p.file_name()) {
                app.status = i18n::trf("Opened {}", &[&name.to_string_lossy()]);
            }
        }
        app.qa_screenshot = screenshot_to.clone().map(|p| (p, 0));
        if qa_select {
            app.sel_anchor = 2;
            app.sel_row = 5;
            app.sel_col_anchor = Some(2);
            app.sel_col = 4;
            app.sel_whole_rows = false;
        }
        if let Some(out) = &qa_copy {
            let _ = std::fs::write(out, app.selection_text().0);
        }
        if let Some(out) = &export_to {
            app.export_quiet(out);
            std::process::exit(if app.message.is_some() { 1 } else { 0 });
        }
        if let Some(z) = zoom_start {
            let z = z.clamp(App::MIN_PPD, App::MAX_PPD);
            app.scroll_x *= z / app.px_per_day;
            app.px_per_day = z;
        }
        if let Some(t) = ui_start.as_deref() {
            app.backstage = match t {
                "file" => Some(app::Backstage::Info),
                "home" => Some(app::Backstage::Home),
                "about" => Some(app::Backstage::About),
                _ => None,
            };
            app.tab = match t {
                "resource" => app::RibbonTab::Resource,
                "report" => app::RibbonTab::Report,
                "project" => app::RibbonTab::Project,
                "view" => app::RibbonTab::View,
                "help" => app::RibbonTab::Help,
                "format" => app::RibbonTab::Format,
                _ => app::RibbonTab::Task,
            };
            // dialogs, for screenshots: `--ui taskinfo|formatbar|barstyles|layout|gridlines|textstyles|timescale|fieldsettings|font|options`
            if app.visible_rows().len() > 4 {
                app.sel_row = 4;
                app.sel_anchor = 4;
            }
            match t {
                "taskinfo" => taskinfo::open_task_info(&mut app, 0),
                "taskinfo-adv" => taskinfo::open_task_info(&mut app, 3),
                "formatbar" => fmt_dialogs::open_format_bar(&mut app),
                "barstyles" => fmt_dialogs::open_bar_styles(&mut app),
                "layout" => fmt_dialogs::open_layout(&mut app),
                "gridlines" => fmt_dialogs::open_gridlines(&mut app),
                "textstyles" => fmt_dialogs::open_text_styles(&mut app),
                "timescale" => fmt_dialogs::open_timescale(&mut app, 1),
                "fieldsettings" => fmt_dialogs::open_field_settings(&mut app),
                "font" => fmt_dialogs::open_font(&mut app),
                "options" => fmt_dialogs::open_options(&mut app, 0),
                "options-schedule" => fmt_dialogs::open_options(&mut app, 2),
                "options-language" => fmt_dialogs::open_options(&mut app, 5),
                "projectinfo" => dialogs::open_project_info(&mut app),
                "about-dialog" => app.dialog = Some(dialogs::Dialog::About),
                "licenses" => app.dialog = Some(dialogs::Dialog::Licenses),
                _ => {}
            }
        }
        Ok(Box::new(app))
    }));
    if let Err(e) = &result {
        // the window could not be created (usually no OpenGL 2.0 driver, e.g. some remote desktops / VMs)
        rfd::MessageDialog::new()
            .set_title(crate::app::product())
            .set_level(rfd::MessageLevel::Error)
            .set_description(format!(
                "Neramit Planner could not start its window.

{e}

Neramit Planner needs a graphics driver with OpenGL 2.0 or newer.                  Please update the display driver, or run Neramit Planner on the local desktop instead of a remote session."
            ))
            .show();
    }
    result
}

impl eframe::App for App {
    fn ui(&mut self, root_ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        app_frame(self, root_ui);
    }
}

/// One frame of the whole program (the window's, or a headless QA run's).
pub fn app_frame(this: &mut App, root_ui: &mut egui::Ui) {
    this.doc.apply_currency();
    {
        let ctx = &root_ui.ctx().clone();
        // A maximized borderless window is placed with its invisible resize frame outside the
        // monitor (and under the taskbar). Lay everything out inside the visible work area.
        let full = root_ui.max_rect();
        let inner = match win11::maximized_insets() {
            Some([l, t, r, b]) => {
                let ppp = ctx.pixels_per_point();
                root_ui.painter().rect_filled(full, 0.0, theme::CLASSIC_GREEN);
                egui::Rect::from_min_max(full.min + egui::vec2(l / ppp, t / ppp), full.max - egui::vec2(r / ppp, b / ppp))
            }
            None => full,
        };
        let mut inner_ui = root_ui.new_child(egui::UiBuilder::new().max_rect(inner));
        let root = &mut inner_ui;
        let mut cmds: Vec<Cmd> = Vec::new();
        this.frame += 1;
        this.undo_tick();
        this.apply_pending_view(ctx);
        if this.frame == 3 && QA_BESTFIT.load(std::sync::atomic::Ordering::Relaxed) {
            for k in 0..this.task_cols.len() {
                this.task_cols[k].1 = grid::best_fit_width(this, ctx, k);
            }
        }

        // window title + close handling
        let title = this.title();
        let changed = ctx.data(|d| d.get_temp::<std::sync::Arc<TitleCacheHolder>>(egui::Id::unique("title")).map(|t| t.0 != title).unwrap_or(true));
        if changed {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            ctx.data_mut(|d| d.insert_temp(egui::Id::unique("title"), std::sync::Arc::new(TitleCacheHolder(title))));
        }
        // NERAMIT_CLOSE_LOG=<file>: one line per frame about the pointer and closing (diagnostics)
        if let Some(log) = std::env::var_os("NERAMIT_CLOSE_LOG") {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(log) {
                let ev: Vec<String> = ctx.input(|i| i.events.iter().filter_map(|e| match e {
                    egui::Event::PointerMoved(p) => Some(format!("move {:.0},{:.0}", p.x, p.y)),
                    egui::Event::PointerButton { pos, pressed, .. } => Some(format!("{} {:.0},{:.0}", if *pressed { "down" } else { "up" }, pos.x, pos.y)),
                    egui::Event::PointerGone => Some("gone".into()),
                    _ => None,
                }).collect());
                let _ = writeln!(f, "frame {} close_requested {} ok {} dialog {} title_rect {:?} events {:?}", this.frame, ctx.input(|i| i.viewport().close_requested()), this.confirm_close_ok, this.dialog.is_some(), this.title_rect, ev);
            }
        }
        if ctx.input(|i| i.viewport().close_requested()) && !this.confirm_close_ok {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            this.cmd_exit();
        }
        if this.confirm_close_ok {
            finish_exit(this, ctx);
        }
        // an update download finished while its dialog was hidden
        if this.dialog.is_none() && this.message.is_none() && update::take_notify() {
            this.dialog = Some(dialogs::Dialog::Update);
        }
        this.handle_dropped_files(ctx);
        global_shortcuts(this, ctx, &mut cmds);
        this.sync_view_scale();

        egui::Panel::top("titlebar").exact_size(ribbon::TITLE_H).frame(egui::Frame::NONE).show_separator_line(false).show(root, |ui| {
            ribbon::title_bar(this, ui, &mut cmds);
        });

        if this.backstage.is_some() {
            egui::CentralPanel::default().frame(egui::Frame::NONE).show(root, |ui| {
                ribbon::backstage(this, ui, &mut cmds);
            });
        } else {
            egui::Panel::top("tabs").exact_size(ribbon::tabs_h()).frame(egui::Frame::NONE).show_separator_line(false).show(root, |ui| {
                ribbon::tab_row(this, ui);
            });
            if !this.ribbon_collapsed {
                egui::Panel::top("ribbon").exact_size(ribbon::RIBBON_H).frame(egui::Frame::NONE).show_separator_line(false).show(root, |ui| {
                    ribbon::ribbon_body(this, ui, &mut cmds);
                });
            }
            egui::Panel::bottom("status").exact_size(24.0).frame(egui::Frame::NONE).show(root, |ui| {
                status_bar(this, ui, &mut cmds);
            });
            if this.show_timeline && this.view != View::ResourceSheet {
                egui::Panel::top("timeline").exact_size(84.0).frame(egui::Frame::NONE).show(root, |ui| {
                    let r = ui.max_rect();
                    gantt::timeline(this, ui, r);
                });
            }
            egui::CentralPanel::default().frame(egui::Frame::NONE).show(root, |ui| {
                windows_view(this, ui, &mut cmds);
            });
            grid::handle_keys(this, ctx, &mut cmds);
        }

        dialogs::show(this, ctx);
        ribbon::window_controls(this, ctx, &mut cmds);
        ribbon::resize_borders(ctx);

        for c in cmds {
            ribbon::run(this, ctx, c);
        }
        picture::tick(this, ctx);
        if let Some((path, frames)) = this.qa_screenshot.as_mut() {
            *frames += 1;
            ctx.request_repaint();
            if *frames == 10 {
                let path = path.clone();
                ctx.request_screenshot(move |img| {
                    let [w, h] = img.size;
                    let rgba: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
                    let _ = std::fs::write(&path, png(w as u32, h as u32, &rgba));
                    std::process::exit(0);
                });
            }
        }
        // a close confirmed during this frame (X button, "No" in the save prompt) must not wait
        // for the next mouse move: close now
        if this.confirm_close_ok {
            finish_exit(this, ctx);
            ctx.request_repaint();
        }
    }
}

fn finish_exit(app: &mut App, ctx: &egui::Context) {
    match update::on_exit() {
        Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        Err(e) => {
            app.confirm_close_ok = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            app.message = Some((tr("Neramit Planner Updates").into(), e));
        }
    }
}

struct TitleCacheHolder(String);

static QA_BESTFIT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn global_shortcuts(app: &mut App, ctx: &egui::Context, cmds: &mut Vec<Cmd>) {
    if app.message.is_some() {
        return;
    }
    let text_focus = ctx.memory(|m| m.focused().is_some());
    ctx.input_mut(|i| {
        use egui::{Key, KeyboardShortcut as KS, Modifiers as M};
        if i.consume_shortcut(&KS::new(M::COMMAND, Key::S)) {
            cmds.push(Cmd::Save);
        }
        if i.consume_shortcut(&KS::new(M::COMMAND, Key::O)) {
            cmds.push(Cmd::Open);
        }
        if i.consume_shortcut(&KS::new(M::COMMAND, Key::N)) {
            cmds.push(Cmd::New);
        }
        if i.consume_shortcut(&KS::new(M::NONE, Key::F12)) {
            cmds.push(Cmd::SaveAs);
        }
        if !text_focus && app.dialog.is_none() {
            if i.consume_shortcut(&KS::new(M::COMMAND, Key::Z)) {
                cmds.push(Cmd::Undo);
            }
            if i.consume_shortcut(&KS::new(M::COMMAND, Key::Y)) {
                cmds.push(Cmd::Redo);
            }
            if i.consume_shortcut(&KS::new(M::COMMAND, Key::P)) {
                cmds.push(Cmd::PrintPage);
            }
            if app.view.has_timescale() && app.backstage.is_none() {
                // Ctrl+/ (MPP) or Ctrl++ / Ctrl+- zoom the timescale
                if i.consume_shortcut(&KS::new(M::COMMAND, Key::Slash))
                    || i.consume_shortcut(&KS::new(M::COMMAND, Key::Plus))
                    || i.consume_shortcut(&KS::new(M::COMMAND, Key::Equals))
                {
                    cmds.push(Cmd::ZoomIn);
                }
                if i.consume_shortcut(&KS::new(M::COMMAND, Key::Minus)) {
                    cmds.push(Cmd::ZoomOut);
                }
                if i.consume_shortcut(&KS::new(M::COMMAND | M::SHIFT, Key::F5)) {
                    cmds.push(Cmd::ScrollToTask);
                }
                let origin = app.gantt_origin();
                let (_, width) = app.chart_view;
                if i.consume_shortcut(&KS::new(M::ALT, Key::Home)) {
                    app.scroll_x = (((app.doc.start - origin).num_days() as f32) * app.px_per_day - 12.0).max(0.0);
                }
                if i.consume_shortcut(&KS::new(M::ALT, Key::End)) {
                    let fin = app.doc.tasks.iter().map(|t| t.finish).max().unwrap_or(app.doc.start);
                    app.scroll_x = (((fin - origin).num_days() + 1) as f32 * app.px_per_day - width + 24.0).max(0.0);
                }
                let step = (width / 8.0).max(40.0);
                if i.consume_shortcut(&KS::new(M::ALT, Key::ArrowLeft)) {
                    app.scroll_x = (app.scroll_x - step).max(0.0);
                }
                if i.consume_shortcut(&KS::new(M::ALT, Key::ArrowRight)) {
                    app.scroll_x += step;
                }
            }
        }
    });
}

/// The project's windows in the frame: the active one filling it, or all of them tiled (View >
/// Arrange All); every window has its own view, selection and scroll (View > New Window).
fn windows_view(app: &mut App, ui: &mut egui::Ui, cmds: &mut Vec<Cmd>) {
    let full = ui.max_rect();
    app.view_rect = full;
    let visible: Vec<u32> = app.window_list().iter().filter(|w| !w.1).map(|w| w.0).collect();
    if visible.is_empty() {
        // every window hidden: an empty frame (View > Switch Windows > Unhide)
        ui.painter().rect_filled(full, 0.0, theme::HEADER_BG);
        ui.scope_builder(egui::UiBuilder::new().max_rect(full.shrink(40.0)), |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(full.height() / 3.0);
                ui.label(egui::RichText::new(tr("All windows of the project are hidden.")).color(theme::TEXT_DIM));
                let hidden: Vec<u32> = app.window_list().iter().map(|w| w.0).collect();
                for num in hidden {
                    if ui.button(format!("{} {}", tr("Unhide"), crate::thai::shape(&app.window_caption(num)))).clicked() {
                        cmds.push(Cmd::UnhideWindow(num));
                    }
                }
            });
        });
        return;
    }
    if app.win_hidden {
        app.switch_window(visible[0]);
    }
    if !app.arranged || visible.len() < 2 {
        main_view(app, ui, cmds);
        return;
    }
    // tiled: up to three windows one above the other, more in two columns
    let n = visible.len();
    let (cols, rows) = if n <= 3 { (1, n) } else { (2, n.div_ceil(2)) };
    let (w, h) = (full.width() / cols as f32, full.height() / rows as f32);
    const CAP_H: f32 = 20.0;
    let mut activate: Option<u32> = None;
    let mut maximize = false;
    let mut later: Vec<Cmd> = Vec::new();
    for (k, &num) in visible.iter().enumerate() {
        let (c, r) = (k % cols, k / cols);
        let tile = Rect::from_min_size(Pos2::new(full.left() + c as f32 * w, full.top() + r as f32 * h), Vec2::new(w, h)).shrink(1.0);
        let cap = Rect::from_min_max(tile.min, Pos2::new(tile.right(), tile.top() + CAP_H));
        let body = Rect::from_min_max(Pos2::new(tile.left(), cap.bottom()), tile.max);
        let active = num == app.win_num;
        let p = ui.painter();
        p.rect_filled(cap, 0.0, if active { theme::CLASSIC_GREEN } else { theme::HEADER_BG });
        p.rect_stroke(tile, 0.0, egui::Stroke::new(1.0, if active { theme::CLASSIC_GREEN } else { theme::RIBBON_BORDER }), egui::StrokeKind::Inside);
        p.text(
            Pos2::new(cap.left() + 6.0, cap.center().y),
            egui::Align2::LEFT_CENTER,
            crate::thai::shape(&app.window_caption(num)).to_string(),
            egui::FontId::proportional(12.0),
            if active { Color32::WHITE } else { theme::TEXT },
        );
        // the caption: click = activate, double-click or the button = fill the frame again
        let max_btn = Rect::from_center_size(Pos2::new(cap.right() - 12.0, cap.center().y), Vec2::splat(14.0));
        p.rect_stroke(max_btn.shrink(2.0), 0.0, egui::Stroke::new(1.0, if active { Color32::WHITE } else { theme::TEXT }), egui::StrokeKind::Inside);
        let cr = ui.interact(cap, egui::Id::unique("win_caption").with(num), egui::Sense::click());
        if cr.double_clicked() || (cr.clicked() && cr.interact_pointer_pos().map(|q| max_btn.contains(q)).unwrap_or(false)) {
            activate = Some(num);
            maximize = true;
        } else if cr.clicked() {
            activate = Some(num);
        }
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body).id_salt(("win_body", num)));
        child.set_clip_rect(body);
        if active {
            main_view(app, &mut child, cmds);
        } else if let Some(i) = app.windows.iter().position(|x| x.num == num) {
            // an inactive window drawn with its own state; using it makes it the active one
            let mut st = app.windows[i].state.clone();
            let dialog_before = app.dialog.is_some();
            app.swap_window_state(&mut st);
            let mut own: Vec<Cmd> = Vec::new();
            main_view(app, &mut child, &mut own);
            app.swap_window_state(&mut st);
            if let Some(x) = app.windows.iter_mut().find(|x| x.num == num) {
                x.state = st;
            }
            let pressed = ui.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().map(|q| body.contains(q)).unwrap_or(false));
            if pressed || !own.is_empty() || (!dialog_before && app.dialog.is_some()) {
                activate = Some(num);
                later.extend(own);
            }
        }
    }
    if let Some(num) = activate {
        app.switch_window(num);
        if maximize {
            app.arranged = false;
        }
    }
    cmds.extend(later);
}

fn main_view(app: &mut App, ui: &mut egui::Ui, cmds: &mut Vec<Cmd>) {
    let full = ui.max_rect();
    let p = ui.painter().clone();
    // vertical view-name bar on the far left
    let label_bar = Rect::from_min_size(full.min, Vec2::new(grid::VIEW_BAR_W, full.height()));
    // the standard grey gradient running into the ID column, no edge line, green label
    for (i, v) in theme::SHEET_VIEW_BAR.iter().enumerate() {
        let x = label_bar.left() + i as f32;
        p.rect_filled(Rect::from_min_max(Pos2::new(x, label_bar.top()), Pos2::new(x + 1.0, label_bar.bottom())), 0.0, Color32::from_gray(*v));
    }
    gantt::vertical_text(&p, label_bar, tr(app.view.label()), theme::SHEET_VIEW_LABEL);
    let area = Rect::from_min_max(Pos2::new(label_bar.right(), full.top()), full.max);

    // mouse wheel: vertical scroll, Shift = horizontal, Ctrl = zoom the timescale around the pointer
    if let Some(pos) = ui.ctx().pointer_hover_pos() {
        if area.contains(pos) && app.dialog.is_none() {
            // over the chart the date under the pointer stays put; over the sheet zoom keeps the centre
            let anchor = (app.view.has_timescale() && pos.x >= app.chart_view.0).then_some(pos.x);
            let events = ui.input(|i| i.events.clone());
            for ev in events {
                if let (egui::Event::Zoom(f), true) = (&ev, app.view.has_timescale()) {
                    // touchpad pinch
                    app.zoom_by(*f, anchor);
                }
                if let egui::Event::MouseWheel { unit, delta, modifiers, .. } = ev {
                    let k = match unit {
                        egui::MouseWheelUnit::Line => grid::ROW_H * 3.0,
                        egui::MouseWheelUnit::Page => area.height(),
                        egui::MouseWheelUnit::Point => 1.0,
                    };
                    if modifiers.ctrl || modifiers.command {
                        if app.view.has_timescale() {
                            // one wheel notch = 25 %; smooth (touchpad) deltas zoom proportionally
                            let notches = match unit {
                                egui::MouseWheelUnit::Line => delta.y,
                                egui::MouseWheelUnit::Page => delta.y * 3.0,
                                egui::MouseWheelUnit::Point => delta.y / 50.0,
                            };
                            app.zoom_by(1.25_f32.powf(notches), anchor);
                        }
                    } else if modifiers.shift {
                        app.scroll_x = (app.scroll_x - delta.y * k).max(0.0);
                    } else {
                        app.scroll_y -= delta.y * k;
                        app.scroll_x = (app.scroll_x - delta.x * k).max(0.0);
                    }
                }
            }
        }
    }

    match app.view {
        View::ResourceSheet => {
            let gr = Rect::from_min_max(area.min, Pos2::new(area.right() - grid::SCROLLBAR, area.bottom()));
            grid::resource_grid(app, ui, gr, cmds);
            let vr = Rect::from_min_max(Pos2::new(area.right() - grid::SCROLLBAR, area.top() + grid::HEADER_H), area.max);
            p.rect_filled(Rect::from_min_max(Pos2::new(vr.left(), area.top()), Pos2::new(vr.right(), vr.top())), 0.0, theme::HEADER_BG);
            grid::vscrollbar(app, ui, vr, area.height() - grid::HEADER_H);
        }
        View::TaskSheet => {
            let gr = Rect::from_min_max(area.min, Pos2::new(area.right() - grid::SCROLLBAR, area.bottom()));
            grid::task_grid(app, ui, gr, cmds);
            let hh = grid::header_h(app);
            let vr = Rect::from_min_max(Pos2::new(area.right() - grid::SCROLLBAR, area.top() + hh), area.max);
            p.rect_filled(Rect::from_min_max(Pos2::new(vr.left(), area.top()), Pos2::new(vr.right(), vr.top())), 0.0, theme::HEADER_BG);
            grid::vscrollbar(app, ui, vr, area.height() - hh);
        }
        View::NetworkDiagram => crate::network::network_view(app, ui, area, cmds),
        View::TaskBoard => crate::board::board_view(app, ui, area, cmds),
        View::SprintBoard => crate::board::sprint_board(app, ui, area, cmds),
        View::Report => crate::reports::report_view(app, ui, area),
        View::Calendar => crate::calendar::calendar_view(app, ui, area, cmds),
        View::TaskUsage | View::ResourceUsage => {
            let resources = app.view == View::ResourceUsage;
            crate::usage::usage_view(app, ui, area, cmds, resources);
        }
        View::Gantt | View::TrackingGantt => {
            // View > Details: the Task Form takes the bottom of the view
            let area = if app.show_details && area.height() > 320.0 {
                let h = (area.height() * 0.38).clamp(200.0, 320.0);
                let form = Rect::from_min_max(Pos2::new(area.left(), area.bottom() - h), area.max);
                crate::taskinfo::task_form(app, ui, form);
                Rect::from_min_max(area.min, Pos2::new(area.right(), form.top() - 3.0))
            } else {
                area
            };
            app.grid_width = app.grid_width.clamp(120.0, (area.width() - 160.0).max(120.0));
            let gx = area.left() + app.grid_width;
            let gr = Rect::from_min_max(area.min, Pos2::new(gx, area.bottom() - grid::SCROLLBAR));
            let split = Rect::from_min_max(Pos2::new(gx, area.top()), Pos2::new(gx + 5.0, area.bottom()));
            let chart = Rect::from_min_max(Pos2::new(split.right(), area.top()), area.max);
            grid::task_grid(app, ui, gr, cmds);
            // horizontal filler under the grid (where a scrollbar would be)
            p.rect_filled(Rect::from_min_max(Pos2::new(area.left(), gr.bottom()), Pos2::new(gx, area.bottom())), 0.0, Color32::from_rgb(0xF0, 0xF0, 0xF0));
            // splitter
            let sresp = ui.interact(split, ui.scope_id().with("splitter"), Sense::drag());
            if !(sresp.hovered() || sresp.dragged()) {
                // the standard divider: light grey with darker edges
                p.rect_filled(split, 0.0, theme::SHEET_HEAD_BG);
                p.line_segment([Pos2::new(split.left() + 0.5, split.top()), Pos2::new(split.left() + 0.5, split.bottom())], Stroke::new(1.0_f32, theme::SHEET_SPLIT_EDGE));
                p.line_segment([Pos2::new(split.right() - 0.5, split.top()), Pos2::new(split.right() - 0.5, split.bottom())], Stroke::new(1.0_f32, theme::SHEET_SPLIT_EDGE));
            } else {
                p.rect_filled(split, 0.0, if sresp.hovered() || sresp.dragged() { theme::GREEN_LIGHT } else { Color32::from_rgb(0xD8, 0xD8, 0xD8) });
            }
            if sresp.hovered() || sresp.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeColumn);
            }
            if sresp.dragged() {
                app.grid_width += sresp.drag_delta().x;
            }
            gantt::gantt(app, ui, chart, cmds);
        }
    }
}

fn status_bar(app: &mut App, ui: &mut egui::Ui, cmds: &mut Vec<Cmd>) {
    let r = ui.max_rect();
    let p = ui.painter().clone();
    p.rect_filled(r, 0.0, if app.settings.classic_ui { theme::CLASSIC_STATUS_BG } else { theme::STATUS_BG });
    p.line_segment([r.left_top(), r.right_top()], Stroke::new(1.0_f32, theme::RIBBON_BORDER));
    let font = FontId::proportional(11.5);
    let mut x = r.left() + 10.0;
    let g = p.layout_no_wrap(tr(&app.status).to_string(), font.clone(), theme::TEXT);
    let w = g.size().x;
    p.galley(Pos2::new(x, r.center().y - g.size().y / 2.0), g, theme::TEXT);
    x += w.max(60.0) + 24.0;
    let mode = tr(if app.doc.new_tasks_auto { "New Tasks : Auto Scheduled" } else { "New Tasks : Manually Scheduled" });
    let mr = Rect::from_min_size(Pos2::new(x, r.top() + 1.0), Vec2::new(230.0, r.height() - 2.0));
    let mresp = ui.interact(mr, ui.scope_id().with("mode_status"), Sense::click());
    if mresp.hovered() {
        p.rect_filled(mr, 0.0, theme::HOVER_BG);
    }
    let ir = Rect::from_center_size(Pos2::new(x + 11.0, r.center().y), Vec2::new(14.0, 12.0));
    theme::paint_icon(&p, ir, if app.doc.new_tasks_auto { theme::Icon::Auto } else { theme::Icon::Manual }, true);
    p.text(Pos2::new(x + 22.0, r.center().y), Align2::LEFT_CENTER, mode, font.clone(), theme::TEXT);
    if mresp.clicked() {
        app.doc.new_tasks_auto = !app.doc.new_tasks_auto;
    }

    status_right(app, ui, r, cmds, font);
}

fn status_right(app: &mut App, ui: &mut egui::Ui, r: Rect, cmds: &mut Vec<Cmd>, font: FontId) {
    let p = ui.painter().clone();
    // right side: view shortcuts + zoom slider
    let mut rx = r.right() - 12.0;
    let slider_w = 110.0;
    let sr = Rect::from_min_max(Pos2::new(rx - slider_w, r.top() + 4.0), Pos2::new(rx, r.bottom() - 4.0));
    rx -= slider_w + 6.0;
    let t = app.zoom_fraction().clamp(0.0, 1.0);
    p.line_segment([Pos2::new(sr.left() + 12.0, sr.center().y), Pos2::new(sr.right() - 12.0, sr.center().y)], Stroke::new(1.0_f32, theme::TEXT_DIM));
    let kx = sr.left() + 12.0 + t * (sr.width() - 24.0);
    p.rect_filled(Rect::from_center_size(Pos2::new(kx, sr.center().y), Vec2::new(5.0, 11.0)), 0.0, theme::TEXT);
    p.text(Pos2::new(sr.left() + 3.0, sr.center().y), Align2::CENTER_CENTER, "−", font.clone(), theme::TEXT);
    p.text(Pos2::new(sr.right() - 3.0, sr.center().y), Align2::CENTER_CENTER, "+", font.clone(), theme::TEXT);
    let zresp = ui.interact(sr, ui.scope_id().with("zoom_slider"), Sense::click_and_drag()).on_hover_text(trf("Zoom: {}", &[tr(gantt::timescale_name(app.px_per_day))]));
    if zresp.clicked() || zresp.dragged() {
        if let Some(pos) = zresp.interact_pointer_pos() {
            if pos.x < sr.left() + 10.0 && zresp.clicked() {
                cmds.push(Cmd::ZoomOut);
            } else if pos.x > sr.right() - 10.0 && zresp.clicked() {
                cmds.push(Cmd::ZoomIn);
            } else {
                let f = ((pos.x - sr.left() - 12.0) / (sr.width() - 24.0)).clamp(0.0, 1.0);
                app.set_zoom_fraction(f);
            }
        }
    }
    for (icon, view, tip) in [
        (theme::Icon::ResourceSheet, View::ResourceSheet, "Resource Sheet"),
        (theme::Icon::Sheet, View::TaskSheet, "Task Sheet"),
        (theme::Icon::Gantt, View::Gantt, "Gantt Chart"),
    ] {
        let br = Rect::from_min_max(Pos2::new(rx - 24.0, r.top() + 2.0), Pos2::new(rx, r.bottom() - 2.0));
        let resp = ui.interact(br, ui.scope_id().with(tip), Sense::click()).on_hover_text(tr(tip));
        if app.view == view {
            p.rect_filled(br, 0.0, theme::PRESSED_BG);
        } else if resp.hovered() {
            p.rect_filled(br, 0.0, theme::HOVER_BG);
        }
        theme::paint_icon(&p, Rect::from_center_size(br.center(), Vec2::splat(15.0)), icon, true);
        if resp.clicked() {
            cmds.push(Cmd::View(view));
        }
        rx -= 26.0;
    }
}

/// Minimal RGBA PNG encoder (QA screenshots).
pub fn png(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    fn crc(data: &[u8]) -> u32 {
        let mut c = 0xFFFF_FFFFu32;
        for b in data {
            c ^= *b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
        }
        !c
    }
    let mut raw = Vec::with_capacity((w * h * 4 + h) as usize);
    for row in rgba.chunks((w * 4) as usize) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let mut out = vec![0x89u8, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let chunk = |tag: &[u8], data: &[u8], out: &mut Vec<u8>| {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut td = tag.to_vec();
        td.extend_from_slice(data);
        out.extend_from_slice(&td);
        out.extend_from_slice(&crc(&td).to_be_bytes());
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(b"IHDR", &ihdr, &mut out);
    chunk(b"IDAT", &miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6), &mut out);
    chunk(b"IEND", &[], &mut out);
    out
}
