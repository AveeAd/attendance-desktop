use std::collections::HashMap;

use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, TimeZone, Utc};

use crate::db::parse_timestamp;
use crate::error::AppResult;
use crate::models::{
    AppSettings, Employee, EmployeeStatus, EventType, ScheduleEntry, Shift, ShiftAssignment,
    TimeEvent, TimesheetReport, TimesheetRow,
};

#[derive(Debug, Clone)]
pub struct WorkPeriod {
    pub employee_id: i64,
    pub clock_in: String,
    pub clock_in_epoch: i64,
    pub clock_out: Option<String>,
    pub clock_out_epoch: Option<i64>,
    pub break_minutes: i64,
    pub break_open: bool,
    pub is_open: bool,
}

impl WorkPeriod {
    pub fn worked_minutes(&self, now_epoch: i64) -> i64 {
        let end = self.clock_out_epoch.unwrap_or(now_epoch);
        (end - self.clock_in_epoch).max(0) / 60 - self.break_minutes
    }

    pub fn local_date(&self) -> Option<String> {
        parse_timestamp(&self.clock_in)
            .ok()
            .map(|dt| dt.format("%Y-%m-%d").to_string())
    }
}

/// Turns a flat, ordered event stream into clock-in/clock-out periods.
/// Invalid transitions (duplicate scans, orphan breaks) are ignored so a
/// single bad scan cannot corrupt the timesheet.
pub fn compute_periods(events: &[TimeEvent]) -> Vec<WorkPeriod> {
    let mut by_employee: HashMap<i64, Vec<&TimeEvent>> = HashMap::new();
    for event in events {
        by_employee.entry(event.employee_id).or_default().push(event);
    }

    let mut periods: Vec<WorkPeriod> = by_employee
        .values()
        .flat_map(|employee_events| compute_periods_for_employee(employee_events))
        .collect();
    periods.sort_by_key(|period| period.clock_in_epoch);
    periods
}

fn compute_periods_for_employee(events: &[&TimeEvent]) -> Vec<WorkPeriod> {
    let mut ordered: Vec<&TimeEvent> = events.to_vec();
    ordered.sort_by_key(|event| (event.occurred_at_epoch, event.id));

    let mut periods = Vec::new();
    let mut current: Option<WorkPeriod> = None;
    let mut break_start: Option<i64> = None;

    for event in ordered {
        match event.event_type {
            EventType::ClockIn => {
                if current.is_none() {
                    current = Some(WorkPeriod {
                        employee_id: event.employee_id,
                        clock_in: event.occurred_at.clone(),
                        clock_in_epoch: event.occurred_at_epoch,
                        clock_out: None,
                        clock_out_epoch: None,
                        break_minutes: 0,
                        break_open: false,
                        is_open: true,
                    });
                    break_start = None;
                }
            }
            EventType::BreakStart => {
                if let Some(period) = current.as_mut() {
                    if !period.break_open {
                        period.break_open = true;
                        break_start = Some(event.occurred_at_epoch);
                    }
                }
            }
            EventType::BreakEnd => {
                if let Some(period) = current.as_mut() {
                    if period.break_open {
                        if let Some(start) = break_start.take() {
                            period.break_minutes += (event.occurred_at_epoch - start).max(0) / 60;
                        }
                        period.break_open = false;
                    }
                }
            }
            EventType::ClockOut => {
                if let Some(mut period) = current.take() {
                    if period.break_open {
                        if let Some(start) = break_start.take() {
                            period.break_minutes += (event.occurred_at_epoch - start).max(0) / 60;
                        }
                        period.break_open = false;
                    }
                    period.clock_out = Some(event.occurred_at.clone());
                    period.clock_out_epoch = Some(event.occurred_at_epoch);
                    period.is_open = false;
                    periods.push(period);
                }
            }
        }
    }

    if let Some(period) = current {
        periods.push(period);
    }

    periods
}

pub fn build_timesheet(
    employees: &[Employee],
    periods: &[WorkPeriod],
    settings: &AppSettings,
    from_epoch: i64,
    to_epoch: i64,
) -> TimesheetReport {
    let now = Utc::now().timestamp();
    let names: HashMap<i64, &str> = employees
        .iter()
        .map(|employee| (employee.id, employee.name.as_str()))
        .collect();
    let daily_limit = (settings.overtime_daily_hours.max(0.0) * 60.0) as i64;
    let weekly_limit = (settings.overtime_weekly_hours.max(0.0) * 60.0) as i64;

    let mut rows: Vec<TimesheetRow> = periods
        .iter()
        .filter(|period| period.clock_in_epoch >= from_epoch && period.clock_in_epoch < to_epoch)
        .filter_map(|period| {
            let date = period.local_date()?;
            let worked = period.worked_minutes(now);
            let overtime = if daily_limit > 0 {
                (worked - daily_limit).max(0)
            } else {
                0
            };
            Some(TimesheetRow {
                employee_id: period.employee_id,
                employee_name: names
                    .get(&period.employee_id)
                    .unwrap_or(&"Unknown")
                    .to_string(),
                date,
                clock_in: Some(period.clock_in.clone()),
                clock_out: period.clock_out.clone(),
                break_minutes: period.break_minutes,
                worked_minutes: worked,
                overtime_minutes: overtime,
                status: if period.is_open {
                    "open".to_string()
                } else {
                    "complete".to_string()
                },
            })
        })
        .collect();

    rows.sort_by(|a, b| {
        a.date
            .cmp(&b.date)
            .then_with(|| a.employee_name.cmp(&b.employee_name))
            .then_with(|| a.clock_in.cmp(&b.clock_in))
    });

    // Allocate weekly overtime to the final row of each employee/ISO week.
    let mut weeks: HashMap<(i64, i32, i64), (i64, usize)> = HashMap::new();
    for (index, row) in rows.iter().enumerate() {
        if let Ok(date) = NaiveDate::parse_from_str(&row.date, "%Y-%m-%d") {
            let key = (
                row.employee_id,
                date.iso_week().year(),
                date.iso_week().week() as i64,
            );
            let entry = weeks.entry(key).or_insert((0, index));
            entry.0 += row.worked_minutes;
            entry.1 = index;
        }
    }
    for ((employee_id, _, _), (total, last_index)) in weeks {
        let extra = if weekly_limit > 0 {
            (total - weekly_limit).max(0)
        } else {
            0
        };
        if extra > 0 && rows[last_index].employee_id == employee_id {
            rows[last_index].overtime_minutes += extra;
        }
    }

    let total_worked_minutes = rows.iter().map(|row| row.worked_minutes).sum();
    let total_overtime_minutes = rows.iter().map(|row| row.overtime_minutes).sum();
    let employee_count = rows
        .iter()
        .map(|row| row.employee_id)
        .collect::<std::collections::HashSet<_>>()
        .len() as i64;

    TimesheetReport {
        rows,
        total_worked_minutes,
        total_overtime_minutes,
        employee_count,
    }
}

pub fn build_statuses(employees: &[Employee], events: &[TimeEvent]) -> Vec<EmployeeStatus> {
    let now = Utc::now().timestamp();
    let periods = compute_periods(events);

    employees
        .iter()
        .filter(|employee| employee.active)
        .map(|employee| {
            let mut employee_events: Vec<&TimeEvent> = events
                .iter()
                .filter(|event| event.employee_id == employee.id)
                .collect();
            employee_events.sort_by_key(|event| (event.occurred_at_epoch, event.id));
            let last_event = employee_events.last().map(|event| event.occurred_at.clone());

            let open_period = periods
                .iter()
                .rev()
                .find(|period| period.employee_id == employee.id && period.is_open);

            match open_period {
                Some(period) => EmployeeStatus {
                    employee: employee.clone(),
                    state: if period.break_open {
                        "on_break".to_string()
                    } else {
                        "in".to_string()
                    },
                    last_event_at: last_event,
                    clock_in_at: Some(period.clock_in.clone()),
                    open_minutes: period.worked_minutes(now),
                },
                None => EmployeeStatus {
                    employee: employee.clone(),
                    state: "out".to_string(),
                    last_event_at: last_event,
                    clock_in_at: None,
                    open_minutes: 0,
                },
            }
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub fn build_schedule(
    assignments: &[ShiftAssignment],
    shifts: &[Shift],
    periods: &[WorkPeriod],
    events: &[TimeEvent],
    week_start: &str,
) -> AppResult<Vec<ScheduleEntry>> {
    let start = NaiveDate::parse_from_str(week_start, "%Y-%m-%d")
        .map_err(|_| crate::error::AppError::InvalidInput("Invalid week start date".to_string()))?;
    let today = Utc::now().date_naive();
    let mut entries = Vec::new();

    for offset in 0..7 {
        let date = start + chrono::Duration::days(offset);
        let date_str = date.format("%Y-%m-%d").to_string();
        let weekday = date.weekday().num_days_from_monday() as i64 + 1;

        for assignment in assignments.iter().filter(|assignment| {
            assignment.effective_from <= date_str
                && assignment
                    .effective_to
                    .as_deref()
                    .is_none_or(|end| date_str.as_str() <= end)
        }) {
            let shift = match shifts.iter().find(|shift| shift.id == assignment.shift_id) {
                Some(shift) if shift.active && shift.days.contains(&weekday) => shift,
                _ => continue,
            };

            let employee_events: Vec<&TimeEvent> = events
                .iter()
                .filter(|event| {
                    event.employee_id == assignment.employee_id
                        && parse_timestamp(&event.occurred_at)
                            .is_ok_and(|dt| dt.format("%Y-%m-%d").to_string() == date_str)
                })
                .collect();

            let offset_zone = employee_events
                .first()
                .and_then(|event| parse_timestamp(&event.occurred_at).ok())
                .map(|dt| *dt.offset())
                .unwrap_or_else(|| FixedOffset::east_opt(0).unwrap());

            let expected_start = naive_at(&date, &shift.start_time, offset_zone);
            let expected_end = naive_at(&date, &shift.end_time, offset_zone);
            let planned_minutes = (expected_end - expected_start).num_minutes()
                - shift.break_minutes.min(
                    ((expected_end - expected_start).num_minutes()).max(0),
                );

            let period = periods.iter().find(|period| {
                period.employee_id == assignment.employee_id
                    && period.local_date().as_deref() == Some(date_str.as_str())
            });

            let actual_in_epoch = period.map(|period| period.clock_in_epoch);
            let actual_out_epoch = period.and_then(|period| period.clock_out_epoch);
            let worked = period
                .map(|period| period.worked_minutes(Utc::now().timestamp()).max(0))
                .unwrap_or(0);
            let late_minutes = actual_in_epoch
                .map(|epoch| (epoch - expected_start.timestamp()).max(0) / 60)
                .unwrap_or(0);
            let early_minutes = actual_out_epoch
                .map(|epoch| (expected_end.timestamp() - epoch).max(0) / 60)
                .unwrap_or(0);

            let status = if actual_in_epoch.is_none() {
                if date < today { "missed" } else { "scheduled" }
            } else if actual_out_epoch.is_none() {
                "in_progress"
            } else if late_minutes > 0 || early_minutes > 0 {
                "late"
            } else {
                "completed"
            };

            entries.push(ScheduleEntry {
                date: date_str.clone(),
                employee_id: assignment.employee_id,
                employee_name: assignment.employee_name.clone().unwrap_or_default(),
                shift_id: shift.id,
                shift_name: shift.name.clone(),
                expected_start: expected_start.to_rfc3339(),
                expected_end: expected_end.to_rfc3339(),
                planned_minutes,
                actual_clock_in: period.map(|period| period.clock_in.clone()),
                actual_clock_out: period.and_then(|period| period.clock_out.clone()),
                worked_minutes: worked,
                late_minutes,
                early_minutes,
                status: status.to_string(),
            });
        }
    }

    entries.sort_by(|a, b| {
        a.date
            .cmp(&b.date)
            .then_with(|| a.employee_name.cmp(&b.employee_name))
    });
    Ok(entries)
}

fn naive_at(date: &NaiveDate, time: &str, zone: FixedOffset) -> DateTime<FixedOffset> {
    let (hour, minute) = crate::db::parse_time(time).unwrap_or((0, 0));
    let naive = date.and_hms_opt(hour, minute, 0).unwrap_or_else(|| {
        Utc.timestamp_opt(0, 0).unwrap().naive_utc()
    });
    zone.from_local_datetime(&naive)
        .earliest()
        .unwrap_or_else(|| zone.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(id: i64, employee_id: i64, kind: EventType, epoch: i64) -> TimeEvent {
        TimeEvent {
            id,
            employee_id,
            employee_name: None,
            event_type: kind,
            occurred_at: DateTime::from_timestamp(epoch, 0)
                .unwrap()
                .to_rfc3339(),
            occurred_at_epoch: epoch,
            source: "scan".to_string(),
            note: None,
            created_at: String::new(),
        }
    }

    #[test]
    fn computes_worked_minutes_with_break() {
        let events = vec![
            event(1, 1, EventType::ClockIn, 0),
            event(2, 1, EventType::BreakStart, 3600),
            event(3, 1, EventType::BreakEnd, 3900),
            event(4, 1, EventType::ClockOut, 10800),
        ];
        let periods = compute_periods(&events);
        assert_eq!(periods.len(), 1);
        assert_eq!(periods[0].break_minutes, 5);
        assert_eq!(periods[0].worked_minutes(10800), 175);
    }

    #[test]
    fn keeps_employees_independent() {
        let events = vec![
            event(1, 1, EventType::ClockIn, 0),
            event(2, 2, EventType::ClockIn, 60),
            event(3, 1, EventType::ClockOut, 120),
            event(4, 2, EventType::ClockOut, 180),
        ];
        let periods = compute_periods(&events);
        assert_eq!(periods.len(), 2);
        assert!(periods.iter().all(|period| !period.is_open));
        assert!(periods.iter().any(|period| period.employee_id == 1 && period.clock_in_epoch == 0));
        assert!(periods.iter().any(|period| period.employee_id == 2 && period.clock_in_epoch == 60));
    }

    #[test]
    fn ignores_duplicate_and_orphan_events() {
        let events = vec![
            event(1, 1, EventType::ClockIn, 0),
            event(2, 1, EventType::ClockIn, 60),
            event(3, 1, EventType::BreakEnd, 120),
            event(4, 1, EventType::ClockOut, 180),
            event(5, 1, EventType::ClockOut, 240),
        ];
        let periods = compute_periods(&events);
        assert_eq!(periods.len(), 1);
        assert_eq!(periods[0].clock_in_epoch, 0);
        assert_eq!(periods[0].clock_out_epoch, Some(180));
        assert_eq!(periods[0].break_minutes, 0);
    }
}
