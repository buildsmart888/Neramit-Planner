//! Core project data model: project, tasks, resources, calendar.

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LinkType {
    #[default]
    FS,
    SS,
    FF,
    SF,
}

impl LinkType {
    pub fn code(self) -> &'static str {
        match self {
            LinkType::FS => "FS",
            LinkType::SS => "SS",
            LinkType::FF => "FF",
            LinkType::SF => "SF",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Link {
    pub pred_uid: u32,
    pub kind: LinkType,
    /// Lag in whole working days (may be negative = lead), or calendar days when `elapsed`
    /// (kept in step with `lag_min` for older code and files).
    pub lag: i32,
    /// Lag counted in elapsed (calendar) time, e.g. "+27 edays".
    #[serde(default)]
    pub elapsed: bool,
    /// Exact lag in minutes (working minutes, or calendar minutes when `elapsed`), as MPP
    /// stores it ("+4 hrs", "+0.5 days"); `None` = whole days of `lag` (files before 2.1).
    #[serde(default)]
    pub lag_min: Option<i64>,
    /// Lag as a percentage of the predecessor's duration ("2FS+50%"), instead of a time.
    #[serde(default)]
    pub lag_pct: Option<f64>,
    /// Unit the lag is shown in.
    #[serde(default)]
    pub lag_unit: DurUnit,
}

impl Link {
    pub fn new(pred_uid: u32, kind: LinkType) -> Link {
        Link { pred_uid, kind, ..Default::default() }
    }

    /// Lag in minutes (working, or calendar when elapsed); a percentage needs the predecessor's
    /// duration in minutes. Whole-day `lag` edited without `lag_min` wins (older code paths).
    pub fn lag_minutes(&self, mpd: i64, pred_dur_min: i64) -> i64 {
        if let Some(p) = self.lag_pct {
            // MPP: the share of the duration in tenths of a minute, truncated (70 % of 600 h
            // is 251999.99... tenths, so a successor starts at 11:59:54, not after lunch)
            let tenths = ((pred_dur_min * 10) as f64 * (p / 100.0)).trunc();
            return (tenths / 10.0).trunc() as i64;
        }
        let day = if self.elapsed { 1440 } else { mpd.max(1) };
        match self.lag_min {
            Some(m) if round_div(m, day) == self.lag as i64 => m,
            _ => self.lag as i64 * day,
        }
    }

    /// Set an exact lag (minutes) and keep the whole-day `lag` in step.
    pub fn set_lag_minutes(&mut self, m: i64, unit: DurUnit, elapsed: bool, mpd: i64) {
        self.elapsed = elapsed;
        self.lag_pct = None;
        self.lag_unit = unit;
        self.lag_min = Some(m);
        self.lag = round_div(m, if elapsed { 1440 } else { mpd.max(1) }) as i32;
    }
}

/// Unit a duration or lag is shown in (the standard DurationFormat).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DurUnit {
    Minutes,
    Hours,
    #[default]
    Days,
    Weeks,
    Months,
}

impl DurUnit {
    /// The standard duration / lag format code (XML DurationFormat, LagFormat): unit, elapsed.
    /// Estimated variants (+32) are folded in; percent formats (19, 20) report days.
    pub fn from_code(code: i64) -> (DurUnit, bool) {
        let c = if code >= 32 { code - 32 } else { code };
        let elapsed = matches!(c, 4 | 6 | 8 | 10 | 12 | 20);
        let unit = match c {
            3 | 4 => DurUnit::Minutes,
            5 | 6 => DurUnit::Hours,
            9 | 10 => DurUnit::Weeks,
            11 | 12 => DurUnit::Months,
            _ => DurUnit::Days,
        };
        (unit, elapsed)
    }

    pub fn code(self, elapsed: bool) -> i64 {
        let c = match self {
            DurUnit::Minutes => 3,
            DurUnit::Hours => 5,
            DurUnit::Days => 7,
            DurUnit::Weeks => 9,
            DurUnit::Months => 11,
        };
        c + elapsed as i64
    }
}

/// a / b rounded half away from zero
pub fn round_div(a: i64, b: i64) -> i64 {
    if b == 0 {
        return 0;
    }
    let q = (a.abs() * 2 + b.abs()) / (b.abs() * 2);
    if (a < 0) != (b < 0) { -q } else { q }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceType {
    Work,
    Material,
    Cost,
}

impl ResourceType {
    pub fn label(self) -> &'static str {
        match self {
            ResourceType::Work => "Work",
            ResourceType::Material => "Material",
            ResourceType::Cost => "Cost",
        }
    }
}

/// When a cost is incurred (the standard AccrueAt / Fixed Cost Accrual).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Accrue {
    Start,
    End,
    #[default]
    Prorated,
}

impl Accrue {
    pub const ALL: [Accrue; 3] = [Accrue::Start, Accrue::Prorated, Accrue::End];
    pub fn label(self) -> &'static str {
        match self {
            Accrue::Start => "Start",
            Accrue::End => "End",
            Accrue::Prorated => "Prorated",
        }
    }
    /// The standard code: 1 start, 2 end, 3 prorated.
    pub fn code(self) -> u8 {
        match self {
            Accrue::Start => 1,
            Accrue::End => 2,
            Accrue::Prorated => 3,
        }
    }
    pub fn from_code(c: i64) -> Accrue {
        match c {
            1 => Accrue::Start,
            2 => Accrue::End,
            _ => Accrue::Prorated,
        }
    }
}

/// A resource assigned to a task (the standard Assignment).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Assignment {
    pub res_uid: u32,
    /// work resources: 1.0 = 100 %; material resources: the quantity
    pub units: f64,
    /// scheduled work in minutes (work resources); -1 = duration x units
    pub work_min: i64,
    /// cost resources: the cost entered on the assignment
    #[serde(default)]
    pub cost: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Resource {
    pub uid: u32,
    pub name: String,
    pub rtype: ResourceType,
    #[serde(default)]
    pub material_label: String,
    #[serde(default)]
    pub initials: String,
    #[serde(default)]
    pub group: String,
    /// Max units in percent (100 = one full-time person).
    pub max_units: f64,
    /// Standard rate per hour.
    pub std_rate: f64,
    pub ovt_rate: f64,
    pub cost_per_use: f64,
    #[serde(default)]
    pub accrue_at: Accrue,
    #[serde(default)]
    pub code: String,
    /// Resource Information > Notes.
    #[serde(default)]
    pub notes: String,
    /// Resource Information > General > Email.
    #[serde(default)]
    pub email: String,
    /// Custom fields (Text1, Number1, Flag1, ...)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom: crate::custom::Values,
    /// Unique ID in the .mpp it was read from.
    #[serde(skip)]
    pub src_uid: Option<u32>,
    // ---- calculated
    /// total work of the resource's assignments (minutes)
    #[serde(skip)]
    pub work_min: i64,
    #[serde(skip)]
    pub cost: f64,
    /// more work than its Max. Units allow on some day
    #[serde(skip)]
    pub overallocated: bool,
}

impl Resource {
    pub fn new(uid: u32, name: &str) -> Self {
        let initials = name.chars().next().map(|c| c.to_string()).unwrap_or_default();
        Resource {
            uid,
            name: name.to_string(),
            rtype: ResourceType::Work,
            material_label: String::new(),
            initials,
            group: String::new(),
            max_units: 100.0,
            std_rate: 0.0,
            ovt_rate: 0.0,
            cost_per_use: 0.0,
            accrue_at: Accrue::Prorated,
            code: String::new(),
            notes: String::new(),
            email: String::new(),
            custom: Vec::new(),
            src_uid: None,
            work_min: 0,
            cost: 0.0,
            overallocated: false,
        }
    }
}

/// Constraint: how a task's dates are constrained.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ConstraintType {
    #[default]
    Asap,
    Alap,
    Mso,
    Mfo,
    Snet,
    Snlt,
    Fnet,
    Fnlt,
}

impl ConstraintType {
    pub const ALL: [ConstraintType; 8] = [
        ConstraintType::Asap,
        ConstraintType::Alap,
        ConstraintType::Mso,
        ConstraintType::Mfo,
        ConstraintType::Snet,
        ConstraintType::Snlt,
        ConstraintType::Fnet,
        ConstraintType::Fnlt,
    ];
    pub fn label(self) -> &'static str {
        match self {
            ConstraintType::Asap => "As Soon As Possible",
            ConstraintType::Alap => "As Late As Possible",
            ConstraintType::Mso => "Must Start On",
            ConstraintType::Mfo => "Must Finish On",
            ConstraintType::Snet => "Start No Earlier Than",
            ConstraintType::Snlt => "Start No Later Than",
            ConstraintType::Fnet => "Finish No Earlier Than",
            ConstraintType::Fnlt => "Finish No Later Than",
        }
    }
    /// The standard numeric code (XML `ConstraintType`, `.mpp` field).
    pub fn code(self) -> u8 {
        match self {
            ConstraintType::Asap => 0,
            ConstraintType::Alap => 1,
            ConstraintType::Mso => 2,
            ConstraintType::Mfo => 3,
            ConstraintType::Snet => 4,
            ConstraintType::Snlt => 5,
            ConstraintType::Fnet => 6,
            ConstraintType::Fnlt => 7,
        }
    }
    pub fn from_code(c: i64) -> ConstraintType {
        ConstraintType::ALL.iter().copied().find(|t| t.code() as i64 == c).unwrap_or_default()
    }
    pub fn needs_date(self) -> bool {
        !matches!(self, ConstraintType::Asap | ConstraintType::Alap)
    }
}

/// TaskFixedType
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TaskType {
    #[default]
    FixedUnits,
    FixedDuration,
    FixedWork,
}

impl TaskType {
    pub const ALL: [TaskType; 3] = [TaskType::FixedUnits, TaskType::FixedDuration, TaskType::FixedWork];
    pub fn label(self) -> &'static str {
        match self {
            TaskType::FixedUnits => "Fixed Units",
            TaskType::FixedDuration => "Fixed Duration",
            TaskType::FixedWork => "Fixed Work",
        }
    }
    pub fn code(self) -> u8 {
        self as u8
    }
    pub fn from_code(c: i64) -> TaskType {
        match c {
            1 => TaskType::FixedDuration,
            2 => TaskType::FixedWork,
            _ => TaskType::FixedUnits,
        }
    }
}

fn default_priority() -> u16 {
    500
}

/// A saved baseline of a task (the standard Baseline, Baseline1 .. Baseline10).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Baseline {
    /// 0 = Baseline, 1..=10 = Baseline1..Baseline10
    pub number: u8,
    #[serde(default)]
    pub start: Option<NaiveDateTime>,
    #[serde(default)]
    pub finish: Option<NaiveDateTime>,
    /// duration in minutes
    #[serde(default)]
    pub dur_min: Option<i64>,
    /// work in minutes
    #[serde(default)]
    pub work_min: Option<i64>,
    #[serde(default)]
    pub cost: Option<f64>,
}

/// % complete of `actual` minutes done out of `total`, as MPP rounds it (never 100 % before
/// the end).
pub fn pct_of(actual: i64, total: i64) -> u8 {
    if total <= 0 {
        return if actual > 0 { 100 } else { 0 };
    }
    if actual >= total {
        return 100;
    }
    ((actual.max(0) as f64 * 100.0 / total as f64).round() as u8).min(99)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    pub uid: u32,
    pub name: String,
    /// 1-based outline level.
    pub outline_level: u8,
    /// Duration in whole working days (0 = milestone); kept in step with `dur_min`.
    pub duration: i32,
    /// Exact duration in minutes of working time (calendar minutes when `dur_elapsed`), as MPP
    /// stores it; -1 = whole days of `duration` (files before 2.1).
    #[serde(default = "minus_one")]
    pub dur_min: i64,
    /// Unit the duration is shown in ("4 hrs", "1 wk", "3 edays").
    #[serde(default)]
    pub dur_unit: DurUnit,
    /// Elapsed duration: counted in calendar time ("3 edays").
    #[serde(default)]
    pub dur_elapsed: bool,
    /// Exact start / finish (with time of day) from the scheduler; for manual tasks the entered start.
    #[serde(default)]
    pub start_dt: Option<NaiveDateTime>,
    #[serde(default)]
    pub finish_dt: Option<NaiveDateTime>,
    /// Manually scheduled task (dates are kept as entered).
    pub manual: bool,
    pub start: NaiveDate,
    pub finish: NaiveDate,
    pub percent_complete: u8,
    #[serde(default)]
    pub predecessors: Vec<Link>,
    /// Assigned resource UIDs.
    #[serde(default)]
    pub resources: Vec<u32>,
    #[serde(default)]
    pub notes: String,
    /// "Start No Earlier Than" constraint.
    #[serde(default)]
    pub constraint_date: Option<NaiveDate>,
    #[serde(default)]
    pub milestone: bool,
    #[serde(default)]
    pub on_timeline: bool,
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub collapsed: bool,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub underline: bool,
    /// Duration is an estimate ("5 days?").
    #[serde(default)]
    pub estimated: bool,
    #[serde(default = "default_priority")]
    pub priority: u16,
    #[serde(default)]
    pub deadline: Option<NaiveDate>,
    /// Kind of `constraint_date` (files written before 1.3 imply Start No Earlier Than).
    #[serde(default)]
    pub constraint_type: ConstraintType,
    /// Exact instant of the constraint (a bar dragged at hour zoom); used while its date is
    /// `constraint_date`, otherwise the working day's start / end applies.
    #[serde(default)]
    pub constraint_dt: Option<NaiveDateTime>,
    #[serde(default)]
    pub hide_bar: bool,
    #[serde(default)]
    pub rollup: bool,
    /// Manually scheduled task without a start or a finish date entered (the standard placeholders:
    /// "Duration-only", "Start-only", "Finish-only")
    #[serde(default)]
    pub placeholder: bool,
    /// Manually scheduled task with neither a start nor a finish entered (a name, perhaps a
    /// duration): MPP shows it at its summary's start, and its links move nothing.
    #[serde(default)]
    pub no_dates: bool,
    /// A manually scheduled task's Start, Finish and Duration shown as text instead of a value:
    /// what was typed ("TBD"), "" for nothing entered (MPP keeps them; Var 0x0B400505-7
    /// and Fixed2Meta byte 88)
    #[serde(default)]
    pub manual_text: [Option<String>; 3],
    /// Task Board: 1 = "Next up" (a task not started that is up next)
    #[serde(default)]
    pub board_status: u8,
    /// Custom fields (Text1, Number1, Flag1, ...)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom: crate::custom::Values,
    /// the sprint the task is in (Project > Manage Sprints)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprint: Option<u32>,
    /// A task of another project shown here for a link (the standard external task, grey; Project >
    /// Links Between Projects): that file, the task's unique ID and ID there
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external: Option<ExternalTask>,
    #[serde(default)]
    pub marked: bool,
    #[serde(default)]
    pub task_type: TaskType,
    #[serde(default)]
    pub effort_driven: bool,
    #[serde(default)]
    pub wbs: String,
    #[serde(default)]
    pub contact: String,
    /// Insert > Hyperlink: the text shown, the address and the sub-address
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub hyperlink: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub hyperlink_address: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub hyperlink_sub: String,
    /// Cell formatting (Font group / Font dialog) keyed by column title, "*" = whole row.
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub cell_formats: std::collections::HashMap<String, crate::format::CellFormat>,
    /// Format Bar: this task's bar drawn differently from its bar style.
    #[serde(default)]
    pub bar_format: Option<crate::format::BarAppearance>,
    /// Format Bar > Bar Text (left, right, top, bottom, inside); `None` = use the bar style.
    #[serde(default)]
    pub bar_text: Option<[crate::format::BarText; 5]>,
    /// Tracking: actual start and finish (None = NA). A started task stays at its actual start.
    #[serde(default)]
    pub actual_start: Option<NaiveDateTime>,
    #[serde(default)]
    pub actual_finish: Option<NaiveDateTime>,
    /// Actual duration in minutes as stored (% complete is derived from it); None =
    /// the share of the duration given by % complete.
    #[serde(default)]
    pub actual_dur_min: Option<i64>,
    #[serde(default)]
    pub physical_percent: u8,
    /// Saved baselines (Project > Set Baseline).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub baselines: Vec<Baseline>,
    /// Units and work of each assigned resource (in the order of `resources`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assignments: Vec<Assignment>,
    #[serde(default)]
    pub fixed_cost: f64,
    #[serde(default)]
    pub fixed_cost_accrual: Accrue,
    /// Leveling delay in working minutes (Resource > Level).
    #[serde(default)]
    pub level_delay_min: i64,
    /// Split task: (working minutes of the task done before the gap, gap in working minutes).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub splits: Vec<(i64, i64)>,
    /// Duration (minutes) the assignments' work was last worked out for (-1 = not yet).
    #[serde(default = "minus_one")]
    pub work_ref_dur: i64,
    /// Duration format code MPP keeps for a summary (21 = none, 53 = none, estimated ...).
    #[serde(default)]
    pub summary_fmt: Option<u8>,
    /// Unique ID of the task in the .mpp it was read from (saving back into a copy of that file).
    #[serde(skip)]
    pub src_uid: Option<u32>,

    // ---- calculated fields ----
    #[serde(skip)]
    pub is_summary: bool,
    #[serde(skip)]
    pub critical: bool,
    #[serde(skip)]
    pub total_slack: i32,
    /// Total slack in working minutes (the standard TotalSlack); `total_slack` is in days.
    #[serde(skip)]
    pub slack_min: i64,
    /// Free slack in working minutes.
    #[serde(skip)]
    pub free_slack_min: i64,
    /// The latest start the task's own links ask for (computed): the instant, and whether it is
    /// the end of a working interval (17:00) rather than the start of one. MPP keeps it in
    /// the .mpp as the task's link date.
    #[serde(skip)]
    pub link_req: Option<(NaiveDateTime, bool)>,
    #[serde(skip)]
    pub cost: f64,
    /// total work of the task's assignments (minutes)
    #[serde(skip)]
    pub work_min: i64,
    #[serde(skip)]
    pub actual_cost: f64,
    /// an assigned resource is overallocated while the task runs
    #[serde(skip)]
    pub overallocated: bool,
}

impl Task {
    pub fn new(uid: u32, name: &str, start: NaiveDate) -> Self {
        Task {
            uid,
            name: name.to_string(),
            outline_level: 1,
            duration: 1,
            dur_min: -1,
            dur_unit: DurUnit::Days,
            dur_elapsed: false,
            start_dt: None,
            finish_dt: None,
            manual: false,
            start,
            finish: start,
            percent_complete: 0,
            predecessors: Vec::new(),
            resources: Vec::new(),
            notes: String::new(),
            constraint_date: None,
            milestone: false,
            on_timeline: false,
            active: true,
            collapsed: false,
            bold: false,
            italic: false,
            underline: false,
            estimated: false,
            priority: 500,
            deadline: None,
            constraint_type: ConstraintType::Asap,
            constraint_dt: None,
            hide_bar: false,
            rollup: false,
            placeholder: false,
            no_dates: false,
            manual_text: [None, None, None],
            board_status: 0,
            custom: Vec::new(),
            sprint: None,
            external: None,
            marked: false,
            task_type: TaskType::FixedUnits,
            effort_driven: false,
            wbs: String::new(),
            contact: String::new(),
            hyperlink: String::new(),
            hyperlink_address: String::new(),
            hyperlink_sub: String::new(),
            cell_formats: std::collections::HashMap::new(),
            bar_format: None,
            bar_text: None,
            actual_start: None,
            actual_finish: None,
            actual_dur_min: None,
            physical_percent: 0,
            baselines: Vec::new(),
            assignments: Vec::new(),
            fixed_cost: 0.0,
            fixed_cost_accrual: Accrue::Prorated,
            level_delay_min: 0,
            splits: Vec::new(),
            work_ref_dur: -1,
            summary_fmt: None,
            src_uid: None,
            is_summary: false,
            critical: false,
            total_slack: 0,
            slack_min: 0,
            free_slack_min: 0,
            link_req: None,
            cost: 0.0,
            work_min: 0,
            actual_cost: 0.0,
            overallocated: false,
        }
    }

    /// Duration in minutes (working, or calendar when elapsed). A whole-day `duration` edited
    /// without `dur_min` wins (older code paths).
    pub fn dur_minutes(&self, mpd: i64) -> i64 {
        let day = if self.dur_elapsed { 1440 } else { mpd.max(1) };
        if self.dur_min >= 0 && round_div(self.dur_min, day) == self.duration as i64 {
            self.dur_min
        } else {
            self.duration.max(0) as i64 * day
        }
    }

    /// Set an exact duration (minutes) and keep the whole-day `duration` in step.
    pub fn set_duration_minutes(&mut self, m: i64, unit: DurUnit, elapsed: bool, mpd: i64) {
        let m = m.max(0);
        self.dur_min = m;
        self.dur_unit = unit;
        self.dur_elapsed = elapsed;
        let day = if elapsed { 1440 } else { mpd.max(1) };
        // a non-zero duration never rounds to a milestone
        self.duration = if m == 0 { 0 } else { round_div(m, day).max(1) as i32 };
    }

    /// Actual duration in minutes: the stored value while it agrees with % complete.
    pub fn actual_minutes(&self, mpd: i64) -> i64 {
        let d = self.dur_minutes(mpd);
        if let Some(a) = self.actual_dur_min {
            if pct_of(a, d) == self.percent_complete {
                return a.max(0);
            }
        }
        (d as f64 * self.percent_complete as f64 / 100.0).round() as i64
    }

    /// Remaining duration in minutes.
    pub fn remaining_minutes(&self, mpd: i64) -> i64 {
        (self.dur_minutes(mpd) - self.actual_minutes(mpd)).max(0)
    }

    /// Instant `m` minutes of this task's time after `t` (working time, or calendar time when the
    /// duration is elapsed).
    pub fn advance(&self, cal: &Calendar, t: NaiveDateTime, m: i64) -> NaiveDateTime {
        if self.dur_elapsed {
            t + Duration::minutes(m)
        } else {
            cal.add_work_minutes(t, m)
        }
    }

    /// The task's current start (actual start once started).
    pub fn current_start(&self, cal: &Calendar) -> NaiveDateTime {
        self.actual_start.or(self.start_dt).unwrap_or_else(|| cal.day_start(self.start))
    }

    /// % Complete as MPP sets it: the task gets an actual start (its current start), an
    /// actual finish at 100 %; 0 % clears the actual dates.
    pub fn set_percent(&mut self, pct: u8, cal: &Calendar) {
        let pct = pct.min(100);
        let mpd = cal.minutes_per_day();
        self.percent_complete = pct;
        if pct == 0 {
            self.actual_start = None;
            self.actual_finish = None;
            self.actual_dur_min = None;
            return;
        }
        let d = self.dur_minutes(mpd);
        self.actual_dur_min = Some((d as f64 * pct as f64 / 100.0).round() as i64);
        let st = self.current_start(cal);
        self.actual_start = Some(st);
        self.actual_finish = if pct == 100 { Some(self.actual_finish.unwrap_or_else(|| self.advance(cal, st, d))) } else { None };
    }

    /// Actual Start typed (None = NA clears the task's progress).
    pub fn set_actual_start(&mut self, t: Option<NaiveDateTime>) {
        self.actual_start = t;
        if t.is_none() {
            self.actual_finish = None;
            self.actual_dur_min = None;
            self.percent_complete = 0;
        }
    }

    /// Actual Finish typed: the task is complete and its duration runs from the actual start.
    /// NA leaves the task started but not complete.
    pub fn set_actual_finish(&mut self, t: Option<NaiveDateTime>, cal: &Calendar) {
        let mpd = cal.minutes_per_day();
        match t {
            None => {
                self.actual_finish = None;
                if self.percent_complete == 100 {
                    self.percent_complete = 0;
                    self.actual_dur_min = None;
                }
            }
            Some(af) => {
                let st = self.current_start(cal).min(af);
                let m = if self.dur_elapsed { (af - st).num_minutes() } else { cal.work_minutes(st, af) }.max(0);
                let (unit, el) = (self.dur_unit, self.dur_elapsed);
                self.set_duration_minutes(m, unit, el, mpd);
                if m > 0 {
                    self.milestone = false;
                }
                self.actual_start = Some(st);
                self.actual_finish = Some(af);
                self.actual_dur_min = Some(m);
                self.percent_complete = 100;
            }
        }
    }

    /// Actual Duration typed: % complete follows; reaching the duration completes the task (a
    /// longer actual duration becomes the duration).
    pub fn set_actual_minutes(&mut self, a: i64, cal: &Calendar) {
        let mpd = cal.minutes_per_day();
        let d = self.dur_minutes(mpd);
        if a <= 0 {
            self.set_percent(0, cal);
            return;
        }
        if a >= d {
            if a > d {
                let (unit, el) = (self.dur_unit, self.dur_elapsed);
                self.set_duration_minutes(a, unit, el, mpd);
            }
            self.actual_finish = None;
            self.set_percent(100, cal);
            return;
        }
        let st = self.current_start(cal);
        self.actual_start = Some(st);
        self.actual_finish = None;
        self.actual_dur_min = Some(a);
        self.percent_complete = pct_of(a, d);
    }

    /// Remaining Duration typed: the duration becomes actual + remaining.
    pub fn set_remaining_minutes(&mut self, r: i64, cal: &Calendar) {
        let mpd = cal.minutes_per_day();
        let a = self.actual_minutes(mpd);
        let (unit, el) = (self.dur_unit, self.dur_elapsed);
        self.set_duration_minutes(a + r.max(0), unit, el, mpd);
        if a > 0 {
            if r <= 0 {
                self.set_percent(100, cal);
            } else {
                self.actual_dur_min = Some(a);
                self.percent_complete = pct_of(a, a + r);
                self.actual_finish = None;
            }
        }
    }

    /// Total length of the task's splits (working minutes).
    pub fn gap_minutes(&self) -> i64 {
        self.splits.iter().map(|s| s.1.max(0)).sum()
    }

    /// The working pieces of the task as instants (one piece when it is not split).
    pub fn segments(&self, cal: &Calendar) -> Vec<(NaiveDateTime, NaiveDateTime)> {
        let (Some(st), Some(ft)) = (self.start_dt, self.finish_dt) else { return Vec::new() };
        if self.splits.is_empty() {
            return vec![(st, ft)];
        }
        let mut sp = self.splits.clone();
        sp.sort();
        let mut out = Vec::new();
        let (mut cur, mut done) = (st, 0i64);
        for (pos, gap) in sp {
            let e = self.advance(cal, cur, (pos - done).max(0));
            out.push((cur, e));
            let n = self.advance(cal, e, gap.max(0));
            cur = if self.dur_elapsed { n } else { cal.next_work_instant(n) };
            done = pos;
        }
        out.push((cur, ft.max(cur)));
        out
    }

    pub fn baseline(&self, n: u8) -> Option<&Baseline> {
        self.baselines.iter().find(|b| b.number == n)
    }

    pub fn set_baseline(&mut self, b: Baseline) {
        self.baselines.retain(|x| x.number != b.number);
        self.baselines.push(b);
        self.baselines.sort_by_key(|x| x.number);
    }

    /// The standard placeholder kinds from a manual task's texts (an automatically scheduled task has
    /// none). Two of Start, Finish and Duration entered make the third one too.
    pub fn sync_manual_text(&mut self) {
        if !self.manual || self.is_summary {
            self.manual_text = [None, None, None];
        } else if self.manual_text.iter().filter(|x| x.is_none()).count() >= 2 {
            self.manual_text = [None, None, None];
        }
        let [s, f, _] = &self.manual_text;
        self.placeholder = s.is_some() || f.is_some();
        self.no_dates = s.is_some() && f.is_some();
    }

    /// A new manually scheduled task with nothing entered but its name (the standard).
    pub fn set_nothing_entered(&mut self) {
        if self.manual && !self.is_summary && !self.milestone {
            self.manual_text = [Some(String::new()), Some(String::new()), Some(String::new())];
            self.estimated = true;
            self.sync_manual_text();
        }
    }

    /// Nothing entered at all: MPP draws no bar.
    pub fn nothing_entered(&self) -> bool {
        self.manual && self.manual_text.iter().all(|x| x.is_some())
    }

    pub fn is_milestone(&self) -> bool {
        self.milestone || (self.duration == 0 && !self.is_summary)
    }

    /// Give the task a "Start No Earlier Than" constraint (what MPP does when a start date is typed).
    pub fn set_snet(&mut self, d: NaiveDate) {
        self.constraint_type = ConstraintType::Snet;
        self.constraint_date = Some(d);
        self.constraint_dt = None;
    }

    /// "Start No Earlier Than" an exact instant (what MPP does when a bar is dragged).
    pub fn set_snet_at(&mut self, t: NaiveDateTime) {
        self.constraint_type = ConstraintType::Snet;
        self.constraint_date = Some(t.date());
        self.constraint_dt = Some(t);
    }

    pub fn clear_constraint(&mut self) {
        self.constraint_type = ConstraintType::Asap;
        self.constraint_date = None;
        self.constraint_dt = None;
    }

    /// The constraint's instant: the exact one when set, else the start of the working day for
    /// start constraints and its end for finish constraints.
    pub fn constraint_instant(&self, cal: &Calendar) -> Option<NaiveDateTime> {
        let d = self.constraint_date?;
        if let Some(t) = self.constraint_dt.filter(|t| t.date() == d) {
            return Some(t);
        }
        Some(match self.constraint_type {
            ConstraintType::Fnet | ConstraintType::Fnlt | ConstraintType::Mfo => cal.day_end(d),
            _ => cal.day_start(d),
        })
    }

    /// Files written before constraint types existed only stored SNET dates.
    pub fn normalize_constraint(&mut self) {
        if self.constraint_date.is_some() && !self.constraint_type.needs_date() {
            self.constraint_type = ConstraintType::Snet;
        }
        if self.constraint_date.is_none() && self.constraint_type.needs_date() {
            self.constraint_type = ConstraintType::Asap;
        }
    }

    /// Effective format of a cell: row format ("*") then the column's own format.
    pub fn cell_format(&self, column: &str) -> crate::format::CellFormat {
        let mut f = self.cell_formats.get("*").cloned().unwrap_or_default();
        if let Some(c) = self.cell_formats.get(column).filter(|c| c.whole) {
            // MPP: the cell's own format replaces the row's
            f = c.clone();
        } else if let Some(c) = self.cell_formats.get(column) {
            f.bold |= c.bold;
            f.italic |= c.italic;
            f.underline |= c.underline;
            f.strike |= c.strike;
            if c.color.is_some() {
                f.color = c.color;
            }
            if c.background.is_some() {
                f.background = c.background;
            }
            if c.size.is_some() {
                f.size = c.size;
            }
            if c.font.is_some() {
                f.font = c.font.clone();
            }
        }
        // legacy row-level toggles
        f.bold |= self.bold;
        f.italic |= self.italic;
        f.underline |= self.underline;
        f
    }

    /// Toggle one attribute of a cell format ("*" = whole row), dropping empty entries.
    pub fn edit_cell_format(&mut self, column: &str, f: impl FnOnce(&mut crate::format::CellFormat)) {
        let mut cf = self.cell_formats.get(column).cloned().unwrap_or_default();
        f(&mut cf);
        if cf.is_empty() {
            self.cell_formats.remove(column);
        } else {
            self.cell_formats.insert(column.to_string(), cf);
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Calendar {
    pub name: String,
    /// Working flag for Mon..Sun (index 0 = Monday).
    pub working_days: [bool; 7],
    pub hours_per_day: f64,
    #[serde(default)]
    pub holidays: Vec<Holiday>,
    /// Working times of each weekday (Mon..Sun) in minutes from midnight; empty = the standard
    /// standard 08:00-12:00, 13:00-17:00 on working days.
    #[serde(default)]
    pub work_times: Vec<Vec<(u16, u16)>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Holiday {
    pub name: String,
    pub start: NaiveDate,
    pub finish: NaiveDate,
}

impl Default for Calendar {
    fn default() -> Self {
        Calendar {
            name: "Standard".into(),
            working_days: [true, true, true, true, true, false, false],
            hours_per_day: 8.0,
            holidays: Vec::new(),
            work_times: Vec::new(),
        }
    }
}

/// The standard working day.
pub const STANDARD_DAY: [(u16, u16); 2] = [(8 * 60, 12 * 60), (13 * 60, 17 * 60)];

fn minus_one() -> i64 {
    -1
}

impl Calendar {
    /// Working intervals (minutes from midnight) of a date; empty on non-working days.
    pub fn day_intervals(&self, d: NaiveDate) -> &[(u16, u16)] {
        if !self.is_working(d) {
            return &[];
        }
        let wd = d.weekday().num_days_from_monday() as usize;
        match self.work_times.get(wd) {
            Some(v) if !v.is_empty() => v,
            _ => &STANDARD_DAY,
        }
    }

    /// Minutes in a working day for converting durations ("days" -> minutes): Options > Schedule
    /// hours per day.
    pub fn minutes_per_day(&self) -> i64 {
        (self.hours_per_day * 60.0).round().max(1.0) as i64
    }

    /// Start of the working day `d` (first working minute) or 08:00.
    pub fn day_start(&self, d: NaiveDate) -> NaiveDateTime {
        let m = self.day_intervals(d).first().map(|x| x.0).unwrap_or(8 * 60);
        d.and_time(NaiveTime::from_hms_opt((m / 60) as u32, (m % 60) as u32, 0).unwrap())
    }

    /// End of the working day `d` (last working minute) or 17:00.
    pub fn day_end(&self, d: NaiveDate) -> NaiveDateTime {
        let m = self.day_intervals(d).last().map(|x| x.1).unwrap_or(17 * 60);
        if m >= 24 * 60 {
            return (d + Duration::days(1)).and_time(NaiveTime::MIN);
        }
        d.and_time(NaiveTime::from_hms_opt((m / 60) as u32, (m % 60) as u32, 0).unwrap())
    }

    /// Working intervals of a date for minute arithmetic (a calendar without any working weekday
    /// works the standard hours every day).
    fn work_iv(&self, d: NaiveDate) -> &[(u16, u16)] {
        if self.has_any_working_day() {
            self.day_intervals(d)
        } else {
            &STANDARD_DAY
        }
    }

    /// Working minutes from `a` to `b` (negative when `b` is before `a`).
    pub fn work_minutes(&self, a: NaiveDateTime, b: NaiveDateTime) -> i64 {
        if b < a {
            return -self.work_minutes(b, a);
        }
        let mut acc = 0;
        let mut d = a.date();
        let mut guard = 0;
        while d <= b.date() && guard < 40_000 {
            let lo = if d == a.date() { (a.hour() * 60 + a.minute()) as i64 } else { 0 };
            let hi = if d == b.date() { (b.hour() * 60 + b.minute()) as i64 } else { 1440 };
            for (x, y) in self.work_iv(d) {
                let (s, e) = ((*x as i64).max(lo), (*y as i64).min(hi));
                if e > s {
                    acc += e - s;
                }
            }
            d += Duration::days(1);
            guard += 1;
        }
        acc
    }

    /// First working instant at or after `t` (the end of a working period moves on to the next).
    pub fn next_work_instant(&self, t: NaiveDateTime) -> NaiveDateTime {
        let mut d = t.date();
        let mut m = (t.hour() * 60 + t.minute()) as i64;
        for _ in 0..3660 {
            for (x, y) in self.work_iv(d) {
                if m < *y as i64 {
                    return d.and_time(NaiveTime::MIN) + Duration::minutes(m.max(*x as i64));
                }
            }
            d += Duration::days(1);
            m = 0;
        }
        t
    }

    /// Last working instant at or before `t` (the start of a working period moves back to the end
    /// of the previous one).
    pub fn prev_work_end(&self, t: NaiveDateTime) -> NaiveDateTime {
        let mut d = t.date();
        let mut m = (t.hour() * 60 + t.minute()) as i64;
        for _ in 0..3660 {
            for (x, y) in self.work_iv(d).iter().rev() {
                if m > *x as i64 {
                    return d.and_time(NaiveTime::MIN) + Duration::minutes(m.min(*y as i64));
                }
            }
            d -= Duration::days(1);
            m = 1440;
        }
        t
    }

    /// Where a manually scheduled task of `m` working minutes starting at `t` finishes (MS
    /// MPP): its start day counts as a working day (a weekend or a holiday takes the usual
    /// hours) and the non-working stretch the start falls in counts as working time, up to the
    /// next working period of that day or midnight (01:00 + 1 day = 09:00, 19:00 + 1 day = 11:00
    /// the next day, 12:30 + 1 day = 11:30 the next day; from 12:00 or 17:00, the end of a period,
    /// nothing extra).
    pub fn manual_finish(&self, t: NaiveDateTime, m: i64) -> NaiveDateTime {
        if m <= 0 {
            return t;
        }
        let d = t.date();
        let iv: Vec<(u16, u16)> = {
            let own = self.work_iv(d);
            if !own.is_empty() {
                own.to_vec()
            } else {
                let wd = d.weekday().num_days_from_monday() as usize;
                let usual = |k: usize| self.work_times.get(k).filter(|v| !v.is_empty()).cloned();
                usual(wd).or_else(|| (0..7).filter(|&k| self.working_days[k]).find_map(usual)).unwrap_or_else(|| STANDARD_DAY.to_vec())
            }
        };
        let cur = (t.hour() * 60 + t.minute()) as i64;
        let mut left = m;
        let mut pos = cur;
        let inside = iv.iter().any(|(a, b)| cur >= *a as i64 && cur < *b as i64);
        let at_period_end = iv.iter().any(|(_, b)| cur == *b as i64);
        if !inside && !at_period_end {
            let gap_end = iv.iter().map(|(a, _)| *a as i64).filter(|a| *a > cur).min().unwrap_or(1440);
            let take = (gap_end - cur).min(left);
            left -= take;
            pos = cur + take;
            if left == 0 {
                return d.and_time(NaiveTime::MIN) + Duration::minutes(pos);
            }
        }
        for (a, b) in &iv {
            let s = (*a as i64).max(pos);
            let e = *b as i64;
            if e > s {
                if left <= e - s {
                    return d.and_time(NaiveTime::MIN) + Duration::minutes(s + left);
                }
                left -= e - s;
            }
        }
        self.add_work_minutes((d + Duration::days(1)).and_time(NaiveTime::MIN), left)
    }

    /// The instant `m` working minutes after `t` (the end of a working period when it lands on one).
    pub fn add_work_minutes(&self, t: NaiveDateTime, m: i64) -> NaiveDateTime {
        if m <= 0 {
            return t;
        }
        let mut left = m;
        let mut d = t.date();
        let mut cur = (t.hour() * 60 + t.minute()) as i64;
        for _ in 0..40_000 {
            for (x, y) in self.work_iv(d) {
                let s = (*x as i64).max(cur);
                let e = *y as i64;
                if e > s {
                    if left <= e - s {
                        return d.and_time(NaiveTime::MIN) + Duration::minutes(s + left);
                    }
                    left -= e - s;
                }
            }
            d += Duration::days(1);
            cur = 0;
        }
        t
    }

    /// The instant `m` working minutes before `t` (the start of a working period when it lands on one).
    pub fn sub_work_minutes(&self, t: NaiveDateTime, m: i64) -> NaiveDateTime {
        if m <= 0 {
            return t;
        }
        let mut left = m;
        let mut d = t.date();
        let mut cur = (t.hour() * 60 + t.minute()) as i64;
        for _ in 0..40_000 {
            for (x, y) in self.work_iv(d).iter().rev() {
                let e = (*y as i64).min(cur);
                let s = *x as i64;
                if e > s {
                    if left <= e - s {
                        return d.and_time(NaiveTime::MIN) + Duration::minutes(e - left);
                    }
                    left -= e - s;
                }
            }
            d -= Duration::days(1);
            cur = 1440;
        }
        t
    }

    pub fn is_working(&self, d: NaiveDate) -> bool {
        let wd = d.weekday().num_days_from_monday() as usize;
        if !self.working_days[wd] {
            return false;
        }
        !self.holidays.iter().any(|h| d >= h.start && d <= h.finish)
    }

    fn has_any_working_day(&self) -> bool {
        self.working_days.iter().any(|w| *w)
    }

    /// First working day on or after `d`.
    pub fn next_working(&self, mut d: NaiveDate) -> NaiveDate {
        if !self.has_any_working_day() {
            return d;
        }
        for _ in 0..3660 {
            if self.is_working(d) {
                return d;
            }
            d += Duration::days(1);
        }
        d
    }

    /// Move `n` working days from working day `d` (n may be negative).
    pub fn add_work(&self, d: NaiveDate, n: i32) -> NaiveDate {
        if !self.has_any_working_day() {
            return d + Duration::days(n as i64);
        }
        let mut d = d;
        let step = if n >= 0 { 1 } else { -1 };
        let mut left = n.abs();
        while left > 0 {
            d += Duration::days(step);
            if self.is_working(d) {
                left -= 1;
            }
        }
        d
    }

    /// Finish date (inclusive) for a task starting on `start` lasting `dur` working days.
    pub fn finish_for(&self, start: NaiveDate, dur: i32) -> NaiveDate {
        let s = self.next_working(start);
        if dur <= 1 {
            return s;
        }
        self.add_work(s, dur - 1)
    }

}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub title: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub company: String,
    #[serde(default)]
    pub manager: String,
    pub start: NaiveDate,
    #[serde(default)]
    pub status_date: Option<NaiveDate>,
    pub calendar: Calendar,
    pub tasks: Vec<Task>,
    pub resources: Vec<Resource>,
    pub next_uid: u32,
    pub next_res_uid: u32,
    /// New tasks are auto-scheduled by default.
    #[serde(default = "default_true")]
    pub new_tasks_auto: bool,
    /// Gantt Chart view formatting (Format tab dialogs).
    #[serde(default)]
    pub formatting: crate::format::Formatting,
    /// Project Options that belong to the project (calendar, schedule, display).
    #[serde(default)]
    pub options: ProjectOptions,
    /// Format the file was read from, e.g. `Some(12)` for an MPP12 .mpp (shown as Compatibility Mode).
    #[serde(skip)]
    pub file_format: Option<u8>,
    /// Show the project summary task (row 0) in task views.
    #[serde(default)]
    pub show_project_summary: bool,
    /// The view that was active when an MPP .mpp was saved (columns, bar styles, text styles,
    /// timescale, cell formats); applied when the file is opened. Not saved.
    #[serde(skip)]
    pub saved_view: Option<Box<crate::mpp::SavedView>>,
    /// Exact project start (with time of day) when the file gives one: milestones at the project
    /// start are shown at that instant, as in MPP.
    #[serde(default)]
    pub start_dt: Option<NaiveDateTime>,
    /// When each baseline was saved (Set Baseline shows "last saved on ...").
    #[serde(default)]
    pub baseline_saved: Vec<(u8, NaiveDateTime)>,
    /// Project > WBS > Define Code (None = the outline numbers)
    #[serde(default)]
    pub wbs_mask: Option<crate::wbs::WbsMask>,
    /// Project > Custom Fields: renamed fields, lookup lists, formulas
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_defs: Vec<crate::custom::CustomFieldDef>,
    /// Project > Manage Sprints
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sprints: Vec<Sprint>,
    /// Format > Drawing: shapes on the Gantt chart
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drawings: Vec<crate::drawing::Drawing>,
    /// Resource > Resource Pool > Share Resources: the file this project's resources come from
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_pool: Option<ResourcePool>,
    /// Project > Payment Schedule (งวดงาน / งวดเงิน)
    #[serde(default, skip_serializing_if = "PaymentPlan::is_empty")]
    pub payments: PaymentPlan,
    /// Native project controls: structured BOQ, physical progress observations and posted costs.
    #[serde(default, skip_serializing_if = "crate::controls::ProjectControls::is_empty")]
    pub controls: crate::controls::ProjectControls,
}

/// One installment of the contract (งวดงาน): paid when its task (usually a milestone) is done.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Payment {
    pub name: String,
    /// share of the contract value, %
    #[serde(default)]
    pub percent: f64,
    /// a fixed amount instead of the percentage (0 = use the percentage)
    #[serde(default)]
    pub amount: f64,
    /// the task whose completion makes the installment due
    #[serde(default)]
    pub task_uid: Option<u32>,
    #[serde(default)]
    pub paid: bool,
    #[serde(default)]
    pub note: String,
}

/// The contract's payment terms and installments.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PaymentPlan {
    /// contract value (0 = the project's total cost)
    #[serde(default)]
    pub contract_value: f64,
    /// advance payment (เงินล่วงหน้า), % of the contract, recovered from each installment
    #[serde(default)]
    pub advance_pct: f64,
    /// retention (เงินประกันผลงาน), % withheld from each installment
    #[serde(default)]
    pub retention_pct: f64,
    #[serde(default)]
    pub items: Vec<Payment>,
}

impl PaymentPlan {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && self.contract_value == 0.0 && self.advance_pct == 0.0 && self.retention_pct == 0.0
    }
}

pub fn default_true() -> bool {
    true
}

/// Project Options stored with the project (Options > Schedule "for this project", Display, Advanced).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectOptions {
    /// 0 = Sunday .. 6 = Saturday
    #[serde(default)]
    pub week_starts_on: u8,
    /// 1..=12
    #[serde(default = "default_one")]
    pub fiscal_year_start: u8,
    #[serde(default = "default_start_time")]
    pub default_start_time: (u32, u32),
    #[serde(default = "default_end_time")]
    pub default_end_time: (u32, u32),
    #[serde(default = "default_hpw")]
    pub hours_per_week: f64,
    #[serde(default = "default_dpm")]
    pub days_per_month: f64,
    #[serde(default)]
    pub new_tasks_start_today: bool,
    /// 0 minutes, 1 hours, 2 days, 3 weeks, 4 months
    #[serde(default = "default_two")]
    pub duration_units: u8,
    /// 0 minutes, 1 hours, 2 days, 3 weeks, 4 months
    #[serde(default = "default_one")]
    pub work_units: u8,
    #[serde(default)]
    pub default_task_type: TaskType,
    #[serde(default)]
    pub new_tasks_effort_driven: bool,
    #[serde(default)]
    pub autolink: bool,
    #[serde(default = "default_true")]
    pub split_in_progress: bool,
    #[serde(default = "default_true")]
    pub honor_constraints: bool,
    #[serde(default = "default_true")]
    pub show_estimated: bool,
    #[serde(default = "default_true")]
    pub new_tasks_estimated: bool,
    #[serde(default = "default_true")]
    pub keep_on_working_day: bool,
    #[serde(default = "default_currency")]
    pub currency_symbol: String,
    /// 0 = $1, 1 = 1$, 2 = $ 1, 3 = 1 $
    #[serde(default)]
    pub currency_position: u8,
    #[serde(default = "default_two")]
    pub currency_digits: u8,
    #[serde(default = "default_currency_code")]
    pub currency_code: String,
    /// Display options for this project: duration unit labels (minute, hour, day, week, month, year)
    #[serde(default = "default_labels")]
    pub duration_labels: [String; 6],
    #[serde(default = "default_true")]
    pub space_before_label: bool,
    #[serde(default = "default_true")]
    pub underline_hyperlinks: bool,
    #[serde(default = "default_true")]
    pub indent_name: bool,
    #[serde(default = "default_true")]
    pub show_outline_symbol: bool,
    /// Resource > Leveling Options
    #[serde(default)]
    pub leveling: LevelingOptions,
}

/// A sprint (Project > Manage Sprints): a named period tasks are planned into.
/// Where an external task lives: its project file, unique ID and ID.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExternalTask {
    pub path: String,
    pub uid: u32,
    pub id: u32,
}

/// Share Resources: the pool file and which side wins a conflict (MPP: pool takes precedence).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResourcePool {
    pub path: String,
    #[serde(default)]
    pub sharer_first: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sprint {
    pub uid: u32,
    pub name: String,
    pub start: NaiveDate,
    pub finish: NaiveDate,
}

/// Resource > Leveling Options (the standard dialog).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LevelingOptions {
    /// Leveling calculations: Automatic (level after every change) or Manual
    #[serde(default)]
    pub automatic: bool,
    /// Look for overallocations on a ... basis: 0 minute, 1 hour, 2 day, 3 week, 4 month
    #[serde(default = "default_two")]
    pub basis: u8,
    #[serde(default = "default_true")]
    pub clear_first: bool,
    /// Level from / to (None = the entire project)
    #[serde(default)]
    pub range: Option<(NaiveDate, NaiveDate)>,
    /// Leveling order: 0 ID Only, 1 Standard, 2 Priority, Standard
    #[serde(default = "default_one")]
    pub order: u8,
    #[serde(default)]
    pub within_slack: bool,
    #[serde(default = "default_true")]
    pub adjust_assignments: bool,
    #[serde(default = "default_true")]
    pub create_splits: bool,
    #[serde(default)]
    pub proposed: bool,
    #[serde(default = "default_true")]
    pub manual_tasks: bool,
}

impl Default for LevelingOptions {
    fn default() -> Self {
        LevelingOptions {
            automatic: false,
            basis: 2,
            clear_first: true,
            range: None,
            order: 1,
            within_slack: false,
            adjust_assignments: true,
            create_splits: true,
            proposed: false,
            manual_tasks: true,
        }
    }
}

fn default_one() -> u8 {
    1
}
fn default_two() -> u8 {
    2
}
fn default_start_time() -> (u32, u32) {
    (8, 0)
}
fn default_end_time() -> (u32, u32) {
    (17, 0)
}
fn default_hpw() -> f64 {
    40.0
}
fn default_dpm() -> f64 {
    20.0
}
fn default_currency() -> String {
    "$".into()
}
fn default_currency_code() -> String {
    "USD".into()
}
fn default_labels() -> [String; 6] {
    ["min".into(), "hr".into(), "day".into(), "wk".into(), "mon".into(), "yr".into()]
}

impl Default for ProjectOptions {
    fn default() -> Self {
        ProjectOptions {
            week_starts_on: 0,
            fiscal_year_start: 1,
            default_start_time: (8, 0),
            default_end_time: (17, 0),
            hours_per_week: 40.0,
            days_per_month: 20.0,
            new_tasks_start_today: false,
            duration_units: 2,
            work_units: 1,
            default_task_type: TaskType::FixedUnits,
            new_tasks_effort_driven: false,
            autolink: false,
            split_in_progress: true,
            honor_constraints: true,
            show_estimated: true,
            new_tasks_estimated: true,
            keep_on_working_day: true,
            currency_symbol: "$".into(),
            currency_position: 0,
            currency_digits: 2,
            currency_code: "USD".into(),
            duration_labels: default_labels(),
            space_before_label: true,
            underline_hyperlinks: true,
            indent_name: true,
            show_outline_symbol: true,
            leveling: LevelingOptions::default(),
        }
    }
}

impl Project {
    /// The standard project summary task (ID 0): the whole project rolled up - its dates, duration,
    /// % complete, work, cost and baseline - as Format > Project Summary Task shows it.
    pub fn project_summary_task(&self, name: &str) -> Task {
        let cal = &self.calendar;
        let mpd = cal.minutes_per_day();
        let mut t = Task::new(0, name, self.start);
        t.outline_level = 0;
        t.is_summary = true;
        let active: Vec<&Task> = self.tasks.iter().filter(|t| t.active && t.external.is_none()).collect();
        let pstart = self.start_dt.unwrap_or_else(|| cal.day_start(self.start));
        let start = active.iter().filter_map(|t| t.start_dt).min().unwrap_or(pstart);
        let finish = active.iter().filter_map(|t| t.finish_dt).max().unwrap_or(start);
        t.start_dt = Some(start);
        t.finish_dt = Some(finish);
        t.start = start.date();
        t.finish = finish.date();
        t.dur_min = cal.work_minutes(start, finish).max(0);
        t.duration = (t.dur_min / mpd.max(1)) as i32;
        t.dur_unit = DurUnit::Days;
        let top: Vec<&Task> = active.iter().copied().filter(|t| t.outline_level == 1).collect();
        t.work_min = top.iter().map(|t| t.work_min).sum();
        t.cost = top.iter().map(|t| t.cost).sum();
        t.actual_cost = top.iter().map(|t| t.actual_cost).sum();
        t.fixed_cost = 0.0;
        let leaves: Vec<&Task> = active.iter().copied().filter(|t| !t.is_summary).collect();
        let (sum_dur, sum_act) = leaves.iter().map(|t| (t.dur_minutes(mpd), t.actual_minutes(mpd).min(t.dur_minutes(mpd)))).fold((0i64, 0i64), |a, b| (a.0 + b.0, a.1 + b.1));
        t.percent_complete = pct_of(sum_act, sum_dur);
        t.actual_start = active.iter().filter_map(|t| t.actual_start).min();
        t.actual_finish = if t.percent_complete == 100 { active.iter().filter_map(|t| t.actual_finish).max() } else { None };
        t.actual_dur_min = None;
        t.priority = 500;
        // the baselines of the whole project
        let mut nums: Vec<u8> = top.iter().flat_map(|t| t.baselines.iter().map(|b| b.number)).collect();
        nums.sort_unstable();
        nums.dedup();
        for n in nums {
            let bs: Vec<&Baseline> = top.iter().filter_map(|t| t.baselines.iter().find(|b| b.number == n)).collect();
            let (bs0, bf0) = (bs.iter().filter_map(|b| b.start).min(), bs.iter().filter_map(|b| b.finish).max());
            t.baselines.push(Baseline {
                number: n,
                start: bs0,
                finish: bf0,
                dur_min: match (bs0, bf0) {
                    (Some(a), Some(b)) => Some(cal.work_minutes(a, b).max(0)),
                    _ => None,
                },
                work_min: Some(bs.iter().filter_map(|b| b.work_min).sum()),
                cost: Some(bs.iter().filter_map(|b| b.cost).sum()),
            });
        }
        t
    }

    /// A new plan for a Thai user (Thai display language or a Thai Windows locale): baht.
    pub fn localize_new(&mut self) {
        if crate::i18n::thai_user() {
            self.options.currency_symbol = "฿".into();
            self.options.currency_code = "THB".into();
            self.options.currency_position = 0;
        }
    }

    /// Make money show with this project's currency options. Plans that still carry the built-in
    /// "$" / USD default (files that never set a currency, .mpp read without one) show baht to a
    /// Thai user, as they always did.
    pub fn apply_currency(&self) {
        let o = &self.options;
        let sym = if o.currency_symbol == "$" && o.currency_code == "USD" && crate::i18n::thai_user() { "฿" } else { o.currency_symbol.as_str() };
        crate::util::set_currency(sym, o.currency_position, o.currency_digits);
    }

    pub fn new() -> Self {
        let today = chrono::Local::now().date_naive();
        let cal = Calendar::default();
        let start = cal.next_working(today);
        Project {
            title: String::new(),
            author: String::new(),
            company: String::new(),
            manager: String::new(),
            start,
            status_date: None,
            calendar: cal,
            tasks: Vec::new(),
            resources: Vec::new(),
            next_uid: 1,
            next_res_uid: 1,
            new_tasks_auto: true,
            formatting: crate::format::Formatting::default(),
            options: ProjectOptions::default(),
            file_format: None,
            show_project_summary: false,
            saved_view: None,
            start_dt: None,
            baseline_saved: Vec::new(),
            wbs_mask: None,
            custom_defs: Vec::new(),
            sprints: Vec::new(),
            resource_pool: None,
            drawings: Vec::new(),
            payments: PaymentPlan::default(),
            controls: crate::controls::ProjectControls::default(),
        }
    }

    pub fn alloc_uid(&mut self) -> u32 {
        let u = self.next_uid;
        self.next_uid += 1;
        u
    }

    pub fn alloc_res_uid(&mut self) -> u32 {
        let u = self.next_res_uid;
        self.next_res_uid += 1;
        u
    }

    pub fn index_of_uid(&self, uid: u32) -> Option<usize> {
        self.tasks.iter().position(|t| t.uid == uid)
    }

    pub fn resource_by_uid(&self, uid: u32) -> Option<&Resource> {
        self.resources.iter().find(|r| r.uid == uid)
    }

    pub fn resource_by_name(&self, name: &str) -> Option<&Resource> {
        self.resources.iter().find(|r| r.name.eq_ignore_ascii_case(name))
    }

    /// Create a new task at `idx` with the given name and default scheduling.
    pub fn new_task(&mut self, name: &str) -> Task {
        let uid = self.alloc_uid();
        let mut t = Task::new(uid, name, self.start);
        t.manual = !self.new_tasks_auto;
        t.estimated = self.options.new_tasks_estimated;
        t.task_type = self.options.default_task_type;
        t.effort_driven = self.options.new_tasks_effort_driven;
        t.finish = self.calendar.finish_for(t.start, t.duration);
        t
    }

    /// Range of child rows (exclusive end) for summary at `idx`.
    pub fn children_end(&self, idx: usize) -> usize {
        let lvl = self.tasks[idx].outline_level;
        let mut j = idx + 1;
        while j < self.tasks.len() && self.tasks[j].outline_level > lvl {
            j += 1;
        }
        j
    }

    /// Index of the parent summary for task at idx.
    pub fn parent_of(&self, idx: usize) -> Option<usize> {
        let lvl = self.tasks[idx].outline_level;
        (0..idx).rev().find(|&j| self.tasks[j].outline_level < lvl)
    }

    /// `parent_of` for every task, in one pass.
    pub fn parents(&self) -> Vec<Option<usize>> {
        let mut out = Vec::with_capacity(self.tasks.len());
        // the open summaries above the current row, outline levels strictly rising
        let mut stack: Vec<usize> = Vec::new();
        for (i, t) in self.tasks.iter().enumerate() {
            while stack.last().is_some_and(|&k| self.tasks[k].outline_level >= t.outline_level) {
                stack.pop();
            }
            out.push(stack.last().copied());
            stack.push(i);
        }
        out
    }

    /// `is_hidden` for every task, in one pass.
    pub fn hidden_rows(&self, parents: &[Option<usize>]) -> Vec<bool> {
        let mut out = vec![false; self.tasks.len()];
        for i in 0..self.tasks.len() {
            if let Some(p) = parents[i] {
                out[i] = out[p] || self.tasks[p].collapsed;
            }
        }
        out
    }

    /// Whether row idx is hidden by a collapsed ancestor.
    pub fn is_hidden(&self, idx: usize) -> bool {
        let mut lvl = self.tasks[idx].outline_level;
        let mut j = idx;
        while j > 0 && lvl > 1 {
            j -= 1;
            let t = &self.tasks[j];
            if t.outline_level < lvl {
                if t.collapsed {
                    return true;
                }
                lvl = t.outline_level;
            }
        }
        false
    }

    /// Outline number like "1.2.3".
    pub fn outline_numbers(&self) -> Vec<String> {
        let mut counters: Vec<u32> = Vec::new();
        let mut out = Vec::with_capacity(self.tasks.len());
        for t in &self.tasks {
            let lvl = t.outline_level.max(1) as usize;
            counters.truncate(lvl);
            while counters.len() < lvl {
                counters.push(0);
            }
            counters[lvl - 1] += 1;
            out.push(counters.iter().map(|c| c.to_string()).collect::<Vec<_>>().join("."));
        }
        out
    }

    pub fn finish_date(&self) -> NaiveDate {
        self.tasks.iter().map(|t| t.finish).max().unwrap_or(self.start)
    }

    pub fn resource_names(&self, t: &Task) -> String {
        t.resources
            .iter()
            .filter_map(|u| self.resource_by_uid(*u).map(|r| r.name.clone()))
            .collect::<Vec<_>>()
            .join(",")
    }
}

impl Default for Project {
    fn default() -> Self {
        Self::new()
    }
}

