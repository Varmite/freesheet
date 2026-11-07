//! Daily streak: a day counts when the learner checks at least GOAL_QUESTIONS practice
//! questions or finishes at least one exam. The streak is the number of consecutive
//! qualifying days ending today, or ending yesterday if today's goal isn't met yet.

use crate::model::{DayEntry, Num, Progress, StreakData};
use chrono::{Datelike, Duration, Local, NaiveDate};

pub const GOAL_QUESTIONS: f64 = 3.0;
pub const GOAL_EXAMS: f64 = 1.0;

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

pub fn day_key(d: NaiveDate) -> String {
    format!("{:04}-{:02}-{:02}", d.year(), d.month(), d.day())
}

fn ensure(p: &mut Progress) -> &mut StreakData {
    p.streak.get_or_insert_with(StreakData::default)
}

fn met(e: Option<&DayEntry>) -> bool {
    e.is_some_and(|e| e.q.0 >= GOAL_QUESTIONS || e.x.0 >= GOAL_EXAMS)
}

fn current(s: &StreakData, today: NaiveDate) -> u32 {
    let mut d = if met(s.days.get(&day_key(today))) { today } else { today - Duration::days(1) };
    let mut n = 0;
    while met(s.days.get(&day_key(d))) {
        n += 1;
        d -= Duration::days(1);
    }
    n
}

pub struct Recorded {
    pub just_met: bool,
    pub current: u32,
}

/// kind: "question" | "exam".
pub fn record(p: &mut Progress, kind: &str, count: f64, now: NaiveDate) -> Recorded {
    let s = ensure(p);
    let e = s.days.entry(day_key(now)).or_default();
    let before = met(Some(e));
    let c = if count > 0.0 { count } else { 1.0 };
    if kind == "exam" {
        e.x = Num(e.x.0 + c);
    } else {
        e.q = Num(e.q.0 + c);
    }
    let just_met = !before && met(Some(e));
    let cur = current(s, now);
    if cur as f64 > s.longest.0 {
        s.longest = Num(cur as f64);
    }
    Recorded { just_met, current: cur }
}

pub struct WeekDay {
    pub date: NaiveDate,
    pub met: bool,
    pub today: bool,
}

pub struct Status {
    pub current: u32,
    pub longest: u32,
    pub today_met: bool,
    pub questions_today: f64,
    pub week: Vec<WeekDay>,
}

pub fn status(p: &Progress, now: NaiveDate) -> Status {
    let empty = StreakData::default();
    let s = p.streak.as_ref().unwrap_or(&empty);
    let e = s.days.get(&day_key(now));
    let week = (0..7)
        .rev()
        .map(|i| {
            let d = now - Duration::days(i);
            WeekDay { date: d, met: met(s.days.get(&day_key(d))), today: i == 0 }
        })
        .collect();
    let cur = current(s, now);
    Status {
        current: cur,
        longest: (s.longest.0 as u32).max(cur),
        today_met: met(e),
        questions_today: e.map_or(0.0, |e| e.q.0),
        week,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streak_counts_days() {
        let mut p = Progress::default();
        let d = NaiveDate::from_ymd_opt(2026, 11, 7).unwrap();
        assert_eq!(status(&p, d).current, 0);
        let r = record(&mut p, "question", 2.0, d - Duration::days(1));
        assert!(!r.just_met);
        let r = record(&mut p, "question", 1.0, d - Duration::days(1));
        assert!(r.just_met);
        assert_eq!(status(&p, d).current, 1); // alive until midnight
        let r = record(&mut p, "exam", 1.0, d);
        assert!(r.just_met && r.current == 2);
        assert_eq!(status(&p, d).longest, 2);
        assert_eq!(status(&p, d).week.len(), 7);
    }
}
