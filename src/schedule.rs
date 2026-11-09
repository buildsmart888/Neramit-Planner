//! Scheduling engine: forward pass, summary roll-up, backward pass (critical path), in minutes.
//!
//! Time is counted in *working minutes* of the project calendar from a base date (a "point"). A
//! task of `d` working minutes occupies `[S, S + d)`. A point between two working intervals is both
//! the end of one (17:00, or 12:00 before lunch) and the start of the next (08:00 the next day, or
//! 13:00): starts are shown at the start of the next working minute and finishes at the end of the
//! last one. Milestones, and finishes driven by a predecessor's start, remember which side of such
//! a point they belong to.

use crate::model::{pct_of, round_div, Accrue, Assignment, Calendar, ConstraintType, LinkType, Project, ResourceType, TaskType, STANDARD_DAY};
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use std::collections::HashMap;

/// The working time of a calendar laid out as a line of working minutes from `base`.
struct Axis<'a> {
    cal: &'a Calendar,
    base: NaiveDate,
    /// every day works the standard hours (a calendar without any working weekday)
    all_days: bool,
    /// per day from `base`: working minutes before the day, and the day's working intervals
    days: Vec<(i64, Vec<(u16, u16)>)>,
}

fn total(iv: &[(u16, u16)]) -> i64 {
    iv.iter().map(|(a, b)| (*b as i64 - *a as i64).max(0)).sum()
}

fn at_minute(d: NaiveDate, m: i64) -> NaiveDateTime {
    d.and_time(NaiveTime::MIN) + Duration::minutes(m)
}

impl<'a> Axis<'a> {
    fn new(cal: &'a Calendar, base: NaiveDate) -> Self {
        let all_days = !cal.working_days.iter().any(|w| *w);
        Axis { cal, base, all_days, days: Vec::new() }
    }

    fn intervals(&self, d: NaiveDate) -> Vec<(u16, u16)> {
        if self.all_days {
            STANDARD_DAY.to_vec()
        } else {
            self.cal.day_intervals(d).to_vec()
        }
    }

    fn extend_to(&mut self, i: usize) {
        while self.days.len() <= i {
            let cum = self.days.last().map(|(c, iv)| c + total(iv)).unwrap_or(0);
            let d = self.base + Duration::days(self.days.len() as i64);
            let iv = self.intervals(d);
            self.days.push((cum, iv));
        }
    }

    /// Scan far enough that some day's work reaches beyond point `k`.
    fn extend_past(&mut self, k: i64) {
        let mut guard = 0;
        while self.days.last().map(|(c, iv)| c + total(iv)).unwrap_or(0) <= k && guard < 200_000 {
            let n = self.days.len();
            self.extend_to(n);
            guard += 1;
        }
    }

    /// Working minutes before instant `t` (an instant in non-working time counts all work before it).
    fn idx(&mut self, t: NaiveDateTime) -> i64 {
        let di = (t.date() - self.base).num_days();
        if di < 0 {
            return 0;
        }
        self.extend_to(di as usize);
        let (cum, iv) = &self.days[di as usize];
        let m = (t.hour() * 60 + t.minute()) as i64;
        let mut acc = *cum;
        for (a, b) in iv {
            let (a, b) = (*a as i64, *b as i64);
            if m >= b {
                acc += b - a;
            } else if m > a {
                acc += m - a;
            }
        }
        acc
    }

    /// Instant at which working minute `k` starts (between two intervals: the start of the later).
    fn at_start(&mut self, k: i64) -> NaiveDateTime {
        let k = k.max(0);
        self.extend_past(k);
        let i = self.days.partition_point(|(c, iv)| c + total(iv) <= k);
        let (cum, iv) = &self.days[i];
        let mut off = k - cum;
        for (a, b) in iv {
            let len = (*b as i64 - *a as i64).max(0);
            if off < len {
                return at_minute(self.base + Duration::days(i as i64), *a as i64 + off);
            }
            off -= len;
        }
        at_minute(self.base + Duration::days(i as i64), 8 * 60)
    }

    /// Instant at which working minute `k` ends (between two intervals: the end of the earlier).
    fn at_end(&mut self, k: i64) -> NaiveDateTime {
        if k <= 0 {
            return self.at_start(0);
        }
        self.extend_past(k - 1);
        let i = self.days.partition_point(|(c, iv)| c + total(iv) < k);
        let (cum, iv) = &self.days[i];
        let mut off = k - cum;
        for (a, b) in iv {
            let len = (*b as i64 - *a as i64).max(0);
            if off <= len {
                return at_minute(self.base + Duration::days(i as i64), *a as i64 + off);
            }
            off -= len;
        }
        at_minute(self.base + Duration::days(i as i64), 17 * 60)
    }

    /// Is `t` exactly the end of a working interval?
    fn ends_work(&mut self, t: NaiveDateTime) -> bool {
        let k = self.idx(t);
        k > 0 && self.at_end(k) == t
    }
}

#[derive(Clone, Copy, PartialEq)]
struct Pts {
    s: i64,
    f: i64,
    /// milestone shown at the end of the previous working interval (after a predecessor's finish)
    eod: bool,
    /// finish shown at the start of the next working interval (driven by a predecessor's start)
    fsod: bool,
}


/// Constraint dates as points.
#[derive(Clone, Copy)]
struct Cons {
    kind: ConstraintType,
    /// start-type date (SNET, SNLT, MSO) or finish-type date (FNET, FNLT, MFO) as a point
    at: i64,
}

/// Recalculate the whole project. Returns an error message when a circular dependency prevents the
/// schedule from converging.
pub fn schedule(p: &mut Project) -> Result<(), String> {
    let n = p.tasks.len();
    // normalise outline levels: first task level 1, never jump more than +1
    let mut prev = 0u8;
    for t in p.tasks.iter_mut() {
        if t.outline_level < 1 {
            t.outline_level = 1;
        }
        if t.outline_level > prev + 1 {
            t.outline_level = prev + 1;
        }
        prev = t.outline_level;
    }
    for i in 0..n {
        let next_lvl = p.tasks.get(i + 1).map(|t| t.outline_level).unwrap_or(0);
        p.tasks[i].is_summary = next_lvl > p.tasks[i].outline_level;
    }
    if n == 0 {
        return Ok(());
    }
    sync_assignments(p);

    let uid_idx: HashMap<u32, usize> = p.tasks.iter().enumerate().map(|(i, t)| (t.uid, i)).collect();
    // drop links to tasks that no longer exist / self links
    for t in p.tasks.iter_mut() {
        let me = t.uid;
        t.predecessors.retain(|l| l.pred_uid != me && uid_idx.contains_key(&l.pred_uid));
    }

    let parents: Vec<Option<usize>> = p.parents();
    let cal = p.calendar.clone();
    let mpd = cal.minutes_per_day();
    let honor = p.options.honor_constraints;

    // the axis starts well before anything in the project (links may schedule before its start)
    let mut earliest = p.start;
    for t in &p.tasks {
        earliest = earliest.min(t.start);
        if let Some(c) = t.constraint_date {
            earliest = earliest.min(c);
        }
    }
    let mut ax = Axis::new(&cal, earliest - Duration::days(1100));
    let proj_start = ax.idx(cal.day_start(p.start));
    // a milestone held only by the project start is shown at its exact instant (e.g. 01:00)
    let proj_instant = p.start_dt.filter(|t| t.date() == p.start);
    let mut at_proj: Vec<bool> = vec![false; p.tasks.len()];

    // a split sits inside the task: after some of its work and before the end
    for t in p.tasks.iter_mut() {
        let d = t.dur_minutes(mpd);
        t.splits.retain(|(pos, gap)| *pos > 0 && *pos < d && *gap > 0);
        if t.is_summary {
            t.splits.clear();
        }
    }
    let gaps: Vec<i64> = p.tasks.iter().map(|t| t.gap_minutes()).collect();
    // durations in minutes (working, or calendar for elapsed tasks)
    let dur_min: Vec<i64> = p.tasks.iter().map(|t| t.dur_minutes(mpd)).collect();
    let elapsed: Vec<bool> = p.tasks.iter().map(|t| t.dur_elapsed && t.dur_minutes(mpd) > 0).collect();
    let active: Vec<bool> = p.tasks.iter().map(|t| t.active).collect();
    // what summaries and the project's dates count: active tasks of this project (an external
    // task only drives its links)
    let counted: Vec<bool> = p.tasks.iter().map(|t| t.active && t.external.is_none()).collect();
    // manual tasks with nothing entered: a summary's dates come from its other subtasks (
    // MPP places such a task at the summary's start, a link can take it far from the others)
    let nothing: Vec<bool> = p.tasks.iter().map(|t| t.manual && t.no_dates).collect();
    let cons: Vec<Option<Cons>> = p
        .tasks
        .iter()
        .map(|t| {
            let kind = t.constraint_type;
            if !kind.needs_date() {
                return None;
            }
            let at = ax.idx(t.constraint_instant(&cal)?);
            Some(Cons { kind, at })
        })
        .collect();
    let deadline: Vec<Option<i64>> = p.tasks.iter().map(|t| t.deadline.map(|d| ax.idx(cal.day_end(d)))).collect();
    let alap: Vec<bool> = p.tasks.iter().map(|t| !t.manual && t.constraint_type == ConstraintType::Alap).collect();
    let level_delay: Vec<i64> = p.tasks.iter().map(|t| if t.manual || t.actual_start.is_some() { 0 } else { t.level_delay_min.max(0) }).collect();
    // tracking: a started task stays at its actual start, a finished one ends at its actual finish
    let act_start: Vec<Option<NaiveDateTime>> = p.tasks.iter().map(|t| t.actual_start.filter(|_| !t.is_summary)).collect();
    let act_finish: Vec<Option<NaiveDateTime>> = p.tasks.iter().map(|t| t.actual_finish.filter(|_| !t.is_summary && t.actual_start.is_some())).collect();

    // working length of a task starting at point `s`
    let length = |ax: &mut Axis, i: usize, s: i64| -> i64 {
        if elapsed[i] {
            let st = ax.at_start(s);
            ax.idx(st + Duration::minutes(dur_min[i] + gaps[i])) - s
        } else {
            dur_min[i] + gaps[i]
        }
    };

    let mut pts: Vec<Pts> = (0..n)
        .map(|i| {
            let s = ax.idx(cal.day_start(p.tasks[i].start));
            Pts { s, f: s, eod: false, fsod: false }
        })
        .collect();
    for i in 0..n {
        let s = pts[i].s;
        pts[i].f = s + length(&mut ax, i, s);
    }
    // effective lag (in points) of every link, measured in the forward pass, reused backwards
    let mut eff_lag: HashMap<(usize, usize), i64> = HashMap::new();
    let mut alap_fix: Vec<Option<i64>> = vec![None; n];
    // the latest start each task's own links ask for (point, at the end of a working interval)
    let mut link_req: Vec<Option<(i64, bool)>> = vec![None; n];
    // start and finish instants of a task in non-working time (MPP): a milestone an
    // elapsed lag puts there (Saturday 07:00 stays Saturday 07:00), a manually scheduled task
    // entered there
    let mut exact: Vec<Option<(NaiveDateTime, NaiveDateTime)>> = vec![None; n];
    let mut converged = false;
    let mut lf: Vec<i64> = vec![0; n];

    for _alap_round in 0..4 {
        converged = false;
        for _iter in 0..(n + 10) {
            let mut changed = false;
            for i in 0..n {
                if p.tasks[i].is_summary {
                    continue;
                }
                let t = &p.tasks[i];
                let mut new_exact: Option<(NaiveDateTime, NaiveDateTime)> = None;
                let np = if let Some(a) = act_start[i] {
                    let s = ax.idx(a);
                    let f = match act_finish[i] {
                        Some(af) => ax.idx(af).max(s),
                        None => s + length(&mut ax, i, s),
                    };
                    let eod = f == s && ax.ends_work(a) && ax.at_start(s) != a;
                    Pts { s, f, eod, fsod: false }
                } else if t.external.is_some() && t.predecessors.is_empty() {
                    // a task of another project: where its file had it (Project > Links Between
                    // Projects brings changes in). One after tasks of this project (an external
                    // successor) follows them, as MPP shows it.
                    let st = t.start_dt.unwrap_or_else(|| cal.day_start(t.start));
                    let ft = t.finish_dt.unwrap_or(st).max(st);
                    new_exact = Some((st, ft));
                    let s = ax.idx(st);
                    Pts { s, f: ax.idx(ft).max(s), eod: false, fsod: false }
                } else if t.manual && t.no_dates {
                    // no dates entered: at its summary's start (from its other subtasks), or the
                    // project's
                    let s = match parents[i] {
                        Some(pi) if p.tasks[pi].manual => pts[pi].s,
                        Some(pi) => (pi + 1..p.children_end(pi))
                            .filter(|&j| j != i && active[j] && !p.tasks[j].is_summary && !(p.tasks[j].manual && p.tasks[j].no_dates))
                            .map(|j| pts[j].s)
                            .min()
                            .unwrap_or(proj_start),
                        None => proj_start,
                    };
                    if parents[i].is_none() {
                        // (at the project's start instant, a holiday or 01:00 too, its
                        // length counted as MPP counts a manual task's)
                        let x = proj_instant.unwrap_or_else(|| cal.day_start(p.start));
                        let fx = match t.finish_dt {
                            Some(ft) if t.start_dt == Some(x) && ft >= x => ft,
                            _ => cal.manual_finish(x, dur_min[i] + gaps[i]),
                        };
                        new_exact = Some((x, fx));
                        let (s, f) = (ax.idx(x), ax.idx(fx));
                        Pts { s, f: f.max(s), eod: false, fsod: false }
                    } else {
                        // (MPP keeps its recorded finish)
                        let f = match t.finish_dt {
                            Some(ft) => ax.idx(ft).max(s),
                            None => s + length(&mut ax, i, s),
                        };
                        Pts { s, f, eod: false, fsod: false }
                    }
                } else if t.manual {
                    let st = t.start_dt.unwrap_or_else(|| cal.day_start(t.start));
                    let s = ax.idx(st);
                    if !elapsed[i] && ax.at_start(s) != st {
                        // entered in non-working time: MPP keeps the start
                        let fx = cal.manual_finish(st, dur_min[i] + gaps[i]);
                        new_exact = Some((st, fx));
                        Pts { s, f: ax.idx(fx).max(s), eod: false, fsod: false }
                    } else {
                        Pts { s, f: s + length(&mut ax, i, s), eod: false, fsod: false }
                    }
                } else {
                    let d_prev = pts[i].f - pts[i].s;
                    // links on this task and on every ancestor summary
                    let mut chain = vec![i];
                    let mut a = parents[i];
                    while let Some(ai) = a {
                        chain.push(ai);
                        a = parents[ai];
                    }
                    let has_links = chain.iter().any(|&ci| {
                        p.tasks[ci].predecessors.iter().any(|l| {
                            let pi = uid_idx[&l.pred_uid];
                            !chain.contains(&pi) && !(p.tasks[pi].manual && p.tasks[pi].no_dates)
                        })
                    });
                    // candidates: start at the start of a working interval / at the end of one
                    // (milestones after a finish), finish driven by a start (shown at 08:00)
                    let mut best_sod = if has_links { i64::MIN } else { proj_start };
                    let mut best_eod = i64::MIN;
                    let mut best_fin: (i64, bool) = (i64::MIN, false);
                    let mut own_req: Option<(i64, bool)> = None;
                    // a milestone's latest elapsed-lag instant in non-working time (point, instant)
                    let mut best_exact: Option<(i64, NaiveDateTime)> = None;
                    for &ci in &chain {
                        // a summary's constraint is only the default for subtasks without their own
                        let c = if ci == i { cons[i] } else if cons[i].is_none() { cons[ci] } else { None };
                        if let Some(c) = c {
                            match c.kind {
                                ConstraintType::Snet => best_sod = best_sod.max(c.at),
                                ConstraintType::Mso if !honor => best_sod = best_sod.max(c.at),
                                // a finish date pins a milestone at the end of that day (17:00)
                                ConstraintType::Fnet | ConstraintType::Mfo if dur_min[i] == 0 && (c.kind == ConstraintType::Fnet || !honor) => best_eod = best_eod.max(c.at),
                                ConstraintType::Fnet => best_fin = best_fin.max((c.at, false)),
                                ConstraintType::Mfo if !honor => best_fin = best_fin.max((c.at, false)),
                                _ => {}
                            }
                        }
                        let ct = &p.tasks[ci];
                        // a manually scheduled summary holds its subtasks at or after its start
                        if ci != i && ct.manual && cons[i].is_none() {
                            best_sod = best_sod.max(pts[ci].s);
                        }
                        for l in &ct.predecessors {
                            let pi = uid_idx[&l.pred_uid];
                            // a child cannot depend on its own ancestor summary; a manual task without
                            // dates drives nothing
                            if chain.contains(&pi) || (p.tasks[pi].manual && p.tasks[pi].no_dates) {
                                continue;
                            }
                            // links attached to a summary act on its subtasks' start
                            let kind = if ci == i { l.kind } else { match l.kind { LinkType::FS | LinkType::FF => LinkType::FS, _ => LinkType::SS } };
                            let pp = pts[pi];
                            // an inactive predecessor drives its successors like a zero-length task at its start
                            let from_finish = active[pi] && matches!(kind, LinkType::FS | LinkType::FF);
                            let anchor = if from_finish { pp.f } else { pp.s };
                            // is the anchor instant the end of a working interval (17:00)?
                            let mut at_end = if from_finish { (pp.f > pp.s && !pp.fsod) || pp.eod } else { pp.eod };
                            let pred_min = if p.tasks[pi].is_summary { (pp.f - pp.s).max(0) } else { dur_min[pi] };
                            let lag = l.lag_minutes(mpd, pred_min);
                            let mut in_gap: Option<NaiveDateTime> = None;
                            let moved = if l.elapsed && l.lag_pct.is_none() {
                                // elapsed lag: calendar time counted from the anchor instant (a
                                // predecessor in non-working time: its own instant)
                                let from = match exact[pi] {
                                    Some((xs, xf)) => if from_finish { xf } else { xs },
                                    None if at_end => ax.at_end(anchor),
                                    None => ax.at_start(anchor),
                                };
                                let inst = from + Duration::minutes(lag);
                                at_end = (lag != 0 && ax.ends_work(inst)) || (lag == 0 && at_end);
                                let k = ax.idx(inst);
                                if ci == i && dur_min[i] == 0 && ax.at_start(k) != inst && ax.at_end(k) != inst {
                                    in_gap = Some(inst);
                                }
                                k
                            } else {
                                // adding working time ends at the end of the last minute consumed
                                if lag > 0 {
                                    at_end = true;
                                }
                                anchor + lag
                            };
                            eff_lag.insert((pi, i), moved - anchor);
                            if ci == i {
                                // as a start: a finish requirement moves back by the duration
                                let req = match kind {
                                    LinkType::FS | LinkType::SS => (moved, at_end),
                                    LinkType::FF | LinkType::SF => (moved - d_prev, at_end && d_prev == 0),
                                };
                                // at equal points the start of the next interval is the later one
                                let later = |a: (i64, bool), b: (i64, bool)| if a.0 != b.0 { a.0 > b.0 } else { !a.1 && b.1 };
                                own_req = Some(match own_req {
                                    Some(r) if !later(req, r) => r,
                                    _ => req,
                                });
                            }
                            if let Some(x) = in_gap {
                                // (a milestone stays at the instant, e.g. Saturday 07:00;
                                // a task moves on to the next working time)
                                if best_exact.map(|b| x > b.1).unwrap_or(true) {
                                    best_exact = Some((moved, x));
                                }
                                continue;
                            }
                            match kind {
                                LinkType::FS | LinkType::SS => {
                                    if at_end && dur_min[i] == 0 {
                                        best_eod = best_eod.max(moved);
                                    } else {
                                        best_sod = best_sod.max(moved);
                                    }
                                }
                                LinkType::FF | LinkType::SF => {
                                    if dur_min[i] == 0 {
                                        if at_end {
                                            best_eod = best_eod.max(moved);
                                        } else {
                                            best_sod = best_sod.max(moved);
                                        }
                                    } else {
                                        best_fin = best_fin.max((moved, !at_end));
                                    }
                                }
                            }
                        }
                    }
                    // the latest requirement wins; at equal points 08:00 of the next day is later
                    // than 17:00 of the previous one
                    let fin_start = if best_fin.0 == i64::MIN { i64::MIN } else { best_fin.0 - d_prev };
                    let mut s = best_sod.max(best_eod).max(fin_start).max(best_exact.map(|e| e.0).unwrap_or(i64::MIN));
                    if s == i64::MIN {
                        s = proj_start;
                    }
                    // Resource > Level: the leveling delay comes on top of links and constraints
                    if level_delay[i] > 0 {
                        s += level_delay[i];
                        best_eod = i64::MIN;
                    }
                    let mut fsod = fin_start == s && best_fin.1 && fin_start > best_sod.max(best_eod);
                    if let Some(fix) = alap_fix[i] {
                        if fix > s {
                            s = fix;
                            fsod = false;
                        }
                    }
                    let mut eod = dur_min[i] == 0 && best_eod > best_sod && best_eod >= s;
                    // MPP honours SNLT / FNLT / MSO / MFO over links (default option)
                    if let (true, Some(c)) = (honor, cons[i]) {
                        match c.kind {
                            ConstraintType::Snlt if s > c.at => {
                                s = c.at;
                                (eod, fsod) = (false, false);
                            }
                            ConstraintType::Fnlt if s + d_prev > c.at => {
                                s = c.at - d_prev;
                                (eod, fsod) = (dur_min[i] == 0, false);
                            }
                            ConstraintType::Mso => {
                                s = c.at;
                                (eod, fsod) = (false, false);
                            }
                            ConstraintType::Mfo => {
                                s = c.at - d_prev;
                                (eod, fsod) = (dur_min[i] == 0, false);
                            }
                            _ => {}
                        }
                    }
                    // the milestone stays at its instant in non-working time unless something asks for
                    // the working time after it
                    if let Some((k, x)) = best_exact {
                        if k == s && best_sod < s && fin_start < s && level_delay[i] == 0 && alap_fix[i].is_none() {
                            new_exact = Some((x, x));
                        }
                    }
                    let f = s + length(&mut ax, i, s);
                    link_req[i] = own_req;
                    at_proj[i] = !has_links && cons[i].is_none() && s == proj_start && dur_min[i] == 0;
                    Pts { s, f, eod, fsod }
                };
                if np != pts[i] {
                    pts[i] = np;
                    changed = true;
                }
                if new_exact != exact[i] {
                    exact[i] = new_exact;
                    changed = true;
                }
            }
            // roll-up summaries bottom-up (inactive tasks do not count); a manually scheduled
            // summary keeps the dates entered for it
            for i in (0..n).rev() {
                if !p.tasks[i].is_summary {
                    continue;
                }
                if p.tasks[i].manual {
                    let t = &p.tasks[i];
                    let s = ax.idx(t.start_dt.unwrap_or_else(|| cal.day_start(t.start)));
                    let f = match t.finish_dt {
                        Some(ft) => ax.idx(ft).max(s),
                        None => s + dur_min[i],
                    };
                    let np = Pts { s, f, eod: false, fsod: false };
                    if np != pts[i] {
                        pts[i] = np;
                        changed = true;
                    }
                    continue;
                }
                let end = p.children_end(i);
                let kids: Vec<usize> = (i + 1..end).filter(|&j| counted[j] && !nothing[j]).collect();
                let kids = if kids.is_empty() { (i + 1..end).filter(|&j| counted[j]).collect() } else { kids };
                let kids = if kids.is_empty() { (i + 1..end).filter(|&j| p.tasks[j].external.is_none()).collect() } else { kids };
                let s = kids.iter().map(|&j| pts[j].s).min().unwrap_or(pts[i].s);
                let f = kids.iter().map(|&j| pts[j].f).max().unwrap_or(pts[i].f);
                if s != pts[i].s || f != pts[i].f {
                    pts[i] = Pts { s, f, eod: false, fsod: false };
                    changed = true;
                }
            }
            if !changed {
                converged = true;
                break;
            }
        }

        // ---------- backward pass (late finish of every leaf) ----------
        let dur_pts: Vec<i64> = (0..n).map(|i| (pts[i].f - pts[i].s).max(0)).collect();
        let proj_f = (0..n).filter(|&i| !p.tasks[i].is_summary && counted[i]).map(|i| pts[i].f).max().unwrap_or(0);
        lf = vec![proj_f; n];
        for i in 0..n {
            if let Some(c) = cons[i] {
                let lim = match c.kind {
                    ConstraintType::Mso | ConstraintType::Snlt => c.at + dur_pts[i],
                    ConstraintType::Mfo | ConstraintType::Fnlt => c.at,
                    _ => i64::MAX,
                };
                lf[i] = lf[i].min(lim);
            }
            if let Some(d) = deadline[i] {
                lf[i] = lf[i].min(d);
            }
        }
        let leaf_desc = |i: usize, p: &Project| -> Vec<usize> {
            if p.tasks[i].is_summary {
                (i + 1..p.children_end(i)).filter(|&j| !p.tasks[j].is_summary).collect()
            } else {
                vec![i]
            }
        };
        for _iter in 0..(n + 10) {
            let mut changed = false;
            let ls_of = |i: usize, lf: &Vec<i64>, p: &Project| -> i64 {
                if p.tasks[i].is_summary {
                    (i + 1..p.children_end(i)).filter(|&j| !p.tasks[j].is_summary).map(|j| lf[j] - dur_pts[j]).min().unwrap_or(lf[i])
                } else {
                    lf[i] - dur_pts[i]
                }
            };
            let lf_of = |i: usize, lf: &Vec<i64>, p: &Project| -> i64 {
                if p.tasks[i].is_summary {
                    (i + 1..p.children_end(i)).filter(|&j| !p.tasks[j].is_summary).map(|j| lf[j]).max().unwrap_or(lf[i])
                } else {
                    lf[i]
                }
            };
            // successors before their predecessors: a chain in ID order settles in one sweep
            for s in (0..n).rev() {
                for l in &p.tasks[s].predecessors {
                    let pi = uid_idx[&l.pred_uid];
                    if !active[pi] {
                        continue;
                    }
                    let lag = *eff_lag.get(&(pi, s)).unwrap_or(&0);
                    let (ls_s, lf_s) = (ls_of(s, &lf, p), lf_of(s, &lf, p));
                    for leaf in leaf_desc(pi, p) {
                        let limit = match l.kind {
                            LinkType::FS => ls_s - lag,
                            LinkType::SS => ls_s - lag + dur_pts[leaf],
                            LinkType::FF => lf_s - lag,
                            LinkType::SF => lf_s - lag + dur_pts[leaf],
                        };
                        if limit < lf[leaf] {
                            lf[leaf] = limit;
                            changed = true;
                        }
                    }
                }
            }
            if !changed {
                break;
            }
        }

        // as-late-as-possible tasks move to their late dates, then everything is scheduled again
        let mut moved = false;
        for i in 0..n {
            if alap[i] && !p.tasks[i].is_summary {
                let ns = lf[i] - dur_pts[i];
                if ns > pts[i].s && alap_fix[i] != Some(ns) {
                    alap_fix[i] = Some(ns);
                    moved = true;
                }
            }
        }
        if !moved {
            break;
        }
    }

    // ---------- dates from points ----------
    let mut starts: Vec<NaiveDateTime> = vec![cal.day_start(p.start); n];
    let mut finishes: Vec<NaiveDateTime> = vec![cal.day_start(p.start); n];
    for i in 0..n {
        if p.tasks[i].is_summary {
            continue;
        }
        let pt = pts[i];
        if let Some(a) = act_start[i] {
            starts[i] = a;
            finishes[i] = match act_finish[i] {
                Some(af) => af.max(a),
                None if pt.f == pt.s => a,
                None if elapsed[i] => a + Duration::minutes(dur_min[i] + gaps[i]),
                None => ax.at_end(pt.f).max(a),
            };
            continue;
        }
        if let Some((xs, xf)) = exact[i] {
            starts[i] = xs;
            finishes[i] = xf;
            continue;
        }
        if pt.f == pt.s && !elapsed[i] {
            let d = match (at_proj[i] && !p.tasks[i].manual, proj_instant) {
                (true, Some(t)) => t,
                _ if pt.eod => ax.at_end(pt.s),
                _ => ax.at_start(pt.s),
            };
            starts[i] = d;
            finishes[i] = d;
        } else {
            starts[i] = ax.at_start(pt.s);
            finishes[i] = if elapsed[i] {
                starts[i] + Duration::minutes(dur_min[i] + gaps[i])
            } else if pt.fsod {
                ax.at_start(pt.f)
            } else {
                ax.at_end(pt.f)
            };
        }
    }
    for i in (0..n).rev() {
        if p.tasks[i].is_summary {
            if p.tasks[i].manual {
                let t = &p.tasks[i];
                starts[i] = t.start_dt.unwrap_or_else(|| ax.at_start(pts[i].s));
                finishes[i] = t.finish_dt.unwrap_or_else(|| ax.at_end(pts[i].f));
                continue;
            }
            let end = p.children_end(i);
            let kids: Vec<usize> = (i + 1..end).filter(|&j| counted[j] && !nothing[j]).collect();
            let kids = if kids.is_empty() { (i + 1..end).filter(|&j| counted[j]).collect() } else { kids };
            let kids = if kids.is_empty() { (i + 1..end).filter(|&j| p.tasks[j].external.is_none()).collect() } else { kids };
            starts[i] = kids.iter().map(|&j| starts[j]).min().unwrap_or(starts[i]);
            finishes[i] = kids.iter().map(|&j| finishes[j]).max().unwrap_or(finishes[i]);
        }
    }
    let summary_end: Vec<usize> = (0..n).map(|i| if p.tasks[i].is_summary { p.children_end(i) } else { i + 1 }).collect();
    let summary_flags: Vec<bool> = p.tasks.iter().map(|t| t.is_summary).collect();
    let link_req: Vec<Option<(NaiveDateTime, bool)>> = link_req.iter().map(|r| r.map(|(k, e)| (if e { ax.at_end(k) } else { ax.at_start(k) }, e))).collect();
    for i in 0..n {
        let t = &mut p.tasks[i];
        t.link_req = if t.is_summary { None } else { link_req[i] };
        t.start_dt = Some(starts[i]);
        t.finish_dt = Some(finishes[i]);
        t.start = starts[i].date();
        t.finish = finishes[i].date();
        if t.is_summary {
            // a manually scheduled summary keeps its own dates, but its Duration is the span of its
            // subtasks (as MPP reports it)
            let m = if t.manual {
                let end = summary_end[i];
                let kids: Vec<usize> = (i + 1..end).filter(|&j| !summary_flags[j] && counted[j]).collect();
                match (kids.iter().map(|&j| pts[j].s).min(), kids.iter().map(|&j| pts[j].f).max()) {
                    (Some(a), Some(b)) => (b - a).max(0),
                    _ => (pts[i].f - pts[i].s).max(0),
                }
            } else {
                // (the standard Duration of a summary leaves out its manual tasks with nothing
                // entered, which a link or the summary's start can place far from the others)
                let end = summary_end[i];
                let all: Vec<usize> = (i + 1..end).filter(|&j| !summary_flags[j] && counted[j]).collect();
                let kids: Vec<usize> = all.iter().copied().filter(|&j| !nothing[j]).collect();
                match (kids.len() < all.len(), kids.iter().map(|&j| pts[j].s).min(), kids.iter().map(|&j| pts[j].f).max()) {
                    (true, Some(a), Some(b)) => (b - a).max(0),
                    _ => (pts[i].f - pts[i].s).max(0),
                }
            };
            t.dur_min = m;
            t.dur_elapsed = false;
            t.duration = round_div(m, mpd) as i32;
        } else {
            // keep the whole-day duration in step with the minutes actually scheduled
            let (unit, el) = (t.dur_unit, t.dur_elapsed);
            // (a manual task without dates spans its start to its recorded finish, as MPP
            // recalculates it)
            // (one placed at an instant keeps its duration: on a holiday the points meet)
            let m = if t.manual && t.no_dates && exact[i].is_none() { (pts[i].f - pts[i].s).max(0) } else { dur_min[i] };
            t.set_duration_minutes(m, unit, el, mpd);
            // the standard invariants: progress means an actual start, 100 % an actual finish
            if t.percent_complete > 0 && t.actual_start.is_none() {
                t.actual_start = Some(starts[i]);
            }
            if t.percent_complete == 100 && t.actual_finish.is_none() {
                t.actual_finish = Some(finishes[i]);
            }
            if t.percent_complete < 100 {
                t.actual_finish = None;
            }
        }
    }

    // ---------- slack and critical path ----------
    for i in 0..n {
        let t = &mut p.tasks[i];
        if !t.is_summary {
            t.slack_min = lf[i] - pts[i].f;
            t.total_slack = round_div(t.slack_min, mpd) as i32;
            t.critical = active[i] && t.slack_min <= 0 && t.percent_complete < 100;
        }
    }
    // free slack: how far a task can slip without delaying any successor
    let mut free: Vec<i64> = (0..n).map(|i| p.tasks[i].slack_min).collect();
    for s in 0..n {
        for l in &p.tasks[s].predecessors {
            let pi = uid_idx[&l.pred_uid];
            if p.tasks[pi].is_summary || p.tasks[s].is_summary || !active[pi] {
                continue;
            }
            let lag = *eff_lag.get(&(pi, s)).unwrap_or(&0);
            let room = match l.kind {
                LinkType::FS => pts[s].s - lag - pts[pi].f,
                LinkType::SS => pts[s].s - lag - pts[pi].s,
                LinkType::FF => pts[s].f - lag - pts[pi].f,
                LinkType::SF => pts[s].f - lag - pts[pi].s,
            };
            free[pi] = free[pi].min(room.max(0));
        }
    }
    for i in 0..n {
        p.tasks[i].free_slack_min = free[i].max(0);
    }
    for i in (0..n).rev() {
        if p.tasks[i].is_summary {
            let end = p.children_end(i);
            let crit = (i + 1..end).any(|j| p.tasks[j].critical);
            // a summary's duration is an estimate when any subtask's is ("91 days?")
            // (a manual summary keeps its own estimate flag; manually scheduled subtasks' estimates
            // do not count, as in MPP)
            let est = (i + 1..end).any(|j| p.tasks[j].estimated && (!p.tasks[j].manual || p.tasks[j].is_summary));
            if !p.tasks[i].manual {
                p.tasks[i].estimated = est;
            }
            let slack = (i + 1..end).map(|j| p.tasks[j].slack_min).min().unwrap_or(0);
            // % complete: the subtasks' actual durations over their durations (a milestone counts
            // as one minute); the summary's actual duration is that share of its own
            let (mut w, mut done) = (0i64, 0i64);
            let mut a_start: Option<NaiveDateTime> = None;
            let mut a_finish: Option<NaiveDateTime> = None;
            let mut all_done = true;
            for j in i + 1..end {
                if p.tasks[j].is_summary || !active[j] {
                    continue;
                }
                let c = &p.tasks[j];
                if dur_min[j] == 0 {
                    w += 1;
                    done += (c.percent_complete == 100) as i64;
                } else {
                    w += dur_min[j];
                    done += c.actual_minutes(mpd).min(dur_min[j]);
                }
                if let Some(a) = c.actual_start {
                    a_start = Some(a_start.map_or(a, |x| x.min(a)));
                }
                match c.actual_finish {
                    Some(f) => a_finish = Some(a_finish.map_or(f, |x| x.max(f))),
                    None => all_done = false,
                }
            }
            let t = &mut p.tasks[i];
            t.critical = crit;
            t.slack_min = slack;
            t.total_slack = round_div(slack, mpd) as i32;
            t.percent_complete = pct_of(done, w);
            let own = t.dur_minutes(mpd);
            t.actual_dur_min = if w > 0 && done > 0 { Some((own as f64 * done as f64 / w as f64).round() as i64) } else { None };
            // a summary under way has started when it starts (the standard files - its actual
            // start is its start, not its first subtask's actual start)
            t.actual_start = a_start.map(|a| t.start_dt.map(|s| s.min(a)).unwrap_or(a));
            t.actual_finish = if all_done { a_finish } else { None };
        }
    }

    // ---------- work and cost (rate x work, cost per use once per
    // unit of a work resource and once for a material, plus the task's fixed cost) ----------
    let ridx: HashMap<u32, usize> = p.resources.iter().enumerate().map(|(k, r)| (r.uid, k)).collect();
    let mut res_work = vec![0i64; p.resources.len()];
    let mut res_cost = vec![0f64; p.resources.len()];
    for i in 0..n {
        if p.tasks[i].is_summary {
            continue;
        }
        let t = &p.tasks[i];
        let d = dur_min[i];
        let done = if d > 0 { t.actual_minutes(mpd) as f64 / d as f64 } else if t.percent_complete == 100 { 1.0 } else { 0.0 };
        let started = t.actual_start.is_some() || t.percent_complete > 0;
        let finished = t.percent_complete == 100;
        let accrued = |acc: Accrue, amount: f64| -> f64 {
            match acc {
                Accrue::Start => if started { amount } else { 0.0 },
                Accrue::End => if finished { amount } else { 0.0 },
                Accrue::Prorated => amount * done,
            }
        };
        let mut cost = t.fixed_cost;
        let mut actual = accrued(t.fixed_cost_accrual, t.fixed_cost);
        let mut work = 0i64;
        for a in &t.assignments {
            let Some(&ri) = ridx.get(&a.res_uid) else { continue };
            let r = &p.resources[ri];
            let (c, per_use) = match r.rtype {
                ResourceType::Work => {
                    let w = a.work_min.max(0);
                    work += w;
                    res_work[ri] += w;
                    (w as f64 / 60.0 * r.std_rate, r.cost_per_use * a.units)
                }
                ResourceType::Material => (a.units * r.std_rate, r.cost_per_use),
                ResourceType::Cost => (a.cost, 0.0),
            };
            cost += c + per_use;
            actual += accrued(r.accrue_at, c) + if started { per_use } else { 0.0 };
            res_cost[ri] += c + per_use;
        }
        let t = &mut p.tasks[i];
        t.cost = cost;
        t.actual_cost = actual;
        t.work_min = work;
    }
    for i in (0..n).rev() {
        if p.tasks[i].is_summary {
            let end = p.children_end(i);
            let leaves: Vec<usize> = (i + 1..end).filter(|&j| !p.tasks[j].is_summary).collect();
            let own = p.tasks[i].fixed_cost;
            let c: f64 = leaves.iter().map(|&j| p.tasks[j].cost).sum::<f64>() + own;
            let ac: f64 = leaves.iter().map(|&j| p.tasks[j].actual_cost).sum();
            let w: i64 = leaves.iter().map(|&j| p.tasks[j].work_min).sum();
            let t = &mut p.tasks[i];
            t.cost = c;
            t.actual_cost = ac;
            t.work_min = w;
        }
    }
    for (k, r) in p.resources.iter_mut().enumerate() {
        r.work_min = res_work[k];
        r.cost = res_cost[k];
        r.overallocated = false;
    }
    overallocation(p, &ridx);

    if converged {
        Ok(())
    } else {
        Err("Circular dependency detected: the schedule could not be resolved. Check task links.".into())
    }
}

/// Assignments follow the task's Resource Names and its work follows the standard task types:
/// - a new assignment works the task's duration x its units;
/// - a duration change gives Fixed Units / Fixed Duration tasks new work, Fixed Work tasks new units;
/// - on an effort-driven task (and every Fixed Work task) adding or removing people keeps the total
///   work: the duration shrinks or grows (units shrink on a Fixed Duration task).
pub fn sync_assignments(p: &mut Project) {
    let mpd = p.calendar.minutes_per_day();
    let res: HashMap<u32, (ResourceType, f64)> = p.resources.iter().map(|r| (r.uid, (r.rtype, r.max_units))).collect();
    let is_work = |u: u32| res.get(&u).map(|r| r.0 == ResourceType::Work).unwrap_or(false);
    for t in p.tasks.iter_mut() {
        t.resources.retain(|u| res.contains_key(u));
        let mut seen = std::collections::HashSet::new();
        t.resources.retain(|u| seen.insert(*u));
        let had: Vec<Assignment> = t.assignments.clone();
        // assignments already worked out (new ones have work -1, also when a dialog set units)
        let had_work: Vec<&Assignment> = had.iter().filter(|a| is_work(a.res_uid) && a.work_min >= 0).collect();
        let before_work: i64 = had_work.iter().map(|a| a.work_min).sum();
        let removed = had_work.iter().any(|a| !t.resources.contains(&a.res_uid));
        t.assignments.retain(|a| t.resources.contains(&a.res_uid));
        for &u in &t.resources.clone() {
            if t.assignments.iter().all(|a| a.res_uid != u) {
                let (rt, mx) = res[&u];
                let units = if rt == ResourceType::Work { (mx / 100.0).min(1.0).max(0.01) } else { 1.0 };
                t.assignments.push(Assignment { res_uid: u, units, work_min: -1, cost: 0.0 });
            }
        }
        let added = t.assignments.iter().any(|a| is_work(a.res_uid) && a.work_min < 0);
        let order: Vec<u32> = t.resources.clone();
        t.assignments.sort_by_key(|a| order.iter().position(|u| *u == a.res_uid).unwrap_or(usize::MAX));
        let effort = t.effort_driven || t.task_type == TaskType::FixedWork;
        let (unit, el) = (t.dur_unit, t.dur_elapsed);
        if effort && !had_work.is_empty() && before_work > 0 && (added || removed) && !t.is_summary {
            let u_total: f64 = t.assignments.iter().filter(|a| is_work(a.res_uid)).map(|a| a.units).sum();
            if u_total > 0.0 {
                for a in t.assignments.iter_mut().filter(|a| is_work(a.res_uid)) {
                    a.work_min = (before_work as f64 * a.units / u_total).round() as i64;
                }
                if t.task_type == TaskType::FixedDuration {
                    let d = t.dur_minutes(mpd);
                    if d > 0 {
                        for a in t.assignments.iter_mut().filter(|a| is_work(a.res_uid)) {
                            a.units = a.work_min as f64 / d as f64;
                        }
                    }
                } else {
                    let nd = (before_work as f64 / u_total).round() as i64;
                    t.set_duration_minutes(nd, unit, el, mpd);
                }
            }
        }
        let d = t.dur_minutes(mpd);
        let dur_changed = t.work_ref_dur >= 0 && t.work_ref_dur != d;
        let fixed_work = t.task_type == TaskType::FixedWork;
        for a in t.assignments.iter_mut() {
            if !is_work(a.res_uid) {
                a.work_min = 0;
                continue;
            }
            if a.work_min < 0 {
                a.work_min = (d as f64 * a.units).round() as i64;
            } else if dur_changed {
                if fixed_work {
                    if d > 0 {
                        a.units = a.work_min as f64 / d as f64;
                    }
                } else {
                    a.work_min = (d as f64 * a.units).round() as i64;
                }
            }
        }
        t.work_ref_dur = d;
    }
}

/// Work of each assignment per day (flat contour over the task's working time), for resource
/// loading and the Usage views: (task index, assignment index, date, minutes).
pub fn daily_work(p: &Project) -> Vec<(usize, usize, NaiveDate, i64)> {
    let cal = &p.calendar;
    let work_res: std::collections::HashSet<u32> = p.resources.iter().filter(|r| r.rtype == ResourceType::Work).map(|r| r.uid).collect();
    let mut out = Vec::new();
    for (i, t) in p.tasks.iter().enumerate() {
        if t.is_summary || !t.active || t.assignments.iter().all(|a| !work_res.contains(&a.res_uid)) {
            continue;
        }
        let (Some(st), Some(ft)) = (t.start_dt, t.finish_dt) else { continue };
        let span = if t.dur_elapsed { (ft - st).num_minutes() } else { cal.work_minutes(st, ft) };
        if span <= 0 {
            continue;
        }
        let mut d = st.date();
        while d <= ft.date() {
            let a0 = st.max(d.and_time(NaiveTime::MIN));
            let b0 = ft.min((d + Duration::days(1)).and_time(NaiveTime::MIN));
            let m = if t.dur_elapsed { (b0 - a0).num_minutes() } else { cal.work_minutes(a0, b0) };
            if m > 0 {
                for (k, a) in t.assignments.iter().enumerate() {
                    if work_res.contains(&a.res_uid) && a.work_min > 0 {
                        out.push((i, k, d, (a.work_min as f64 * m as f64 / span as f64).round() as i64));
                    }
                }
            }
            d += Duration::days(1);
        }
    }
    out
}

/// A resource is overallocated on a day when its assignments need more than Max. Units of that
/// day's working time; the tasks it works on then are flagged too.
fn overallocation(p: &mut Project, ridx: &HashMap<u32, usize>) {
    for t in p.tasks.iter_mut() {
        t.overallocated = false;
    }
    let daily = daily_work(p);
    if daily.is_empty() {
        return;
    }
    let mut load: HashMap<(usize, NaiveDate), i64> = HashMap::new();
    for &(i, k, d, m) in &daily {
        if let Some(&ri) = ridx.get(&p.tasks[i].assignments[k].res_uid) {
            *load.entry((ri, d)).or_default() += m;
        }
    }
    let mut over: std::collections::HashSet<(usize, NaiveDate)> = std::collections::HashSet::new();
    for (&(ri, d), &m) in &load {
        let day_min: i64 = p.calendar.day_intervals(d).iter().map(|(a, b)| (*b as i64 - *a as i64).max(0)).sum();
        let day_min = if day_min == 0 { p.calendar.minutes_per_day() } else { day_min };
        let cap = p.resources[ri].max_units / 100.0 * day_min as f64;
        if m as f64 > cap + 0.5 {
            over.insert((ri, d));
            p.resources[ri].overallocated = true;
        }
    }
    for &(i, k, d, _) in &daily {
        if let Some(&ri) = ridx.get(&p.tasks[i].assignments[k].res_uid) {
            if over.contains(&(ri, d)) {
                p.tasks[i].overallocated = true;
            }
        }
    }
}

/// Resource > Level All: delay tasks (lowest priority first, then most slack, latest start,
/// highest ID) until no resource is overallocated. Only auto-scheduled tasks that have not
/// started are delayed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LevelingReport {
    pub delayed_tasks: usize,
    /// Number of resources still overallocated after leveling (for example, no movable tasks).
    pub unresolved_resources: usize,
}

/// Run leveling on a staging project, rolling back on any scheduling error.
/// The caller gets an explicit report when there are no legal moves left.
pub fn level_checked(p: &mut Project, only: Option<u32>, movable_tasks: Option<&[usize]>) -> Result<LevelingReport, String> {
    let mut staged = p.clone();
    let delayed_tasks = level_tasks_inner(&mut staged, only, movable_tasks)?;
    let unresolved_resources = staged.resources.iter()
        .filter(|r| only.map(|uid| r.uid == uid).unwrap_or(true) && r.overallocated)
        .count();
    *p = staged;
    Ok(LevelingReport { delayed_tasks, unresolved_resources })
}

/// Backwards-compatible convenience functions. UI and MCP must use level_checked
/// to avoid silently losing scheduling errors.
pub fn level(p: &mut Project, only: Option<u32>) -> usize {
    level_checked(p, only, None).map(|r| r.delayed_tasks).unwrap_or(0)
}

pub fn level_tasks(p: &mut Project, only: Option<u32>, movable_tasks: Option<&[usize]>) -> usize {
    level_checked(p, only, movable_tasks).map(|r| r.delayed_tasks).unwrap_or(0)
}

/// Leveling that may only delay the tasks in `movable` (None = any task): Task > Move > When
/// Resources Are Available levels the selected tasks.
fn level_tasks_inner(p: &mut Project, only: Option<u32>, movable_tasks: Option<&[usize]>) -> Result<usize, String> {
    let opt = p.options.leveling.clone();
    let may_move = |i: usize| movable_tasks.map(|m| m.contains(&i)).unwrap_or(true);
    if opt.clear_first {
        for t in p.tasks.iter_mut() {
            if !t.manual && t.actual_start.is_none() {
                t.level_delay_min = 0;
            }
        }
    }
    schedule(p)?;
    // the period overallocations are looked for in (Leveling Options > basis): its first day
    let period = |d: NaiveDate| -> NaiveDate {
        match opt.basis {
            3 => d - Duration::days(d.weekday().num_days_from_monday() as i64),
            4 => d.with_day(1).unwrap_or(d),
            _ => d,
        }
    };
    let in_range = |d: NaiveDate| opt.range.map(|(a, b)| d >= a && d <= b).unwrap_or(true);
    // the slack each task had before leveling (Level only within available slack)
    let slack0: Vec<i64> = p.tasks.iter().map(|t| t.slack_min.max(0)).collect();
    let mut delayed = std::collections::HashSet::new();
    for _round in 0..2000 {
        let ridx: HashMap<u32, usize> = p.resources.iter().enumerate().map(|(k, r)| (r.uid, k)).collect();
        let daily = daily_work(p);
        let mut load: HashMap<(usize, NaiveDate), Vec<(usize, i64)>> = HashMap::new();
        for &(i, k, d, m) in &daily {
            if !in_range(d) {
                continue;
            }
            if let Some(&ri) = ridx.get(&p.tasks[i].assignments[k].res_uid) {
                if only.map(|u| u == p.resources[ri].uid).unwrap_or(true) {
                    load.entry((ri, period(d))).or_default().push((i, m));
                }
            }
        }
        // the earliest overallocated day
        let mut worst: Option<(NaiveDate, usize)> = None;
        for (&(ri, d), v) in &load {
            // working minutes of the period (a day, a week or a month)
            let days = match opt.basis {
                3 => 7,
                4 => {
                    let next = if d.month() == 12 { NaiveDate::from_ymd_opt(d.year() + 1, 1, 1) } else { NaiveDate::from_ymd_opt(d.year(), d.month() + 1, 1) };
                    next.map(|n| (n - d).num_days()).unwrap_or(30)
                }
                _ => 1,
            };
            let mut day_min: i64 = (0..days).map(|k| p.calendar.day_intervals(d + Duration::days(k)).iter().map(|(a, b)| (*b as i64 - *a as i64).max(0)).sum::<i64>()).sum();
            if day_min == 0 {
                day_min = p.calendar.minutes_per_day();
            }
            let cap = p.resources[ri].max_units / 100.0 * day_min as f64;
            let m: i64 = v.iter().map(|x| x.1).sum();
            let tasks_here = {
                let mut t: Vec<usize> = v.iter().map(|x| x.0).collect();
                t.sort_unstable();
                t.dedup();
                t.len()
            };
            if m as f64 > cap + 0.5 && tasks_here > 1 && worst.map(|w| (d, ri) < w).unwrap_or(true) {
                worst = Some((d, ri));
            }
        }
        let Some((d, ri)) = worst else { break };
        let mut tasks: Vec<usize> = load[&(ri, d)].iter().map(|x| x.0).collect();
        tasks.sort();
        tasks.dedup();
        // keep the most important task; delay the others to just after it. Leveling order: ID
        // Only keeps the lowest ID; Standard looks at slack, dates and priority; Priority, Standard
        // at priority first
        let key = |i: usize| {
            let t = &p.tasks[i];
            let movable = !t.manual && t.actual_start.is_none() && may_move(i);
            match opt.order {
                0 => (!movable, 0u16, std::cmp::Reverse(0i64), std::cmp::Reverse(None), std::cmp::Reverse(i)),
                2 => (!movable, t.priority, std::cmp::Reverse(t.slack_min), std::cmp::Reverse(t.start_dt), std::cmp::Reverse(i)),
                _ => (!movable, 0u16, std::cmp::Reverse(t.slack_min), std::cmp::Reverse(t.start_dt), std::cmp::Reverse(i)),
            }
        };
        tasks.sort_by_key(|&i| std::cmp::Reverse(key(i)));
        let keep = tasks[0];
        let keep_f = p.tasks[keep].finish_dt.unwrap_or_else(|| p.calendar.day_end(p.tasks[keep].finish));
        let mut moved = false;
        for &i in &tasks[1..] {
            let t = &p.tasks[i];
            if t.manual || t.actual_start.is_some() || !may_move(i) {
                continue;
            }
            let st = t.start_dt.unwrap_or_else(|| p.calendar.day_start(t.start));
            let extra = p.calendar.work_minutes(st, keep_f).max(1);
            // Level only within available slack: never past the task's slack
            if opt.within_slack && p.tasks[i].level_delay_min + extra > slack0[i] {
                continue;
            }
            p.tasks[i].level_delay_min += extra;
            delayed.insert(i);
            moved = true;
        }
        if !moved {
            break;
        }
        schedule(p)?;
    }
    Ok(delayed.len())
}

/// Would adding `pred -> succ` create a cycle?
pub fn creates_cycle(p: &Project, pred_uid: u32, succ_uid: u32) -> bool {
    if pred_uid == succ_uid {
        return true;
    }
    // DFS from pred backwards through its predecessors looking for succ
    let mut stack = vec![pred_uid];
    let mut seen = std::collections::HashSet::new();
    while let Some(u) = stack.pop() {
        if u == succ_uid {
            return true;
        }
        if !seen.insert(u) {
            continue;
        }
        if let Some(i) = p.index_of_uid(u) {
            for l in &p.tasks[i].predecessors {
                stack.push(l.pred_uid);
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DurUnit, Link, Task};

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn at(y: i32, m: u32, d: u32, h: u32, mi: u32) -> NaiveDateTime {
        day(y, m, d).and_hms_opt(h, mi, 0).unwrap()
    }

    fn project() -> Project {
        let mut p = Project::new();
        p.start = day(2026, 10, 19); // Monday
        p
    }

    fn link(pred: u32, kind: LinkType, lag_days: i32) -> Link {
        Link { pred_uid: pred, kind, lag: lag_days, ..Default::default() }
    }

    #[test]
    fn fs_chain_skips_weekend_and_finds_critical_path() {
        let mut p = project();
        let mut a = Task::new(1, "A", p.start);
        a.duration = 3;
        let mut b = Task::new(2, "B", p.start);
        b.duration = 4;
        b.predecessors.push(link(1, LinkType::FS, 0));
        let mut c = Task::new(3, "C", p.start);
        c.duration = 1;
        p.tasks = vec![a, b, c];
        schedule(&mut p).unwrap();
        assert_eq!(p.tasks[0].finish, day(2026, 10, 21));
        assert_eq!(p.tasks[1].start, day(2026, 10, 22));
        assert_eq!(p.tasks[1].finish, day(2026, 10, 27)); // Thu..Tue over weekend
        assert!(p.tasks[0].critical && p.tasks[1].critical);
        assert!(!p.tasks[2].critical);
        assert_eq!(p.tasks[2].total_slack, 6);
    }

    #[test]
    fn summary_rolls_up_children() {
        let mut p = project();
        let mut s = Task::new(1, "Phase", p.start);
        s.outline_level = 1;
        let mut a = Task::new(2, "A", p.start);
        a.outline_level = 2;
        a.duration = 2;
        a.percent_complete = 100;
        let mut b = Task::new(3, "B", p.start);
        b.outline_level = 2;
        b.duration = 2;
        b.predecessors.push(link(2, LinkType::FS, 1));
        p.tasks = vec![s, a, b];
        schedule(&mut p).unwrap();
        assert!(p.tasks[0].is_summary);
        assert_eq!(p.tasks[0].start, day(2026, 10, 19));
        assert_eq!(p.tasks[2].start, day(2026, 10, 22));
        assert_eq!(p.tasks[0].finish, day(2026, 10, 23));
        assert_eq!(p.tasks[0].percent_complete, 50);
    }

    #[test]
    fn cycle_is_detected() {
        let mut p = project();
        let mut a = Task::new(1, "A", p.start);
        a.predecessors.push(link(2, LinkType::FS, 0));
        let b = Task::new(2, "B", p.start);
        p.tasks = vec![a, b];
        assert!(creates_cycle(&p, 1, 2));
        p.tasks[1].predecessors.push(link(1, LinkType::FS, 0));
        assert!(schedule(&mut p).is_err());
    }

    #[test]
    fn holidays_are_skipped() {
        let mut p = project();
        p.calendar.holidays.push(crate::model::Holiday { name: "H".into(), start: day(2026, 10, 20), finish: day(2026, 10, 20) });
        let mut a = Task::new(1, "A", p.start);
        a.duration = 2;
        p.tasks = vec![a];
        schedule(&mut p).unwrap();
        assert_eq!(p.tasks[0].finish, day(2026, 10, 21));
    }

    #[test]
    fn milestone_timing_matches_project() {
        // start-of-day milestone does not push a successor to the next day
        let mut p = project();
        let mut m = Task::new(1, "M", p.start);
        m.duration = 0;
        let mut a = Task::new(2, "A", p.start);
        a.duration = 2;
        a.predecessors.push(link(1, LinkType::FS, 0));
        // milestone after A finishes is shown on A's finish day at 17:00
        let mut m2 = Task::new(3, "M2", p.start);
        m2.duration = 0;
        m2.predecessors.push(link(2, LinkType::FS, 0));
        // +3 working days after a start-of-day milestone ends at 17:00 of the 3rd day
        let mut m3 = Task::new(4, "M3", p.start);
        m3.duration = 0;
        m3.predecessors.push(link(1, LinkType::FS, 3));
        p.tasks = vec![m, a, m2, m3];
        schedule(&mut p).unwrap();
        assert_eq!(p.tasks[1].start, day(2026, 10, 19));
        assert_eq!(p.tasks[2].start, day(2026, 10, 20));
        assert_eq!(p.tasks[2].start_dt, Some(at(2026, 10, 20, 17, 0)));
        assert_eq!(p.tasks[3].start, day(2026, 10, 21));
    }

    #[test]
    fn elapsed_lag_counts_calendar_days() {
        let mut p = project();
        let mut a = Task::new(1, "A", p.start);
        a.duration = 1; // Mon 19 Oct
        let mut b = Task::new(2, "B", p.start);
        b.duration = 1;
        // finish Mon 17:00 + 5 calendar days = Sat 17:00 -> next working day Mon 26
        b.predecessors.push(Link { pred_uid: 1, kind: LinkType::FS, lag: 5, elapsed: true, ..Default::default() });
        p.tasks = vec![a, b];
        schedule(&mut p).unwrap();
        assert_eq!(p.tasks[1].start, day(2026, 10, 26));
    }

    /// Elapsed lags ending in non-working time (Project, A Thu 22 Oct 08:00 - Wed 28 Oct
    /// 17:00): a milestone stays at the instant, a one-day task moves on to the next working time.
    #[test]
    fn elapsed_lag_milestone_keeps_the_instant() {
        let mut p = Project::new();
        p.start = day(2026, 10, 22);
        let mut a = Task::new(1, "A", p.start);
        a.set_duration_minutes(5 * 480, DurUnit::Days, false, 480);
        p.tasks.push(a);
        let cases: [(i64, bool, NaiveDateTime); 10] = [
            (62 * 60, true, at(2026, 10, 31, 7, 0)),
            (62 * 60, false, at(2026, 11, 2, 8, 0)),
            (67 * 60 + 30, true, at(2026, 10, 31, 12, 30)),
            (74 * 60, true, at(2026, 10, 31, 19, 0)),
            (74 * 60, false, at(2026, 11, 2, 8, 0)),
            (60, true, at(2026, 10, 28, 18, 0)),
            (24 * 60, true, at(2026, 10, 29, 17, 0)),
            (24 * 60, false, at(2026, 10, 30, 8, 0)),
            (17 * 60, true, at(2026, 10, 29, 10, 0)),
            (-5 * 60, true, at(2026, 10, 28, 12, 0)),
        ];
        for (k, (lag, milestone, _)) in cases.iter().enumerate() {
            let mut t = Task::new(k as u32 + 2, "S", p.start);
            t.set_duration_minutes(if *milestone { 0 } else { 480 }, DurUnit::Days, false, 480);
            t.milestone = *milestone;
            let mut l = Link::new(1, LinkType::FS);
            l.set_lag_minutes(*lag, DurUnit::Hours, true, 480);
            t.predecessors.push(l);
            p.tasks.push(t);
        }
        // a task after the Saturday 07:00 milestone: the next working time; an elapsed hour after it
        let mut t = Task::new(20, "after", p.start);
        t.set_duration_minutes(480, DurUnit::Days, false, 480);
        t.predecessors.push(Link::new(2, LinkType::FS));
        p.tasks.push(t);
        let mut m = Task::new(21, "after-1eh", p.start);
        m.set_duration_minutes(0, DurUnit::Days, false, 480);
        m.milestone = true;
        let mut l = Link::new(2, LinkType::FS);
        l.set_lag_minutes(60, DurUnit::Hours, true, 480);
        m.predecessors.push(l);
        p.tasks.push(m);
        schedule(&mut p).unwrap();
        assert_eq!(p.tasks[0].finish_dt, Some(at(2026, 10, 28, 17, 0)));
        for (k, (_, _, want)) in cases.iter().enumerate() {
            assert_eq!(p.tasks[k + 1].start_dt, Some(*want), "case {k}");
        }
        assert_eq!(p.tasks[11].start_dt, Some(at(2026, 11, 2, 8, 0)));
        assert_eq!(p.tasks[12].start_dt, Some(at(2026, 10, 31, 8, 0)));
    }

    /// A manually scheduled task entered in non-working time (MPP): its start stays and
    /// its day counts as a working one, the stretch it starts in too.
    #[test]
    fn manual_task_in_non_working_time() {
        let c = Calendar::default();
        let cases = [
            (at(2026, 11, 2, 8, 0), 480, at(2026, 11, 2, 17, 0)),
            (at(2026, 11, 2, 10, 0), 480, at(2026, 11, 3, 10, 0)),
            (at(2026, 11, 2, 12, 30), 480, at(2026, 11, 3, 11, 30)),
            (at(2026, 11, 2, 12, 0), 480, at(2026, 11, 3, 12, 0)),
            (at(2026, 11, 2, 17, 0), 480, at(2026, 11, 3, 17, 0)),
            (at(2026, 11, 2, 19, 0), 480, at(2026, 11, 3, 11, 0)),
            (at(2026, 11, 7, 8, 0), 480, at(2026, 11, 7, 17, 0)),
            (at(2026, 11, 7, 1, 0), 480, at(2026, 11, 7, 9, 0)),
            (at(2026, 11, 2, 1, 0), 480, at(2026, 11, 2, 9, 0)),
            (at(2026, 11, 7, 8, 0), 960, at(2026, 11, 9, 17, 0)),
            (at(2026, 11, 7, 1, 0), 960, at(2026, 11, 9, 9, 0)),
            (at(2026, 11, 2, 1, 0), 960, at(2026, 11, 3, 9, 0)),
        ];
        for (s, m, f) in cases {
            assert_eq!(c.manual_finish(s, m), f, "{s}");
        }
        // in a schedule: a manual task entered on Saturday keeps it
        let mut p = Project::new();
        p.start = day(2026, 11, 2);
        let mut t = Task::new(1, "Sat", p.start);
        t.manual = true;
        t.set_duration_minutes(960, DurUnit::Days, false, 480);
        t.start_dt = Some(at(2026, 11, 7, 8, 0));
        t.start = day(2026, 11, 7);
        p.tasks.push(t);
        let mut n = Task::new(2, "Nothing", p.start);
        n.manual = true;
        n.set_duration_minutes(480, DurUnit::Days, false, 480);
        n.set_nothing_entered();
        p.tasks.push(n);
        p.start_dt = Some(at(2026, 11, 2, 1, 0));
        schedule(&mut p).unwrap();
        assert_eq!((p.tasks[0].start_dt, p.tasks[0].finish_dt), (Some(at(2026, 11, 7, 8, 0)), Some(at(2026, 11, 9, 17, 0))));
        assert_eq!((p.tasks[1].start_dt, p.tasks[1].finish_dt), (Some(at(2026, 11, 2, 1, 0)), Some(at(2026, 11, 2, 9, 0))));
    }

    // ---- reference cases (project start Wed 21 Oct 2026) ----

    fn wed() -> Project {
        let mut p = Project::new();
        p.start = day(2026, 10, 21);
        p
    }

    fn task(uid: u32, minutes: i64, unit: DurUnit) -> Task {
        let mut t = Task::new(uid, "T", day(2026, 10, 21));
        t.set_duration_minutes(minutes, unit, false, 480);
        t
    }

    #[test]
    fn part_day_durations_end_at_noon() {
        let mut p = wed();
        p.tasks = vec![task(1, 240, DurUnit::Days), task(2, 240, DurUnit::Hours), task(3, 1200, DurUnit::Days)];
        schedule(&mut p).unwrap();
        assert_eq!(p.tasks[0].finish_dt, Some(at(2026, 10, 21, 12, 0)));
        assert_eq!(p.tasks[1].finish_dt, Some(at(2026, 10, 21, 12, 0)));
        assert_eq!(p.tasks[2].finish_dt, Some(at(2026, 10, 23, 12, 0)));
    }

    #[test]
    fn four_hour_lag_starts_after_lunch() {
        let mut p = wed();
        let a = task(1, 480, DurUnit::Days);
        let mut f = task(2, 480, DurUnit::Days);
        let mut l = Link::new(1, LinkType::FS);
        l.set_lag_minutes(240, DurUnit::Hours, false, 480);
        f.predecessors.push(l);
        p.tasks = vec![a, f];
        schedule(&mut p).unwrap();
        assert_eq!(p.tasks[1].start_dt, Some(at(2026, 10, 22, 13, 0)));
        assert_eq!(p.tasks[1].finish_dt, Some(at(2026, 10, 23, 12, 0)));
    }

    #[test]
    fn elapsed_duration_runs_over_the_weekend() {
        let mut p = wed();
        let mut t = Task::new(1, "3ed", p.start);
        t.set_duration_minutes(3 * 1440, DurUnit::Days, true, 480);
        p.tasks = vec![t];
        schedule(&mut p).unwrap();
        assert_eq!(p.tasks[0].finish_dt, Some(at(2026, 10, 24, 8, 0)));
    }

    #[test]
    fn inactive_predecessor_acts_at_its_start() {
        let mut p = wed();
        let mut s = Task::new(1, "S", p.start);
        s.outline_level = 1;
        let mut a = task(2, 960, DurUnit::Days);
        a.outline_level = 2;
        let mut b = task(3, 1440, DurUnit::Days);
        b.outline_level = 2;
        b.active = false;
        b.predecessors.push(link(2, LinkType::FS, 0));
        let mut c = task(4, 480, DurUnit::Days);
        c.outline_level = 2;
        c.predecessors.push(link(3, LinkType::FS, 0));
        p.tasks = vec![s, a, b, c];
        schedule(&mut p).unwrap();
        assert_eq!(p.tasks[2].start, day(2026, 10, 23));
        assert_eq!(p.tasks[3].start_dt, Some(at(2026, 10, 23, 8, 0)));
        // the inactive task does not stretch its summary
        assert_eq!(p.tasks[0].finish_dt, Some(at(2026, 10, 23, 17, 0)));
    }

    #[test]
    fn start_to_finish_can_schedule_before_the_project_start() {
        let mut p = wed();
        let a = task(1, 2400, DurUnit::Days);
        let mut e = task(2, 480, DurUnit::Days);
        e.predecessors.push(link(1, LinkType::SF, 0));
        p.tasks = vec![a, e];
        schedule(&mut p).unwrap();
        assert_eq!(p.tasks[1].start_dt, Some(at(2026, 10, 20, 8, 0)));
        assert_eq!(p.tasks[1].finish_dt, Some(at(2026, 10, 21, 8, 0)));
    }

    #[test]
    fn alap_task_moves_to_the_project_finish() {
        let mut p = wed();
        let a = task(1, 2400, DurUnit::Days);
        let mut h = task(2, 960, DurUnit::Days);
        h.constraint_type = ConstraintType::Alap;
        p.tasks = vec![a, h];
        schedule(&mut p).unwrap();
        assert_eq!(p.tasks[1].start_dt, Some(at(2026, 10, 26, 8, 0)));
        assert_eq!(p.tasks[1].finish_dt, Some(at(2026, 10, 27, 17, 0)));
    }

    #[test]
    fn percent_lag_uses_the_predecessor_duration() {
        let mut p = wed();
        let a = task(1, 1920, DurUnit::Days); // 4 days
        let mut d = task(2, 480, DurUnit::Days);
        d.predecessors.push(Link { pred_uid: 1, kind: LinkType::FS, lag_pct: Some(50.0), ..Default::default() });
        p.tasks = vec![a, d];
        schedule(&mut p).unwrap();
        // A Wed 21 - Mon 26 + 2 days -> Thu 29
        assert_eq!(p.tasks[1].start, day(2026, 10, 29));
    }

    #[test]
    fn calendar_minute_arithmetic() {
        let c = crate::model::Calendar::default();
        // Fri 23 Oct 2026 13:00 -> Mon 26 10:00: 4 h Friday + 2 h Monday
        assert_eq!(c.work_minutes(at(2026, 10, 23, 13, 0), at(2026, 10, 26, 10, 0)), 360);
        assert_eq!(c.work_minutes(at(2026, 10, 26, 10, 0), at(2026, 10, 23, 13, 0)), -360);
        assert_eq!(c.next_work_instant(at(2026, 10, 23, 17, 0)), at(2026, 10, 26, 8, 0));
        assert_eq!(c.next_work_instant(at(2026, 10, 23, 12, 30)), at(2026, 10, 23, 13, 0));
        assert_eq!(c.prev_work_end(at(2026, 10, 26, 8, 0)), at(2026, 10, 23, 17, 0));
        assert_eq!(c.prev_work_end(at(2026, 10, 24, 9, 0)), at(2026, 10, 23, 17, 0));
        assert_eq!(c.add_work_minutes(at(2026, 10, 23, 13, 0), 240), at(2026, 10, 23, 17, 0));
        assert_eq!(c.add_work_minutes(at(2026, 10, 23, 13, 0), 300), at(2026, 10, 26, 9, 0));
        assert_eq!(c.sub_work_minutes(at(2026, 10, 26, 9, 0), 300), at(2026, 10, 23, 13, 0));
        assert_eq!(c.sub_work_minutes(at(2026, 10, 26, 8, 0), 480), at(2026, 10, 23, 8, 0));
    }

    #[test]
    fn snet_at_an_exact_instant() {
        // a bar dragged at hour zoom: Start No Earlier Than Wed 13:00
        let mut p = wed();
        let mut a = task(1, 480, DurUnit::Days);
        a.set_snet_at(at(2026, 10, 21, 13, 0));
        p.tasks = vec![a];
        schedule(&mut p).unwrap();
        assert_eq!(p.tasks[0].start_dt, Some(at(2026, 10, 21, 13, 0)));
        assert_eq!(p.tasks[0].finish_dt, Some(at(2026, 10, 22, 12, 0)));
        // typing a date drops the time again
        p.tasks[0].set_snet(day(2026, 10, 21));
        schedule(&mut p).unwrap();
        assert_eq!(p.tasks[0].start_dt, Some(at(2026, 10, 21, 8, 0)));
    }

    #[test]
    fn a_started_task_stays_at_its_actual_start() {
        // A 2 days (Wed-Thu) -> B; B was actually started on Wed at 13:00, before A finished
        let mut p = wed();
        let a = task(1, 960, DurUnit::Days);
        let mut b = task(2, 960, DurUnit::Days);
        b.predecessors.push(Link::new(1, LinkType::FS));
        b.actual_start = Some(at(2026, 10, 21, 13, 0));
        b.percent_complete = 25;
        p.tasks = vec![a, b];
        schedule(&mut p).unwrap();
        assert_eq!(p.tasks[1].start_dt, Some(at(2026, 10, 21, 13, 0)));
        assert_eq!(p.tasks[1].finish_dt, Some(at(2026, 10, 23, 12, 0)));
        // an actual finish ends it there
        p.tasks[1].actual_finish = Some(at(2026, 10, 22, 17, 0));
        p.tasks[1].percent_complete = 100;
        schedule(&mut p).unwrap();
        assert_eq!(p.tasks[1].finish_dt, Some(at(2026, 10, 22, 17, 0)));
    }

    #[test]
    fn percent_complete_sets_actual_dates_like_project() {
        let mut p = wed();
        p.tasks = vec![task(1, 1920, DurUnit::Days)];
        schedule(&mut p).unwrap();
        let cal = p.calendar.clone();
        p.tasks[0].set_percent(50, &cal);
        assert_eq!(p.tasks[0].actual_start, Some(at(2026, 10, 21, 8, 0)));
        assert_eq!(p.tasks[0].actual_minutes(480), 960);
        assert_eq!(p.tasks[0].remaining_minutes(480), 960);
        assert_eq!(p.tasks[0].actual_finish, None);
        p.tasks[0].set_percent(100, &cal);
        // 4 days from Wed 08:00: Mon 17:00
        assert_eq!(p.tasks[0].actual_finish, Some(at(2026, 10, 26, 17, 0)));
        p.tasks[0].set_percent(0, &cal);
        assert_eq!((p.tasks[0].actual_start, p.tasks[0].actual_finish), (None, None));
        // an actual duration of 75 minutes out of 80 hours shows as 2 %, as in MPP
        let mut t = task(2, 4800, DurUnit::Days);
        t.set_actual_minutes(75, &cal);
        assert_eq!((t.percent_complete, t.actual_minutes(480)), (2, 75));
        // an actual finish typed sets the duration from the actual start
        let mut t = task(3, 480, DurUnit::Days);
        t.start_dt = Some(at(2026, 10, 21, 8, 0));
        t.set_actual_finish(Some(at(2026, 10, 23, 17, 0)), &cal);
        assert_eq!((t.dur_min, t.percent_complete), (1440, 100));
    }

    #[test]
    fn summary_progress_is_weighted_by_duration() {
        let mut p = wed();
        let mut s = Task::new(1, "S", p.start);
        s.outline_level = 1;
        let mut a = task(2, 480, DurUnit::Days);
        a.outline_level = 2;
        let mut b = task(3, 1440, DurUnit::Days);
        b.outline_level = 2;
        p.tasks = vec![s, a, b];
        schedule(&mut p).unwrap();
        let cal = p.calendar.clone();
        p.tasks[1].set_percent(100, &cal);
        schedule(&mut p).unwrap();
        // 1 day done of 4 days of subtasks
        assert_eq!(p.tasks[0].percent_complete, 25);
        assert_eq!(p.tasks[0].actual_start, Some(at(2026, 10, 21, 8, 0)));
        assert_eq!(p.tasks[0].actual_finish, None);
    }

    fn with_engineers(n: usize) -> Project {
        let mut p = wed();
        for k in 0..n {
            let mut r = crate::model::Resource::new(k as u32 + 1, &format!("E{k}"));
            r.std_rate = 100.0;
            r.cost_per_use = 25.0;
            p.resources.push(r);
        }
        p.next_res_uid = n as u32 + 1;
        p
    }

    #[test]
    fn work_follows_the_task_type() {
        let mut p = with_engineers(2);
        let mut t = task(1, 2400, DurUnit::Days);
        t.resources = vec![1];
        p.tasks = vec![t];
        schedule(&mut p).unwrap();
        // 5 days at 100 %: 40 hours, $4,000 + $25 per use
        assert_eq!(p.tasks[0].work_min, 2400);
        assert!((p.tasks[0].cost - 4025.0).abs() < 1e-6);
        // Fixed Units: a longer duration means more work
        p.tasks[0].set_duration_minutes(4800, DurUnit::Days, false, 480);
        schedule(&mut p).unwrap();
        assert_eq!(p.tasks[0].assignments[0].work_min, 4800);
        // Fixed Work: the work stays and the units drop
        p.tasks[0].task_type = TaskType::FixedWork;
        p.tasks[0].set_duration_minutes(9600, DurUnit::Days, false, 480);
        schedule(&mut p).unwrap();
        assert_eq!(p.tasks[0].assignments[0].work_min, 4800);
        assert!((p.tasks[0].assignments[0].units - 0.5).abs() < 1e-9);
    }

    #[test]
    fn effort_driven_task_gets_shorter_with_more_people() {
        let mut p = with_engineers(2);
        let mut t = task(1, 1920, DurUnit::Days);
        t.effort_driven = true;
        t.resources = vec![1];
        p.tasks = vec![t];
        schedule(&mut p).unwrap();
        assert_eq!(p.tasks[0].work_min, 1920);
        p.tasks[0].resources.push(2);
        schedule(&mut p).unwrap();
        // 32 hours shared by two people: 2 days
        assert_eq!((p.tasks[0].dur_min, p.tasks[0].work_min), (960, 1920));
        assert_eq!(p.tasks[0].assignments.iter().map(|a| a.work_min).collect::<Vec<_>>(), vec![960, 960]);
        // taking one away gives the work back to the other
        p.tasks[0].resources.retain(|u| *u != 2);
        schedule(&mut p).unwrap();
        assert_eq!((p.tasks[0].dur_min, p.tasks[0].work_min), (1920, 1920));
    }

    #[test]
    fn leveling_reports_unresolved_overallocations_without_changing_the_plan() {
        let mut p = with_engineers(1);
        let mut a = task(1, 960, DurUnit::Days);
        a.resources = vec![1];
        let mut b = task(2, 960, DurUnit::Days);
        b.resources = vec![1];
        p.tasks = vec![a, b];
        schedule(&mut p).unwrap();
        let starts: Vec<_> = p.tasks.iter().map(|t| t.start_dt).collect();
        let report = level_checked(&mut p, None, Some(&[])).unwrap();
        assert_eq!(report.delayed_tasks, 0);
        assert_eq!(report.unresolved_resources, 1);
        assert_eq!(p.tasks.iter().map(|t| t.start_dt).collect::<Vec<_>>(), starts);
    }

    #[test]
    fn leveling_rolls_back_on_a_circular_dependency() {
        let mut p = with_engineers(1);
        let mut a = task(1, 480, DurUnit::Days);
        let mut b = task(2, 480, DurUnit::Days);
        a.predecessors.push(Link::new(2, LinkType::FS));
        b.predecessors.push(Link::new(1, LinkType::FS));
        p.tasks = vec![a, b];
        let before = serde_json::to_value(&p).unwrap();
        assert!(level_checked(&mut p, None, None).is_err());
        assert_eq!(serde_json::to_value(&p).unwrap(), before);
    }

    #[test]
    fn overallocation_and_leveling() {
        let mut p = with_engineers(1);
        let mut a = task(1, 960, DurUnit::Days);
        a.resources = vec![1];
        let mut b = task(2, 960, DurUnit::Days);
        b.resources = vec![1];
        b.priority = 400;
        p.tasks = vec![a, b];
        schedule(&mut p).unwrap();
        assert!(p.resources[0].overallocated && p.tasks[0].overallocated && p.tasks[1].overallocated);
        // leveling delays the lower-priority task to after the other one
        let n = level(&mut p, None);
        assert_eq!(n, 1);
        assert_eq!(p.tasks[1].start_dt, Some(at(2026, 10, 23, 8, 0)));
        assert!(!p.resources[0].overallocated);
        assert_eq!(p.tasks[0].start_dt, Some(at(2026, 10, 21, 8, 0)));
    }
}
