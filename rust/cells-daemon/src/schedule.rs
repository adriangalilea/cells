//! When a script runs (SPEC.md, A table from a script): its schedule's next moment after it
//! last ran. A trigger with a time of day is read in its schedule's zone (`in Europe/Madrid`),
//! so 18:00 is one moment on every machine and moves with daylight saving as a person's
//! clock does: a time that does not exist that day (the hour skipped in spring) runs at the
//! moment after the gap, and one that happens twice (autumn) runs once, the first time.
//!
//! The rule for a machine that slept or an engine that was not running: the next moment
//! after the last run, if it has passed, is now, so a missed run runs once on waking,
//! never once per miss. A script never run is due at once for `every`, and at its first
//! moment from now for a time of day.

use cells::{Schedule, Trigger};
use jiff::Timestamp;
use jiff::civil::{Date, Weekday};
use jiff::tz::TimeZone;

/// The next moment of `schedule` after `last` (when it last ran, whatever came of it), or,
/// never run, the first from `seen` (when the engine first saw it): nothing when only a
/// file changing starts it, or when its zone is none this Mac knows, which is why.
pub fn next(schedule: &Schedule, last: Option<Timestamp>, seen: Timestamp) -> Result<Option<Timestamp>, String> {
    let zone = match &schedule.zone {
        Some(z) => Some(TimeZone::get(z).map_err(|e| format!("the zone {z}: {e}"))?),
        None => None,
    };
    let mut soonest: Option<Timestamp> = None;
    for t in &schedule.triggers {
        let at = match t {
            Trigger::Every(minutes) => Some(match last {
                Some(l) => l.checked_add(jiff::SignedDuration::from_mins(*minutes as i64)).map_err(|e| e.to_string())?,
                None => seen,
            }),
            Trigger::Changes(_) => None,
            clock => {
                let zone = zone.as_ref().ok_or("a time of day needs its zone: `in Europe/Madrid`, `in UTC`")?;
                Some(after(clock, zone, last.unwrap_or(seen))?)
            }
        };
        soonest = match (soonest, at) {
            (Some(s), Some(a)) => Some(s.min(a)),
            (s, a) => s.or(a),
        };
    }
    Ok(soonest)
}

/// The first moment of a trigger with a time of day strictly after `from`, within the
/// year after it (a cron that names a day no year has, the 30th of February, never comes).
fn after(trigger: &Trigger, zone: &TimeZone, from: Timestamp) -> Result<Timestamp, String> {
    let cron = match trigger {
        Trigger::Cron(fields) => Some(Cron::parse(fields)?),
        _ => None,
    };
    let mut day = from.to_zoned(zone.clone()).date();
    for _ in 0..=366 {
        let minutes: Vec<u16> = match (trigger, &cron) {
            (Trigger::Daily(m), _) => vec![*m],
            (Trigger::Weekdays(m), _) => (day.weekday().to_monday_one_offset() <= 5).then_some(*m).into_iter().collect(),
            (Trigger::Weekly(d, m), _) => (day.weekday().to_monday_one_offset() == *d as i8).then_some(*m).into_iter().collect(),
            // the 31st of a month that has 30 days is its last
            (Trigger::Monthly(d, m), _) => (day.day() == (*d as i8).min(day.days_in_month())).then_some(*m).into_iter().collect(),
            (Trigger::Cron(_), Some(c)) => c.minutes(day),
            _ => unreachable!("a trigger with a time of day"),
        };
        for m in minutes {
            let local = day.at((m / 60) as i8, (m % 60) as i8, 0, 0);
            // a time skipped by daylight saving is the moment after the gap, a time that
            // happens twice the first of them (jiff's "compatible", as a person's clock)
            let at = local.to_zoned(zone.clone()).map_err(|e| e.to_string())?.timestamp();
            if at > from {
                return Ok(at);
            }
        }
        day = day.tomorrow().map_err(|e| e.to_string())?;
    }
    Err("no moment in the year to come matches it".into())
}

/// `cron 0 9 * * 1-5`: minute, hour, day of the month, month, day of the week (0 or 7
/// Sunday), each `*`, a number, a range `a-b`, a step `*/n` or `a-b/n`, or a list of those.
/// As every cron reads it: when both days are restricted, either one matching is the day.
struct Cron {
    minutes: Vec<u8>,
    hours: Vec<u8>,
    days: Vec<u8>,
    months: Vec<u8>,
    weekdays: Vec<u8>,
    days_any: bool,
    weekdays_any: bool,
}

impl Cron {
    fn parse(spelled: &str) -> Result<Cron, String> {
        let f: Vec<&str> = spelled.split_whitespace().collect();
        let [m, h, dom, mon, dow] = f[..] else { return Err(format!("cron {spelled}: five fields")) };
        let field = |s: &str, lo: u8, hi: u8, what: &str| -> Result<Vec<u8>, String> {
            let mut out = Vec::new();
            for part in s.split(',') {
                let (range, step) = match part.split_once('/') {
                    Some((r, n)) => (r, n.parse::<u8>().map_err(|_| format!("cron {spelled}: {what} step {n}"))?),
                    None => (part, 1),
                };
                let (a, b) = match range {
                    "*" => (lo, hi),
                    r => match r.split_once('-') {
                        Some((a, b)) => (num(a, spelled, what)?, num(b, spelled, what)?),
                        None => {
                            let n = num(r, spelled, what)?;
                            (n, if step > 1 { hi } else { n })
                        }
                    },
                };
                if a < lo || b > hi || a > b || step == 0 {
                    return Err(format!("cron {spelled}: {what} {part} is outside {lo}-{hi}"));
                }
                out.extend((a..=b).step_by(step as usize));
            }
            out.sort();
            out.dedup();
            Ok(out)
        };
        let mut weekdays = field(dow, 0, 7, "day of the week")?;
        // 7 is Sunday as 0 is
        if weekdays.contains(&7) {
            weekdays.retain(|d| *d != 7);
            weekdays.insert(0, 0);
            weekdays.dedup();
        }
        Ok(Cron {
            minutes: field(m, 0, 59, "minute")?,
            hours: field(h, 0, 23, "hour")?,
            days: field(dom, 1, 31, "day of the month")?,
            months: field(mon, 1, 12, "month")?,
            weekdays,
            days_any: dom == "*",
            weekdays_any: dow == "*",
        })
    }

    /// The minutes of `day` it runs at, from midnight, in order.
    fn minutes(&self, day: Date) -> Vec<u16> {
        let weekday = match day.weekday() {
            Weekday::Sunday => 0,
            w => w.to_monday_one_offset() as u8,
        };
        let by_day = self.days.contains(&(day.day() as u8));
        let by_weekday = self.weekdays.contains(&weekday);
        let today = match (self.days_any, self.weekdays_any) {
            (true, true) => true,
            (false, true) => by_day,
            (true, false) => by_weekday,
            (false, false) => by_day || by_weekday,
        };
        if !today || !self.months.contains(&(day.month() as u8)) {
            return Vec::new();
        }
        self.hours.iter().flat_map(|h| self.minutes.iter().map(move |m| *h as u16 * 60 + *m as u16)).collect()
    }
}

fn num(s: &str, spelled: &str, what: &str) -> Result<u8, String> {
    s.parse().map_err(|_| format!("cron {spelled}: {what} {s} is no number"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> Timestamp {
        s.parse().unwrap()
    }

    fn schedule(triggers: Vec<Trigger>, zone: &str) -> Schedule {
        Schedule { triggers, zone: Some(zone.into()), spelled: String::new() }
    }

    #[test]
    fn a_time_of_day_is_its_zone_s_and_follows_daylight_saving() {
        let daily = schedule(vec![Trigger::Daily(18 * 60)], "Europe/Madrid");
        // winter: 18:00 in Madrid is 17:00 UTC
        assert_eq!(next(&daily, Some(at("2026-01-10T18:00:00Z")), at("2026-01-01T00:00:00Z")).unwrap(), Some(at("2026-01-11T17:00:00Z")));
        // summer: 16:00 UTC
        assert_eq!(next(&daily, Some(at("2026-07-10T18:00:00Z")), at("2026-01-01T00:00:00Z")).unwrap(), Some(at("2026-07-11T16:00:00Z")));
        // 02:30 does not exist on the morning clocks go forward: the moment after the gap
        let early = schedule(vec![Trigger::Daily(2 * 60 + 30)], "Europe/Madrid");
        assert_eq!(next(&early, Some(at("2026-03-28T12:00:00Z")), at("2026-01-01T00:00:00Z")).unwrap(), Some(at("2026-03-29T01:30:00Z")));
    }

    #[test]
    fn a_missed_run_is_due_once_and_a_new_script_by_its_kind() {
        let daily = schedule(vec![Trigger::Daily(18 * 60)], "UTC");
        // ran Monday, the machine slept until Thursday: due Monday-after 18:00, passed, so now
        let due = next(&daily, Some(at("2026-11-02T18:00:00Z")), at("2026-11-01T00:00:00Z")).unwrap().unwrap();
        assert_eq!(due, at("2026-11-03T18:00:00Z"));
        let every = schedule(vec![Trigger::Every(15)], "UTC");
        assert_eq!(next(&every, None, at("2026-11-08T10:00:00Z")).unwrap(), Some(at("2026-11-08T10:00:00Z")), "never run: now");
        assert_eq!(next(&daily, None, at("2026-11-08T19:00:00Z")).unwrap(), Some(at("2026-11-09T18:00:00Z")), "never run: its next moment");
        let changes = Schedule { triggers: vec![Trigger::Changes("../a.cells".into())], zone: None, spelled: String::new() };
        assert_eq!(next(&changes, None, at("2026-11-08T10:00:00Z")).unwrap(), None);
    }

    #[test]
    fn weekdays_weeks_months_and_the_soonest_trigger() {
        let friday = at("2026-11-06T10:00:00Z");
        let weekdays = schedule(vec![Trigger::Weekdays(9 * 60)], "UTC");
        assert_eq!(next(&weekdays, Some(friday), friday).unwrap(), Some(at("2026-11-09T09:00:00Z")), "Monday");
        let monthly = schedule(vec![Trigger::Monthly(31, 8 * 60)], "UTC");
        assert_eq!(next(&monthly, Some(at("2026-10-31T09:00:00Z")), friday).unwrap(), Some(at("2026-11-30T08:00:00Z")), "a month's last");
        let both = schedule(vec![Trigger::Weekly(7, 12 * 60), Trigger::Daily(23 * 60)], "UTC");
        assert_eq!(next(&both, Some(friday), friday).unwrap(), Some(at("2026-11-06T23:00:00Z")));
    }

    #[test]
    fn cron_as_every_cron_reads_it() {
        let friday = at("2026-11-06T10:00:00Z");
        let c = |f: &str| next(&schedule(vec![Trigger::Cron(f.into())], "UTC"), Some(friday), friday).unwrap().unwrap();
        assert_eq!(c("0 9 * * 1-5"), at("2026-11-09T09:00:00Z"));
        assert_eq!(c("*/15 * * * *"), at("2026-11-06T10:15:00Z"));
        assert_eq!(c("30 8 1 * *"), at("2026-12-01T08:30:00Z"));
        assert_eq!(c("0 0 7 * 5"), at("2026-11-07T00:00:00Z"), "the 7th, a Saturday: either day restricted matches");
        assert_eq!(c("0 12 * * 7"), at("2026-11-08T12:00:00Z"), "7 is Sunday");
        assert!(next(&schedule(vec![Trigger::Cron("61 * * * *".into())], "UTC"), None, friday).unwrap_err().contains("outside"));
    }
}
