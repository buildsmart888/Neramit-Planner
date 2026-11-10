//! Application state and all editing commands.

use crate::dialogs::Dialog;
use crate::fileio;
use crate::model::*;
use crate::schedule;
use crate::util;
use chrono::NaiveDate;
use eframe::egui;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum View {
    Gantt,
    TaskSheet,
    ResourceSheet,
    TrackingGantt,
    TaskUsage,
    ResourceUsage,
    NetworkDiagram,
    Calendar,
    /// View > Task Board
    TaskBoard,
    /// View > Sprint Planning Board (Project > Manage Sprints)
    SprintBoard,
    /// Report tab: the report in app.report
    Report,
}

impl View {
    pub fn label(self) -> &'static str {
        match self {
            View::Gantt => "GANTT CHART",
            View::TrackingGantt => "TRACKING GANTT",
            View::TaskUsage => "TASK USAGE",
            View::ResourceUsage => "RESOURCE USAGE",
            View::NetworkDiagram => "NETWORK DIAGRAM",
            View::Calendar => "CALENDAR",
            View::TaskSheet => "TASK SHEET",
            View::ResourceSheet => "RESOURCE SHEET",
            View::TaskBoard => "TASK BOARD",
            View::SprintBoard => "SPRINT PLANNING BOARD",
            View::Report => "REPORT",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RibbonTab {
    Task,
    Resource,
    Report,
    Project,
    View,
    Format,
    Help,
}

/// Task table columns.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum TCol {
    Indicators,
    Mode,
    Name,
    Duration,
    Start,
    Finish,
    Predecessors,
    Successors,
    Resources,
    Percent,
    Cost,
    TotalSlack,
    OutlineNumber,
    Notes,
    Uid,
    Critical,
    Deadline,
    Priority,
    ConstraintType,
    ConstraintDate,
    Wbs,
    ActualStart,
    ActualFinish,
    ActualDuration,
    RemainingDuration,
    BaselineStart,
    BaselineFinish,
    BaselineDuration,
    StartVariance,
    FinishVariance,
    DurationVariance,
    PhysicalPercent,
    Status,
    EarlyStart,
    EarlyFinish,
    LateStart,
    LateFinish,
    FreeSlack,
    Work,
    ActualWork,
    ActualCost,
    FixedCost,
    FixedCostAccrual,
    BaselineCost,
    CostVariance,
    RemainingCost,
    BaselineWork,
    WorkVariance,
    RemainingWork,
    PercentWorkComplete,
    LevelingDelay,
    /// a custom field (Text1, Number1, ...)
    Custom(crate::custom::CustomField),
    Hyperlink,
    HyperlinkAddress,
    HyperlinkSubAddress,
    Sprint,
}

impl TCol {
    pub const ALL: [TCol; 51] = [
        TCol::Indicators,
        TCol::Mode,
        TCol::Name,
        TCol::Duration,
        TCol::Start,
        TCol::Finish,
        TCol::Predecessors,
        TCol::Successors,
        TCol::Resources,
        TCol::Percent,
        TCol::Cost,
        TCol::TotalSlack,
        TCol::OutlineNumber,
        TCol::Notes,
        TCol::Uid,
        TCol::Critical,
        TCol::Deadline,
        TCol::Priority,
        TCol::ConstraintType,
        TCol::ConstraintDate,
        TCol::Wbs,
        TCol::ActualStart,
        TCol::ActualFinish,
        TCol::ActualDuration,
        TCol::RemainingDuration,
        TCol::BaselineStart,
        TCol::BaselineFinish,
        TCol::BaselineDuration,
        TCol::StartVariance,
        TCol::FinishVariance,
        TCol::DurationVariance,
        TCol::PhysicalPercent,
        TCol::Status,
        TCol::EarlyStart,
        TCol::EarlyFinish,
        TCol::LateStart,
        TCol::LateFinish,
        TCol::FreeSlack,
        TCol::Work,
        TCol::ActualWork,
        TCol::ActualCost,
        TCol::FixedCost,
        TCol::FixedCostAccrual,
        TCol::BaselineCost,
        TCol::CostVariance,
        TCol::RemainingCost,
        TCol::BaselineWork,
        TCol::WorkVariance,
        TCol::RemainingWork,
        TCol::PercentWorkComplete,
        TCol::LevelingDelay,
    ];
    pub fn title(self) -> &'static str {
        match self {
            TCol::Indicators => "ℹ",
            TCol::Mode => "Task Mode",
            TCol::Name => "Task Name",
            TCol::Duration => "Duration",
            TCol::Start => "Start",
            TCol::Finish => "Finish",
            TCol::Predecessors => "Predecessors",
            TCol::Successors => "Successors",
            TCol::Resources => "Resource Names",
            TCol::Percent => "% Complete",
            TCol::Cost => "Cost",
            TCol::TotalSlack => "Total Slack",
            TCol::OutlineNumber => "Outline Number",
            TCol::Notes => "Notes",
            TCol::Uid => "Unique ID",
            TCol::Critical => "Critical",
            TCol::Deadline => "Deadline",
            TCol::Priority => "Priority",
            TCol::ConstraintType => "Constraint Type",
            TCol::ConstraintDate => "Constraint Date",
            TCol::Wbs => "WBS",
            TCol::ActualStart => "Actual Start",
            TCol::ActualFinish => "Actual Finish",
            TCol::ActualDuration => "Actual Duration",
            TCol::RemainingDuration => "Remaining Duration",
            TCol::BaselineStart => "Baseline Start",
            TCol::BaselineFinish => "Baseline Finish",
            TCol::BaselineDuration => "Baseline Duration",
            TCol::StartVariance => "Start Variance",
            TCol::FinishVariance => "Finish Variance",
            TCol::DurationVariance => "Duration Variance",
            TCol::PhysicalPercent => "Physical % Complete",
            TCol::Status => "Status",
            TCol::EarlyStart => "Early Start",
            TCol::EarlyFinish => "Early Finish",
            TCol::LateStart => "Late Start",
            TCol::LateFinish => "Late Finish",
            TCol::FreeSlack => "Free Slack",
            TCol::Work => "Work",
            TCol::ActualWork => "Actual Work",
            TCol::ActualCost => "Actual Cost",
            TCol::FixedCost => "Fixed Cost",
            TCol::FixedCostAccrual => "Fixed Cost Accrual",
            TCol::BaselineCost => "Baseline Cost",
            TCol::CostVariance => "Cost Variance",
            TCol::RemainingCost => "Remaining Cost",
            TCol::BaselineWork => "Baseline Work",
            TCol::WorkVariance => "Work Variance",
            TCol::RemainingWork => "Remaining Work",
            TCol::PercentWorkComplete => "% Work Complete",
            TCol::LevelingDelay => "Leveling Delay",
            TCol::Custom(f) => f.name(),
            TCol::Hyperlink => "Hyperlink",
            TCol::HyperlinkAddress => "Address",
            TCol::HyperlinkSubAddress => "SubAddress",
            TCol::Sprint => "Sprint",
        }
    }
    /// Default data alignment (the standard Entry table): 0 left, 1 center, 2 right.
    pub fn default_align(self) -> u8 {
        match self {
            TCol::Percent | TCol::Cost | TCol::TotalSlack | TCol::Uid | TCol::Priority | TCol::PhysicalPercent | TCol::FreeSlack | TCol::Work | TCol::ActualWork | TCol::ActualCost
            | TCol::FixedCost
            | TCol::BaselineCost
            | TCol::CostVariance
            | TCol::RemainingCost
            | TCol::BaselineWork
            | TCol::WorkVariance
            | TCol::RemainingWork
            | TCol::PercentWorkComplete => 2,
            TCol::Indicators | TCol::Mode | TCol::Critical => 1,
            TCol::Custom(f) if matches!(f.kind, crate::custom::CfKind::Number | crate::custom::CfKind::Cost) => 2,
            _ => 0,
        }
    }
    pub fn default_width(self) -> f32 {
        match self {
            TCol::Indicators => 24.0,
            TCol::Mode => 58.0,
            TCol::Name => 230.0,
            TCol::Duration => 70.0,
            TCol::Start | TCol::Finish | TCol::Deadline | TCol::ConstraintDate => 92.0,
            TCol::ActualStart | TCol::ActualFinish | TCol::BaselineStart | TCol::BaselineFinish | TCol::EarlyStart | TCol::EarlyFinish | TCol::LateStart | TCol::LateFinish => 92.0,
            TCol::Status => 90.0,
            TCol::ConstraintType => 130.0,
            TCol::Priority | TCol::Wbs => 60.0,
            TCol::Predecessors | TCol::Successors => 90.0,
            TCol::Resources => 120.0,
            TCol::Notes => 160.0,
            _ => 80.0,
        }
    }
    pub fn editable(self) -> bool {
        matches!(
            self,
            TCol::Mode
                | TCol::Name
                | TCol::Duration
                | TCol::Start
                | TCol::Finish
                | TCol::Predecessors
                | TCol::Resources
                | TCol::Percent
                | TCol::Notes
                | TCol::Deadline
                | TCol::Priority
                | TCol::ConstraintType
                | TCol::ConstraintDate
                | TCol::Wbs
                | TCol::ActualStart
                | TCol::ActualFinish
                | TCol::ActualDuration
                | TCol::RemainingDuration
                | TCol::BaselineStart
                | TCol::BaselineFinish
                | TCol::BaselineDuration
                | TCol::PhysicalPercent
                | TCol::Work
                | TCol::FixedCost
                | TCol::FixedCostAccrual
                | TCol::BaselineCost
                | TCol::BaselineWork
                | TCol::LevelingDelay
                | TCol::Custom(_)
                | TCol::Hyperlink
                | TCol::HyperlinkAddress
                | TCol::HyperlinkSubAddress
                | TCol::Sprint
        )
    }
}

/// The standard built-in task filters (View > Filter, View > Highlight).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum TaskFilter {
    #[default]
    All,
    Active,
    Completed,
    Critical,
    DateRange,
    Incomplete,
    Late,
    Manual,
    Milestones,
    Slipping,
    Summary,
    Estimated,
    UsingResource,
}

impl TaskFilter {
    pub const ALL: [TaskFilter; 13] = [
        TaskFilter::Active,
        TaskFilter::Completed,
        TaskFilter::Critical,
        TaskFilter::DateRange,
        TaskFilter::Incomplete,
        TaskFilter::Late,
        TaskFilter::Manual,
        TaskFilter::Milestones,
        TaskFilter::Slipping,
        TaskFilter::Summary,
        TaskFilter::Estimated,
        TaskFilter::UsingResource,
        TaskFilter::All,
    ];
    pub fn label(self) -> &'static str {
        match self {
            TaskFilter::All => "All Tasks",
            TaskFilter::Active => "Active Tasks",
            TaskFilter::Completed => "Completed Tasks",
            TaskFilter::Critical => "Critical",
            TaskFilter::DateRange => "Date Range...",
            TaskFilter::Incomplete => "Incomplete Tasks",
            TaskFilter::Late => "Late Tasks",
            TaskFilter::Manual => "Manually Scheduled Tasks",
            TaskFilter::Milestones => "Milestones",
            TaskFilter::Slipping => "Slipping Tasks",
            TaskFilter::Summary => "Summary Tasks",
            TaskFilter::Estimated => "Tasks With Estimated Durations",
            TaskFilter::UsingResource => "Using Resource...",
        }
    }
    /// The filter asks for dates or a resource.
    pub fn needs_input(self) -> bool {
        matches!(self, TaskFilter::DateRange | TaskFilter::UsingResource)
    }
}

/// A filter with what it asked for.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct FilterSpec {
    pub kind: TaskFilter,
    pub range: Option<(NaiveDate, NaiveDate)>,
    pub resource: Option<u32>,
}

/// View > Group by (the standard built-in groups).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum TaskGroup {
    #[default]
    None,
    ActiveInactive,
    AutoManual,
    Completion,
    Critical,
    Duration,
    Milestones,
    Priority,
    Status,
}

impl TaskGroup {
    pub const ALL: [TaskGroup; 8] = [
        TaskGroup::ActiveInactive,
        TaskGroup::AutoManual,
        TaskGroup::Completion,
        TaskGroup::Critical,
        TaskGroup::Duration,
        TaskGroup::Milestones,
        TaskGroup::Priority,
        TaskGroup::Status,
    ];
    pub fn label(self) -> &'static str {
        match self {
            TaskGroup::None => "No Group",
            TaskGroup::ActiveInactive => "Active v. Inactive",
            TaskGroup::AutoManual => "Auto Scheduled v. Manually Scheduled",
            TaskGroup::Completion => "Complete and Incomplete Tasks",
            TaskGroup::Critical => "Critical",
            TaskGroup::Duration => "Duration",
            TaskGroup::Milestones => "Milestones",
            TaskGroup::Priority => "Priority",
            TaskGroup::Status => "Status",
        }
    }
}

/// Height of a group header band.
pub const GROUP_H: f32 = 23.0;

/// View > Sort.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum SortKey {
    #[default]
    Id,
    Start,
    Finish,
    Priority,
    Cost,
    Duration,
    Name,
}

impl SortKey {
    pub const ALL: [SortKey; 7] = [SortKey::Start, SortKey::Finish, SortKey::Priority, SortKey::Cost, SortKey::Id, SortKey::Duration, SortKey::Name];
    pub fn label(self) -> &'static str {
        match self {
            SortKey::Id => "by ID",
            SortKey::Start => "by Start Date",
            SortKey::Finish => "by Finish Date",
            SortKey::Priority => "by Priority",
            SortKey::Cost => "by Cost",
            SortKey::Duration => "by Duration",
            SortKey::Name => "by Name",
        }
    }
}

impl View {
    /// A view with a Gantt chart beside the sheet.
    pub fn is_gantt(self) -> bool {
        matches!(self, View::Gantt | View::TrackingGantt)
    }

    /// Task Usage / Resource Usage (their own timescale zoom).
    pub fn is_usage(self) -> bool {
        matches!(self, View::TaskUsage | View::ResourceUsage)
    }

    /// A view with a timescale (zoom and horizontal scrolling).
    pub fn has_timescale(self) -> bool {
        matches!(self, View::Gantt | View::TrackingGantt | View::TaskUsage | View::ResourceUsage)
    }
}

/// Resource sheet columns.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RCol {
    Name,
    Type,
    Material,
    Initials,
    Group,
    Max,
    StdRate,
    OvtRate,
    CostUse,
    Calendar,
    AccrueAt,
    Code,
    Work,
    Cost,
}

impl RCol {
    pub const ALL: [RCol; 14] = [
        RCol::Name,
        RCol::Type,
        RCol::Material,
        RCol::Initials,
        RCol::Group,
        RCol::Max,
        RCol::StdRate,
        RCol::OvtRate,
        RCol::CostUse,
        RCol::AccrueAt,
        RCol::Calendar,
        RCol::Code,
        RCol::Work,
        RCol::Cost,
    ];
    /// The standard Resource Sheet (Entry table).
    pub const DEFAULT: [RCol; 12] =
        [RCol::Name, RCol::Type, RCol::Material, RCol::Initials, RCol::Group, RCol::Max, RCol::StdRate, RCol::OvtRate, RCol::CostUse, RCol::AccrueAt, RCol::Calendar, RCol::Code];
    pub fn title(self) -> &'static str {
        match self {
            RCol::Name => "Resource Name",
            RCol::Type => "Type",
            RCol::Material => "Material",
            RCol::Initials => "Initials",
            RCol::Group => "Group",
            RCol::Max => "Max.",
            RCol::StdRate => "Std. Rate",
            RCol::OvtRate => "Ovt. Rate",
            RCol::CostUse => "Cost/Use",
            RCol::Calendar => "Base Calendar",
            RCol::AccrueAt => "Accrue At",
            RCol::Code => "Code",
            RCol::Work => "Work",
            RCol::Cost => "Cost",
        }
    }
    pub fn default_width(self) -> f32 {
        match self {
            RCol::Name => 180.0,
            RCol::Type | RCol::Initials | RCol::Max => 70.0,
            RCol::Calendar => 100.0,
            _ => 90.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct EditState {
    pub row: usize,
    pub col: usize,
    pub text: String,
    pub just_opened: bool,
    /// Select the whole text when the editor opens (false = caret at end, used when typing starts the edit).
    pub select_all: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum BarDragKind {
    /// pressed on the middle of a bar: becomes Move along its row, Link into another row
    Press,
    Move,
    /// the right end of a bar: duration
    Resize,
    /// the end of the progress bar: % complete
    Progress,
    /// dragged onto another task: Finish-to-Start link
    Link,
    /// dragged in an empty row: a new task with those dates
    Draw,
}

#[derive(Clone, Debug)]
pub struct BarDrag {
    pub task_idx: usize,
    pub kind: BarDragKind,
    /// start / finish of the bar when pressed (the pressed instant for Draw)
    pub orig_s: chrono::NaiveDateTime,
    pub orig_f: chrono::NaiveDateTime,
    pub press: (f32, f32),
    /// chart row of the pressed bar
    pub row: usize,
    /// piece of a split task being dragged (0 = the first piece / whole task)
    pub piece: usize,
}

#[derive(Clone, Debug)]
pub enum PendingAction {
    New,
    Open,
    OpenPath(PathBuf),
    Exit,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Backstage {
    /// Start screen ("Home"): templates + recent / favorites
    Home,
    Info,
    New,
    Open,
    SaveAs,
    Export,
    Print,
    Options,
    About,
}

/// File > Print settings.
#[derive(Clone, Debug)]
pub struct PrintSettings {
    pub copies: u32,
    /// Print Specific Dates (text as typed)
    pub specific_dates: bool,
    pub from: String,
    pub to: String,
    pub landscape: bool,
    /// 0 A4, 1 A3, 2 Letter, 3 Legal, 4 Tabloid
    pub paper: u8,
    /// page shown in the preview
    pub page: usize,
    /// signature boxes on the last page (on by default for Thai users)
    pub signoff: bool,
}

impl Default for PrintSettings {
    fn default() -> Self {
        PrintSettings { copies: 1, specific_dates: false, from: String::new(), to: String::new(), landscape: true, paper: 0, page: 0, signoff: crate::i18n::thai_user() }
    }
}

pub const PAPERS: [(&str, f32, f32); 5] = [("A4", 595.28, 841.89), ("A3", 841.89, 1190.55), ("Letter", 612.0, 792.0), ("Legal", 612.0, 1008.0), ("Tabloid", 792.0, 1224.0)];

/// Product name shown in the title bar and dialogs.
/// Designer credit (Cargo.toml `authors`), shown on the About pages, the start screen and in the
/// executable's file properties.
pub const DESIGNER: &str = env!("CARGO_PKG_AUTHORS");
/// The maker credit shown in About, on the start screen and in the exe's copyright.
pub const CREDIT: &str = concat!("Neramit Planner Made by ", env!("CARGO_PKG_AUTHORS"));

pub fn product() -> &'static str {
    "Neramit Planner"
}

/// What Format Painter carries: the active cell's format, or a whole row's (cells and bar).
#[derive(Clone, Debug)]
pub struct FormatPaint {
    pub whole_row: bool,
    pub cell: crate::format::CellFormat,
    pub row: std::collections::HashMap<String, crate::format::CellFormat>,
    pub bar: Option<crate::format::BarAppearance>,
    pub bar_text: Option<[crate::format::BarText; 5]>,
}

/// What each window of a project shows (View > New Window): its view, table, filter, group,
/// sort, selection, scroll, zoom and panes.
macro_rules! window_state {
    ($($f:ident: $t:ty),* $(,)?) => {
        #[derive(Clone)]
        pub struct WinState {
            $(pub $f: $t,)*
        }

        impl App {
            /// The active window's state out into `st`, `st`'s in.
            pub fn swap_window_state(&mut self, st: &mut WinState) {
                $(std::mem::swap(&mut self.$f, &mut st.$f);)*
            }

            pub fn window_state(&self) -> WinState {
                WinState { $($f: self.$f.clone(),)* }
            }
        }
    };
}

window_state! {
    view: View,
    tab: RibbonTab,
    task_cols: Vec<(TCol, f32)>,
    table: u8,
    filter: FilterSpec,
    highlight: FilterSpec,
    related_summary: bool,
    sort: SortKey,
    group: TaskGroup,
    split_mode: bool,
    task_path: u8,
    report: crate::reports::ReportKind,
    board_scroll: Vec<f32>,
    board_drag: Option<usize>,
    group_bands: Vec<(usize, String)>,
    usage_sel: usize,
    usage_collapsed: std::collections::HashSet<(bool, u32)>,
    nd_zoom: f32,
    nd_scroll: eframe::egui::Vec2,
    cal_month: NaiveDate,
    res_cols: Vec<(RCol, f32)>,
    sel_row: usize,
    sel_anchor: usize,
    sel_col: usize,
    sel_col_anchor: Option<usize>,
    sel_whole_rows: bool,
    res_sel_row: usize,
    res_sel_col: usize,
    editing: Option<EditState>,
    scroll_y: f32,
    scroll_x: f32,
    px_per_day: f32,
    usage_scale: bool,
    other_scale: Option<(f32, f32)>,
    chart_view: (f32, f32),
    grid_width: f32,
    show_timeline: bool,
    drawing_sel: Option<usize>,
    show_details: bool,
    details: Option<crate::taskinfo::TaskInfoState>,
    details_rev: usize,
    row_tops: Vec<f32>,
    wrap_cols: Vec<TCol>,
    col_align: Vec<(TCol, u8)>,
    col_title_align: Vec<(TCol, u8)>,
    col_titles: Vec<(TCol, String)>,
    id_w: f32,
    bar_drag: Option<BarDrag>,
    scroll_to_row: Option<usize>,
    grid_has_focus: bool,
}

/// What a linked file has now for an external task: (name, start, finish, % complete, duration
/// in minutes, ID).
pub type ExtNow = (String, Option<chrono::NaiveDateTime>, Option<chrono::NaiveDateTime>, u8, i64, u32);

/// A link to a task of another project (Project > Links Between Projects).
#[derive(Clone, Debug)]
pub struct ExtLinkRow {
    pub ghost: u32,
    pub local: u32,
    /// the external task is the predecessor (else the successor)
    pub predecessor: bool,
    pub kind: LinkType,
    pub now: Option<ExtNow>,
    pub changed: bool,
}

/// One more window onto the project (View > New Window).
pub struct DocWindow {
    pub num: u32,
    pub hidden: bool,
    pub state: WinState,
}

pub struct App {
    /// View > New Window: the project's other windows (this one's state is in the fields below),
    /// numbered as MPP numbers them ("Project1:2")
    pub windows: Vec<DocWindow>,
    pub win_num: u32,
    pub win_next: u32,
    /// View > Hide hid this window
    pub win_hidden: bool,
    /// View > Arrange All: the windows tiled (else the active one fills the frame)
    pub arranged: bool,
    pub doc: Project,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub(crate) undo: crate::undo::UndoStack,
    redo: crate::undo::UndoStack,
    pub view: View,
    pub tab: RibbonTab,
    pub backstage: Option<Backstage>,
    pub ribbon_collapsed: bool,
    pub task_cols: Vec<(TCol, f32)>,
    /// View > Tables: 0 Entry, 1 Schedule, 2 Summary, 3 Tracking, 4 Variance
    pub table: u8,
    /// View > Filter / Highlight / Sort
    pub filter: FilterSpec,
    pub highlight: FilterSpec,
    pub related_summary: bool,
    pub sort: SortKey,
    pub group: TaskGroup,
    /// Task > Split Task: the next click on a bar splits it
    pub split_mode: bool,
    /// Task > Format Painter: the formatting picked up, waiting for the cells to paint
    pub format_painter: Option<FormatPaint>,
    /// Format > Task Path: 1 predecessors, 2 driving predecessors, 4 successors, 8 driven successors
    pub task_path: u8,
    /// Report tab: the report shown in View::Report, and the ones opened recently
    pub report: crate::reports::ReportKind,
    pub recent_reports: Vec<crate::reports::ReportKind>,
    /// Task Board: each column's scroll, the card being dragged
    pub board_scroll: Vec<f32>,
    pub board_drag: Option<usize>,
    /// Report > Compare Projects: the last comparison
    pub comparison: Option<crate::compare::Comparison>,
    /// header bands of the groups: (first row of the group, label)
    pub group_bands: Vec<(usize, String)>,
    /// Usage views: selected row, collapsed rows ((resource view, uid); uid 0 = Unassigned)
    pub usage_sel: usize,
    pub usage_collapsed: std::collections::HashSet<(bool, u32)>,
    /// Network Diagram zoom and scroll
    pub print: PrintSettings,
    /// the .mpp the project was opened from (path, bytes): Save As .mpp writes a copy of it
    pub mpp_source: Option<(PathBuf, Vec<u8>)>,
    pub nd_zoom: f32,
    pub nd_scroll: eframe::egui::Vec2,
    /// Calendar view: the month shown (its first day)
    pub cal_month: NaiveDate,
    pub res_cols: Vec<(RCol, f32)>,
    pub sel_row: usize,
    pub sel_anchor: usize,
    pub sel_col: usize,
    /// first column of a dragged cell range (with `sel_col` the other end), or whole rows
    /// (None: only the active column; set when a cell range is dragged or Shift-extended)
    pub sel_col_anchor: Option<usize>,
    pub sel_whole_rows: bool,
    /// frame counter: one Undo step per frame, however many parts of a command ask for one
    pub frame: u64,
    cp_frame: Option<u64>,
    pub res_sel_row: usize,
    pub res_sel_col: usize,
    pub editing: Option<EditState>,
    pub scroll_y: f32,
    pub scroll_x: f32,
    pub px_per_day: f32,
    /// the Usage views keep their own zoom and scroll, like the standard per-view timescale:
    /// true while `px_per_day` / `scroll_x` hold the Usage views' values
    pub usage_scale: bool,
    /// the other family's (px_per_day, scroll_x): the Gantt's while a Usage view shows, and back
    pub other_scale: Option<(f32, f32)>,
    /// screen x and width of the Gantt chart pane (zoom anchor; updated every frame)
    pub chart_view: (f32, f32),
    pub grid_width: f32,
    pub show_timeline: bool,
    /// View > Macros: the macros recorded in this session (name, commands) and the one being
    /// recorded
    pub macros: Vec<(String, Vec<crate::ribbon::Cmd>)>,
    pub recording: Option<(String, Vec<crate::ribbon::Cmd>)>,
    /// Format > Drawing: the shape being drawn next, the selected shape, where a drag started
    pub drawing_tool: Option<u8>,
    pub drawing_sel: Option<usize>,
    pub drawing_from: Option<eframe::egui::Pos2>,
    /// View > Details: the Task Form under the chart, and its fields being edited
    pub show_details: bool,
    pub details: Option<crate::taskinfo::TaskInfoState>,
    /// (undo depth the form was filled at: a change elsewhere fills it again)
    pub details_rev: usize,
    pub show_outline_number: bool,
    pub show_summary_tasks: bool,
    /// user (application) settings: date format, Office look, user name ...
    pub settings: UserSettings,
    /// regional settings in effect (Windows locale, calendar override from settings)
    pub loc: crate::locale::Locale,
    /// top y of every visible sheet row (plus one trailing entry = total height), rebuilt each frame
    pub row_tops: Vec<f32>,
    /// columns with "Wrap Text" on (MPP wraps the Name column by default)
    pub wrap_cols: Vec<TCol>,
    /// wrapped cells' line counts by (text, width, font): measuring Thai-aware line breaks for
    /// every row each frame is slow on large projects
    /// wrapped cell -> line count, by a hash of (text, width, font size, font family)
    pub wrap_cache: HashMap<u64, usize>,
    /// task UID -> ID, kept with a checksum of the task list (the Predecessors column needs it
    /// for every row it draws)
    uid_ids: std::cell::RefCell<(u64, std::rc::Rc<HashMap<u32, usize>>)>,
    /// per-column data alignment overrides (0 left, 1 center, 2 right)
    pub col_align: Vec<(TCol, u8)>,
    /// per-column title alignment overrides
    pub col_title_align: Vec<(TCol, u8)>,
    /// column title overrides (Field Settings > Title)
    pub col_titles: Vec<(TCol, String)>,
    /// title bar rectangle of the last frame (window controls are drawn on a top layer)
    pub title_rect: egui::Rect,
    /// view saved in the opened .mpp, applied over two frames (load its fonts, then apply)
    pending_view: Option<Box<crate::mpp::SavedView>>,
    pending_view_fonts_loaded: bool,
    /// width of the sheet's ID column (taken from the .mpp file's table)
    pub id_w: f32,
    /// `--screenshot` QA output path and frames rendered so far
    pub qa_screenshot: Option<(PathBuf, u32)>,
    /// Copy Picture / Save Picture in progress
    pub picture: crate::picture::PictureState,
    /// where the view (sheet, chart, report ...) was drawn last frame
    pub view_rect: egui::Rect,
    pub dialog: Option<Dialog>,
    pub clipboard: Vec<Task>,
    pub status: String,
    pub message: Option<(String, String)>,
    pub pending: Option<PendingAction>,
    pub confirm_close_ok: bool,
    pub bar_drag: Option<BarDrag>,
    pub scroll_to_row: Option<usize>,
    pub find_text: String,
    pub recent: Vec<PathBuf>,
    pub favorites: Vec<PathBuf>,
    /// Home page list: false = Recent, true = Favorites
    pub home_favorites: bool,
    pub home_search: String,
    pub grid_has_focus: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, open: Option<PathBuf>) -> Self {
        // the UI language is global: chosen here, for the window (tests build apps without it)
        let lang = UserSettings::load().language;
        crate::i18n::set_lang(if lang == 1 { crate::i18n::Lang::Thai } else { crate::i18n::Lang::English });
        let mut app = Self::with_ctx(&cc.egui_ctx, open);
        if app.path.is_none() && !app.dirty {
            app.doc.localize_new();
        }
        app
    }

    /// The app on an egui context (the window's, or a headless one in tests).
    pub fn with_ctx(ctx: &egui::Context, open: Option<PathBuf>) -> Self {
        crate::theme::install_fonts(ctx);
        // list the PC's fonts for the Font box while the window opens
        crate::sysfonts::warm_up();
        // Ctrl +/- zoom the timescale (as in MPP), not the whole UI
        ctx.options_mut(|o| o.zoom_with_keyboard = false);
        let recent = load_list("recent.txt");
        let favorites = load_list("favorites.txt");
        let settings = UserSettings::load();
        let loc = settings.locale();
        let mut app = App {
            doc: Project::new(),
            path: None,
            dirty: false,
            undo: Default::default(),
            redo: Default::default(),
            view: View::Gantt,
            tab: RibbonTab::Task,
            backstage: None,
            ribbon_collapsed: false,
            task_cols: [TCol::Indicators, TCol::Mode, TCol::Name, TCol::Duration, TCol::Start, TCol::Finish, TCol::Predecessors, TCol::Resources]
                .into_iter()
                .map(|c| (c, c.default_width()))
                .collect(),
            table: 0,
            filter: FilterSpec::default(),
            highlight: FilterSpec::default(),
            related_summary: false,
            sort: SortKey::Id,
            group: TaskGroup::None,
            split_mode: false,
            format_painter: None,
            task_path: 0,
            report: crate::reports::ReportKind::ProjectOverview,
            recent_reports: Vec::new(),
            board_scroll: vec![0.0; 4],
            board_drag: None,
            comparison: None,
            group_bands: Vec::new(),
            usage_sel: 0,
            usage_collapsed: std::collections::HashSet::new(),
            print: PrintSettings::default(),
            mpp_source: None,
            nd_zoom: 1.0,
            nd_scroll: eframe::egui::Vec2::ZERO,
            cal_month: chrono::Local::now().date_naive(),
            res_cols: RCol::DEFAULT.iter().map(|c| (*c, c.default_width())).collect(),
            sel_row: 0,
            sel_anchor: 0,
            sel_col: 2,
            res_sel_row: 0,
            res_sel_col: 0,
            editing: None,
            scroll_y: 0.0,
            scroll_x: 0.0,
            px_per_day: 24.0,
            usage_scale: false,
            other_scale: None,
            chart_view: (0.0, 800.0),
            grid_width: 620.0,
            show_timeline: true,
            show_details: false,
            drawing_tool: None,
            macros: load_macros(),
            windows: Vec::new(),
            win_num: 1,
            win_next: 2,
            win_hidden: false,
            arranged: false,
            recording: None,
            drawing_sel: None,
            drawing_from: None,
            details: None,
            details_rev: 0,
            show_outline_number: false,
            show_summary_tasks: true,
            settings,
            loc,
            row_tops: Vec::new(),
            wrap_cols: vec![TCol::Name],
            wrap_cache: HashMap::new(),
            uid_ids: Default::default(),
            col_align: Vec::new(),
            col_title_align: Vec::new(),
            col_titles: Vec::new(),
            title_rect: egui::Rect::NOTHING,
            pending_view: None,
            pending_view_fonts_loaded: false,
            id_w: crate::grid::ID_W,
            sel_col_anchor: None,
            sel_whole_rows: false,
            frame: 0,
            cp_frame: None,
            qa_screenshot: None,
            picture: Default::default(),
            view_rect: egui::Rect::NOTHING,
            dialog: None,
            clipboard: Vec::new(),
            status: "Ready".into(),
            message: None,
            pending: None,
            confirm_close_ok: false,
            bar_drag: None,
            scroll_to_row: None,
            find_text: String::new(),
            recent,
            favorites,
            home_favorites: false,
            home_search: String::new(),
            grid_has_focus: true,
        };
        match open {
            Some(p) => app.open_path(p),
            // like Project, start on the Home screen when no file was given
            None => app.backstage = Some(Backstage::Home),
        }
        app
    }

    // ------------------------------------------------------------ helpers

    /// File name shown in the title bar ("Project1" for a new project).
    pub fn doc_name(&self) -> String {
        self.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Project1".into())
    }

    /// MPP12 (and older) .mpp files open in Compatibility Mode, as in MPP.
    pub fn compat_mode(&self) -> bool {
        matches!(self.doc.file_format, Some(v) if v < 14)
    }

    pub fn title(&self) -> String {
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Project1".into());
        // MPP12 (and older) .mpp files open in Compatibility Mode, as in MPP
        let compat = if matches!(self.doc.file_format, Some(v) if v < 14) { " [Compatibility Mode]" } else { "" };
        let num = if self.windows.is_empty() { String::new() } else { format!(":{}", self.win_num) };
        format!("{}{}{}{} - Neramit Planner", name, num, if self.dirty { " *" } else { "" }, compat)
    }

    /// A window's caption: "Plan.mpp:2" while the project has more than one window.
    pub fn window_caption(&self, num: u32) -> String {
        if self.windows.is_empty() {
            self.doc_name()
        } else {
            format!("{}:{}", self.doc_name(), num)
        }
    }

    /// Every window of the project: (number, hidden), in number order.
    pub fn window_list(&self) -> Vec<(u32, bool)> {
        let mut v: Vec<(u32, bool)> = self.windows.iter().map(|w| (w.num, w.hidden)).collect();
        v.push((self.win_num, self.win_hidden));
        v.sort_unstable();
        v
    }

    /// View > New Window: another window onto the project showing `view`; it becomes the active one.
    pub fn new_window(&mut self, view: View) {
        self.commit_edit();
        let mut st = self.window_state();
        st.view = view;
        st.editing = None;
        st.bar_drag = None;
        st.details = None;
        st.show_details = false;
        let num = self.win_next;
        self.win_next += 1;
        self.windows.push(DocWindow { num, hidden: false, state: st });
        self.switch_window(num);
        self.backstage = None;
    }

    /// Window > Switch Windows: make window `num` the active one (shown, if it was hidden).
    pub fn switch_window(&mut self, num: u32) {
        if num == self.win_num {
            self.win_hidden = false;
            return;
        }
        let Some(k) = self.windows.iter().position(|w| w.num == num) else { return };
        self.commit_edit();
        self.editing = None;
        self.bar_drag = None;
        let mut w = self.windows.remove(k);
        self.swap_window_state(&mut w.state);
        std::mem::swap(&mut w.num, &mut self.win_num);
        std::mem::swap(&mut w.hidden, &mut self.win_hidden);
        self.win_hidden = false;
        self.windows.push(w);
        self.windows.sort_by_key(|w| w.num);
        self.clamp_window_selection();
    }

    /// The doc may have changed while a window was not active: its selection stays inside.
    fn clamp_window_selection(&mut self) {
        let rows = self.visible_rows().len();
        self.sel_row = self.sel_row.min(rows.saturating_sub(1));
        self.sel_anchor = self.sel_anchor.min(rows.saturating_sub(1));
        self.res_sel_row = self.res_sel_row.min(self.doc.resources.len().saturating_sub(1));
    }

    /// View > Hide: this window goes out of sight (View > Unhide brings it back); another one
    /// becomes active.
    pub fn hide_window(&mut self) {
        self.commit_edit();
        self.win_hidden = true;
        if let Some(n) = self.windows.iter().find(|w| !w.hidden).map(|w| w.num) {
            let me = self.win_num;
            self.switch_window(n);
            if let Some(w) = self.windows.iter_mut().find(|w| w.num == me) {
                w.hidden = true;
            }
        }
    }

    /// Close the active window of a project that has others (MPP keeps the project open).
    pub fn close_window(&mut self) {
        let Some(n) = self.windows.iter().find(|w| !w.hidden).or(self.windows.first()).map(|w| w.num) else { return };
        let me = self.win_num;
        self.switch_window(n);
        self.windows.retain(|w| w.num != me);
        if self.windows.len() <= 1 && self.windows.iter().all(|w| w.hidden) {
            self.arranged = self.arranged && !self.windows.is_empty();
        }
    }

    // ------------------------------------------------------------ dates (regional settings)

    /// A date in the Options > General date format, with the project's default start time.
    pub fn fmt_date(&self, d: NaiveDate) -> String {
        crate::locale::format(self.settings.date_format, d, self.doc.options.default_start_time, &self.loc)
    }

    /// A finish date (end of the working day).
    pub fn fmt_finish(&self, d: NaiveDate) -> String {
        crate::locale::format(self.settings.date_format, d, self.doc.options.default_end_time, &self.loc)
    }

    /// Dates shown next to Gantt bars (Layout > Date format).
    pub fn fmt_bar_date(&self, d: NaiveDate) -> String {
        let f = self.doc.formatting.layout.date_format;
        let f = if f == 255 { self.settings.date_format } else { f };
        crate::locale::format(f, d, self.doc.options.default_start_time, &self.loc)
    }

    pub fn parse_date(&self, s: &str) -> Option<NaiveDate> {
        crate::locale::parse(s, &self.loc)
    }

    /// The project's duration conversions (Options > Schedule).
    pub fn dur_units(&self) -> util::DurUnits {
        util::DurUnits::of(&self.doc)
    }

    /// Task::manual_text's slot of a column (Start, Finish, Duration).
    pub fn text_slot(col: TCol) -> usize {
        match col {
            TCol::Start => 0,
            TCol::Finish => 1,
            _ => 2,
        }
    }

    /// A task's duration as MPP shows it: its own unit ("0.5 days", "4 hrs", "3 edays") and the
    /// "?" of estimates.
    pub fn task_duration_text(&self, t: &Task) -> String {
        let du = self.dur_units();
        let (unit, el) = if t.is_summary { (DurUnit::Days, false) } else { (t.dur_unit, t.dur_elapsed) };
        let mut s = util::fmt_dur(t.dur_minutes(du.mpd), unit, el, du, &self.doc.options, crate::i18n::is_thai());
        if t.estimated && self.doc.options.show_estimated {
            s.push('?');
        }
        s
    }

    pub fn apply_settings(&mut self) {
        crate::i18n::set_lang(if self.settings.language == 1 { crate::i18n::Lang::Thai } else { crate::i18n::Lang::English });
        self.loc = self.settings.locale();
        self.settings.save();
    }

    // ------------------------------------------------------------ row layout

    /// Top of a sheet row relative to the body top (rows past the tasks are one line high).
    pub fn row_top(&self, row: usize) -> f32 {
        let n = self.row_tops.len().saturating_sub(1);
        if row < self.row_tops.len() {
            self.row_tops[row]
        } else {
            self.row_tops.last().copied().unwrap_or(0.0) + (row - n) as f32 * crate::grid::ROW_H
        }
    }

    pub fn row_height(&self, row: usize) -> f32 {
        // a group's header band sits above its first row, outside the row before it
        let band = if self.group_bands.binary_search_by_key(&(row + 1), |b| b.0).is_ok() && row + 1 < self.row_tops.len() { GROUP_H } else { 0.0 };
        self.row_top(row + 1) - self.row_top(row) - band
    }

    /// The group of a task: (order key, header label).
    pub fn group_of(&self, i: usize) -> (i64, String) {
        let t = &self.doc.tasks[i];
        let yes_no = |b: bool| crate::i18n::tr(if b { "Yes" } else { "No" }).to_string();
        let tr = |s: &str| crate::i18n::tr(s).to_string();
        match self.group {
            TaskGroup::None => (0, String::new()),
            TaskGroup::ActiveInactive => (t.active as i64, format!("{}: {}", tr("Active"), yes_no(t.active))),
            TaskGroup::AutoManual => (t.manual as i64, format!("{}: {}", tr("Task Mode"), tr(if t.manual { "Manually Scheduled" } else { "Auto Scheduled" }))),
            TaskGroup::Completion => {
                let k = if t.percent_complete == 0 { 0 } else if t.percent_complete < 100 { 1 } else { 2 };
                (k, format!("{}: {}", tr("% Complete"), ["0%", "1% - 99%", "100%"][k as usize]))
            }
            TaskGroup::Critical => (t.critical as i64, format!("{}: {}", tr("Critical"), yes_no(t.critical))),
            TaskGroup::Duration => (t.dur_min, format!("{}: {}", tr("Duration"), self.task_duration_text(t))),
            TaskGroup::Milestones => (t.is_milestone() as i64, format!("{}: {}", tr("Milestone"), yes_no(t.is_milestone()))),
            TaskGroup::Priority => (t.priority as i64, format!("{}: {}", tr("Priority"), t.priority)),
            TaskGroup::Status => {
                let s = self.status_text(i);
                let k = ["Complete", "On Schedule", "Late", "Future Task"].iter().position(|x| *x == s).unwrap_or(0) as i64;
                (k, format!("{}: {}", tr("Status"), tr(s)))
            }
        }
    }

    /// Rolled-up values of the rows of the group starting at row `r`: start, finish, work, cost.
    pub fn group_totals(&self, band: usize) -> Option<(chrono::NaiveDateTime, chrono::NaiveDateTime, i64, f64)> {
        let vis = self.visible_rows();
        let (r, _) = self.group_bands.get(band)?;
        let end = self.group_bands.get(band + 1).map(|b| b.0).unwrap_or(vis.len());
        let rows: Vec<usize> = vis.get(*r..end.min(vis.len()))?.to_vec();
        let st = rows.iter().filter_map(|&i| self.doc.tasks[i].start_dt).min()?;
        let fi = rows.iter().filter_map(|&i| self.doc.tasks[i].finish_dt).max()?;
        let work = rows.iter().map(|&i| self.doc.tasks[i].work_min).sum();
        let cost = rows.iter().map(|&i| self.doc.tasks[i].cost).sum();
        Some((st, fi, work, cost))
    }

    /// Row under a y offset from the body top.
    pub fn row_at(&self, y: f32) -> usize {
        let y = y.max(0.0);
        let n = self.row_tops.len().saturating_sub(1);
        if n > 0 && y < self.row_tops[n] {
            match self.row_tops.binary_search_by(|t| t.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal)) {
                Ok(i) => i.min(n - 1),
                Err(i) => i.saturating_sub(1).min(n - 1),
            }
        } else {
            n + ((y - self.row_tops.last().copied().unwrap_or(0.0)) / crate::grid::ROW_H).floor() as usize
        }
    }

    /// Alignment of a column's data (0 left, 1 center, 2 right).
    pub fn col_alignment(&self, c: TCol) -> u8 {
        self.col_align.iter().find(|(k, _)| *k == c).map(|(_, a)| *a).unwrap_or(c.default_align())
    }

    pub fn col_title(&self, c: TCol) -> String {
        if let (TCol::Custom(f), None) = (c, self.col_titles.iter().find(|(k, _)| *k == c)) {
            return crate::thai::shape(&crate::custom::title(&self.doc, f, false)).to_string();
        }
        self.col_titles.iter().find(|(k, _)| *k == c).map(|(_, t)| crate::i18n::tr(t).to_string()).unwrap_or_else(|| crate::i18n::tr(c.title()).to_string())
    }

    // ------------------------------------------------------------ cell formatting (Font group)

    /// Column key used for cell formats of the current column.
    pub fn current_col_key(&self) -> String {
        self.task_cols.get(self.sel_col).map(|c| c.0.title().to_string()).unwrap_or_else(|| "*".into())
    }

    /// Effective format of the active cell (drives the Font group state).
    pub fn selected_format(&self) -> crate::format::CellFormat {
        match self.current_task() {
            Some(i) => self.doc.tasks[i].cell_format(&self.current_col_key()),
            None => crate::format::CellFormat::default(),
        }
    }

    /// Text style of the active cell including the row's text-style item (font name / size shown in the ribbon).
    pub fn selected_text_style(&self) -> crate::format::TextStyle {
        let base = match self.current_task() {
            Some(i) => self.doc.formatting.text_style(crate::grid::text_item_of(&self.doc, i)),
            None => self.doc.formatting.text_style(crate::format::TextItem::All),
        };
        base.merged(&self.selected_format())
    }

    /// Change the cell format of the selected cells (current column of every selected row).
    pub fn format_selected_cells(&mut self, f: impl Fn(&mut crate::format::CellFormat)) {
        let sel = self.selected_tasks();
        if sel.is_empty() {
            return;
        }
        self.commit_edit();
        self.checkpoint();
        let key = self.current_col_key();
        for i in sel {
            self.doc.tasks[i].edit_cell_format(&key, &f);
        }
    }

    /// Toggle bold / italic / underline on the selected cells (state taken from the active cell).
    pub fn toggle_cell_attr(&mut self, which: u8) {
        let cur = self.selected_format();
        let on = match which {
            0 => !cur.bold,
            1 => !cur.italic,
            _ => !cur.underline,
        };
        self.format_selected_cells(move |c| match which {
            0 => c.bold = on,
            1 => c.italic = on,
            _ => c.underline = on,
        });
        // the legacy row flags must not fight the cell format
        for i in self.selected_tasks() {
            let t = &mut self.doc.tasks[i];
            match which {
                0 => t.bold = false,
                1 => t.italic = false,
                _ => t.underline = false,
            }
        }
    }

    /// The project summary task (Format > Project Summary Task), named as MPP names it: the
    /// title in Project Information, else the file name.
    pub fn project_summary(&self) -> Task {
        let name = if !self.doc.title.trim().is_empty() {
            self.doc.title.clone()
        } else {
            self.path.as_ref().and_then(|p| p.file_stem()).map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Project1".into())
        };
        self.doc.project_summary_task(&name)
    }

    /// Is the project summary task row shown above the tasks (ungrouped Gantt / task sheets)?
    pub fn shows_project_summary(&self) -> bool {
        self.doc.show_project_summary && self.group == TaskGroup::None && !self.doc.tasks.is_empty()
    }

    /// Task indices that are visible (not hidden under collapsed summaries).
    pub fn visible_rows(&self) -> Vec<usize> {
        let rows = self.filtered_rows();
        if self.group == TaskGroup::None {
            return rows;
        }
        let mut rows: Vec<usize> = rows.into_iter().filter(|&i| !self.doc.tasks[i].is_summary).collect();
        rows.sort_by_key(|&i| self.group_of(i).0);
        rows
    }

    /// Rows after the filter, in sort order (before grouping).
    fn filtered_rows(&self) -> Vec<usize> {
        let n = self.doc.tasks.len();
        let parents = self.doc.parents();
        let order = self.sorted_order_with(&parents);
        if self.filter.kind == TaskFilter::All {
            let hidden = self.doc.hidden_rows(&parents);
            return order.into_iter().filter(|&i| !hidden[i] && (self.show_summary_tasks || !self.doc.tasks[i].is_summary)).collect();
        }
        // MPP: the tasks the filter selects (with their summaries when Show Related Summary
        // Rows is on); a collapsed summary still hides its subtasks
        let mut keep = vec![false; n];
        for i in 0..n {
            if self.task_matches(i, &self.filter) {
                keep[i] = true;
                if self.related_summary {
                    let mut a = parents[i];
                    while let Some(p) = a {
                        if keep[p] {
                            break;
                        }
                        keep[p] = true;
                        a = parents[p];
                    }
                }
            }
        }
        // a kept, collapsed summary hides its subtasks (parents come first, so one pass)
        let mut hidden = vec![false; n];
        for i in 0..n {
            if let Some(p) = parents[i] {
                hidden[i] = hidden[p] || (keep[p] && self.doc.tasks[p].collapsed);
            }
        }
        let hidden_by = |i: usize| hidden[i];
        order.into_iter().filter(|&i| keep[i] && !hidden_by(i) && (self.show_summary_tasks || !self.doc.tasks[i].is_summary)).collect()
    }

    /// Task indices in display order: ID order, or sorted with the outline structure kept
    /// (each summary's subtasks sorted among themselves), as MPP sorts by default.
    pub fn sorted_order(&self) -> Vec<usize> {
        self.sorted_order_with(&self.doc.parents())
    }

    fn sorted_order_with(&self, parents: &[Option<usize>]) -> Vec<usize> {
        let n = self.doc.tasks.len();
        if self.sort == SortKey::Id {
            return (0..n).collect();
        }
        let key = |i: usize| -> (i64, f64, Vec<u8>) {
            let t = &self.doc.tasks[i];
            let ts = |d: Option<chrono::NaiveDateTime>| d.map(|x| x.and_utc().timestamp()).unwrap_or(0);
            match self.sort {
                SortKey::Start => (ts(t.start_dt), 0.0, Vec::new()),
                SortKey::Finish => (ts(t.finish_dt), 0.0, Vec::new()),
                // MPP lists the highest priority and the highest cost first
                SortKey::Priority => (-(t.priority as i64), 0.0, Vec::new()),
                SortKey::Cost => (0, -t.cost, Vec::new()),
                SortKey::Duration => (t.dur_min, 0.0, Vec::new()),
                // Thai dictionary order: เ แ โ ใ ไ sort by the consonant after them
                SortKey::Name => (0, 0.0, crate::thai::sort_key(&t.name)),
                SortKey::Id => (i as i64, 0.0, Vec::new()),
            }
        };
        let keys: Vec<_> = (0..n).map(key).collect();
        // children of each parent (None = top level), sorted
        let mut kids: std::collections::HashMap<Option<usize>, Vec<usize>> = std::collections::HashMap::new();
        for i in 0..n {
            kids.entry(parents[i]).or_default().push(i);
        }
        for v in kids.values_mut() {
            v.sort_by(|&a, &b| keys[a].partial_cmp(&keys[b]).unwrap_or(std::cmp::Ordering::Equal).then(a.cmp(&b)));
        }
        let mut out = Vec::with_capacity(n);
        let mut stack: Vec<usize> = kids.get(&None).cloned().unwrap_or_default().into_iter().rev().collect();
        while let Some(i) = stack.pop() {
            out.push(i);
            if let Some(c) = kids.get(&Some(i)) {
                stack.extend(c.iter().rev());
            }
        }
        out
    }

    /// Does task `i` pass a filter?
    pub fn task_matches(&self, i: usize, f: &FilterSpec) -> bool {
        let t = &self.doc.tasks[i];
        match f.kind {
            TaskFilter::All => true,
            TaskFilter::Active => t.active,
            TaskFilter::Completed => t.percent_complete == 100,
            TaskFilter::Critical => t.critical,
            TaskFilter::Incomplete => t.percent_complete < 100,
            TaskFilter::Late => self.status_text(i) == "Late",
            TaskFilter::Manual => t.manual,
            TaskFilter::Milestones => t.is_milestone(),
            TaskFilter::Summary => t.is_summary,
            TaskFilter::Estimated => t.estimated,
            TaskFilter::Slipping => t.percent_complete < 100 && t.baseline(0).and_then(|b| b.finish).zip(t.finish_dt).map(|(b, f)| f > b).unwrap_or(false),
            TaskFilter::DateRange => f.range.map(|(a, b)| t.start <= b && t.finish >= a).unwrap_or(true),
            TaskFilter::UsingResource => f.resource.map(|r| t.resources.contains(&r)).unwrap_or(true),
        }
    }

    /// View > Highlight: the task is shown highlighted.
    pub fn is_highlighted(&self, i: usize) -> bool {
        self.highlight.kind != TaskFilter::All && self.task_matches(i, &self.highlight)
    }

    /// Apply a filter or highlight (the ones that ask for input open their dialog first).
    pub fn set_filter(&mut self, kind: TaskFilter, highlight: bool) {
        if kind.needs_input() {
            let today = chrono::Local::now().date_naive();
            self.dialog = Some(crate::dialogs::Dialog::FilterInput(crate::dialogs::FilterInputState {
                highlight,
                kind,
                from: self.fmt_date(self.doc.start),
                to: self.fmt_date(self.doc.finish_date().max(today)),
                resource: self.doc.resources.first().map(|r| r.uid).unwrap_or(0),
                error: None,
            }));
            return;
        }
        let spec = FilterSpec { kind, range: None, resource: None };
        if highlight {
            self.highlight = spec;
        } else {
            self.filter = spec;
            self.sel_row = 0;
            self.sel_anchor = 0;
            self.scroll_y = 0.0;
        }
    }

    /// Selected task indices (contiguous visible range).
    pub fn selected_tasks(&self) -> Vec<usize> {
        let vis = self.visible_rows();
        let (a, b) = (self.sel_anchor.min(self.sel_row), self.sel_anchor.max(self.sel_row));
        (a..=b).filter_map(|r| vis.get(r).copied()).collect()
    }

    // ------------------------------------------------------------ the .mpp file's saved view

    /// Apply the view saved in an opened .mpp (call at the start of a frame, before drawing).
    pub fn apply_pending_view(&mut self, ctx: &egui::Context) {
        if self.pending_view.is_none() {
            return;
        }
        if !self.pending_view_fonts_loaded {
            // fonts registered now become usable from the next frame
            for f in self.pending_view.as_ref().map(|v| v.fonts.clone()).unwrap_or_default() {
                crate::theme::ensure_font(ctx, &f);
            }
            self.pending_view_fonts_loaded = true;
            ctx.request_repaint();
            return;
        }
        let Some(v) = self.pending_view.take() else { return };
        let v = *v;
        // the file's view sets the Gantt's timescale (and may bring new fonts)
        self.wrap_cache.clear();
        self.usage_scale = false;
        self.other_scale = None;
        let font_ok = |name: &str| crate::theme::ensure_font(ctx, name);
        let fix = |name: &mut String| {
            if !font_ok(name) {
                *name = crate::format::DEFAULT_FONT.to_string();
            }
        };
        let f = &mut self.doc.formatting;
        if !v.bar_styles.is_empty() {
            if v.name.contains("Tracking Gantt") {
                f.tracking_bar_styles = v.bar_styles;
                self.view = View::TrackingGantt;
            } else {
                f.show_critical = v.bar_styles.iter().any(|b| b.show_for.iter().any(|s| s.cat == crate::format::Category::Critical && !s.not));
                f.bar_styles = v.bar_styles;
            }
        }
        for (item, mut st) in v.text_styles {
            fix(&mut st.font);
            if item == crate::format::TextItem::Noncritical {
                // items the file does not list use the task font, as in MPP
                f.set_text_style(crate::format::TextItem::All, st.clone());
            }
            f.set_text_style(item, st);
        }
        for (kind, lt, c) in v.gridlines {
            let mut g = f.gridline(kind);
            g.normal = lt;
            g.normal_color = c;
            f.set_gridline(g);
        }
        if let Some((mu, mc, ml, bu, bc, bl)) = v.tiers {
            use crate::format::{Tier, TsUnit};
            // the standard label codes seen in files -> our label lists; otherwise MPP-like defaults
            let label = |u: TsUnit, code: u16, middle: bool, count: u8| -> u8 {
                match (u, code) {
                    (TsUnit::HalfYears, 124) => 6,
                    (TsUnit::Months, 11) => 2,
                    (TsUnit::Months, 7) => 0,
                    (TsUnit::Days, 21) => 5,
                    _ => match (u, middle) {
                        (TsUnit::HalfYears, true) => 6,
                        (TsUnit::HalfYears, false) => 4,
                        (TsUnit::Quarters, true) => 1,
                        (TsUnit::Quarters, false) => 3,
                        (TsUnit::Months, true) => 0,
                        (TsUnit::Months, false) => 2,
                        (TsUnit::Weeks, true) => 1,
                        (TsUnit::Weeks, false) => 2,
                        (TsUnit::Days, true) => 2,
                        (TsUnit::Days, false) => if count > 1 { 5 } else { 6 },
                        (TsUnit::Hours, _) => 2,
                        _ => 0,
                    },
                }
            };
            f.timescale.auto = false;
            f.timescale.show = 2;
            f.timescale.middle = Tier { units: mu, count: mc, label: label(mu, ml, true, mc), align: 0, tick_lines: true, fiscal_year: false };
            f.timescale.bottom = Tier { units: bu, count: bc, label: label(bu, bl, false, bc), align: 1, tick_lines: true, fiscal_year: false };
            // MPP: a bottom-tier column is 22.4 px at Size 100 % (30 px at the 134 % of the
            // reference file); about 30 px when the file has no size
            let col_px = v.timescale_size.map(|z| 22.4 * z as f32 / 100.0).unwrap_or(30.0);
            let days = match bu {
                TsUnit::Years => 365.25,
                TsUnit::HalfYears => 182.6,
                TsUnit::Quarters => 91.3,
                TsUnit::Months => 30.44,
                TsUnit::ThirdsOfMonths => 10.15,
                TsUnit::Weeks => 7.0,
                TsUnit::Days => 1.0,
                TsUnit::Hours => 1.0 / 24.0,
                TsUnit::Minutes => 1.0 / 1440.0,
            };
            self.px_per_day = (col_px / (days * bc as f32)).clamp(Self::MIN_PPD, Self::MAX_PPD);
            if let Some(first) = self.doc.tasks.iter().map(|t| t.start).min() {
                // MPP opens the chart at the start of the middle tier's unit holding the first task
                let left = crate::gantt::unit_start(first, crate::gantt::ts_unit(mu, self.px_per_day));
                self.scroll_x = ((left - self.gantt_origin()).num_days() as f32).max(0.0) * self.px_per_day;
            }
        }
        for (uid, key, mut cf) in v.cell_formats {
            if let Some(font) = cf.font.as_mut() {
                fix(font);
            }
            if let Some(t) = self.doc.tasks.iter_mut().find(|t| t.uid == uid) {
                t.cell_formats.insert(key, cf);
            }
        }
        if !v.columns.is_empty() {
            self.task_cols.clear();
            self.col_align.clear();
            self.col_title_align.clear();
            self.col_titles.clear();
            for c in v.columns {
                // the standard widths are in characters; 4 px per character + 3
                // (Duration 17 -> 71 px, Start 16 -> 67 px), Task Name 93 -> 515 px
                let w = if c.col == TCol::Name { c.width_chars as f32 * 5.54 } else { c.width_chars as f32 * 4.0 + 3.0 };
                self.task_cols.push((c.col, w.max(24.0).round()));
                self.col_align.push((c.col, c.align));
                self.col_title_align.push((c.col, c.title_align));
                if let Some(t) = c.title.filter(|t| t.as_str() != c.col.title()) {
                    self.col_titles.push((c.col, t));
                }
            }
            self.sel_col = self.task_cols.iter().position(|c| c.0 == TCol::Name).unwrap_or(0);
            self.sel_col_anchor = None;
        }
        // ID column: 4 px per character (6 -> 24 px with its edge)
        if let Some(w) = v.id_width {
            self.id_w = (w as f32 * 4.0).clamp(16.0, 80.0);
        }
        // the divider where MPP left it (its width counts the view bar)
        if let Some(w) = v.sheet_width {
            self.grid_width = w as f32 - crate::grid::VIEW_BAR_W;
        }
        self.show_timeline = v.show_timeline;
    }

    /// Columns of the selected cell range; whole rows span every column.
    pub fn sel_cols(&self) -> (usize, usize) {
        let last = self.task_cols.len().saturating_sub(1);
        if self.sel_whole_rows {
            (0, last)
        } else {
            let a = self.sel_col_anchor.unwrap_or(self.sel_col);
            (a.min(self.sel_col).min(last), a.max(self.sel_col).min(last))
        }
    }

    /// Just one cell selected (no range, no whole rows).
    pub fn single_cell_selected(&self) -> bool {
        self.sel_anchor == self.sel_row && !self.sel_whole_rows && self.sel_col_anchor.is_none_or(|a| a == self.sel_col)
    }

    /// The selected cells as tab-separated text (pastes as a table into Excel, Word, chat ...).
    pub fn selection_text(&self) -> (String, usize) {
        let vis = self.visible_rows();
        let (sa, sb) = (self.sel_anchor.min(self.sel_row), self.sel_anchor.max(self.sel_row));
        let (ca, cb) = self.sel_cols();
        let mut lines = Vec::new();
        let mut n = 0;
        for r in sa..=sb {
            let Some(&i) = vis.get(r) else { continue };
            let cells: Vec<String> = (ca..=cb)
                .filter_map(|c| self.task_cols.get(c))
                .filter(|(col, _)| *col != TCol::Indicators)
                .map(|(col, _)| crate::i18n::tr_raw(&self.cell_text(i, *col)).replace(['\t', '\n'], " "))
                .collect();
            n += cells.len();
            lines.push(cells.join("\t"));
        }
        (lines.join("\r\n"), n)
    }

    pub fn current_task(&self) -> Option<usize> {
        self.visible_rows().get(self.sel_row).copied()
    }

    /// Task UID -> ID (row number), cached while the task list is unchanged.
    pub fn uid_to_id(&self) -> std::rc::Rc<HashMap<u32, usize>> {
        let mut sum = self.doc.tasks.len() as u64;
        for (i, t) in self.doc.tasks.iter().enumerate() {
            sum = sum.wrapping_mul(1_000_003).wrapping_add((t.uid as u64) ^ ((i as u64) << 32));
        }
        let mut c = self.uid_ids.borrow_mut();
        if c.0 != sum || c.1.len() != self.doc.tasks.len() {
            *c = (sum, std::rc::Rc::new(self.doc.tasks.iter().enumerate().map(|(i, t)| (t.uid, i + 1)).collect()));
        }
        c.1.clone()
    }

    pub fn id_maps(&self) -> (HashMap<u32, usize>, HashMap<usize, u32>) {
        let a: HashMap<u32, usize> = self.doc.tasks.iter().enumerate().map(|(i, t)| (t.uid, i + 1)).collect();
        let b: HashMap<usize, u32> = self.doc.tasks.iter().enumerate().map(|(i, t)| (i + 1, t.uid)).collect();
        (a, b)
    }

    /// True if one task is an outline ancestor of the other.
    pub fn is_outline_relative(&self, a: usize, b: usize) -> bool {
        let (lo, hi) = (a.min(b), a.max(b));
        self.doc.tasks[lo].is_summary && hi < self.doc.children_end(lo)
    }

    pub fn checkpoint(&mut self) {
        self.dirty = true;
        // one Undo step per frame, however many parts of a command ask for one
        if self.cp_frame == Some(self.frame) {
            return;
        }
        self.cp_frame = Some(self.frame);
        self.undo.push(self.doc.clone());
        // MPP keeps 20 levels by default (File > Options > Advanced > Undo levels)
        let keep = (self.settings.undo_levels as usize).max(1);
        self.undo.keep_last(keep);
        self.redo.clear();
    }

    /// Forget the last Undo step (a change that was given up and put back).
    pub fn forget_checkpoint(&mut self) {
        self.undo.discard_last();
    }

    /// Lets go of Undo / Redo steps whose packed copies are ready.
    pub fn undo_tick(&mut self) {
        self.undo.tick();
        self.redo.tick();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) {
        if let Some(p) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.doc, p));
            self.dirty = true;
            self.recalc();
            self.status = "Undo".into();
        }
    }

    pub fn redo(&mut self) {
        if let Some(p) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.doc, p));
            self.dirty = true;
            self.recalc();
            self.status = "Redo".into();
        }
    }

    pub fn recalc(&mut self) {
        match schedule::schedule(&mut self.doc) {
            Ok(()) => {
                crate::custom::apply_formulas(&mut self.doc);
                // Leveling Options > Automatic: level after every change
                if self.doc.options.leveling.automatic && self.doc.resources.iter().any(|r| r.overallocated) {
                    match schedule::level_checked(&mut self.doc, None, None) {
                        Ok(report) if report.unresolved_resources > 0 => {
                            self.status = format!("Resource leveling: {} resource(s) remain overallocated", report.unresolved_resources);
                        }
                        Ok(_) => {}
                        Err(e) => self.message = Some((product().into(), format!("Resource leveling failed: {e}"))),
                    }
                }
            }
            Err(e) => self.message = Some((product().into(), e)),
        }
    }

    /// Task > Format Painter: pick up the active cell's format (or the selected rows' whole format).
    pub fn pick_format(&mut self) {
        let Some(i) = self.current_task() else { return };
        let t = &self.doc.tasks[i];
        let key = self.current_col_key();
        let mut cell = t.cell_format(&key);
        cell.whole = true;
        self.format_painter = Some(FormatPaint {
            whole_row: self.sel_whole_rows,
            cell,
            row: t.cell_formats.clone(),
            bar: t.bar_format,
            bar_text: t.bar_text,
        });
        self.status = crate::i18n::tr("Format Painter: click the cells to apply the formatting").into();
    }

    /// Format Painter: paint the formatting picked up onto the selected cells (or rows).
    pub fn paint_format(&mut self) {
        let Some(fp) = self.format_painter.take() else { return };
        let sel = self.selected_tasks();
        if sel.is_empty() {
            return;
        }
        self.checkpoint();
        let whole = fp.whole_row || self.sel_whole_rows;
        let (ca, cb) = self.sel_cols();
        let keys: Vec<String> = (ca..=cb).filter_map(|c| self.task_cols.get(c).map(|x| x.0.title().to_string())).collect();
        for i in sel {
            let t = &mut self.doc.tasks[i];
            if whole {
                t.cell_formats = fp.row.clone();
                t.bar_format = fp.bar;
                t.bar_text = fp.bar_text;
            } else {
                for k in &keys {
                    if fp.cell.is_empty() {
                        t.cell_formats.remove(k);
                    } else {
                        t.cell_formats.insert(k.clone(), fp.cell.clone());
                    }
                }
            }
        }
        self.dirty = true;
        self.status = "Ready".into();
    }

    /// Format > Task Path: the tasks linked to the selected one, by kind (1 predecessors,
    /// 2 driving predecessors, 4 successors, 8 driven successors), each through the whole chain.
    pub fn task_path_sets(&self) -> std::collections::HashMap<usize, u8> {
        let mut out = std::collections::HashMap::new();
        let Some(sel) = self.current_task() else { return out };
        if self.task_path == 0 {
            return out;
        }
        let cal = &self.doc.calendar;
        let mpd = cal.minutes_per_day();
        let n = self.doc.tasks.len();
        let idx: std::collections::HashMap<u32, usize> = self.doc.tasks.iter().enumerate().map(|(k, t)| (t.uid, k)).collect();
        // successors of every task
        let mut succs: Vec<Vec<(usize, usize)>> = vec![Vec::new(); n];
        for (s, t) in self.doc.tasks.iter().enumerate() {
            for (k, l) in t.predecessors.iter().enumerate() {
                if let Some(&p) = idx.get(&l.pred_uid) {
                    succs[p].push((s, k));
                }
            }
        }
        // does link k of task s decide its dates (the instant it asks for is where s starts/ends)?
        let drives = |s: usize, k: usize| -> bool {
            let t = &self.doc.tasks[s];
            let l = &t.predecessors[k];
            let Some(&p) = idx.get(&l.pred_uid) else { return false };
            let pt = &self.doc.tasks[p];
            let (Some(ps), Some(pf), Some(ss), Some(sf)) = (pt.start_dt, pt.finish_dt, t.start_dt, t.finish_dt) else { return false };
            let lag = l.lag_minutes(mpd, pt.dur_minutes(mpd));
            let at = |x: chrono::NaiveDateTime| if lag >= 0 { cal.add_work_minutes(x, lag) } else { cal.sub_work_minutes(x, -lag) };
            let (want, got) = match l.kind {
                crate::model::LinkType::FS => (at(pf), ss),
                crate::model::LinkType::SS => (at(ps), ss),
                crate::model::LinkType::FF => (at(pf), sf),
                crate::model::LinkType::SF => (at(ps), sf),
            };
            want == got || cal.next_work_instant(want) == got
        };
        let walk = |start: usize, back: bool, driving: bool, bit: u8, out: &mut std::collections::HashMap<usize, u8>| {
            let mut stack = vec![start];
            let mut seen = std::collections::HashSet::new();
            while let Some(c) = stack.pop() {
                if !seen.insert(c) {
                    continue;
                }
                if back {
                    for (k, l) in self.doc.tasks[c].predecessors.iter().enumerate() {
                        if let Some(&p) = idx.get(&l.pred_uid) {
                            if !driving || drives(c, k) {
                                *out.entry(p).or_default() |= bit;
                                stack.push(p);
                            }
                        }
                    }
                } else {
                    for &(s, k) in &succs[c] {
                        if !driving || drives(s, k) {
                            *out.entry(s).or_default() |= bit;
                            stack.push(s);
                        }
                    }
                }
            }
        };
        if self.task_path & 1 != 0 {
            walk(sel, true, false, 1, &mut out);
        }
        if self.task_path & 2 != 0 {
            walk(sel, true, true, 2, &mut out);
        }
        if self.task_path & 4 != 0 {
            walk(sel, false, false, 4, &mut out);
        }
        if self.task_path & 8 != 0 {
            walk(sel, false, true, 8, &mut out);
        }
        out
    }

    /// How many changes can be undone (the Task Form refills itself when this moves).
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    /// Project > WBS > Renumber (all tasks, or the selected ones).
    pub fn renumber_wbs(&mut self, selected_only: bool) {
        self.commit_edit();
        self.checkpoint();
        let sel = self.selected_tasks();
        crate::wbs::renumber(&mut self.doc, if selected_only { Some(&sel) } else { None });
        self.dirty = true;
    }

    /// Task > Move > Reschedule: 0 When Resources Are Available (level the selected tasks),
    /// 1 Incomplete Parts to Status Date (what is not done moves after the status date: a task not
    /// started starts then, one under way is split there), 2 Completed Parts to Status Date (work
    /// shown done after the status date moves back to end at it).
    pub fn reschedule(&mut self, kind: u8) {
        let sel = self.selected_tasks();
        if sel.is_empty() {
            return;
        }
        self.commit_edit();
        self.checkpoint();
        let status = self.status_instant();
        let cal = self.doc.calendar.clone();
        let mpd = cal.minutes_per_day();
        match kind {
            0 => {
                match schedule::level_checked(&mut self.doc, None, Some(&sel)) {
                    Ok(report) => self.status = format!("Leveling delayed {} task(s); {} resource(s) still overallocated", report.delayed_tasks, report.unresolved_resources),
                    Err(e) => self.message = Some((product().into(), format!("Leveling failed: {e}"))),
                }
            }
            1 => {
                for &i in &sel {
                    let t = &mut self.doc.tasks[i];
                    if t.is_summary || t.percent_complete == 100 {
                        continue;
                    }
                    let start = t.start_dt.unwrap_or_else(|| cal.day_start(t.start));
                    if start >= status {
                        continue;
                    }
                    match t.actual_start {
                        None => {
                            // a task that should have started: it starts at the status date
                            let at = cal.next_work_instant(status);
                            if t.manual {
                                let d = t.dur_minutes(mpd);
                                t.start_dt = Some(at);
                                t.start = at.date();
                                t.finish_dt = Some(cal.add_work_minutes(at, d));
                            } else {
                                t.set_snet_at(at);
                            }
                        }
                        Some(a) => {
                            // under way: the remaining work resumes at the status date
                            let done = t.actual_minutes(mpd);
                            let through = cal.add_work_minutes(a, done);
                            let gap = cal.work_minutes(through, status).max(0);
                            if gap > 0 && done > 0 {
                                t.splits = vec![(done, gap)];
                            }
                        }
                    }
                }
            }
            _ => {
                for &i in &sel {
                    let t = &mut self.doc.tasks[i];
                    let Some(a) = t.actual_start else { continue };
                    if t.is_summary {
                        continue;
                    }
                    let done = t.actual_minutes(mpd);
                    let through = cal.add_work_minutes(a, done);
                    if through > status {
                        // the part shown done after the status date moves back to end at it
                        let new_start = cal.sub_work_minutes(status, done);
                        t.actual_start = Some(new_start);
                        t.splits.clear();
                    }
                }
            }
        }
        self.recalc();
        self.dirty = true;
    }

    /// Resource > Resource Pool > Share Resources: the resources of another project file join this
    /// one's (by name; their rates and units come along for new ones).
    /// Resource > Resource Pool > Refresh Resource Pool: the pool's resources joined to this
    /// project's by name; with the pool taking precedence a resource both have takes the pool's
    /// fields (its unique ID and assignments stay). Returns (added, updated).
    pub fn refresh_pool(&mut self) -> Result<(usize, usize), String> {
        let Some(rp) = self.doc.resource_pool.clone() else { return Ok((0, 0)) };
        let pool = fileio::load(std::path::Path::new(&rp.path))?;
        let norm = |r: &crate::model::Resource| -> String {
            let mut c = r.clone();
            c.uid = 0;
            c.src_uid = None;
            serde_json::to_string(&c).unwrap_or_default()
        };
        let (mut added, mut updated) = (0, 0);
        let before = self.doc.clone();
        for r in &pool.resources {
            if r.name.trim().is_empty() {
                continue;
            }
            match self.doc.resources.iter().position(|x| x.name.trim().eq_ignore_ascii_case(r.name.trim())) {
                Some(k) => {
                    if !rp.sharer_first && norm(&self.doc.resources[k]) != norm(r) {
                        let (uid, src) = (self.doc.resources[k].uid, self.doc.resources[k].src_uid);
                        self.doc.resources[k] = r.clone();
                        self.doc.resources[k].uid = uid;
                        self.doc.resources[k].src_uid = src;
                        updated += 1;
                    }
                }
                None => {
                    let mut nr = r.clone();
                    nr.uid = self.doc.alloc_res_uid();
                    nr.src_uid = None;
                    self.doc.resources.push(nr);
                    added += 1;
                }
            }
        }
        if added + updated > 0 {
            self.undo.push(before);
            self.redo.clear();
            self.recalc();
            self.dirty = true;
        }
        Ok((added, updated))
    }

    /// Resource > Resource Pool > Update Resource Pool: this project's resources into the pool
    /// file (new ones added; with the sharer taking precedence its fields win). A .neramit / .xml
    /// pool is saved in place; a .mpp is never overwritten (as Save does): the pool is saved next
    /// to it as a new .mpp and this project shares that one from now on. Returns the pool file.
    pub fn update_pool(&mut self) -> Result<PathBuf, String> {
        let rp = self.doc.resource_pool.clone().ok_or("This project does not share the resources of a pool.")?;
        let path = PathBuf::from(&rp.path);
        let mut pool = fileio::load(&path)?;
        for r in &self.doc.resources {
            if r.name.trim().is_empty() {
                continue;
            }
            match pool.resources.iter().position(|x| x.name.trim().eq_ignore_ascii_case(r.name.trim())) {
                Some(k) if rp.sharer_first => {
                    let (uid, src) = (pool.resources[k].uid, pool.resources[k].src_uid);
                    pool.resources[k] = r.clone();
                    pool.resources[k].uid = uid;
                    pool.resources[k].src_uid = src;
                }
                Some(_) => {}
                None => {
                    let mut nr = r.clone();
                    nr.uid = pool.alloc_res_uid();
                    nr.src_uid = None;
                    pool.resources.push(nr);
                }
            }
        }
        if is_mpp(&path) {
            let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "pool".into());
            let dir = path.parent().map(|d| d.to_path_buf()).unwrap_or_default();
            let mut target = dir.join(format!("{stem} - updated.mpp"));
            let mut k = 2;
            while target.exists() {
                target = dir.join(format!("{stem} - updated ({k}).mpp"));
                k += 1;
            }
            let orig = std::fs::read(&path).map_err(|e| format!("Cannot read the pool: {e}"))?;
            let bytes = crate::mppt::save_mpp_at(&orig, &pool, Some(&target))?;
            std::fs::write(&target, bytes).map_err(|e| format!("Cannot write the file: {e}"))?;
            self.doc.resource_pool = Some(crate::model::ResourcePool { path: target.display().to_string(), ..rp });
            self.dirty = true;
            Ok(target)
        } else {
            fileio::save(&path, &pool)?;
            Ok(path)
        }
    }

    /// The external task for task `id` of the project at `path` (one this project has, else a new
    /// one inserted at row `at`, read from that file). Returns its unique ID.
    pub fn external_ghost(&mut self, path: &str, id: u32, at: usize) -> u32 {
        if let Some(g) = self.doc.tasks.iter().find(|t| t.external.as_ref().map(|x| x.path.eq_ignore_ascii_case(path) && x.id == id).unwrap_or(false)) {
            return g.uid;
        }
        let uid = self.doc.alloc_uid();
        let level = self.doc.tasks.get(at).map(|t| t.outline_level).unwrap_or(1);
        let g = fileio::external_task(path, id, uid, level, self.doc.start, self.doc.calendar.minutes_per_day());
        self.doc.tasks.insert(at.min(self.doc.tasks.len()), g);
        uid
    }

    /// Project > Links Between Projects: every link to a task of another project, with what that
    /// file has now (read once per file; by unique ID, else by ID).
    pub fn external_links(&self) -> Vec<ExtLinkRow> {
        let mut files: HashMap<String, Option<Project>> = HashMap::new();
        let mut out = Vec::new();
        for g in self.doc.tasks.iter() {
            let Some(x) = &g.external else { continue };
            let other = files.entry(x.path.to_lowercase()).or_insert_with(|| {
                fileio::load(std::path::Path::new(&x.path)).ok().map(|mut q| {
                    let _ = schedule::schedule(&mut q);
                    q
                })
            });
            let now = other.as_ref().and_then(|q| {
                let by_uid = q.tasks.iter().find(|t| t.src_uid.unwrap_or(t.uid) == x.uid && t.external.is_none());
                by_uid.or_else(|| q.tasks.get(x.id as usize - 1).filter(|_| x.id >= 1)).map(|t| {
                    let id = q.tasks.iter().position(|y| y.uid == t.uid).map(|k| k as u32 + 1).unwrap_or(x.id);
                    (t.name.clone(), t.start_dt, t.finish_dt, t.percent_complete, t.dur_minutes(q.calendar.minutes_per_day()), id)
                })
            });
            let changed = match &now {
                Some((n, s, f, pc, _, id)) => *n != g.name || *s != g.start_dt || *f != g.finish_dt || *pc != g.percent_complete || *id != x.id,
                None => true,
            };
            // the ghost drives these tasks (external predecessor) / follows them (external successor)
            for t in self.doc.tasks.iter() {
                for l in t.predecessors.iter().filter(|l| l.pred_uid == g.uid) {
                    out.push(ExtLinkRow { ghost: g.uid, local: t.uid, predecessor: true, kind: l.kind, now: now.clone(), changed });
                }
            }
            for l in &g.predecessors {
                out.push(ExtLinkRow { ghost: g.uid, local: l.pred_uid, predecessor: false, kind: l.kind, now: now.clone(), changed });
            }
        }
        out
    }

    /// Links Between Projects > Accept: the external task as its file has it now.
    pub fn accept_external(&mut self, ghost: u32, now: &ExtNow) {
        let mpd = self.doc.calendar.minutes_per_day();
        let Some(g) = self.doc.tasks.iter_mut().find(|t| t.uid == ghost) else { return };
        let (name, s, f, pc, dur, id) = now.clone();
        g.name = name;
        g.start_dt = s;
        g.finish_dt = f;
        if let Some(s) = s {
            g.start = s.date();
        }
        if let Some(f) = f {
            g.finish = f.date();
        }
        g.percent_complete = pc;
        let (unit, el) = (g.dur_unit, g.dur_elapsed);
        g.set_duration_minutes(dur, unit, el, mpd);
        g.milestone = dur == 0;
        if let Some(x) = g.external.as_mut() {
            x.id = id;
        }
    }

    /// Links Between Projects > Delete Link (the external task goes with its last link).
    pub fn delete_external_link(&mut self, ghost: u32, local: u32, predecessor: bool) {
        if predecessor {
            if let Some(t) = self.doc.tasks.iter_mut().find(|t| t.uid == local) {
                t.predecessors.retain(|l| l.pred_uid != ghost);
            }
        } else if let Some(g) = self.doc.tasks.iter_mut().find(|t| t.uid == ghost) {
            g.predecessors.retain(|l| l.pred_uid != local);
        }
        let used = self.doc.tasks.iter().any(|t| t.predecessors.iter().any(|l| l.pred_uid == ghost)) || self.doc.tasks.iter().any(|t| t.uid == ghost && !t.predecessors.is_empty());
        if !used {
            self.doc.tasks.retain(|t| t.uid != ghost);
        }
    }

    pub fn share_resources(&mut self, path: &std::path::Path) {
        let pool = match fileio::load(path) {
            Ok(p) => p,
            Err(e) => {
                self.show_error(e);
                return;
            }
        };
        self.checkpoint();
        let mut added = 0;
        for r in &pool.resources {
            if self.doc.resources.iter().any(|x| x.name.trim().eq_ignore_ascii_case(r.name.trim())) {
                continue;
            }
            let uid = self.doc.alloc_res_uid();
            let mut nr = r.clone();
            nr.uid = uid;
            nr.src_uid = None;
            self.doc.resources.push(nr);
            added += 1;
        }
        self.recalc();
        self.dirty = true;
        self.status = crate::i18n::trf("Added {} resource(s) from the pool", &[&added.to_string()]);
    }

    /// Project > Insert > Subproject: another project's tasks inserted above the selected row
    /// under a summary named after it (a copy - the standard "Link to project" left off). Its links
    /// stay between its own tasks; its resources join this project's by name.
    pub fn insert_subproject(&mut self, path: &std::path::Path) {
        let mut sub = match fileio::load(path) {
            Ok(p) => p,
            Err(e) => {
                self.show_error(e);
                return;
            }
        };
        let _ = schedule::schedule(&mut sub);
        self.commit_edit();
        self.checkpoint();
        let vis = self.visible_rows();
        let at = vis.get(self.sel_row).copied().unwrap_or(self.doc.tasks.len()).min(self.doc.tasks.len());
        let level = self.doc.tasks.get(at).map(|t| t.outline_level).unwrap_or(1);
        // resources by name
        let mut res_map: HashMap<u32, u32> = HashMap::new();
        for r in &sub.resources {
            match self.doc.resources.iter().find(|x| x.name.trim().eq_ignore_ascii_case(r.name.trim())) {
                Some(x) => {
                    res_map.insert(r.uid, x.uid);
                }
                None => {
                    let uid = self.doc.alloc_res_uid();
                    let mut nr = r.clone();
                    nr.uid = uid;
                    nr.src_uid = None;
                    self.doc.resources.push(nr);
                    res_map.insert(r.uid, uid);
                }
            }
        }
        let name = if sub.title.trim().is_empty() { path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default() } else { sub.title.clone() };
        let mut head = self.doc.new_task(&name);
        head.outline_level = level;
        head.manual = false;
        let mut uid_map: HashMap<u32, u32> = HashMap::new();
        let mut rows = vec![head];
        for t in &sub.tasks {
            let mut nt = t.clone();
            let uid = self.doc.alloc_uid();
            uid_map.insert(t.uid, uid);
            nt.uid = uid;
            nt.src_uid = None;
            nt.outline_level = t.outline_level.saturating_add(level);
            rows.push(nt);
        }
        for nt in rows.iter_mut().skip(1) {
            nt.predecessors.retain(|l| uid_map.contains_key(&l.pred_uid));
            for l in nt.predecessors.iter_mut() {
                l.pred_uid = uid_map[&l.pred_uid];
            }
            for a in nt.assignments.iter_mut() {
                a.res_uid = res_map.get(&a.res_uid).copied().unwrap_or(a.res_uid);
            }
            nt.resources = nt.resources.iter().filter_map(|u| res_map.get(u).copied()).collect();
        }
        let n = rows.len();
        for (k, t) in rows.into_iter().enumerate() {
            self.doc.tasks.insert(at + k, t);
        }
        self.recalc();
        self.dirty = true;
        self.status = crate::i18n::trf("Inserted {} task(s)", &[&n.to_string()]);
    }

    /// Resource > Next Overallocation: the next task (after the selected row) whose resources are
    /// overallocated while it runs.
    pub fn next_overallocation(&mut self) {
        let vis = self.visible_rows();
        let from = self.sel_row + 1;
        match (from..vis.len()).find(|&r| self.doc.tasks[vis[r]].overallocated && !self.doc.tasks[vis[r]].is_summary) {
            Some(r) => {
                let i = vis[r];
                self.select_task(i);
                self.scroll_to_task();
            }
            None => self.status = crate::i18n::tr("There are no more overallocations.").into(),
        }
    }

    pub fn show_error(&mut self, msg: impl Into<String>) {
        self.message = Some((product().into(), msg.into()));
    }

    fn row_of_task(&self, idx: usize) -> Option<usize> {
        self.visible_rows().iter().position(|&i| i == idx)
    }

    pub fn select_task(&mut self, idx: usize) {
        // expand ancestors so the task is visible
        let mut cur = idx;
        while let Some(p) = self.doc.parent_of(cur) {
            self.doc.tasks[p].collapsed = false;
            cur = p;
        }
        if let Some(r) = self.row_of_task(idx) {
            self.sel_row = r;
            self.sel_anchor = r;
            self.scroll_to_row = Some(r);
        }
    }

    // ------------------------------------------------------------ task commands

    /// Insert a new task before the current row (MPP "Insert Task").
    pub fn insert_task(&mut self, kind: &str) {
        self.commit_edit();
        self.checkpoint();
        let vis = self.visible_rows();
        let at = vis.get(self.sel_row).copied().unwrap_or(self.doc.tasks.len());
        let level = if at < self.doc.tasks.len() {
            self.doc.tasks[at].outline_level
        } else {
            self.doc.tasks.last().map(|t| t.outline_level).unwrap_or(1)
        };
        let name = match kind {
            "milestone" => "<New Milestone>",
            "summary" => "<New Summary Task>",
            _ => "<New Task>",
        };
        let mut t = self.doc.new_task(name);
        t.outline_level = level;
        if kind == "milestone" {
            t.duration = 0;
            t.milestone = true;
        }
        if kind != "summary" {
            t.set_nothing_entered();
        }
        if kind == "summary" {
            // summary: insert summary above the selection and indent the selected tasks
            let sel = self.selected_tasks();
            let (first, last) = match (sel.first(), sel.last()) {
                (Some(&f), Some(&l)) => (f, self.doc.children_end(l)),
                _ => (at, at),
            };
            let mut sum = t;
            sum.outline_level = if first < self.doc.tasks.len() { self.doc.tasks[first].outline_level } else { level };
            let child_name_needed = first == last;
            for i in first..last {
                self.doc.tasks[i].outline_level += 1;
            }
            self.doc.tasks.insert(first, sum);
            if child_name_needed {
                let mut child = self.doc.new_task("<New Task>");
                child.set_nothing_entered();
                child.outline_level = self.doc.tasks[first].outline_level + 1;
                self.doc.tasks.insert(first + 1, child);
            }
            self.recalc();
            if let Some(r) = self.row_of_task(first) {
                self.sel_row = r;
                self.sel_anchor = r;
            }
            self.begin_edit_name();
            return;
        }
        self.doc.tasks.insert(at, t);
        if let Some(c) = crate::wbs::code_for_new(&self.doc, at) {
            self.doc.tasks[at].wbs = c;
        }
        if self.doc.options.autolink {
            self.autolink_inserted(at);
        }
        self.recalc();
        if let Some(r) = self.row_of_task(at) {
            self.sel_row = r;
            self.sel_anchor = r;
        }
        self.begin_edit_name();
    }

    /// Autolink inserted or moved tasks (Options > Schedule): a task inserted between two tasks
    /// linked Finish-to-Start takes its place in the chain.
    fn autolink_inserted(&mut self, k: usize) {
        if k == 0 || k + 1 >= self.doc.tasks.len() {
            return;
        }
        let (p, n) = (k - 1, k + 1);
        if self.doc.tasks[p].is_summary || self.doc.tasks[n].is_summary {
            return;
        }
        let pu = self.doc.tasks[p].uid;
        let Some(li) = self.doc.tasks[n].predecessors.iter().position(|l| l.pred_uid == pu && l.kind == LinkType::FS) else { return };
        let mut old = self.doc.tasks[n].predecessors.remove(li);
        let ku = self.doc.tasks[k].uid;
        self.doc.tasks[k].predecessors.push(Link::new(pu, LinkType::FS));
        old.pred_uid = ku;
        self.doc.tasks[n].predecessors.push(old);
    }

    /// Task > Move: move the selected tasks forward (or back, negative) by working minutes. An
    /// auto-scheduled task gets Start No Earlier Than its new start, a manual one the new start.
    /// Project > Move Project: the project start moves to `new_start` and every date that pins a
    /// task (constraint dates, manually scheduled dates, the status date and, when asked,
    /// deadlines) moves with it by the same number of days. Actual dates and baselines are
    /// history and stay as they are.
    pub fn move_project(&mut self, new_start: NaiveDate, move_deadlines: bool) {
        let days = (new_start - self.doc.start).num_days();
        if days == 0 {
            return;
        }
        self.checkpoint();
        let d = chrono::Duration::days(days);
        self.doc.start = new_start;
        self.doc.start_dt = self.doc.start_dt.map(|x| x + d);
        self.doc.status_date = self.doc.status_date.map(|x| x + d);
        for t in self.doc.tasks.iter_mut() {
            t.constraint_date = t.constraint_date.map(|x| x + d);
            t.constraint_dt = t.constraint_dt.map(|x| x + d);
            if move_deadlines {
                t.deadline = t.deadline.map(|x| x + d);
            }
            if t.manual && t.actual_start.is_none() {
                t.start += d;
                t.finish += d;
                t.start_dt = t.start_dt.map(|x| x + d);
                t.finish_dt = t.finish_dt.map(|x| x + d);
            }
        }
        self.recalc();
        self.status = "Project moved".into();
    }

    pub fn move_tasks_by(&mut self, m: i64) {
        let sel: Vec<usize> = self.selected_tasks().into_iter().filter(|&i| !self.doc.tasks[i].is_summary).collect();
        if sel.is_empty() || m == 0 {
            return;
        }
        self.checkpoint();
        let cal = self.doc.calendar.clone();
        for i in sel {
            let t = &self.doc.tasks[i];
            let st = t.start_dt.unwrap_or_else(|| cal.day_start(t.start));
            let ns = if m > 0 { cal.next_work_instant(cal.add_work_minutes(cal.next_work_instant(st), m)) } else { cal.sub_work_minutes(st, -m) };
            let t = &mut self.doc.tasks[i];
            if t.manual {
                t.start_dt = Some(ns);
                t.start = ns.date();
            } else {
                t.set_snet_at(ns);
            }
        }
        self.recalc();
    }

    fn begin_edit_name(&mut self) {
        if let Some(ci) = self.task_cols.iter().position(|c| c.0 == TCol::Name) {
            self.sel_col = ci;
            if let Some(i) = self.current_task() {
                self.editing = Some(EditState { row: self.sel_row, col: ci, text: self.doc.tasks[i].name.clone(), just_opened: true, select_all: true });
            }
        }
    }

    pub fn delete_selected(&mut self) {
        let sel = self.selected_tasks();
        if sel.is_empty() {
            return;
        }
        // deleting a summary deletes its children (as in MPP)
        let mut del = std::collections::BTreeSet::new();
        for i in sel {
            for j in i..self.doc.children_end(i) {
                del.insert(j);
            }
        }
        let uids: Vec<u32> = del.iter().map(|&i| self.doc.tasks[i].uid).collect();
        let ids: std::collections::HashSet<u32> = uids.iter().copied().collect();
        if let Err(error) = crate::controls::ensure_deletable(&self.doc, &ids) {
            self.show_error(error);
            return;
        }
        self.checkpoint();
        // Autolink: tasks deleted out of a Finish-to-Start chain leave the tasks around them linked
        let mut relink: Vec<(u32, Link)> = Vec::new();
        if self.doc.options.autolink {
            let idx: Vec<usize> = del.iter().copied().collect();
            let mut k = 0;
            while k < idx.len() {
                let a = idx[k];
                let mut b = a;
                while k + 1 < idx.len() && idx[k + 1] == b + 1 {
                    k += 1;
                    b = idx[k];
                }
                k += 1;
                if a == 0 || b + 1 >= self.doc.tasks.len() {
                    continue;
                }
                let (p, n) = (a - 1, b + 1);
                let chain = (a..=n).all(|j| {
                    let pu = self.doc.tasks[j - 1].uid;
                    !self.doc.tasks[j].is_summary && self.doc.tasks[j].predecessors.iter().any(|l| l.pred_uid == pu && l.kind == LinkType::FS)
                });
                if chain && !self.doc.tasks[p].is_summary {
                    let bu = self.doc.tasks[b].uid;
                    if let Some(l) = self.doc.tasks[n].predecessors.iter().find(|l| l.pred_uid == bu) {
                        let mut l = l.clone();
                        l.pred_uid = self.doc.tasks[p].uid;
                        relink.push((self.doc.tasks[n].uid, l));
                    }
                }
            }
        }
        for &i in del.iter().rev() {
            self.doc.tasks.remove(i);
        }
        for t in self.doc.tasks.iter_mut() {
            t.predecessors.retain(|l| !uids.contains(&l.pred_uid));
        }
        for (nu, l) in relink {
            if let Some(ni) = self.doc.index_of_uid(nu) {
                if !self.doc.tasks[ni].predecessors.iter().any(|x| x.pred_uid == l.pred_uid) {
                    self.doc.tasks[ni].predecessors.push(l);
                }
            }
        }
        self.recalc();
        let n = self.visible_rows().len();
        self.sel_row = self.sel_row.min(self.sel_anchor).min(n);
        self.sel_anchor = self.sel_row;
        self.status = format!("Deleted {} task(s)", uids.len());
    }

    pub fn indent(&mut self, delta: i32) {
        let sel = self.selected_tasks();
        if sel.is_empty() {
            return;
        }
        self.checkpoint();
        // move whole blocks (task + its children) together
        let mut blocks = Vec::new();
        let mut i = 0;
        while i < sel.len() {
            let s = sel[i];
            let e = self.doc.children_end(s);
            blocks.push((s, e));
            while i < sel.len() && sel[i] < e {
                i += 1;
            }
        }
        for (s, e) in blocks {
            if delta > 0 {
                let prev_lvl = if s == 0 { 0 } else { self.doc.tasks[s - 1].outline_level };
                if self.doc.tasks[s].outline_level > prev_lvl {
                    continue;
                }
                for j in s..e {
                    self.doc.tasks[j].outline_level += 1;
                }
            } else {
                if self.doc.tasks[s].outline_level <= 1 {
                    continue;
                }
                for j in s..e {
                    self.doc.tasks[j].outline_level -= 1;
                }
            }
        }
        self.recalc();
    }

    pub fn link_selected(&mut self) {
        let sel: Vec<usize> = self.selected_tasks();
        if sel.len() < 2 {
            self.status = "Select two or more tasks to link".into();
            return;
        }
        self.checkpoint();
        let mut rejected = 0;
        for w in sel.windows(2) {
            let (a, b) = (self.doc.tasks[w[0]].uid, self.doc.tasks[w[1]].uid);
            // MPP does not link a summary to its own subtasks
            let (ia, ib) = (w[0], w[1]);
            if self.is_outline_relative(ia, ib) {
                continue;
            }
            if self.doc.tasks[ib].predecessors.iter().any(|l| l.pred_uid == a) {
                continue;
            }
            if schedule::creates_cycle(&self.doc, a, b) {
                rejected += 1;
                continue;
            }
            self.doc.tasks[ib].predecessors.push(Link { pred_uid: a, kind: LinkType::FS, lag: 0, elapsed: false, ..Default::default() });
        }
        self.recalc();
        if rejected > 0 {
            self.show_error("Some links were not created because they would create a circular relationship.");
        }
    }

    /// Link two tasks Finish-to-Start (a bar dragged onto another in the Gantt chart).
    pub fn link_tasks(&mut self, from: usize, to: usize) {
        if from == to || from >= self.doc.tasks.len() || to >= self.doc.tasks.len() {
            return;
        }
        if self.is_outline_relative(from, to) {
            self.show_error("You cannot link a summary task to one of its subtasks.");
            return;
        }
        let (a, b) = (self.doc.tasks[from].uid, self.doc.tasks[to].uid);
        if self.doc.tasks[to].predecessors.iter().any(|l| l.pred_uid == a) {
            return;
        }
        if schedule::creates_cycle(&self.doc, a, b) {
            self.show_error("This link would create a circular relationship.");
            return;
        }
        self.checkpoint();
        self.doc.tasks[to].predecessors.push(Link::new(a, LinkType::FS));
        self.recalc();
    }

    /// A task drawn in an empty row of the Gantt chart: it starts where the drag began and lasts
    /// the dragged working time (an auto-scheduled one gets Start No Earlier Than, like a drag).
    pub fn draw_new_task(&mut self, s: chrono::NaiveDateTime, dur: i64) {
        self.checkpoint();
        let du = self.dur_units();
        let mut t = self.doc.new_task("");
        let unit = match self.doc.options.duration_units {
            0 => DurUnit::Minutes,
            1 => DurUnit::Hours,
            3 => DurUnit::Weeks,
            4 => DurUnit::Months,
            _ => DurUnit::Days,
        };
        t.set_duration_minutes(dur, unit, false, du.mpd);
        t.estimated = false;
        t.start = s.date();
        if t.manual {
            t.start_dt = Some(s);
        } else {
            t.set_snet_at(s);
        }
        t.outline_level = self.doc.tasks.last().map(|x| x.outline_level).unwrap_or(1);
        self.doc.tasks.push(t);
        self.recalc();
        let idx = self.doc.tasks.len() - 1;
        self.select_task(idx);
    }

    pub fn unlink_selected(&mut self) {
        let sel = self.selected_tasks();
        if sel.is_empty() {
            return;
        }
        self.checkpoint();
        let uids: Vec<u32> = sel.iter().map(|&i| self.doc.tasks[i].uid).collect();
        if sel.len() == 1 {
            let u = uids[0];
            self.doc.tasks[sel[0]].predecessors.clear();
            for t in self.doc.tasks.iter_mut() {
                t.predecessors.retain(|l| l.pred_uid != u);
            }
        } else {
            for &i in &sel {
                self.doc.tasks[i].predecessors.retain(|l| !uids.contains(&l.pred_uid));
            }
        }
        self.recalc();
    }

    pub fn set_percent(&mut self, pct: u8) {
        let sel = self.selected_tasks();
        if sel.is_empty() {
            return;
        }
        self.checkpoint();
        let cal = self.doc.calendar.clone();
        for i in sel {
            if self.doc.tasks[i].is_summary {
                let end = self.doc.children_end(i);
                for j in i + 1..end {
                    if !self.doc.tasks[j].is_summary {
                        self.doc.tasks[j].set_percent(pct, &cal);
                    }
                }
            } else {
                self.doc.tasks[i].set_percent(pct, &cal);
            }
        }
        self.recalc();
    }

    /// The status date as an instant (end of that day), or now.
    pub fn status_instant(&self) -> chrono::NaiveDateTime {
        match self.doc.status_date {
            Some(d) => self.doc.calendar.day_end(d),
            None => chrono::Local::now().naive_local(),
        }
    }

    /// Progress that should have been made by `through`: the task's working time from its start.
    pub fn planned_minutes(&self, i: usize, through: chrono::NaiveDateTime) -> i64 {
        let t = &self.doc.tasks[i];
        let cal = &self.doc.calendar;
        let st = t.current_start(cal);
        let d = t.dur_minutes(cal.minutes_per_day());
        let m = if t.dur_elapsed { (through - st).num_minutes() } else { cal.work_minutes(st, through) };
        m.clamp(0, d)
    }

    /// Project > Update Project / Task > Mark on Track: tasks are updated as scheduled through
    /// `through` (all-or-nothing marks only the tasks that should be finished). Progress already
    /// recorded is never reduced.
    pub fn update_progress(&mut self, idx: &[usize], through: chrono::NaiveDateTime, all_or_nothing: bool) {
        let cal = self.doc.calendar.clone();
        let mpd = cal.minutes_per_day();
        for &i in idx {
            if self.doc.tasks[i].is_summary || self.doc.tasks[i].percent_complete == 100 {
                continue;
            }
            let planned = self.planned_minutes(i, through);
            let t = &mut self.doc.tasks[i];
            let d = t.dur_minutes(mpd);
            let start_due = t.current_start(&cal) <= through;
            if (d == 0 && start_due) || (d > 0 && planned >= d) {
                t.set_percent(100, &cal);
            } else if !all_or_nothing && planned > t.actual_minutes(mpd) {
                t.set_actual_minutes(planned, &cal);
            }
        }
    }

    /// The standard Status field: Complete, On Schedule, Late or Future Task (against the status date).
    pub fn status_text(&self, i: usize) -> &'static str {
        let t = &self.doc.tasks[i];
        let mpd = self.doc.calendar.minutes_per_day();
        let through = self.status_instant();
        if t.percent_complete == 100 {
            "Complete"
        } else if t.current_start(&self.doc.calendar) > through {
            "Future Task"
        } else if t.is_summary {
            let end = self.doc.children_end(i);
            if (i + 1..end).any(|j| !self.doc.tasks[j].is_summary && self.status_text(j) == "Late") { "Late" } else { "On Schedule" }
        } else if t.actual_minutes(mpd) >= self.planned_minutes(i, through) {
            "On Schedule"
        } else {
            "Late"
        }
    }

    /// Work of a task in minutes (its assignments; summaries add up), as last scheduled.
    pub fn task_work_min(&self, i: usize) -> i64 {
        self.doc.tasks[i].work_min
    }

    /// Indices of a task's assignments to work resources.
    fn work_assignments(&self, i: usize) -> Vec<usize> {
        let t = &self.doc.tasks[i];
        (0..t.assignments.len()).filter(|&k| self.doc.resource_by_uid(t.assignments[k].res_uid).map(|r| r.rtype == ResourceType::Work).unwrap_or(false)).collect()
    }

    /// Work typed for a task (the standard task types): Fixed Units and Fixed Work tasks get the
    /// duration total work / total units; a Fixed Duration task gets new units.
    pub fn set_task_work(&mut self, i: usize, w: i64) -> Result<(), String> {
        let ks = self.work_assignments(i);
        if ks.is_empty() {
            return Err("Assign a work resource to this task before you enter its work.".into());
        }
        let mpd = self.doc.calendar.minutes_per_day();
        let t = &mut self.doc.tasks[i];
        let u: f64 = ks.iter().map(|&k| t.assignments[k].units).sum::<f64>().max(0.0001);
        let old: i64 = ks.iter().map(|&k| t.assignments[k].work_min.max(0)).sum();
        if t.task_type == TaskType::FixedDuration {
            let d = t.dur_minutes(mpd).max(1);
            for &k in &ks {
                let a = &mut t.assignments[k];
                a.work_min = if old > 0 { (w as f64 * a.work_min.max(0) as f64 / old as f64).round() as i64 } else { (w as f64 * a.units / u).round() as i64 };
                a.units = a.work_min as f64 / d as f64;
            }
        } else {
            for &k in &ks {
                let a = &mut t.assignments[k];
                a.work_min = (w as f64 * a.units / u).round() as i64;
            }
            let (unit, el) = (t.dur_unit, t.dur_elapsed);
            t.set_duration_minutes((w as f64 / u).round() as i64, unit, el, mpd);
        }
        t.work_ref_dur = t.dur_minutes(mpd);
        Ok(())
    }

    /// Units typed for an assignment: a Fixed Duration task gets new work, the other types keep
    /// the work and get the duration of the longest assignment.
    pub fn set_assignment_units(&mut self, i: usize, res_uid: u32, units: f64) {
        let mpd = self.doc.calendar.minutes_per_day();
        let is_work = self.doc.resource_by_uid(res_uid).map(|r| r.rtype == ResourceType::Work).unwrap_or(false);
        let ks = self.work_assignments(i);
        let t = &mut self.doc.tasks[i];
        let Some(k) = t.assignments.iter().position(|a| a.res_uid == res_uid) else { return };
        if !is_work || t.assignments[k].work_min < 0 {
            t.assignments[k].units = units;
            return;
        }
        let units = units.max(0.0001);
        let d = t.dur_minutes(mpd);
        if t.task_type == TaskType::FixedDuration {
            t.assignments[k].units = units;
            t.assignments[k].work_min = (d as f64 * units).round() as i64;
        } else {
            t.assignments[k].units = units;
            let nd = ks.iter().map(|&j| (t.assignments[j].work_min.max(0) as f64 / t.assignments[j].units.max(0.0001)).round() as i64).max().unwrap_or(d);
            let (unit, el) = (t.dur_unit, t.dur_elapsed);
            t.set_duration_minutes(nd, unit, el, mpd);
        }
        t.work_ref_dur = t.dur_minutes(mpd);
    }

    /// Split Task: split task `i` at instant `t` with a gap of `gap` working minutes (MPP
    /// pushes the rest of the task one timescale unit later).
    pub fn split_task_at(&mut self, i: usize, t: chrono::NaiveDateTime, gap: i64) -> bool {
        let cal = self.doc.calendar.clone();
        let mpd = cal.minutes_per_day();
        let task = &self.doc.tasks[i];
        if task.is_summary || task.is_milestone() {
            return false;
        }
        let d = task.dur_minutes(mpd);
        // working time of the task before `t`, without the gaps already in it
        let segs = task.segments(&cal);
        let mut done = 0i64;
        let mut at = None;
        for (a, b) in &segs {
            if t > *a && t < *b {
                at = Some(done + if task.dur_elapsed { (t - *a).num_minutes() } else { cal.work_minutes(*a, t) });
                break;
            }
            done += if task.dur_elapsed { (*b - *a).num_minutes() } else { cal.work_minutes(*a, *b) };
        }
        let Some(pos) = at.filter(|p| *p > 0 && *p < d) else { return false };
        self.checkpoint();
        let t = &mut self.doc.tasks[i];
        t.splits.push((pos, gap.max(1)));
        t.splits.sort();
        self.recalc();
        true
    }

    /// Resource > Level All / Level Resource.
    pub fn level_resources(&mut self, only: Option<u32>) {
        self.commit_edit();
        self.checkpoint();
        match schedule::level_checked(&mut self.doc, only, None) {
            Ok(report) => {
                // schedule::level_checked has already recalculated the staged project.
                crate::custom::apply_formulas(&mut self.doc);
                self.status = format!("Leveling delayed {} task(s); {} resource(s) still overallocated", report.delayed_tasks, report.unresolved_resources);
                self.dirty = true;
            }
            Err(e) => self.message = Some((product().into(), format!("Leveling failed: {e}"))),
        }
    }

    /// Resource > Clear Leveling.
    pub fn clear_leveling(&mut self) {
        self.checkpoint();
        for t in self.doc.tasks.iter_mut() {
            t.level_delay_min = 0;
        }
        self.recalc();
    }

    /// A duration of this task's kind ("2 days", "-1 day", "3 edays").
    pub fn fmt_task_minutes(&self, t: &Task, m: i64) -> String {
        let du = self.dur_units();
        let (unit, el) = if t.is_summary { (DurUnit::Days, false) } else { (t.dur_unit, t.dur_elapsed) };
        let s = util::fmt_dur(m.abs(), unit, el, du, &self.doc.options, crate::i18n::is_thai());
        if m < 0 {
            format!("-{s}")
        } else {
            s
        }
    }

    /// Working days between two instants as MPP shows variances ("0 days", "-2 days").
    pub fn fmt_variance(&self, a: chrono::NaiveDateTime, b: chrono::NaiveDateTime) -> String {
        let du = self.dur_units();
        let m = self.doc.calendar.work_minutes(a, b);
        let s = util::fmt_dur(m.abs(), DurUnit::Days, false, du, &self.doc.options, crate::i18n::is_thai());
        if m < 0 {
            format!("-{s}")
        } else {
            s
        }
    }

    /// View > Tables: the standard built-in tables (columns and their titles).
    pub fn apply_table(&mut self, k: u8) {
        use TCol::*;
        let cols: Vec<(TCol, Option<&str>)> = match k {
            1 => vec![(Name, None), (Start, None), (Finish, None), (LateStart, None), (LateFinish, None), (FreeSlack, None), (TotalSlack, None)],
            2 => vec![(Name, None), (Duration, None), (Start, None), (Finish, None), (Percent, Some("% Comp.")), (Cost, None), (Work, None)],
            3 => vec![
                (Name, None),
                (ActualStart, Some("Act. Start")),
                (ActualFinish, Some("Act. Finish")),
                (Percent, Some("% Comp.")),
                (PhysicalPercent, Some("Phys. % Comp.")),
                (ActualDuration, Some("Act. Dur.")),
                (RemainingDuration, Some("Rem. Dur.")),
                (ActualCost, Some("Act. Cost")),
                (ActualWork, Some("Act. Work")),
            ],
            4 => vec![(Name, None), (Start, None), (Finish, None), (BaselineStart, None), (BaselineFinish, None), (StartVariance, Some("Start Var.")), (FinishVariance, Some("Finish Var."))],
            5 => vec![
                (Name, None),
                (FixedCost, None),
                (FixedCostAccrual, None),
                (Cost, Some("Total Cost")),
                (BaselineCost, Some("Baseline")),
                (CostVariance, Some("Variance")),
                (ActualCost, Some("Actual")),
                (RemainingCost, Some("Remaining")),
            ],
            6 => vec![
                (Name, None),
                (Work, None),
                (BaselineWork, Some("Baseline")),
                (WorkVariance, Some("Variance")),
                (ActualWork, Some("Actual")),
                (RemainingWork, Some("Remaining")),
                (PercentWorkComplete, Some("% W. Comp.")),
            ],
            7 => vec![(Indicators, None), (Name, None), (Work, None), (Duration, None), (Start, None), (Finish, None)],
            8 => vec![(Indicators, None), (Name, None), (Hyperlink, None), (HyperlinkAddress, None), (HyperlinkSubAddress, None)],
            _ => vec![(Indicators, None), (Mode, None), (Name, None), (Duration, None), (Start, None), (Finish, None), (Predecessors, None), (Resources, None)],
        };
        self.commit_edit();
        self.table = k.min(8);
        // the Task Name column keeps the width the user (or the file's view) gave it
        let name_w = self.task_cols.iter().find(|c| c.0 == TCol::Name).map(|c| c.1);
        self.task_cols = cols.iter().map(|(c, _)| (*c, if *c == TCol::Name { name_w.unwrap_or(c.default_width()) } else { c.default_width() })).collect();
        self.col_titles = cols.iter().filter_map(|(c, t)| t.map(|t| (*c, t.to_string()))).collect();
        self.col_align.clear();
        self.col_title_align.clear();
        self.sel_col = self.task_cols.iter().position(|c| c.0 == TCol::Name).unwrap_or(0);
        self.sel_col_anchor = None;
    }

    /// Project > Set Baseline: copy the current start, finish, duration, work and cost into
    /// baseline `n` of every task (or of the selected ones, optionally rolled up to summaries).
    pub fn set_baseline(&mut self, n: u8, selected_only: bool, to_all_summaries: bool, into_selected_summaries: bool) {
        let idx: Vec<usize> = if selected_only { self.selected_tasks() } else { (0..self.doc.tasks.len()).collect() };
        if idx.is_empty() {
            return;
        }
        self.checkpoint();
        let mpd = self.doc.calendar.minutes_per_day();
        let work: Vec<i64> = idx.iter().map(|&i| self.task_work_min(i)).collect();
        for (k, &i) in idx.iter().enumerate() {
            let t = &mut self.doc.tasks[i];
            let b = crate::model::Baseline { number: n, start: t.start_dt, finish: t.finish_dt, dur_min: Some(t.dur_minutes(mpd)), work_min: Some(work[k]), cost: Some(t.cost) };
            t.set_baseline(b);
        }
        if selected_only {
            let mut sums: Vec<usize> = Vec::new();
            if to_all_summaries {
                for &i in &idx {
                    let mut a = self.doc.parent_of(i);
                    while let Some(p) = a {
                        if !sums.contains(&p) {
                            sums.push(p);
                        }
                        a = self.doc.parent_of(p);
                    }
                }
            }
            if into_selected_summaries {
                for &i in &idx {
                    if self.doc.tasks[i].is_summary && !sums.contains(&i) {
                        sums.push(i);
                    }
                }
            }
            // deepest summaries first so outer ones see their children's new values
            sums.sort_by_key(|&s| std::cmp::Reverse(self.doc.tasks[s].outline_level));
            let cal = self.doc.calendar.clone();
            for s in sums {
                let end = self.doc.children_end(s);
                let kids: Vec<crate::model::Baseline> = (s + 1..end).filter(|&j| self.doc.parent_of(j) == Some(s)).filter_map(|j| self.doc.tasks[j].baseline(n).cloned()).collect();
                if kids.is_empty() {
                    continue;
                }
                let start = kids.iter().filter_map(|b| b.start).min();
                let finish = kids.iter().filter_map(|b| b.finish).max();
                let dur = match (start, finish) {
                    (Some(a), Some(b)) => Some(cal.work_minutes(a, b).max(0)),
                    _ => None,
                };
                let work = Some(kids.iter().filter_map(|b| b.work_min).sum());
                let cost = Some(kids.iter().filter_map(|b| b.cost).sum());
                self.doc.tasks[s].set_baseline(crate::model::Baseline { number: n, start, finish, dur_min: dur, work_min: work, cost });
            }
        }
        let now = chrono::Local::now().naive_local();
        self.doc.baseline_saved.retain(|(k, _)| *k != n);
        self.doc.baseline_saved.push((n, now));
        self.doc.baseline_saved.sort_by_key(|(k, _)| *k);
        self.dirty = true;
    }

    /// Project > Clear Baseline.
    pub fn clear_baseline(&mut self, n: u8, selected_only: bool) {
        let idx: Vec<usize> = if selected_only { self.selected_tasks() } else { (0..self.doc.tasks.len()).collect() };
        self.checkpoint();
        for i in idx {
            self.doc.tasks[i].baselines.retain(|b| b.number != n);
        }
        if !selected_only || !self.doc.tasks.iter().any(|t| t.baseline(n).is_some()) {
            self.doc.baseline_saved.retain(|(k, _)| *k != n);
        }
        self.dirty = true;
    }

    /// "Baseline", "Baseline 1" ... with "(last saved on ...)" once saved.
    pub fn baseline_label(&self, n: u8) -> String {
        let name = if n == 0 { crate::i18n::tr("Baseline").to_string() } else { crate::i18n::trf("Baseline {}", &[&n.to_string()]) };
        match self.doc.baseline_saved.iter().find(|(k, _)| *k == n) {
            Some((_, at)) => format!("{name} {}", crate::i18n::trf("(last saved on {})", &[&self.fmt_date(at.date())])),
            None => name,
        }
    }

    /// Task > Mark on Track: the selected tasks are complete through the status date (or now),
    /// as scheduled.
    pub fn mark_on_track(&mut self) {
        let sel = self.selected_tasks();
        if sel.is_empty() {
            return;
        }
        self.checkpoint();
        let mut idx = Vec::new();
        for i in sel {
            if self.doc.tasks[i].is_summary {
                idx.extend((i + 1..self.doc.children_end(i)).filter(|&j| !self.doc.tasks[j].is_summary));
            } else {
                idx.push(i);
            }
        }
        let through = self.status_instant();
        self.update_progress(&idx, through, false);
        self.recalc();
    }

    pub fn set_mode(&mut self, manual: bool) {
        let sel = self.selected_tasks();
        if sel.is_empty() {
            return;
        }
        self.checkpoint();
        for i in sel {
            if !self.doc.tasks[i].is_summary {
                self.doc.tasks[i].manual = manual;
                self.doc.tasks[i].sync_manual_text();
            }
        }
        self.recalc();
    }

    pub fn toggle_inactive(&mut self) {
        let sel = self.selected_tasks();
        if sel.is_empty() {
            return;
        }
        self.checkpoint();
        for i in sel {
            self.doc.tasks[i].active = !self.doc.tasks[i].active;
        }
        self.recalc();
    }

    pub fn toggle_timeline_for_selection(&mut self) {
        let sel = self.selected_tasks();
        if sel.is_empty() {
            return;
        }
        self.checkpoint();
        let on = !self.doc.tasks[sel[0]].on_timeline;
        for i in sel {
            self.doc.tasks[i].on_timeline = on;
        }
        self.show_timeline = true;
    }

    pub fn toggle_collapse(&mut self, idx: usize) {
        if self.doc.tasks[idx].is_summary {
            self.doc.tasks[idx].collapsed = !self.doc.tasks[idx].collapsed;
            if let Some(r) = self.row_of_task(idx) {
                self.sel_row = r;
                self.sel_anchor = r;
            }
        }
    }

    pub fn set_all_collapsed(&mut self, level: Option<u8>) {
        for t in self.doc.tasks.iter_mut() {
            t.collapsed = match level {
                None => false,
                Some(l) => t.is_summary && t.outline_level >= l,
            };
        }
        self.sel_row = 0;
        self.sel_anchor = 0;
    }

    pub fn move_selected(&mut self, delta: i32) {
        let sel = self.selected_tasks();
        if sel.len() != 1 {
            return;
        }
        let i = sel[0];
        let e = self.doc.children_end(i);
        let lvl = self.doc.tasks[i].outline_level;
        if delta < 0 {
            // find previous sibling block start
            let Some(prev) = (0..i).rev().find(|&j| self.doc.tasks[j].outline_level <= lvl) else { return };
            if self.doc.tasks[prev].outline_level < lvl {
                return;
            }
            self.checkpoint();
            let block: Vec<Task> = self.doc.tasks.drain(i..e).collect();
            for (k, t) in block.into_iter().enumerate() {
                self.doc.tasks.insert(prev + k, t);
            }
            self.recalc();
            self.select_task(prev);
        } else {
            if e >= self.doc.tasks.len() || self.doc.tasks[e].outline_level != lvl {
                return;
            }
            let ne = self.doc.children_end(e);
            self.checkpoint();
            let block: Vec<Task> = self.doc.tasks.drain(i..e).collect();
            let insert_at = ne - block.len();
            let n = block.len();
            for (k, t) in block.into_iter().enumerate() {
                self.doc.tasks.insert(insert_at + k, t);
            }
            self.recalc();
            self.select_task(insert_at);
            let _ = n;
        }
    }

    pub fn copy_selected(&mut self, cut: bool) {
        let sel = self.selected_tasks();
        if sel.is_empty() {
            return;
        }
        self.clipboard = sel.iter().map(|&i| self.doc.tasks[i].clone()).collect();
        if cut {
            self.delete_selected();
        }
        self.status = format!("{} task(s) copied", self.clipboard.len());
    }

    pub fn paste(&mut self) {
        if self.clipboard.is_empty() {
            return;
        }
        self.checkpoint();
        let vis = self.visible_rows();
        let at = vis.get(self.sel_row).copied().unwrap_or(self.doc.tasks.len());
        let mut map: HashMap<u32, u32> = HashMap::new();
        let mut items = self.clipboard.clone();
        for t in items.iter_mut() {
            let nu = self.doc.alloc_uid();
            map.insert(t.uid, nu);
            t.uid = nu;
        }
        for t in items.iter_mut() {
            // keep links only between pasted tasks (as MPP does)
            t.predecessors = t
                .predecessors
                .iter()
                .filter_map(|l| map.get(&l.pred_uid).map(|u| Link { pred_uid: *u, ..l.clone() }))
                .collect();
        }
        let n = items.len();
        for (k, t) in items.into_iter().enumerate() {
            self.doc.tasks.insert(at + k, t);
        }
        self.recalc();
        self.status = format!("Pasted {n} task(s)");
    }

    pub fn fill_down(&mut self) {
        let sel = self.selected_tasks();
        if sel.len() < 2 {
            return;
        }
        let Some(&(col, _)) = self.task_cols.get(self.sel_col) else { return };
        let src = self.cell_text(sel[0], col);
        self.checkpoint();
        for &i in &sel[1..] {
            let _ = self.apply_cell(i, col, &src);
        }
        self.recalc();
    }

    pub fn clear_selection_cells(&mut self) {
        let sel = self.selected_tasks();
        let Some(&(col, _)) = self.task_cols.get(self.sel_col) else { return };
        if sel.is_empty() {
            return;
        }
        self.checkpoint();
        for i in sel {
            match col {
                TCol::Predecessors => self.doc.tasks[i].predecessors.clear(),
                TCol::Resources => self.doc.tasks[i].resources.clear(),
                TCol::Notes => self.doc.tasks[i].notes.clear(),
                TCol::Percent => self.doc.tasks[i].percent_complete = 0,
                _ => {}
            }
        }
        self.recalc();
    }

    pub fn find_next(&mut self) {
        let needle = self.find_text.to_lowercase();
        if needle.is_empty() {
            return;
        }
        let start = self.current_task().map(|i| i + 1).unwrap_or(0);
        let n = self.doc.tasks.len();
        for k in 0..n {
            let i = (start + k) % n;
            if self.doc.tasks[i].name.to_lowercase().contains(&needle) {
                self.select_task(i);
                self.status = format!("Found: {}", self.doc.tasks[i].name);
                return;
            }
        }
        self.show_error(format!("The search item '{}' was not found.", self.find_text));
    }

    // ------------------------------------------------------------ cells

    pub fn cell_text(&self, i: usize, col: TCol) -> String {
        self.task_cell_text(&self.doc.tasks[i], Some(i), col)
    }

    /// A task's cell; `i` = None for the project summary task (ID 0).
    pub fn task_cell_text(&self, t: &Task, i: Option<usize>, col: TCol) -> String {
        let mpd = self.doc.calendar.minutes_per_day();
        match col {
            TCol::Indicators => String::new(),
            TCol::Successors | TCol::Predecessors | TCol::Mode if i.is_none() => String::new(),
            TCol::Mode => if t.manual { "Manually Scheduled".into() } else { "Auto Scheduled".into() },
            TCol::Name => t.name.clone(),
            TCol::Duration | TCol::Start | TCol::Finish if t.manual && t.manual_text[Self::text_slot(col)].is_some() => t.manual_text[Self::text_slot(col)].clone().unwrap_or_default(),
            TCol::Duration => self.task_duration_text(t),
            TCol::Start => self.fmt_date(t.start),
            TCol::Finish => self.fmt_finish(t.finish),
            TCol::Predecessors => {
                let ids = self.uid_to_id();
                // a task of another project: "path\ID" (as MPP shows it)
                let ext_of = |u: u32| self.doc.tasks.iter().find(|x| x.uid == u).and_then(|x| x.external.as_ref());
                let (ext, local): (Vec<Link>, Vec<Link>) = t.predecessors.iter().cloned().partition(|l| ext_of(l.pred_uid).is_some());
                let mut parts: Vec<String> = Vec::new();
                let s = fileio::format_predecessors_with(&local, &ids, self.dur_units(), &self.doc.options, crate::i18n::is_thai());
                if !s.is_empty() {
                    parts.push(s);
                }
                for l in ext {
                    let x = ext_of(l.pred_uid).unwrap();
                    let one: HashMap<u32, usize> = [(l.pred_uid, 1usize)].into_iter().collect();
                    let s = fileio::format_predecessors_with(std::slice::from_ref(&l), &one, self.dur_units(), &self.doc.options, crate::i18n::is_thai());
                    parts.push(format!("{}\\{}{}", x.path, x.id, s.strip_prefix('1').unwrap_or(&s)));
                }
                parts.join(",")
            }
            TCol::Successors => {
                let ids: Vec<String> = self
                    .doc
                    .tasks
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.predecessors.iter().any(|l| l.pred_uid == t.uid))
                    .map(|(k, s)| match &s.external {
                        Some(x) => format!("{}\\{}", x.path, x.id),
                        None => (k + 1).to_string(),
                    })
                    .collect();
                ids.join(",")
            }
            TCol::Resources => self.doc.resource_names(t),
            TCol::Percent => format!("{}%", t.percent_complete),
            TCol::Cost => util::fmt_money(t.cost),
            TCol::TotalSlack => fileio::format_duration(t.total_slack),
            TCol::OutlineNumber => i.map(|i| self.doc.outline_numbers()[i].clone()).unwrap_or_else(|| "0".into()),
            TCol::Notes => t.notes.replace('\n', " "),
            TCol::Uid => t.uid.to_string(),
            TCol::Critical => if t.critical { "Yes".into() } else { "No".into() },
            TCol::Deadline => t.deadline.map(|d| self.fmt_finish(d)).unwrap_or_else(|| "NA".into()),
            TCol::Priority => t.priority.to_string(),
            TCol::ConstraintType => t.constraint_type.label().to_string(),
            TCol::ConstraintDate => t.constraint_date.filter(|_| t.constraint_type.needs_date()).map(|d| self.fmt_date(d)).unwrap_or_else(|| "NA".into()),
            TCol::Wbs => match i {
                None => "0".into(),
                Some(i) if t.wbs.is_empty() => self.doc.outline_numbers()[i].clone(),
                _ => t.wbs.clone(),
            },
            TCol::ActualStart => t.actual_start.map(|a| self.fmt_date(a.date())).unwrap_or_else(|| "NA".into()),
            TCol::ActualFinish => t.actual_finish.map(|a| self.fmt_finish(a.date())).unwrap_or_else(|| "NA".into()),
            TCol::ActualDuration => self.fmt_task_minutes(t, t.actual_minutes(mpd)),
            TCol::RemainingDuration => self.fmt_task_minutes(t, t.remaining_minutes(mpd)),
            TCol::BaselineStart => t.baseline(0).and_then(|b| b.start).map(|a| self.fmt_date(a.date())).unwrap_or_else(|| "NA".into()),
            TCol::BaselineFinish => t.baseline(0).and_then(|b| b.finish).map(|a| self.fmt_finish(a.date())).unwrap_or_else(|| "NA".into()),
            TCol::BaselineDuration => self.fmt_task_minutes(t, t.baseline(0).and_then(|b| b.dur_min).unwrap_or(0)),
            TCol::StartVariance => match (t.baseline(0).and_then(|b| b.start), t.start_dt) {
                (Some(b), Some(s)) => self.fmt_variance(b, s),
                _ => self.fmt_task_minutes(t, 0),
            },
            TCol::FinishVariance => match (t.baseline(0).and_then(|b| b.finish), t.finish_dt) {
                (Some(b), Some(f)) => self.fmt_variance(b, f),
                _ => self.fmt_task_minutes(t, 0),
            },
            TCol::DurationVariance => self.fmt_task_minutes(t, t.baseline(0).and_then(|b| b.dur_min).map(|b| t.dur_minutes(mpd) - b).unwrap_or(0)),
            TCol::PhysicalPercent => format!("{}%", t.physical_percent),
            TCol::Status => match i {
                Some(i) => self.status_text(i).to_string(),
                None if t.percent_complete == 100 => "Complete".into(),
                None if t.current_start(&self.doc.calendar) > self.status_instant() => "Future Task".into(),
                None => {
                    let late = (0..self.doc.tasks.len()).any(|j| !self.doc.tasks[j].is_summary && self.status_text(j) == "Late");
                    if late { "Late".into() } else { "On Schedule".into() }
                }
            },
            TCol::EarlyStart => self.fmt_date(t.start),
            TCol::EarlyFinish => self.fmt_finish(t.finish),
            TCol::LateStart | TCol::LateFinish => {
                let cal = &self.doc.calendar;
                let (st, ft) = (t.start_dt.unwrap_or_else(|| cal.day_start(t.start)), t.finish_dt.unwrap_or_else(|| cal.day_end(t.finish)));
                let k = t.slack_min;
                let shift = |x: chrono::NaiveDateTime, start: bool| -> chrono::NaiveDateTime {
                    if k > 0 {
                        let y = cal.add_work_minutes(x, k);
                        if start { cal.next_work_instant(y) } else { y }
                    } else if k < 0 {
                        cal.sub_work_minutes(x, -k)
                    } else {
                        x
                    }
                };
                if col == TCol::LateStart { self.fmt_date(shift(st, true).date()) } else { self.fmt_finish(shift(ft, false).date()) }
            }
            TCol::FreeSlack => {
                let du = self.dur_units();
                util::fmt_dur(t.free_slack_min.max(0), DurUnit::Days, false, du, &self.doc.options, crate::i18n::is_thai())
            }
            TCol::Work | TCol::ActualWork | TCol::RemainingWork | TCol::BaselineWork | TCol::WorkVariance => {
                let w = t.work_min;
                let actual = (w as f64 * t.percent_complete as f64 / 100.0).round() as i64;
                let base = t.baseline(0).and_then(|b| b.work_min).unwrap_or(0);
                let v = match col {
                    TCol::Work => w,
                    TCol::ActualWork => actual,
                    TCol::RemainingWork => w - actual,
                    TCol::BaselineWork => base,
                    _ => w - base,
                };
                let s = util::fmt_dur(v.abs(), DurUnit::Hours, false, self.dur_units(), &self.doc.options, crate::i18n::is_thai());
                if v < 0 { format!("-{s}") } else { s }
            }
            TCol::PercentWorkComplete => format!("{}%", t.percent_complete),
            TCol::ActualCost => util::fmt_money(t.actual_cost),
            TCol::RemainingCost => util::fmt_money((t.cost - t.actual_cost).max(0.0)),
            TCol::FixedCost => util::fmt_money(t.fixed_cost),
            TCol::FixedCostAccrual => t.fixed_cost_accrual.label().to_string(),
            TCol::BaselineCost => util::fmt_money(t.baseline(0).and_then(|b| b.cost).unwrap_or(0.0)),
            TCol::CostVariance => util::fmt_money(t.cost - t.baseline(0).and_then(|b| b.cost).unwrap_or(0.0)),
            TCol::LevelingDelay => util::fmt_dur(t.level_delay_min.max(0), DurUnit::Days, false, self.dur_units(), &self.doc.options, crate::i18n::is_thai()),
            TCol::Hyperlink => t.hyperlink.clone(),
            TCol::HyperlinkAddress => t.hyperlink_address.clone(),
            TCol::HyperlinkSubAddress => t.hyperlink_sub.clone(),
            TCol::Sprint => t.sprint.and_then(|u| self.doc.sprints.iter().find(|s| s.uid == u)).map(|s| s.name.clone()).unwrap_or_default(),
            TCol::Custom(f) => {
                let du = self.dur_units();
                let opts = self.doc.options.clone();
                crate::custom::format(
                    &self.doc,
                    f,
                    crate::custom::get(&t.custom, f),
                    &|d| self.fmt_date(d),
                    &|m| util::fmt_dur(m, DurUnit::Days, false, du, &opts, crate::i18n::is_thai()),
                )
            }
        }
    }

    /// Text placed into the editor when a cell starts editing.
    pub fn edit_text(&self, i: usize, col: TCol) -> String {
        match col {
            TCol::Percent => self.doc.tasks[i].percent_complete.to_string(),
            TCol::PhysicalPercent => self.doc.tasks[i].physical_percent.to_string(),
            TCol::Notes => self.doc.tasks[i].notes.clone(),
            _ => self.cell_text(i, col),
        }
    }

    /// Apply typed text to a task cell. Returns an error message on invalid input.
    pub fn apply_cell(&mut self, i: usize, col: TCol, text: &str) -> Result<(), String> {
        if self.doc.tasks[i].external.is_some() {
            return Err("This is a task of another project. Open its project to change it (Project > Links Between Projects).".into());
        }
        if matches!(col, TCol::Start | TCol::Finish | TCol::Duration) && self.doc.tasks[i].manual && !self.doc.tasks[i].is_summary {
            let du = self.dur_units();
            let valid = match col {
                TCol::Duration => util::parse_dur(text, du, DurUnit::Days).is_some(),
                _ => self.parse_date(text).is_some(),
            };
            let k = Self::text_slot(col);
            if !valid || self.doc.tasks[i].manual_text[k].as_deref() == Some(text.trim()) {
                // MPP: a manually scheduled task takes any text there ("TBD", or nothing)
                let t = &mut self.doc.tasks[i];
                t.manual_text[k] = Some(text.trim().to_string());
                t.sync_manual_text();
                return Ok(());
            }
        }
        let res = self.apply_cell_value(i, col, text);
        if res.is_ok() && matches!(col, TCol::Start | TCol::Finish | TCol::Duration | TCol::Mode) {
            let t = &mut self.doc.tasks[i];
            if col != TCol::Mode {
                t.manual_text[Self::text_slot(col)] = None;
            }
            t.sync_manual_text();
        }
        res
    }

    fn apply_cell_value(&mut self, i: usize, col: TCol, text: &str) -> Result<(), String> {
        let cal = self.doc.calendar.clone();
        match col {
            TCol::Name => self.doc.tasks[i].name = text.trim().to_string(),
            TCol::Mode => {
                let l = text.to_lowercase();
                self.doc.tasks[i].manual = l.starts_with('m');
            }
            TCol::Duration => {
                if self.doc.tasks[i].is_summary {
                    return Err("You cannot change the duration of a summary task.".into());
                }
                let du = self.dur_units();
                let default = match self.doc.options.duration_units {
                    0 => DurUnit::Minutes,
                    1 => DurUnit::Hours,
                    3 => DurUnit::Weeks,
                    4 => DurUnit::Months,
                    _ => DurUnit::Days,
                };
                let (m, unit, el) = util::parse_dur(text, du, default).ok_or("The duration is not valid. Enter a number such as 5, 5d, 0.5d or 4h.")?;
                let t = &mut self.doc.tasks[i];
                t.set_duration_minutes(m, unit, el, du.mpd);
                t.milestone = m == 0;
                // "5 days?" keeps / sets the estimate flag, a plain value confirms the duration
                t.estimated = text.contains('?');
            }
            TCol::Start => {
                let d = self.parse_date(text).ok_or("The date you entered isn't supported.")?;
                if self.doc.tasks[i].is_summary {
                    return Err("Summary task dates are calculated from their subtasks.".into());
                }
                let t = &mut self.doc.tasks[i];
                t.no_dates = false;
                if t.manual && t.manual_text[1].is_none() && t.manual_text[2].is_some() {
                    // a finish entered, no duration: the duration runs from the new start to it
                    let f = t.finish_dt.unwrap_or_else(|| cal.day_end(t.finish));
                    let s = cal.day_start(d);
                    let m = cal.work_minutes(s, f.max(s));
                    let unit = t.dur_unit;
                    t.set_duration_minutes(m, unit, false, cal.minutes_per_day());
                    t.start = d;
                    t.start_dt = Some(s);
                } else if t.manual {
                    t.start = d;
                    t.start_dt = Some(cal.day_start(d));
                } else {
                    // MPP sets a Start No Earlier Than constraint
                    t.set_snet(d);
                    t.start = d;
                }
            }
            TCol::Deadline => {
                let v = text.trim();
                self.doc.tasks[i].deadline = if v.is_empty() || v.eq_ignore_ascii_case("NA") { None } else { Some(self.parse_date(v).ok_or("The date you entered isn't supported.")?) };
            }
            TCol::Priority => {
                let v: u32 = text.trim().parse().map_err(|_| "Enter a priority between 0 and 1000.")?;
                if v > 1000 {
                    return Err("Enter a priority between 0 and 1000.".into());
                }
                self.doc.tasks[i].priority = v as u16;
            }
            TCol::ConstraintType => {
                let l = text.trim().to_lowercase();
                let ct = ConstraintType::ALL.iter().copied().find(|c| c.label().to_lowercase().starts_with(&l) || l == c.label().to_lowercase()).ok_or("Enter a constraint type such as As Soon As Possible or Must Start On.")?;
                let t = &mut self.doc.tasks[i];
                t.constraint_type = ct;
                if ct.needs_date() && t.constraint_date.is_none() {
                    // MPP takes the task's current start (finish for a finish constraint)
                    let finish_kind = matches!(ct, ConstraintType::Fnet | ConstraintType::Fnlt | ConstraintType::Mfo);
                    t.constraint_date = Some(if finish_kind { t.finish } else { t.start });
                    t.constraint_dt = if finish_kind { t.finish_dt } else { t.start_dt };
                }
                if !ct.needs_date() {
                    t.constraint_date = None;
                    t.constraint_dt = None;
                }
            }
            TCol::ConstraintDate => {
                let v = text.trim();
                let t = &mut self.doc.tasks[i];
                if v.is_empty() || v.eq_ignore_ascii_case("NA") {
                    t.clear_constraint();
                } else {
                    let d = self.parse_date(v).ok_or("The date you entered isn't supported.")?;
                    let t = &mut self.doc.tasks[i];
                    t.constraint_date = Some(d);
                    t.constraint_dt = None;
                    if !t.constraint_type.needs_date() {
                        t.constraint_type = ConstraintType::Snet;
                    }
                }
            }
            TCol::Wbs => self.doc.tasks[i].wbs = text.trim().to_string(),
            TCol::Finish => {
                let d = self.parse_date(text).ok_or("The date you entered isn't supported.")?;
                if self.doc.tasks[i].is_summary {
                    return Err("Summary task dates are calculated from their subtasks.".into());
                }
                let t = &mut self.doc.tasks[i];
                if t.manual && t.manual_text[0].is_some() {
                    // no start entered (the standard finish-only task): the start follows from the
                    // duration
                    t.no_dates = false;
                    let f = cal.day_end(d);
                    let s = cal.sub_work_minutes(f, t.dur_minutes(cal.minutes_per_day()));
                    t.start = s.date();
                    t.start_dt = Some(s);
                    return Ok(());
                }
                if d < t.start {
                    return Err("The finish date cannot be earlier than the start date.".into());
                }
                t.no_dates = false;
                if t.manual {
                    // a manual task keeps its start; the duration is the working time up to the
                    // end of the finish day, in the task's duration unit
                    let s = t.start_dt.unwrap_or_else(|| cal.day_start(t.start));
                    let m = cal.work_minutes(s, cal.day_end(d)).max(if t.duration == 0 { 0 } else { 1 });
                    let mpd = cal.minutes_per_day();
                    let unit = t.dur_unit;
                    t.set_duration_minutes(m, unit, false, mpd);
                } else {
                    // MPP: a finish typed on an auto-scheduled task is a Finish No Earlier Than constraint
                    t.constraint_type = ConstraintType::Fnet;
                    t.constraint_date = Some(d);
                    t.constraint_dt = None;
                }
            }
            TCol::Predecessors => {
                // "path\ID" (the standard way): a task of another project; its external task joins
                // this project above the task
                let mut local: Vec<String> = Vec::new();
                let mut ext: Vec<(String, u32, String)> = Vec::new();
                for raw in text.split([',', ';']) {
                    let tok = raw.trim();
                    if let Some((path, rest)) = tok.rsplit_once('\\') {
                        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                        if let Ok(id) = digits.parse::<u32>() {
                            ext.push((path.trim().to_string(), id, rest[digits.len()..].to_string()));
                            continue;
                        }
                    }
                    if !tok.is_empty() {
                        local.push(tok.to_string());
                    }
                }
                let me_uid = self.doc.tasks[i].uid;
                let mut ext_links: Vec<Link> = Vec::new();
                for (path, id, suffix) in ext {
                    if !std::path::Path::new(&path).exists() {
                        return Err(format!("The project {path} cannot be found."));
                    }
                    let at = self.doc.index_of_uid(me_uid).unwrap_or(i);
                    let g = self.external_ghost(&path, id, at);
                    let one: HashMap<usize, u32> = [(1usize, g)].into_iter().collect();
                    ext_links.extend(util::parse_predecessors(&format!("1{suffix}"), &one, self.dur_units())?);
                }
                let i = self.doc.index_of_uid(me_uid).unwrap_or(i);
                let (_, id_to_uid) = self.id_maps();
                let mut links = util::parse_predecessors(&local.join(","), &id_to_uid, self.dur_units())?;
                links.extend(ext_links);
                for l in &links {
                    if let Some(pi) = self.doc.index_of_uid(l.pred_uid) {
                        if self.is_outline_relative(pi, i) {
                            return Err("You cannot link a summary task to one of its subtasks.".into());
                        }
                    }
                }
                let me = self.doc.tasks[i].uid;
                let old = std::mem::take(&mut self.doc.tasks[i].predecessors);
                for l in &links {
                    if schedule::creates_cycle(&self.doc, l.pred_uid, me) {
                        self.doc.tasks[i].predecessors = old;
                        return Err("This link would create a circular relationship.".into());
                    }
                }
                self.doc.tasks[i].predecessors = links;
            }
            TCol::Resources => {
                let mut uids = Vec::new();
                for raw in text.split([',', ';']) {
                    let name = raw.split('[').next().unwrap_or("").trim();
                    if name.is_empty() {
                        continue;
                    }
                    let uid = match self.doc.resource_by_name(name) {
                        Some(r) => r.uid,
                        None => {
                            let u = self.doc.alloc_res_uid();
                            self.doc.resources.push(Resource::new(u, name));
                            u
                        }
                    };
                    if !uids.contains(&uid) {
                        uids.push(uid);
                    }
                }
                self.doc.tasks[i].resources = uids;
            }
            TCol::Percent => {
                let v: f64 = text.trim().trim_end_matches('%').trim().parse().map_err(|_| "Enter a percentage between 0 and 100.")?;
                if !(0.0..=100.0).contains(&v) {
                    return Err("Enter a percentage between 0 and 100.".into());
                }
                let pct = v.round() as u8;
                if self.doc.tasks[i].is_summary {
                    // a summary's % complete is passed on to its subtasks
                    let end = self.doc.children_end(i);
                    for j in i + 1..end {
                        if !self.doc.tasks[j].is_summary {
                            self.doc.tasks[j].set_percent(pct, &cal);
                        }
                    }
                } else {
                    self.doc.tasks[i].set_percent(pct, &cal);
                }
            }
            TCol::PhysicalPercent => {
                let v: f64 = text.trim().trim_end_matches('%').trim().parse().map_err(|_| "Enter a percentage between 0 and 100.")?;
                if !(0.0..=100.0).contains(&v) {
                    return Err("Enter a percentage between 0 and 100.".into());
                }
                self.doc.tasks[i].physical_percent = v.round() as u8;
            }
            TCol::ActualStart | TCol::ActualFinish => {
                if self.doc.tasks[i].is_summary {
                    return Err("Summary task dates are calculated from their subtasks.".into());
                }
                let v = text.trim();
                let d = if v.is_empty() || v.eq_ignore_ascii_case("NA") { None } else { Some(self.parse_date(v).ok_or("The date you entered isn't supported.")?) };
                let t = &mut self.doc.tasks[i];
                if col == TCol::ActualStart {
                    t.set_actual_start(d.map(|d| cal.day_start(d)));
                } else {
                    let af = d.map(|d| cal.day_end(d));
                    if let (Some(af), Some(a)) = (af, t.actual_start) {
                        if af < a {
                            return Err("The actual finish date can't be earlier than the actual start date.".into());
                        }
                    }
                    t.set_actual_finish(af, &cal);
                }
            }
            TCol::ActualDuration | TCol::RemainingDuration => {
                if self.doc.tasks[i].is_summary {
                    return Err("You cannot change the duration of a summary task.".into());
                }
                let du = self.dur_units();
                let unit = self.doc.tasks[i].dur_unit;
                let (m, _, _) = util::parse_dur(text, du, unit).ok_or("The duration is not valid. Enter a number such as 5, 5d, 0.5d or 4h.")?;
                let t = &mut self.doc.tasks[i];
                if col == TCol::ActualDuration {
                    t.set_actual_minutes(m, &cal);
                } else {
                    t.set_remaining_minutes(m, &cal);
                }
            }
            TCol::BaselineStart | TCol::BaselineFinish | TCol::BaselineDuration => {
                let v = text.trim();
                let na = v.is_empty() || v.eq_ignore_ascii_case("NA");
                let mut b = self.doc.tasks[i].baseline(0).cloned().unwrap_or_default();
                match col {
                    TCol::BaselineStart => b.start = if na { None } else { Some(cal.day_start(self.parse_date(v).ok_or("The date you entered isn't supported.")?)) },
                    TCol::BaselineFinish => b.finish = if na { None } else { Some(cal.day_end(self.parse_date(v).ok_or("The date you entered isn't supported.")?)) },
                    _ => {
                        let unit = self.doc.tasks[i].dur_unit;
                        b.dur_min = if na { None } else { Some(util::parse_dur(v, self.dur_units(), unit).ok_or("The duration is not valid. Enter a number such as 5, 5d, 0.5d or 4h.")?.0) };
                    }
                }
                let t = &mut self.doc.tasks[i];
                if b.start.is_none() && b.finish.is_none() && b.dur_min.is_none() {
                    t.baselines.retain(|x| x.number != 0);
                } else {
                    t.set_baseline(b);
                }
            }
            TCol::Work => {
                if self.doc.tasks[i].is_summary {
                    return Err("You cannot change the work of a summary task.".into());
                }
                let (m, _, _) = util::parse_dur(text, self.dur_units(), DurUnit::Hours).ok_or("The work is not valid. Enter a number such as 8, 8h or 2d.")?;
                self.set_task_work(i, m)?;
            }
            TCol::FixedCost => self.doc.tasks[i].fixed_cost = Self::parse_money(text.trim()).ok_or("The cost is not valid.")?,
            TCol::FixedCostAccrual => {
                let l = text.trim().to_lowercase();
                self.doc.tasks[i].fixed_cost_accrual = Accrue::ALL.iter().copied().find(|a| !l.is_empty() && a.label().to_lowercase().starts_with(&l)).ok_or("Enter Start, Prorated or End.")?;
            }
            TCol::LevelingDelay => {
                let v = text.trim();
                let m = if v.is_empty() { 0 } else { util::parse_dur(v, self.dur_units(), DurUnit::Days).ok_or("The duration is not valid.")?.0 };
                self.doc.tasks[i].level_delay_min = m;
            }
            TCol::BaselineCost | TCol::BaselineWork => {
                let mut b = self.doc.tasks[i].baseline(0).cloned().unwrap_or_default();
                if col == TCol::BaselineCost {
                    b.cost = Some(Self::parse_money(text.trim()).ok_or("The cost is not valid.")?);
                } else {
                    b.work_min = Some(util::parse_dur(text, self.dur_units(), DurUnit::Hours).ok_or("The work is not valid.")?.0);
                }
                self.doc.tasks[i].set_baseline(b);
            }
            TCol::Notes => self.doc.tasks[i].notes = text.to_string(),
            TCol::Hyperlink => self.doc.tasks[i].hyperlink = text.trim().to_string(),
            TCol::HyperlinkAddress => self.doc.tasks[i].hyperlink_address = text.trim().to_string(),
            TCol::HyperlinkSubAddress => self.doc.tasks[i].hyperlink_sub = text.trim().to_string(),
            TCol::Sprint => {
                let name = text.trim();
                self.doc.tasks[i].sprint = if name.is_empty() {
                    None
                } else {
                    Some(self.doc.sprints.iter().find(|s| s.name.eq_ignore_ascii_case(name)).map(|s| s.uid).ok_or("There is no sprint with this name (Project > Manage Sprints).")?)
                };
            }
            TCol::Custom(f) => {
                let d = crate::custom::def(&self.doc, f, false).cloned();
                if d.as_ref().map(|d| !d.formula.trim().is_empty()).unwrap_or(false) {
                    return Err("This field is calculated by a formula.".into());
                }
                if let Some(d) = d.filter(|d| d.restrict && !d.values.is_empty() && !text.trim().is_empty()) {
                    if !d.values.iter().any(|(v, _)| v.trim().eq_ignore_ascii_case(text.trim())) {
                        return Err("The value is not in the field's value list.".into());
                    }
                }
                let du = self.dur_units();
                let v = crate::custom::parse_value(
                    f,
                    text,
                    &|s| self.parse_date(s),
                    &|s| util::parse_dur(s, du, DurUnit::Days).map(|(m, _, _)| m),
                    &|d| cal.day_start(d),
                )?;
                crate::custom::set(&mut self.doc.tasks[i].custom, f, v);
            }
            _ => {}
        }
        Ok(())
    }

    /// Commit the currently open cell editor (if any).
    pub fn commit_edit(&mut self) {
        let Some(ed) = self.editing.take() else { return };
        match self.view {
            View::ResourceSheet => self.commit_res_edit(ed),
            _ => self.commit_task_edit(ed),
        }
    }

    fn commit_task_edit(&mut self, ed: EditState) {
        let Some(&(col, _)) = self.task_cols.get(ed.col) else { return };
        let vis = self.visible_rows();
        let idx = vis.get(ed.row).copied();
        match idx {
            Some(i) => {
                if self.edit_text(i, col) == ed.text {
                    return;
                }
                let before = self.doc.clone();
                match self.apply_cell(i, col, &ed.text) {
                    Ok(()) => {
                        self.undo.push(before);
                        self.redo.clear();
                        self.dirty = true;
                        self.recalc();
                    }
                    Err(e) => {
                        self.doc = before;
                        self.show_error(e);
                    }
                }
            }
            None => {
                // typing in an empty row creates a new task at the end
                if ed.text.trim().is_empty() {
                    return;
                }
                self.checkpoint();
                let level = self.doc.tasks.last().map(|t| t.outline_level).unwrap_or(1);
                let name = if col == TCol::Name { ed.text.trim().to_string() } else { String::new() };
                let mut t = self.doc.new_task(&name);
                t.outline_level = level;
                t.set_nothing_entered();
                self.doc.tasks.push(t);
                let i = self.doc.tasks.len() - 1;
                if col != TCol::Name {
                    if let Err(e) = self.apply_cell(i, col, &ed.text) {
                        self.show_error(e);
                    }
                }
                self.recalc();
                if let Some(r) = self.row_of_task(i) {
                    self.sel_row = r;
                    self.sel_anchor = r;
                }
            }
        }
    }

    pub fn res_cell_text(&self, i: usize, col: RCol) -> String {
        let r = &self.doc.resources[i];
        match col {
            RCol::Name => r.name.clone(),
            RCol::Type => r.rtype.label().into(),
            RCol::Material => r.material_label.clone(),
            RCol::Initials => r.initials.clone(),
            RCol::Group => r.group.clone(),
            RCol::Max => if r.rtype == ResourceType::Work { format!("{}%", r.max_units) } else { String::new() },
            RCol::StdRate => match r.rtype {
                ResourceType::Work => format!("{}/hr", util::fmt_money(r.std_rate)),
                ResourceType::Material => util::fmt_money(r.std_rate),
                ResourceType::Cost => String::new(),
            },
            RCol::OvtRate => if r.rtype == ResourceType::Work { format!("{}/hr", util::fmt_money(r.ovt_rate)) } else { String::new() },
            RCol::CostUse => if r.rtype == ResourceType::Cost { String::new() } else { util::fmt_money(r.cost_per_use) },
            RCol::Calendar => if r.rtype == ResourceType::Work { self.doc.calendar.name.clone() } else { String::new() },
            RCol::AccrueAt => r.accrue_at.label().into(),
            RCol::Code => r.code.clone(),
            RCol::Work => match r.rtype {
                ResourceType::Work => util::fmt_dur(r.work_min, DurUnit::Hours, false, self.dur_units(), &self.doc.options, crate::i18n::is_thai()),
                _ => String::new(),
            },
            RCol::Cost => util::fmt_money(r.cost),
        }
    }

    fn parse_money(s: &str) -> Option<f64> {
        let cleaned: String = s.chars().filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
        if cleaned.is_empty() {
            return Some(0.0);
        }
        cleaned.parse().ok()
    }

    pub fn res_edit_text(&self, i: usize, col: RCol) -> String {
        let r = &self.doc.resources[i];
        match col {
            RCol::Max => r.max_units.to_string(),
            RCol::StdRate => r.std_rate.to_string(),
            RCol::OvtRate => r.ovt_rate.to_string(),
            RCol::CostUse => r.cost_per_use.to_string(),
            _ => self.res_cell_text(i, col),
        }
    }

    pub fn apply_res_cell(&mut self, i: usize, col: RCol, text: &str) -> Result<(), String> {
        let text_t = text.trim();
        match col {
            RCol::Name => {
                if text_t.is_empty() {
                    return Err("Resource name cannot be empty.".into());
                }
                if text_t.contains([',', '[', ']']) {
                    return Err("Resource names cannot contain commas or square brackets.".into());
                }
                if self.doc.resources.iter().enumerate().any(|(k, r)| k != i && r.name.eq_ignore_ascii_case(text_t)) {
                    return Err("A resource with this name already exists.".into());
                }
                self.doc.resources[i].name = text_t.to_string();
            }
            RCol::Type => {
                let l = text_t.to_lowercase();
                self.doc.resources[i].rtype = if l.starts_with('m') {
                    ResourceType::Material
                } else if l.starts_with('c') {
                    ResourceType::Cost
                } else {
                    ResourceType::Work
                };
            }
            RCol::Material => self.doc.resources[i].material_label = text_t.to_string(),
            RCol::Initials => self.doc.resources[i].initials = text_t.to_string(),
            RCol::Group => self.doc.resources[i].group = text_t.to_string(),
            RCol::Max => {
                let v: f64 = text_t.trim_end_matches('%').trim().parse().map_err(|_| "Enter a number for Max. Units.")?;
                if v <= 0.0 {
                    return Err("Max. Units must be greater than 0%.".into());
                }
                self.doc.resources[i].max_units = v;
            }
            RCol::StdRate => self.doc.resources[i].std_rate = Self::parse_money(text_t).ok_or("Invalid rate")?,
            RCol::OvtRate => self.doc.resources[i].ovt_rate = Self::parse_money(text_t).ok_or("Invalid rate")?,
            RCol::CostUse => self.doc.resources[i].cost_per_use = Self::parse_money(text_t).ok_or("Invalid cost")?,
            RCol::Calendar | RCol::Work | RCol::Cost => {}
            RCol::AccrueAt => {
                let l = text_t.to_lowercase();
                self.doc.resources[i].accrue_at = Accrue::ALL.iter().copied().find(|a| a.label().to_lowercase().starts_with(&l) && !l.is_empty()).ok_or("Enter Start, Prorated or End.")?;
            }
            RCol::Code => self.doc.resources[i].code = text_t.to_string(),
        }
        Ok(())
    }

    fn commit_res_edit(&mut self, ed: EditState) {
        let Some(&(col, _)) = self.res_cols.get(ed.col) else { return };
        if ed.row < self.doc.resources.len() {
            if self.res_edit_text(ed.row, col) == ed.text {
                return;
            }
            let before = self.doc.clone();
            match self.apply_res_cell(ed.row, col, &ed.text) {
                Ok(()) => {
                    self.undo.push(before);
                    self.redo.clear();
                    self.dirty = true;
                    self.recalc();
                }
                Err(e) => {
                    self.doc = before;
                    self.show_error(e);
                }
            }
        } else if !ed.text.trim().is_empty() {
            let before = self.doc.clone();
            let uid = self.doc.alloc_res_uid();
            let name = if col == RCol::Name { ed.text.trim().to_string() } else { format!("Resource {uid}") };
            self.doc.resources.push(Resource::new(uid, &name));
            let i = self.doc.resources.len() - 1;
            if col != RCol::Name {
                if let Err(e) = self.apply_res_cell(i, col, &ed.text) {
                    self.doc = before;
                    self.show_error(e);
                    return;
                }
            }
            self.undo.push(before);
            self.redo.clear();
            self.dirty = true;
            self.recalc();
            self.res_sel_row = i;
        }
    }

    pub fn delete_resource(&mut self) {
        if self.res_sel_row >= self.doc.resources.len() {
            return;
        }
        self.checkpoint();
        let r = self.doc.resources.remove(self.res_sel_row);
        for t in self.doc.tasks.iter_mut() {
            t.resources.retain(|u| *u != r.uid);
        }
        self.recalc();
    }

    pub fn add_resource(&mut self, rtype: ResourceType) {
        self.commit_edit();
        self.checkpoint();
        let uid = self.doc.alloc_res_uid();
        let mut r = Resource::new(uid, &format!("Resource {uid}"));
        r.rtype = rtype;
        if rtype != ResourceType::Work {
            r.max_units = 100.0;
        }
        self.doc.resources.push(r);
        self.view = View::ResourceSheet;
        self.res_sel_row = self.doc.resources.len() - 1;
        self.res_sel_col = 0;
        self.editing = Some(EditState { row: self.res_sel_row, col: 0, text: format!("Resource {uid}"), just_opened: true, select_all: true });
    }

    // ------------------------------------------------------------ zoom

    /// Zoom In / Zoom Out stops (px per day), one per MPP timescale preset:
    /// Years/Half years … Weeks/Days … Days/Hours. Ctrl+wheel and the slider zoom continuously between them.
    pub const ZOOMS: [f32; 14] = [0.08, 0.15, 0.3, 0.6, 1.2, 2.4, 4.0, 7.0, 12.0, 24.0, 48.0, 120.0, 300.0, 720.0];
    pub const MIN_PPD: f32 = 0.06;
    pub const MAX_PPD: f32 = 960.0;

    /// Set the timescale zoom, keeping the date under screen x `anchor` (default: chart centre) in place.
    pub fn set_zoom(&mut self, ppd: f32, anchor: Option<f32>) {
        let ppd = ppd.clamp(Self::MIN_PPD, Self::MAX_PPD);
        // zooming leaves a timescale read from the file for the standard automatic tiers (the Usage
        // views always use automatic tiers and leave the Gantt's timescale as it is)
        if (ppd - self.px_per_day).abs() > 1e-4 && !self.usage_scale {
            self.doc.formatting.timescale.auto = true;
        }
        let (left, width) = self.chart_view;
        let ax = anchor.map(|x| x - left).unwrap_or(width / 2.0).max(0.0);
        let day = (self.scroll_x + ax) / self.px_per_day;
        self.px_per_day = ppd;
        self.scroll_x = (day * ppd - ax).max(0.0);
    }

    /// Swap in the zoom and scroll of the view family now showing (Gantt-type or Usage), the way
    /// each MPP view keeps its own timescale. The Usage views start at Weeks over Days with
    /// day columns wide enough for hours of work, at the date the Gantt was showing.
    pub fn sync_view_scale(&mut self) {
        let want = self.view.is_usage();
        if want == self.usage_scale {
            return;
        }
        let cur = (self.px_per_day, self.scroll_x);
        let (ppd, sx) = self.other_scale.take().unwrap_or_else(|| {
            let ppd = Self::USAGE_PPD;
            (ppd, (cur.1 / cur.0.max(1e-3) * ppd).max(0.0))
        });
        self.px_per_day = ppd;
        self.scroll_x = sx;
        self.other_scale = Some(cur);
        self.usage_scale = want;
    }

    /// Usage views' first zoom: Weeks over Days, 36 px a day.
    pub const USAGE_PPD: f32 = 36.0;

    /// Zoom In (`dir > 0`) / Zoom Out to the next preset level.
    pub fn zoom(&mut self, dir: i32) {
        let cur = self.px_per_day;
        let next = if dir > 0 {
            Self::ZOOMS.iter().copied().find(|z| *z > cur * 1.01).unwrap_or(Self::MAX_PPD)
        } else {
            Self::ZOOMS.iter().rev().copied().find(|z| *z < cur * 0.99).unwrap_or(Self::MIN_PPD)
        };
        self.set_zoom(next, None);
    }

    /// Continuous zoom by `factor` around screen x `anchor` (Ctrl+wheel, touchpad pinch).
    pub fn zoom_by(&mut self, factor: f32, anchor: Option<f32>) {
        self.set_zoom(self.px_per_day * factor, anchor);
    }

    /// Slider position 0..1 on a logarithmic scale between the zoom limits.
    pub fn zoom_fraction(&self) -> f32 {
        (self.px_per_day / Self::MIN_PPD).ln() / (Self::MAX_PPD / Self::MIN_PPD).ln()
    }

    pub fn set_zoom_fraction(&mut self, f: f32) {
        self.set_zoom(Self::MIN_PPD * (Self::MAX_PPD / Self::MIN_PPD).powf(f.clamp(0.0, 1.0)), None);
    }

    pub fn gantt_origin(&self) -> NaiveDate {
        use chrono::Datelike;
        let first = self.doc.tasks.iter().map(|t| t.start).min().unwrap_or(self.doc.start).min(self.doc.start);
        let d = first - chrono::Duration::days(7);
        // from the start of that year, so a saved view can open at the start of its timescale
        // unit (MPP shows "Half 2, 2568" from 1 Jul for a 19 Dec start)
        let d = NaiveDate::from_ymd_opt(d.year(), 1, 1).unwrap_or(d);
        // align to Sunday like MPP
        d - chrono::Duration::days(d.weekday().num_days_from_sunday() as i64)
    }

    pub fn zoom_to_range(&mut self, a: NaiveDate, b: NaiveDate, width: f32) {
        // fit the range plus a small margin on both sides, like View > Entire Project
        let days = ((b - a).num_days() + 1).max(1) as f32;
        let width = if width > 0.0 { width } else { self.chart_view.1 };
        self.px_per_day = ((width - 24.0).max(60.0) / (days * 1.04)).clamp(Self::MIN_PPD, Self::MAX_PPD);
        self.scroll_x = ((a - self.gantt_origin()).num_days() as f32 * self.px_per_day - 12.0).max(0.0);
    }

    pub fn scroll_to_task(&mut self) {
        if let Some(i) = self.current_task() {
            let d = self.doc.tasks[i].start;
            self.scroll_x = (((d - self.gantt_origin()).num_days() as f32) - 2.0).max(0.0) * self.px_per_day;
        }
    }

    // ------------------------------------------------------------ files

    fn request(&mut self, action: PendingAction) {
        self.commit_edit();
        if self.dirty {
            self.pending = Some(action);
            self.dialog = Some(Dialog::ConfirmSave);
        } else {
            self.run_pending(action);
        }
    }

    pub fn run_pending(&mut self, action: PendingAction) {
        match action {
            PendingAction::New => self.new_doc(),
            PendingAction::Open => {
                if let Some(p) = rfd::FileDialog::new()
                    .set_title("Open")
                    .add_filter("All Projects", &["neramit", "mpp", "mpt", "xml"])
                    .add_filter("Neramit Planner (*.neramit)", &["neramit"])
                    .add_filter("XML Format (*.xml)", &["xml"])
                    .add_filter("MPP (*.mpp;*.mpt)", &["mpp", "mpt"])
                    .add_filter("All Files", &["*"])
                    .pick_file()
                {
                    self.open_path(p);
                }
            }
            PendingAction::OpenPath(p) => self.open_path(p),
            PendingAction::Exit => {
                self.confirm_close_ok = true;
            }
        }
    }

    pub fn cmd_new(&mut self) {
        self.request(PendingAction::New);
    }
    pub fn cmd_open(&mut self) {
        self.request(PendingAction::Open);
    }
    pub fn cmd_open_path(&mut self, p: PathBuf) {
        self.request(PendingAction::OpenPath(p));
    }
    pub fn cmd_exit(&mut self) {
        self.request(PendingAction::Exit);
    }

    fn new_doc(&mut self) {
        self.doc = Project::new();
        self.doc.localize_new();
        self.path = None;
        self.mpp_source = None;
        self.dirty = false;
        self.undo.clear();
        self.redo.clear();
        self.reset_view();
        // MPP opens a new plan at its start date (today), not at the start of the year
        self.scroll_x = (((self.doc.start - self.gantt_origin()).num_days() as f32) - 2.0).max(0.0) * self.px_per_day;
        self.other_scale = None;
        self.status = "Ready".into();
    }

    fn reset_view(&mut self) {
        // another project: its windows go
        self.windows.clear();
        self.win_num = 1;
        self.win_next = 2;
        self.win_hidden = false;
        self.arranged = false;
        self.sel_row = 0;
        self.sel_anchor = 0;
        self.res_sel_row = 0;
        self.scroll_y = 0.0;
        self.scroll_x = 0.0;
        self.editing = None;
        self.backstage = None;
    }

    pub fn open_path(&mut self, p: PathBuf) {
        match fileio::load(&p) {
            Ok(doc) => {
                self.doc = doc;
                self.mpp_source = if is_mpp(&p) { std::fs::read(&p).ok().map(|b| (p.clone(), b)) } else { None };
                self.recalc();
                // Save writes back in the same format the file was opened from (.neramit or .xml)
                self.path = Some(p.clone());
                self.dirty = false;
                self.undo.clear();
                self.redo.clear();
                self.reset_view();
                // show the first task in the Gantt chart
                if let Some(first) = self.doc.tasks.iter().map(|t| t.start).min() {
                    self.scroll_x = (((first - self.gantt_origin()).num_days() as f32) - 2.0).max(0.0) * self.px_per_day;
                }
                self.add_recent(&p);
                // the .mpp file's own Gantt Chart view (table, bar and text styles, timescale ...)
                self.pending_view = self.doc.saved_view.take();
                self.pending_view_fonts_loaded = false;
                self.id_w = crate::grid::ID_W;
                // like the standard status bar: short, the full path is in File > Info
                self.status = crate::i18n::trf("Opened {}", &[&p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()]);
                // links to other projects that changed: MPP shows Links Between Projects
                if self.doc.tasks.iter().any(|t| t.external.is_some()) && self.external_links().iter().any(|r| r.changed) {
                    self.dialog = Some(crate::dialogs::Dialog::ProjectLinks(0, 0));
                }
                // a project sharing a resource pool takes the pool's resources as they are now
                if let Some(rp) = self.doc.resource_pool.clone() {
                    match self.refresh_pool() {
                        Ok((a, u)) if a + u > 0 => {
                            self.undo.clear();
                            self.status = crate::i18n::trf("Resources refreshed from the pool {} ({} added, {} updated)", &[&rp.path, &a.to_string(), &u.to_string()]);
                        }
                        Ok(_) => {}
                        Err(_) => self.status = crate::i18n::trf("The resource pool {} cannot be opened: this project keeps its own copies", &[&rp.path]),
                    }
                }
            }
            Err(e) => self.show_error(e),
        }
    }

    pub fn cmd_save(&mut self) -> bool {
        self.commit_edit();
        match self.path.clone() {
            // the original .mpp is never overwritten: Save offers a new .mpp next to it (or .xml / .neramit)
            Some(p) if is_mpp(&p) => {
                self.status = crate::i18n::tr("Saving a .mpp file as a new file - the original .mpp is kept").into();
                self.cmd_save_as()
            }
            Some(p) => self.save_to(p),
            None => self.cmd_save_as(),
        }
    }

    pub fn cmd_save_as(&mut self) -> bool {
        self.commit_edit();
        let default_name = self
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| if self.doc.title.is_empty() { "Project1".into() } else { self.doc.title.clone() });
        // a project opened from .mpp is saved as a new .mpp by default (the opened file is kept)
        let from_mpp = self.path.as_ref().is_some_and(|p| is_mpp(p));
        let dialog = rfd::FileDialog::new().set_title("Save As");
        let dialog = if from_mpp {
            dialog
                .set_file_name(format!("{default_name} (edited).mpp"))
                .add_filter("Project (*.mpp)", &["mpp"])
                .add_filter("XML Format (*.xml)", &["xml"])
                .add_filter("Neramit Planner (*.neramit)", &["neramit"])
        } else {
            dialog
                .set_file_name(format!("{default_name}.neramit"))
                .add_filter("Neramit Planner (*.neramit)", &["neramit"])
                .add_filter("Project (*.mpp)", &["mpp"])
                .add_filter("XML Format (*.xml)", &["xml"])
        };
        let picked = dialog
            .add_filter("Excel Workbook (*.xlsx)", &["xlsx"])
            .add_filter("PDF Files (*.pdf)", &["pdf"])
            .add_filter("CSV (Comma delimited) (*.csv)", &["csv"])
            .save_file();
        match picked {
            Some(p) => {
                let ext = p.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
                if matches!(ext.as_str(), "csv" | "pdf" | "xlsx") {
                    // exports: keep the current document path
                    self.export_to(&p);
                    return false;
                }
                self.save_to(p)
            }
            None => false,
        }
    }

    /// Save As .mpp: a copy of the .mpp the project was opened from, with the edits written into
    /// it. The opened file itself is never overwritten.
    fn save_mpp_copy(&mut self, p: PathBuf) -> bool {
        if !self.doc.controls.is_empty() {
            self.show_error("Native BOQ, dated progress and posted costs would be lost in .mpp. Choose Save As .neramit instead.");
            return false;
        }
        let Some((src_path, src)) = self.mpp_source.clone() else {
            // a project that was not opened from a .mpp: a new .mpp from the standard template
            return match crate::mppt::save_new_mpp(&self.doc).and_then(|b| std::fs::write(&p, b).map_err(|e| format!("Cannot write the file: {e}"))) {
                Ok(()) => {
                    self.dirty = false;
                    self.add_recent(&p);
                    self.status = crate::i18n::trf("Saved {}", &[&p.display().to_string()]);
                    self.backstage = None;
                    true
                }
                Err(e) => {
                    self.show_error(e);
                    false
                }
            };
        };
        let same = |a: &std::path::Path, b: &std::path::Path| match (a.canonicalize(), b.canonicalize()) {
            (Ok(x), Ok(y)) => x == y,
            _ => a == b,
        };
        if same(&src_path, &p) {
            self.show_error("Choose a new file name: the original .mpp file is kept unchanged.");
            return false;
        }
        match crate::mppt::save_mpp_at(&src, &self.doc, Some(&p)).and_then(|b| std::fs::write(&p, b).map_err(|e| format!("Cannot write the file: {e}"))) {
            Ok(()) => {
                self.dirty = false;
                self.add_recent(&p);
                self.status = crate::i18n::trf("Saved {}", &[&p.display().to_string()]);
                self.backstage = None;
                true
            }
            Err(e) => {
                self.show_error(e);
                false
            }
        }
    }

    pub fn cmd_export(&mut self, ext: &str) {
        self.commit_edit();
        let (label, filter) = match ext {
            "xml" => ("XML Format (*.xml)", "xml"),
            "pdf" => ("PDF Files (*.pdf)", "pdf"),
            "xlsx" => ("Excel Workbook (*.xlsx)", "xlsx"),
            _ => ("CSV (Comma delimited) (*.csv)", "csv"),
        };
        let stem = self.path.as_ref().and_then(|p| p.file_stem()).map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Project1".into());
        if let Some(p) = rfd::FileDialog::new().set_title("Export").set_file_name(format!("{stem}.{filter}")).add_filter(label, &[filter]).save_file() {
            self.export_to(&p);
        }
    }

    /// Write an export (PDF, Excel, CSV or XML) without changing the open document's path.
    pub fn export_to(&mut self, p: &std::path::Path) {
        let ext = p.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        let result = match ext.as_str() {
            "pdf" => crate::export::write_pdf(p, &self.export_data(true)),
            "xlsx" => crate::export::write_xlsx(p, &self.export_data(false)),
            _ => fileio::save(p, &self.doc),
        };
        match result {
            Ok(()) => {
                self.status = crate::i18n::trf("Exported {}", &[&p.display().to_string()]);
                if ext == "pdf" || ext == "xlsx" {
                    // open the result like MPP does after "Save As PDF"
                    let _ = std::process::Command::new("explorer").arg(p).spawn();
                }
            }
            Err(e) => self.show_error(e),
        }
    }

    /// Command-line export (no file dialog, no viewer).
    pub fn export_quiet(&mut self, p: &std::path::Path) {
        let ext = p.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        let result = match ext.as_str() {
            "pdf" => crate::export::write_pdf(p, &self.export_data(true)),
            "xlsx" => crate::export::write_xlsx(p, &self.export_data(false)),
            _ => fileio::save(p, &self.doc),
        };
        if let Err(e) = result {
            eprintln!("error: {e}");
            self.message = Some((product().into(), e));
        }
    }

    /// Rows and texts for PDF / Excel export. `view_rows`: the visible rows of the current view
    /// (collapsed outline respected, like printing); otherwise every task.
    /// The pages File > Print prints: the view's rows (only those running in the dates asked
    /// for), on the paper and orientation chosen.
    pub fn print_data(&self) -> crate::export::ExportData {
        let mut rows = self.visible_rows();
        let range = if self.print.specific_dates { self.parse_date(&self.print.from).zip(self.parse_date(&self.print.to)) } else { None };
        if let Some((a, b)) = range {
            rows.retain(|&i| self.doc.tasks[i].start <= b.max(a) && self.doc.tasks[i].finish >= a.min(b));
        }
        let mut d = crate::export::ExportData::build(&self.doc, &rows, self);
        if let Some((a, b)) = range {
            d.start = a.min(b);
            d.finish = a.max(b);
            d.start_text = self.fmt_date(d.start);
            d.finish_text = self.fmt_finish(d.finish);
        }
        let (_, w, h) = PAPERS[(self.print.paper as usize).min(PAPERS.len() - 1)];
        (d.page_w, d.page_h) = if self.print.landscape { (h.max(w), h.min(w)) } else { (w.min(h), w.max(h)) };
        d
    }

    /// File > Print > Print: the pages go to the Windows printer through the PDF handler's
    /// "print" verb; without one the PDF opens so it can be printed from the viewer.
    pub fn cmd_print(&mut self) {
        self.commit_edit();
        let d = self.print_data();
        let path = std::env::temp_dir().join(format!("NeramitPlanner-print-{}.pdf", chrono::Local::now().format("%Y%m%d-%H%M%S")));
        if let Err(e) = crate::export::write_pdf(&path, &d) {
            self.show_error(e);
            return;
        }
        let mut printed = true;
        for _ in 0..self.print.copies.clamp(1, 99) {
            if !shell_print(&path) {
                printed = false;
                break;
            }
        }
        if printed {
            self.status = crate::i18n::trf("Sent {} to the printer", &[&d.title]);
            self.backstage = None;
        } else {
            let _ = std::process::Command::new("explorer").arg(&path).spawn();
            self.show_error("No program is registered to print PDF files. The pages were opened instead; print them from there.");
        }
    }

    pub fn export_data(&self, view_rows: bool) -> crate::export::ExportData {
        let rows: Vec<usize> = if view_rows { self.visible_rows() } else { (0..self.doc.tasks.len()).collect() };
        crate::export::ExportData::build(&self.doc, &rows, self)
    }

    pub(crate) fn save_to(&mut self, p: PathBuf) -> bool {
        if is_mpp(&p) {
            return self.save_mpp_copy(p);
        }
        match fileio::save(&p, &self.doc) {
            Ok(()) => {
                self.path = Some(p.clone());
                self.dirty = false;
                self.add_recent(&p);
                self.status = crate::i18n::trf("Saved {}", &[&p.display().to_string()]);
                self.backstage = None;
                true
            }
            Err(e) => {
                self.show_error(e);
                false
            }
        }
    }

    fn add_recent(&mut self, p: &std::path::Path) {
        let p = p.to_path_buf();
        self.recent.retain(|r| r != &p);
        self.recent.insert(0, p);
        self.recent.truncate(20);
        save_list("recent.txt", &self.recent);
    }

    pub fn toggle_favorite(&mut self, p: &std::path::Path) {
        let p = p.to_path_buf();
        if self.favorites.contains(&p) {
            self.favorites.retain(|f| f != &p);
        } else {
            self.favorites.insert(0, p);
        }
        save_list("favorites.txt", &self.favorites);
    }

    /// Home / New page templates: 0 = blank, 1 = construction sample, 2 = copy of an existing file.
    pub fn new_from_template(&mut self, which: u8) {
        match which {
            0 => {
                self.run_pending(PendingAction::New);
                self.backstage = None;
            }
            1 => {
                let bytes = include_bytes!("../examples/sample.neramit");
                let tmp = std::env::temp_dir().join("neramit-planner-template.neramit");
                if std::fs::write(&tmp, bytes).is_ok() {
                    self.open_path(tmp.clone());
                    let _ = std::fs::remove_file(&tmp);
                    self.recent.retain(|r| r != &tmp);
                    save_list("recent.txt", &self.recent);
                    self.path = None;
                    self.dirty = true;
                    self.backstage = None;
                }
            }
            _ => {
                if let Some(p) = rfd::FileDialog::new()
                    .set_title("New from existing project")
                    .add_filter("All Projects", &["neramit", "mpp", "mpt", "xml"])
                    .pick_file()
                {
                    self.open_path(p);
                    // a new, unsaved copy
                    self.path = None;
                    self.dirty = true;
                    self.backstage = None;
                }
            }
        }
    }

    pub fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        if let Some(p) = dropped.into_iter().next() {
            self.cmd_open_path(p);
        }
    }
}

fn is_mpp(p: &std::path::Path) -> bool {
    p.extension().map(|e| e.eq_ignore_ascii_case("mpp") || e.eq_ignore_ascii_case("mpt")).unwrap_or(false)
}

/// Settings live in %APPDATA%\Neramit Planner; portable mode uses a `data` folder next to
/// NeramitPlanner.exe when that folder already exists and can be written (USB stick).
/// Set by the headless QA runner: settings, recent files and favourites are not written.
pub static NO_PERSIST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// View > Macros: the recorded macros, kept in macros.json next to the settings.
pub fn load_macros() -> Vec<(String, Vec<crate::ribbon::Cmd>)> {
    if NO_PERSIST.load(std::sync::atomic::Ordering::Relaxed) {
        return Vec::new();
    }
    settings_dir()
        .and_then(|d| std::fs::read_to_string(d.join("macros.json")).ok())
        // (Cmd borrows its export format as &'static str: the text lives for the program)
        .and_then(|t| serde_json::from_str(Box::leak(t.into_boxed_str())).ok())
        .unwrap_or_default()
}

pub fn save_macros(m: &[(String, Vec<crate::ribbon::Cmd>)]) {
    if NO_PERSIST.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    if let Some(d) = settings_dir() {
        let _ = std::fs::create_dir_all(&d);
        if let Ok(t) = serde_json::to_string_pretty(m) {
            let _ = std::fs::write(d.join("macros.json"), t);
        }
    }
}

pub fn settings_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.join("data"))) {
        // Neramit Planner ships as a single .exe: running it from the Desktop leaves no folder behind
        if !dir.is_dir() {
            return std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("Neramit Planner"));
        }
        if std::fs::create_dir_all(&dir).is_ok() {
            let probe = dir.join(".write-test");
            if std::fs::write(&probe, b"ok").is_ok() {
                let _ = std::fs::remove_file(&probe);
                return Some(dir);
            }
        }
    }
    std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("Neramit Planner"))
}

fn load_list(file: &str) -> Vec<PathBuf> {
    settings_dir()
        .and_then(|d| std::fs::read_to_string(d.join(file)).ok())
        .map(|s| s.lines().filter(|l| !l.trim().is_empty()).map(PathBuf::from).collect())
        .unwrap_or_default()
}

fn save_list(file: &str, list: &[PathBuf]) {
    if NO_PERSIST.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    if let Some(d) = settings_dir() {
        let _ = std::fs::create_dir_all(&d);
        let s: Vec<String> = list.iter().map(|p| p.display().to_string()).collect();
        let _ = std::fs::write(d.join(file), s.join("\n"));
    }
}


fn default_language() -> u8 {
    0
}

/// Application-level options (Project Options that are not stored in the project file).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct UserSettings {
    /// DateFormat (Options > General > Date format)
    #[serde(default = "default_date_format")]
    pub date_format: u8,
    /// Classic chrome (green tab row, "Gantt Chart Tools") or the modern look
    #[serde(default = "crate::model::default_true")]
    pub classic_ui: bool,
    #[serde(default)]
    pub user_name: String,
    #[serde(default)]
    pub initials: String,
    /// 0 = Windows regional settings, 1 = Gregorian, 2 = Thai Buddhist
    #[serde(default)]
    pub calendar_type: u8,
    /// 0 = English, 1 = Thai (File > Options > Language)
    #[serde(default = "default_language")]
    pub language: u8,
    /// The user picked the language (language button or Options): until then the program starts
    /// in English whatever an older settings file says.
    #[serde(default)]
    pub language_chosen: bool,
    #[serde(default = "crate::model::default_true")]
    pub show_status_bar: bool,
    #[serde(default = "crate::model::default_true")]
    pub screentips: bool,
    #[serde(default = "default_undo")]
    pub undo_levels: u8,
    #[serde(default = "default_recent")]
    pub recent_count: u8,
    /// 0 = Gantt Chart, 1 = Task Sheet, 2 = Resource Sheet
    #[serde(default)]
    pub default_view: u8,
    #[serde(default = "crate::model::default_true")]
    pub auto_calculate: bool,
}

fn default_date_format() -> u8 {
    crate::locale::DEFAULT_FORMAT
}
fn default_undo() -> u8 {
    20
}
fn default_recent() -> u8 {
    25
}

impl Default for UserSettings {
    fn default() -> Self {
        UserSettings {
            date_format: crate::locale::DEFAULT_FORMAT,
            classic_ui: true,
            user_name: String::new(),
            initials: String::new(),
            calendar_type: 0,
            // starts in English; the language button switches it
            language: 0,
            language_chosen: false,
            show_status_bar: true,
            screentips: true,
            undo_levels: 20,
            recent_count: 25,
            default_view: 0,
            auto_calculate: true,
        }
    }
}

impl UserSettings {
    pub fn load() -> UserSettings {
        let mut s: UserSettings = settings_dir()
            .and_then(|d| std::fs::read_to_string(d.join("settings.json")).ok())
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        if !s.language_chosen {
            s.language = 0;
        }
        if s.user_name.is_empty() {
            // like Project, default the user name to the Windows account name
            s.user_name = std::env::var("USERNAME").unwrap_or_default();
            s.initials = s.user_name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
        }
        s
    }

    pub fn save(&self) {
        if NO_PERSIST.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        if let Some(d) = settings_dir() {
            let _ = std::fs::create_dir_all(&d);
            if let Ok(t) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(d.join("settings.json"), t);
            }
        }
    }

    /// Regional settings with day / month names in the display language and the calendar
    /// override applied.
    pub fn locale(&self) -> crate::locale::Locale {
        crate::locale::for_display(self.language == 1, self.calendar_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_ctx() -> egui::Context {
        egui::Context::default()
    }

    fn app_with(tasks: &[(i64, u8)]) -> App {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(&ctx, None);
        let mon = NaiveDate::from_ymd_opt(2026, 10, 19).unwrap();
        app.doc = Project::new();
        app.doc.start = mon;
        for (k, (minutes, level)) in tasks.iter().enumerate() {
            let mut t = Task::new(k as u32 + 1, &format!("T{k}"), mon);
            t.outline_level = *level;
            t.set_duration_minutes(*minutes, DurUnit::Days, false, 480);
            app.doc.tasks.push(t);
        }
        app.doc.next_uid = tasks.len() as u32 + 1;
        app.recalc();
        app
    }

    /// Share Resources with a pool (.neramit): refresh takes the pool's resources (and with the pool
    /// first its fields); update writes this project's new resources into the pool; saved and
    /// opened again the project still shares the pool (NERAMIT_POOL_DIR: a folder to write in).
    #[test]
    fn resource_pool_live() {
        let Ok(dir) = std::env::var("NERAMIT_POOL_DIR") else { return };
        let dir = std::path::Path::new(&dir);
        let pool_path = dir.join("pool.neramit");
        let mut pool = Project::new();
        let mut a = crate::model::Resource::new(1, "Crane");
        a.std_rate = 900.0;
        pool.resources.push(a);
        pool.resources.push(crate::model::Resource::new(2, "Welder"));
        pool.next_res_uid = 3;
        fileio::save(&pool_path, &pool).unwrap();
        let mut app = app_with(&[(480, 1)]);
        let mut own = crate::model::Resource::new(app.doc.alloc_res_uid(), "crane");
        own.std_rate = 100.0;
        app.doc.resources.push(own);
        let painter = app.doc.alloc_res_uid();
        app.doc.resources.push(crate::model::Resource::new(painter, "Painter"));
        app.doc.resource_pool = Some(crate::model::ResourcePool { path: pool_path.display().to_string(), sharer_first: false });
        assert_eq!(app.refresh_pool().unwrap(), (1, 1));
        assert_eq!(app.doc.resources.iter().find(|r| r.name == "Crane").map(|r| r.std_rate), Some(900.0));
        app.undo();
        assert!(app.doc.resources.iter().all(|r| r.name != "Welder"));
        app.redo();
        let p = app.update_pool().unwrap();
        assert_eq!(p, pool_path);
        let back = fileio::load(&pool_path).unwrap();
        assert_eq!(back.resources.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), vec!["Crane", "Welder", "Painter"]);
        // a pool that changed: the project takes it when opened
        let mut changed = back.clone();
        changed.resources[1].std_rate = 55.0;
        fileio::save(&pool_path, &changed).unwrap();
        let sharer = dir.join("sharer.neramit");
        fileio::save(&sharer, &app.doc).unwrap();
        let mut again = app_with(&[]);
        again.open_path(sharer);
        assert_eq!(again.doc.resource_pool.as_ref().map(|r| r.path.clone()), Some(pool_path.display().to_string()));
        assert_eq!(again.doc.resources.iter().find(|r| r.name == "Welder").map(|r| r.std_rate), Some(55.0));
    }

    #[test]
    fn set_and_clear_baseline() {
        let mut app = app_with(&[(0, 1), (960, 2), (1440, 2)]);
        app.set_baseline(0, false, false, false);
        let b = app.doc.tasks[2].baseline(0).cloned().unwrap();
        assert_eq!((b.start, b.finish, b.dur_min), (app.doc.tasks[2].start_dt, app.doc.tasks[2].finish_dt, Some(1440)));
        assert!(app.doc.baseline_saved.iter().any(|(n, _)| *n == 0));
        // (the UI language is global and other tests switch it: compare numbers, not words)
        // Variance table: no slip yet
        assert!(app.cell_text(2, TCol::FinishVariance).starts_with('0'));
        // slip the task one day: the finish variance is one day
        app.doc.tasks[2].set_snet(NaiveDate::from_ymd_opt(2026, 10, 20).unwrap());
        app.recalc();
        assert!(app.cell_text(2, TCol::FinishVariance).starts_with('1'));
        app.clear_baseline(0, false);
        assert!(app.doc.tasks.iter().all(|t| t.baselines.is_empty()));
        assert!(app.doc.baseline_saved.is_empty());
    }

    #[test]
    fn tracking_table_and_update_project() {
        let mut app = app_with(&[(2400, 1), (960, 1)]);
        app.apply_table(3);
        assert_eq!(app.task_cols.iter().map(|c| c.0).collect::<Vec<_>>()[1], TCol::ActualStart);
        assert!(app.col_titles.contains(&(TCol::ActualDuration, "Act. Dur.".to_string())));
        // Update Project through Tue 17:00: the 5-day task is 40 % done, the 2-day one complete
        let through = app.doc.calendar.day_end(NaiveDate::from_ymd_opt(2026, 10, 20).unwrap());
        app.update_progress(&[0, 1], through, false);
        app.recalc();
        assert_eq!((app.doc.tasks[0].percent_complete, app.doc.tasks[1].percent_complete), (40, 100));
        assert!(app.cell_text(0, TCol::ActualDuration).starts_with('2'));
        assert!(app.doc.tasks[1].actual_finish.is_some());
        // all-or-nothing leaves the unfinished task alone
        let mut b = app_with(&[(2400, 1)]);
        b.update_progress(&[0], through, true);
        assert_eq!(b.doc.tasks[0].percent_complete, 0);
    }

    #[test]
    fn typed_work_and_units_follow_the_task_type() {
        let mut app = app_with(&[(2400, 1)]);
        let mut r = Resource::new(1, "Engineer");
        r.std_rate = 100.0;
        app.doc.resources.push(r);
        app.doc.next_res_uid = 2;
        app.doc.tasks[0].resources = vec![1];
        app.recalc();
        assert_eq!(app.doc.tasks[0].work_min, 2400);
        // Fixed Units: 16 hours of work at 100 % is a 2-day task
        app.set_task_work(0, 960).unwrap();
        app.recalc();
        assert_eq!((app.doc.tasks[0].dur_min, app.doc.tasks[0].work_min), (960, 960));
        // ... and 50 % units stretch it to 4 days
        app.set_assignment_units(0, 1, 0.5);
        app.recalc();
        assert_eq!((app.doc.tasks[0].dur_min, app.doc.tasks[0].work_min), (1920, 960));
        // Fixed Duration: units change the work instead
        app.doc.tasks[0].task_type = TaskType::FixedDuration;
        app.set_assignment_units(0, 1, 1.0);
        app.recalc();
        assert_eq!((app.doc.tasks[0].dur_min, app.doc.tasks[0].work_min), (1920, 1920));
        assert!((app.doc.tasks[0].cost - 3200.0).abs() < 1e-6);
    }

    #[test]
    fn filters_and_sort_keep_the_outline() {
        // S { A 3d, M milestone }, B 1d
        let mut app = app_with(&[(0, 1), (1440, 2), (0, 2), (480, 1)]);
        app.doc.tasks[2].milestone = true;
        // A starts after B: B -> A
        app.doc.tasks[1].predecessors.push(Link::new(4, LinkType::FS));
        app.recalc();
        app.filter = FilterSpec { kind: TaskFilter::Milestones, ..Default::default() };
        assert_eq!(app.visible_rows(), vec![2]);
        app.related_summary = true;
        assert_eq!(app.visible_rows(), vec![0, 2]);
        app.filter = FilterSpec::default();
        app.highlight = FilterSpec { kind: TaskFilter::Summary, ..Default::default() };
        assert!(app.is_highlighted(0) && !app.is_highlighted(1));
        // by start date: B (Mon) before S, and inside S the milestone (Mon) before A (Tue)
        app.sort = SortKey::Start;
        assert_eq!(app.visible_rows().len(), 4);
        let v = app.visible_rows();
        assert!(v.iter().position(|&i| i == 0) < v.iter().position(|&i| i == 1), "a summary stays above its subtasks");
        assert!(v.iter().position(|&i| i == 2) < v.iter().position(|&i| i == 1), "sorted within the summary");
    }

    #[test]
    fn print_pages_follow_paper_and_orientation() {
        let mut app = app_with(&[(2400, 1), (960, 1)]);
        app.print.landscape = false;
        app.print.paper = 2; // Letter
        let d = app.print_data();
        assert_eq!((d.page_w, d.page_h), (612.0, 792.0));
        let pages = crate::export::preview(&d);
        assert_eq!(pages.len(), 1);
        assert!(pages[0].iter().any(|op| matches!(op, crate::export::PrevOp::Text(_, _, _, _, s, _) if s.contains("T0"))));
        let pdf = crate::export::pdf_bytes(&d);
        assert!(String::from_utf8_lossy(&pdf).contains("/MediaBox [0 0 612.00 792.00]"));
        // Print Specific Dates keeps the tasks running then
        app.print.specific_dates = true;
        app.print.from = app.fmt_date(NaiveDate::from_ymd_opt(2026, 10, 23).unwrap());
        app.print.to = app.fmt_date(NaiveDate::from_ymd_opt(2026, 10, 30).unwrap());
        assert_eq!(app.print_data().rows.len(), 1);
    }

    #[test]
    fn group_by_critical_lists_tasks_under_header_bands() {
        // S { A 3d critical chain -> C }, B 1d (has slack)
        let mut app = app_with(&[(0, 1), (1440, 2), (480, 2), (960, 2)]);
        app.doc.tasks[3].predecessors.push(Link::new(2, LinkType::FS));
        app.recalc();
        app.group = TaskGroup::Critical;
        let v = app.visible_rows();
        // summary hidden; non-critical (B) first, then the critical chain A, C
        assert_eq!(v, vec![2, 1, 3]);
        // drawing the sheet lays out the bands
        let ctx = app_ctx();
        let screen = eframe::egui::Rect::from_min_size(eframe::egui::Pos2::ZERO, eframe::egui::Vec2::new(900.0, 400.0));
        for k in 0..3 {
            let input = eframe::egui::RawInput { screen_rect: Some(screen), time: Some(k as f64 * 0.03), ..Default::default() };
            let _ = ctx.run_pass(input, |ctx| {
                eframe::egui::Area::new(eframe::egui::Id::unique("group_test")).fixed_pos(eframe::egui::Pos2::ZERO).show(ctx, |ui| {
                    ui.set_min_size(screen.size());
                    let mut cmds = Vec::new();
                    crate::grid::task_grid(&mut app, ui, screen, &mut cmds);
                });
            });
        }
        assert_eq!(app.group_bands.iter().map(|b| b.0).collect::<Vec<_>>(), vec![0, 1]);
        // the second group's band adds its height above row 1
        assert_eq!(app.row_top(1) - app.row_top(0), crate::grid::ROW_H + GROUP_H);
        assert_eq!(app.row_height(0), crate::grid::ROW_H);
        let (st, fi, _, _) = app.group_totals(1).unwrap();
        assert_eq!((st, fi), (app.doc.tasks[1].start_dt.unwrap(), app.doc.tasks[3].finish_dt.unwrap()));
    }

    #[test]
    fn tables_keep_the_task_name_width() {
        let mut app = app_with(&[(480, 1), (960, 1)]);
        app.apply_table(0);
        if let Some(c) = app.task_cols.iter_mut().find(|c| c.0 == TCol::Name) {
            c.1 = 412.0;
        }
        for k in [3u8, 5, 6, 0] {
            app.apply_table(k);
            assert_eq!(app.task_cols.iter().find(|c| c.0 == TCol::Name).map(|c| c.1), Some(412.0), "table {k}");
        }
        // other columns get the table's widths
        assert_eq!(app.task_cols.iter().find(|c| c.0 == TCol::Duration).map(|c| c.1), Some(TCol::Duration.default_width()));
    }

    #[test]
    fn usage_views_keep_their_own_zoom() {
        let mut app = app_with(&[(480, 1), (960, 1)]);
        app.view = View::Gantt;
        app.px_per_day = 2.0;
        app.scroll_x = 100.0;
        app.sync_view_scale();
        assert_eq!((app.px_per_day, app.scroll_x), (2.0, 100.0));
        // first visit: Weeks over Days at the date the Gantt showed (day 50)
        app.view = View::TaskUsage;
        app.sync_view_scale();
        assert_eq!(app.px_per_day, App::USAGE_PPD);
        assert_eq!(app.scroll_x, 50.0 * App::USAGE_PPD);
        // zooming the Usage view leaves the Gantt's timescale as read from the file
        app.doc.formatting.timescale.auto = false;
        app.set_zoom(48.0, None);
        assert!(!app.doc.formatting.timescale.auto);
        // Resource Usage shares the Usage zoom; the Gantt gets its own back
        app.view = View::ResourceUsage;
        app.sync_view_scale();
        assert_eq!(app.px_per_day, 48.0);
        app.view = View::TrackingGantt;
        app.sync_view_scale();
        assert_eq!((app.px_per_day, app.scroll_x), (2.0, 100.0));
        app.view = View::TaskUsage;
        app.sync_view_scale();
        assert_eq!(app.px_per_day, 48.0);
    }

    #[test]
    fn move_project_shifts_pinned_dates_but_not_actuals() {
        // T0 ASAP, T1 SNET Wed, T2 manual Thu, T3 started on Mon
        let mut app = app_with(&[(480, 1), (480, 1), (480, 1), (960, 1)]);
        let d = |day: u32| NaiveDate::from_ymd_opt(2026, 10, day).unwrap();
        app.doc.tasks[1].constraint_type = ConstraintType::Snet;
        app.doc.tasks[1].constraint_date = Some(d(21));
        app.doc.tasks[1].deadline = Some(d(23));
        app.doc.tasks[2].manual = true;
        app.doc.tasks[2].start = d(22);
        app.doc.tasks[2].finish = d(22);
        app.doc.tasks[2].start_dt = Some(d(22).and_hms_opt(8, 0, 0).unwrap());
        app.doc.tasks[2].finish_dt = Some(d(22).and_hms_opt(17, 0, 0).unwrap());
        app.doc.tasks[3].set_actual_start(Some(d(19).and_hms_opt(8, 0, 0).unwrap()));
        app.recalc();
        app.move_project(d(26), false);
        let t = &app.doc.tasks;
        assert_eq!(app.doc.start, d(26));
        assert_eq!(t[0].start, d(26));
        assert_eq!(t[1].constraint_date, Some(d(28)));
        assert_eq!(t[1].start, d(28));
        assert_eq!(t[1].deadline, Some(d(23)), "deadlines move only when asked");
        assert_eq!((t[2].start, t[2].finish), (d(29), d(29)));
        assert_eq!(t[3].start, d(19), "a started task keeps its actual start");
        app.undo();
        assert_eq!(app.doc.start, d(19));
        app.move_project(d(26), true);
        assert_eq!(app.doc.tasks[1].deadline, Some(d(30)));
    }

    #[test]
    fn predecessor_ids_follow_task_changes() {
        let mut app = app_with(&[(480, 1), (480, 1), (480, 1)]);
        app.doc.tasks[2].predecessors.push(Link::new(2, LinkType::FS));
        assert_eq!(app.cell_text(2, TCol::Predecessors), "2");
        // a task inserted above moves the predecessor to ID 3; the cached UID map notices
        let mon = app.doc.start;
        app.doc.tasks.insert(0, Task::new(9, "new", mon));
        assert_eq!(app.cell_text(3, TCol::Predecessors), "3");
        app.doc.tasks.remove(0);
        assert_eq!(app.cell_text(2, TCol::Predecessors), "2");
    }
}



/// ShellExecute "print" on a file (true when Windows accepted it).
fn shell_print(path: &std::path::Path) -> bool {
    use std::os::windows::ffi::OsStrExt;
    let wide = |s: &std::ffi::OsStr| s.encode_wide().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let file = wide(path.as_os_str());
    let verb = wide(std::ffi::OsStr::new("print"));
    // SAFETY: both strings are NUL-terminated UTF-16 that outlive the call
    let r = unsafe { windows_sys::Win32::UI::Shell::ShellExecuteW(std::ptr::null_mut(), verb.as_ptr(), file.as_ptr(), std::ptr::null(), std::ptr::null(), windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE) };
    r as isize > 32

}
