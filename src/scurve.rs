//! S-Curve: cumulative planned progress against cumulative actual progress, each task weighted by
//! its share of the project (น้ำหนักงาน) — the progress chart Thai owners and consultants ask for in
//! every monthly report.
//!
//! - Weight: the task's cost, else its work, else its duration (or one of them chosen).
//! - Plan: the baseline dates when the plan has a baseline, else the current schedule; with a
//!   baseline the current schedule is drawn as a third curve (the forecast).
//! - Actual: each task's % complete spread over its working days from its actual start to its
//!   actual finish (or to the status date while it is in progress).

use crate::model::{Project, Task};
use chrono::{Datelike, Duration, NaiveDate};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Basis {
    Cost,
    Work,
    Duration,
}

impl Basis {
    pub fn label(self) -> &'static str {
        match self {
            Basis::Cost => "Weights from task cost",
            Basis::Work => "Weights from task work",
            Basis::Duration => "Weights from task duration",
        }
    }
}

#[derive(Clone, Debug)]
pub struct GroupRow {
    pub name: String,
    /// share of the project, %
    pub weight: f64,
    /// planned and actual progress of the group itself at the status date, %
    pub plan: f64,
    pub actual: f64,
}

#[derive(Clone, Debug)]
pub struct SCurve {
    /// period boundaries: `ends[0]` is the start (0 %), `ends[k]` the first day after period k
    pub ends: Vec<NaiveDate>,
    pub weekly: bool,
    /// cumulative %, one value per boundary
    pub plan: Vec<f64>,
    /// the current schedule, when the plan comes from a baseline
    pub current: Option<Vec<f64>>,
    /// cumulative %, boundaries up to the period holding the status date
    pub actual: Vec<f64>,
    pub basis: Basis,
    pub plan_at_status: f64,
    pub actual_at_status: f64,
    /// False only for legacy projects: physical progress is then a reconstructed estimate.
    pub actual_is_recorded: bool,
    pub groups: Vec<GroupRow>,
}

/// Working days before each date of a range, so a task's share of any period is O(1).
struct Days {
    lo: NaiveDate,
    pre: Vec<i64>,
}

impl Days {
    fn new(p: &Project, lo: NaiveDate, hi: NaiveDate) -> Days {
        let n = ((hi - lo).num_days().max(0) + 2) as usize;
        let mut pre = Vec::with_capacity(n + 1);
        pre.push(0);
        let mut d = lo;
        for _ in 0..n {
            let last = *pre.last().unwrap();
            pre.push(last + p.calendar.is_working(d) as i64);
            d += Duration::days(1);
        }
        Days { lo, pre }
    }

    fn before(&self, d: NaiveDate) -> i64 {
        let k = (d - self.lo).num_days().clamp(0, self.pre.len() as i64 - 1) as usize;
        self.pre[k]
    }

    /// Fraction of the work of a task running `s..=f` done before `e`.
    fn frac(&self, s: NaiveDate, f: NaiveDate, e: NaiveDate) -> f64 {
        if f < s {
            return if s < e { 1.0 } else { 0.0 };
        }
        let end = f + Duration::days(1);
        let total = self.before(end) - self.before(s);
        if total == 0 {
            // only non-working days: spread over calendar days
            let all = (end - s).num_days().max(1) as f64;
            return ((e.min(end) - s).num_days().max(0) as f64 / all).min(1.0);
        }
        let upto = e.clamp(s, end);
        (self.before(upto) - self.before(s)) as f64 / total as f64
    }
}

fn plan_dates(t: &Task) -> (NaiveDate, NaiveDate) {
    match t.baseline(0).and_then(|b| Some((b.start?.date(), b.finish?.date()))) {
        Some((s, f)) => (s, f),
        None => (t.start, t.finish),
    }
}

fn actual_dates(t: &Task, status: NaiveDate) -> (NaiveDate, NaiveDate) {
    let s = t.actual_start.map(|d| d.date()).unwrap_or(t.start);
    let f = if t.percent_complete >= 100 { t.actual_finish.map(|d| d.date()).unwrap_or(t.finish) } else { status.max(s) };
    (s, f.max(s))
}

fn next_month(d: NaiveDate) -> NaiveDate {
    if d.month() == 12 { NaiveDate::from_ymd_opt(d.year() + 1, 1, 1) } else { NaiveDate::from_ymd_opt(d.year(), d.month() + 1, 1) }.unwrap_or(d + Duration::days(31))
}

/// `basis`: 0 automatic, 1 cost, 2 work, 3 duration. `period`: 0 automatic, 1 weekly, 2 monthly.
pub fn compute(p: &Project, status: NaiveDate, basis: u8, period: u8) -> SCurve {
    let leaves: Vec<usize> = (0..p.tasks.len()).filter(|&i| !p.tasks[i].is_summary && p.tasks[i].active).collect();
    let sum = |f: &dyn Fn(&Task) -> f64| leaves.iter().map(|&i| f(&p.tasks[i])).sum::<f64>();
    let cost = |t: &Task| t.cost.max(0.0);
    let work = |t: &Task| t.work_min.max(0) as f64;
    let dur = |t: &Task| t.dur_min.max(0) as f64;
    let basis = match basis {
        1 => Basis::Cost,
        2 => Basis::Work,
        3 => Basis::Duration,
        _ if sum(&cost) > 0.0 => Basis::Cost,
        _ if sum(&work) > 0.0 => Basis::Work,
        _ => Basis::Duration,
    };
    let weight = |t: &Task| match basis {
        Basis::Cost => cost(t),
        Basis::Work => work(t),
        Basis::Duration => dur(t),
    };
    let mut w: Vec<f64> = leaves.iter().map(|&i| weight(&p.tasks[i])).collect();
    let mut total: f64 = w.iter().sum();
    if total <= 0.0 {
        // nothing to weigh (all milestones, no costs): every task counts the same
        w = vec![1.0; leaves.len()];
        total = leaves.len().max(1) as f64;
    }
    let has_baseline = leaves.iter().any(|&i| p.tasks[i].baseline(0).is_some_and(|b| b.start.is_some() && b.finish.is_some()));
    let pd: Vec<(NaiveDate, NaiveDate)> = leaves.iter().map(|&i| plan_dates(&p.tasks[i])).collect();
    let cd: Vec<(NaiveDate, NaiveDate)> = leaves.iter().map(|&i| (p.tasks[i].start, p.tasks[i].finish)).collect();
    let ad: Vec<(NaiveDate, NaiveDate)> = leaves.iter().map(|&i| actual_dates(&p.tasks[i], status)).collect();

    let lo = pd.iter().chain(cd.iter()).map(|x| x.0).min().unwrap_or(p.start).min(p.start);
    let hi = pd.iter().chain(cd.iter()).chain(ad.iter()).map(|x| x.1.max(x.0)).max().unwrap_or(lo).max(lo);
    let weekly = match period {
        1 => true,
        2 => false,
        _ => (hi - lo).num_days() <= 183,
    };
    // period boundaries
    let mut ends = vec![lo];
    let mut e = if weekly {
        let ws = p.options.week_starts_on.min(6) as i64;
        let wd = lo.weekday().num_days_from_sunday() as i64;
        lo + Duration::days(7 - (wd - ws).rem_euclid(7))
    } else {
        next_month(lo)
    };
    let last = hi + Duration::days(1);
    while ends.len() < 400 {
        ends.push(e);
        if e >= last {
            break;
        }
        e = if weekly { e + Duration::days(7) } else { next_month(e) };
    }
    let days = Days::new(p, lo, (*ends.last().unwrap()).max(status + Duration::days(1)));
    let cum = |dates: &[(NaiveDate, NaiveDate)], share: &dyn Fn(usize) -> f64, at: NaiveDate| -> f64 {
        dates.iter().enumerate().map(|(k, &(s, f))| w[k] * share(k) * days.frac(s, f, at)).sum::<f64>() / total * 100.0
    };
    let one = |_: usize| 1.0;
    let done = |k: usize| p.tasks[leaves[k]].percent_complete.min(100) as f64 / 100.0;
    let actual_is_recorded = !p.controls.progress.is_empty();
    // Physical progress history is a dated series of cumulative observations. Unknown
    // progress before the first observation is zero, NOT the current task percentage.
    let historic = |end_exclusive: NaiveDate| -> f64 {
        let cutoff = end_exclusive - Duration::days(1);
        leaves.iter().enumerate().map(|(k, &i)| {
            let pct = p.controls.progress_as_of(p.tasks[i].uid, cutoff)
                .map(|v| v.percent_complete).unwrap_or(0.0);
            w[k] * pct / 100.0
        }).sum::<f64>() / total * 100.0
    };
    let plan: Vec<f64> = ends.iter().map(|&e| cum(&pd, &one, e)).collect();
    let current = has_baseline.then(|| ends.iter().map(|&e| cum(&cd, &one, e)).collect());
    let after_status = status + Duration::days(1);
    let mut actual = Vec::new();
    for (k, &e) in ends.iter().enumerate() {
        if k > 0 && ends[k - 1] > status {
            break;
        }
        actual.push(if actual_is_recorded { historic(e.min(after_status)) } else { cum(&ad, &done, e.min(after_status)) });
    }
    let plan_at_status = cum(&pd, &one, after_status);
    let actual_at_status = if actual_is_recorded { historic(after_status) } else { cum(&ad, &done, after_status) };

    // groups: the top-level tasks
    let mut group_of = vec![None; p.tasks.len()];
    let mut cur = None;
    for (i, t) in p.tasks.iter().enumerate() {
        if t.outline_level <= 1 {
            cur = Some(i);
        }
        group_of[i] = cur;
    }
    let mut groups: Vec<(usize, f64, f64, f64)> = Vec::new(); // (top task, weight, planned, earned)
    for (k, &i) in leaves.iter().enumerate() {
        let Some(g) = group_of[i] else { continue };
        let pl = w[k] * days.frac(pd[k].0, pd[k].1, after_status);
        let ac = if actual_is_recorded {
            w[k] * p.controls.progress_as_of(p.tasks[i].uid, status)
                .map(|x| x.percent_complete / 100.0).unwrap_or(0.0)
        } else {
            w[k] * done(k) * days.frac(ad[k].0, ad[k].1, after_status)
        };
        match groups.iter_mut().find(|x| x.0 == g) {
            Some(x) => {
                x.1 += w[k];
                x.2 += pl;
                x.3 += ac;
            }
            None => groups.push((g, w[k], pl, ac)),
        }
    }
    let pct = |a: f64, b: f64| if b > 0.0 { a / b * 100.0 } else { 0.0 };
    let groups = groups
        .into_iter()
        .map(|(g, gw, pl, ac)| GroupRow { name: p.tasks[g].name.clone(), weight: pct(gw, total), plan: pct(pl, gw), actual: pct(ac, gw) })
        .collect();
    SCurve { ends, weekly, plan, current, actual, basis, plan_at_status, actual_at_status, actual_is_recorded, groups }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Task;

    fn task(p: &mut Project, name: &str, start: NaiveDate, days: i64, cost: f64, pc: u8) {
        let mut t = Task::new(p.next_uid, name, start);
        p.next_uid += 1;
        t.start = start;
        t.finish = p.calendar.add_work(start, days as i32 - 1);
        t.dur_min = days * 480;
        t.cost = cost;
        t.percent_complete = pc;
        t.outline_level = 1;
        t.active = true;
        p.tasks.push(t);
    }

    #[test]
    fn dated_actual_progress_is_stepwise_and_not_reconstructed() {
        let mut p = Project::new();
        p.tasks.clear();
        let mon = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();
        p.start = mon;
        task(&mut p, "A", mon, 5, 100.0, 90);
        let uid = p.tasks[0].uid;
        let before = compute(&p, mon + Duration::days(2), 1, 1);
        assert!(!before.actual_is_recorded);
        crate::controls::record_progress(&mut p, uid, mon + Duration::days(2), Some(20.0), None, "".into()).unwrap();
        crate::controls::record_progress(&mut p, uid, mon + Duration::days(4), Some(60.0), None, "".into()).unwrap();
        let on_wed = compute(&p, mon + Duration::days(2), 1, 1);
        assert!(on_wed.actual_is_recorded);
        assert!((on_wed.actual_at_status - 20.0).abs() < 1e-6);
        let on_fri = compute(&p, mon + Duration::days(4), 1, 1);
        assert!((on_fri.actual_at_status - 60.0).abs() < 1e-6);
        assert!((compute(&p, mon, 1, 1).actual_at_status).abs() < 1e-6);
    }

    #[test]
    fn weighted_curves() {
        let mut p = Project::new();
        p.tasks.clear();
        let mon = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap(); // Monday
        p.start = mon;
        // two weeks, two tasks: the first worth 3x the second
        task(&mut p, "A", mon, 5, 300.0, 100);
        task(&mut p, "B", mon + Duration::days(7), 5, 100.0, 0);
        let s = compute(&p, mon + Duration::days(4), 0, 1);
        assert_eq!(s.basis, Basis::Cost);
        assert!(s.weekly);
        assert_eq!(s.plan[0], 0.0);
        assert!((s.plan[1] - 75.0).abs() < 1e-6, "{:?}", s.plan);
        assert!((s.plan.last().unwrap() - 100.0).abs() < 1e-6);
        assert!((s.plan_at_status - 75.0).abs() < 1e-6);
        assert!((s.actual_at_status - 75.0).abs() < 1e-6);
        assert!(s.current.is_none());
        assert_eq!(s.groups.len(), 2);
        assert!((s.groups[0].weight - 75.0).abs() < 1e-6 && (s.groups[0].actual - 100.0).abs() < 1e-6);
        // half of A done by Wednesday → 3 of 5 days → actual = 75 % x 50 % ... spread over A's days
        p.tasks[0].percent_complete = 50;
        let s = compute(&p, mon + Duration::days(2), 0, 1);
        assert!((s.actual_at_status - 37.5).abs() < 1e-6, "{}", s.actual_at_status);
        assert!((s.plan_at_status - 45.0).abs() < 1e-6, "{}", s.plan_at_status);
        // duration weights when nothing has a cost
        p.tasks.iter_mut().for_each(|t| t.cost = 0.0);
        let s = compute(&p, mon, 0, 2);
        assert_eq!(s.basis, Basis::Duration);
        assert!(!s.weekly);
        assert!((s.groups[0].weight - 50.0).abs() < 1e-6);
    }
}
