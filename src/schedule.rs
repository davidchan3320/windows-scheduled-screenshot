use crate::config::{DAYS, Task, clock};
use chrono::{DateTime, Datelike, Duration, LocalResult, NaiveDateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Progress {
    pub interval_anchor_utc: Option<DateTime<Utc>>,
    pub duration_deadline_utc: Option<DateTime<Utc>>,
    pub completed_captures: u32,
    pub completed_fixed_times: Vec<String>,
    pub last_fixed_occurrence: Option<String>,
    pub once_result: Option<String>,
    pub last_result: Option<String>,
    pub last_run_utc: Option<DateTime<Utc>>,
    pub last_files: Vec<String>,
    pub schedule_signature: Option<String>,
    pub stop_signature: Option<String>,
    pub generation: u64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Runtime {
    pub tasks: BTreeMap<String, Progress>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Occurrence {
    pub due: DateTime<Utc>,
    pub key: Option<String>,
    pub slot: Option<String>,
}

// Choose the later UTC instant in a fold, matching the legacy scheduler. Never
// return both instants for one wall-clock slot; gaps have no occurrence.
pub fn local_to_utc<T: TimeZone>(tz: &T, n: NaiveDateTime) -> Option<DateTime<Utc>> {
    match tz.from_local_datetime(&n) {
        LocalResult::Single(d) => Some(d.with_timezone(&Utc)),
        LocalResult::Ambiguous(a, b) => Some(a.max(b).with_timezone(&Utc)),
        LocalResult::None => None,
    }
}
fn parse_local<T: TimeZone>(tz: &T, s: Option<&str>) -> Option<DateTime<Utc>> {
    local_to_utc(
        tz,
        NaiveDateTime::parse_from_str(s?, "%Y-%m-%dT%H:%M:%S").ok()?,
    )
}
pub fn signature(t: &Task) -> String {
    serde_json::to_string(&t.schedule).expect("serializable schedule")
}
pub fn stop_signature(t: &Task) -> String {
    serde_json::to_string(&t.stop_condition).expect("serializable stop")
}

pub fn reconcile(t: &Task, p: &mut Progress, now: DateTime<Utc>) {
    let sig = signature(t);
    if p.schedule_signature.as_ref() != Some(&sig) {
        p.interval_anchor_utc = Some(now);
        p.completed_fixed_times.clear();
        p.last_fixed_occurrence = None;
        p.once_result = None;
        p.schedule_signature = Some(sig);
        p.generation += 1;
    }
    let stop = stop_signature(t);
    if p.stop_signature.as_ref() != Some(&stop) {
        p.duration_deadline_utc = if t.stop_condition.mode == "duration" {
            Some(now + Duration::seconds(t.stop_condition.duration_seconds.unwrap_or(1) as i64))
        } else {
            None
        };
        p.completed_captures = 0;
        p.stop_signature = Some(stop);
        p.generation += 1;
    }
    p.interval_anchor_utc.get_or_insert(now);
}
pub fn deadline<T: TimeZone>(t: &Task, p: &Progress, tz: &T) -> Option<DateTime<Utc>> {
    match t.stop_condition.mode.as_str() {
        "at" => parse_local(tz, t.stop_condition.end_at_local.as_deref()),
        "duration" => p.duration_deadline_utc,
        _ => None,
    }
}
pub fn exhausted<T: TimeZone>(t: &Task, p: &Progress, now: DateTime<Utc>, tz: &T) -> bool {
    (t.stop_condition.mode == "count"
        && p.completed_captures >= t.stop_condition.capture_count.unwrap_or(u32::MAX))
        || deadline(t, p, tz).is_some_and(|d| d <= now)
        || (t.schedule.kind == "once" && p.once_result.is_some())
        || (t.schedule.kind == "fixedOnce"
            && t.schedule
                .times
                .iter()
                .all(|s| p.completed_fixed_times.contains(s)))
}

pub fn next<T: TimeZone>(
    t: &Task,
    p: &Progress,
    after: DateTime<Utc>,
    tz: &T,
) -> Option<Occurrence> {
    if !t.enabled || exhausted(t, p, after, tz) {
        return None;
    }
    let candidate = next_unbounded(t, p, after, tz)?;
    if deadline(t, p, tz).is_some_and(|d| candidate.due >= d) {
        None
    } else {
        Some(candidate)
    }
}

fn next_unbounded<T: TimeZone>(
    t: &Task,
    p: &Progress,
    after: DateTime<Utc>,
    tz: &T,
) -> Option<Occurrence> {
    let s = &t.schedule;
    if s.kind == "once" {
        let due = parse_local(tz, s.at_local.as_deref())?;
        return (due > after).then_some(Occurrence {
            due,
            key: s.at_local.clone(),
            slot: None,
        });
    }
    let mut date = after.with_timezone(tz).date_naive();
    let mut times = s.times.clone();
    times.sort();
    // Jump over long intervals instead of losing schedules outside a 7-day horizon.
    // A bounded search also handles cadences that never intersect active hours.
    for _ in 0..3660 {
        let current_date = date;
        date = date.succ_opt()?;
        let date_this_iteration = current_date;
        if !s
            .weekdays
            .iter()
            .any(|d| d == DAYS[date_this_iteration.weekday().num_days_from_monday() as usize])
        {
            continue;
        }
        if s.kind == "interval" {
            let start = if s.active_hours_enabled {
                current_date.and_time(clock(&s.active_start).ok()?)
            } else {
                current_date.and_hms_opt(0, 0, 0)?
            };
            let end = if s.active_hours_enabled {
                current_date.and_time(clock(&s.active_end).ok()?)
            } else {
                current_date.succ_opt()?.and_hms_opt(0, 0, 0)?
            };
            let (Some(start), Some(end)) = (local_to_utc(tz, start), local_to_utc(tz, end)) else {
                continue;
            };
            let anchor = p.interval_anchor_utc?;
            let lower = start.max(after + Duration::milliseconds(1));
            let interval = i64::from(s.interval_seconds) * 1000;
            if interval == 0 {
                return None;
            }
            let delta = (lower - anchor).num_milliseconds().max(0);
            let steps = (delta + interval - 1) / interval;
            let due = anchor + Duration::milliseconds(steps * interval);
            if due >= start && due < end {
                return Some(Occurrence {
                    due,
                    key: None,
                    slot: None,
                });
            }
            date = date.max(due.with_timezone(tz).date_naive());
        } else {
            for time in &times {
                if s.kind == "fixedOnce" && p.completed_fixed_times.contains(time) {
                    continue;
                }
                let local = current_date.and_time(clock(time).ok()?);
                let key = local.format("%Y-%m-%dT%H:%M:%S").to_string();
                if p.last_fixed_occurrence.as_ref().is_some_and(|k| k >= &key) {
                    continue;
                }
                if let Some(due) = local_to_utc(tz, local).filter(|d| *d > after) {
                    return Some(Occurrence {
                        due,
                        key: Some(key),
                        slot: (s.kind == "fixedOnce").then(|| time.clone()),
                    });
                }
            }
            if current_date >= after.with_timezone(tz).date_naive() + Duration::days(8) {
                return None;
            }
        }
    }
    None
}

pub fn mark_missed_once<T: TimeZone>(
    t: &Task,
    p: &mut Progress,
    now: DateTime<Utc>,
    tz: &T,
) -> bool {
    if t.enabled
        && t.schedule.kind == "once"
        && p.once_result.is_none()
        && parse_local(tz, t.schedule.at_local.as_deref()).is_some_and(|d| d <= now)
    {
        p.once_result = Some("missed".into());
        p.last_result =
            Some("Missed — the scheduler was unavailable at the scheduled time.".into());
        return true;
    }
    false
}
