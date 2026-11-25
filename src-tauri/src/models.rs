use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    ClockIn,
    ClockOut,
    BreakStart,
    BreakEnd,
}

impl EventType {
    pub fn as_str(self) -> &'static str {
        match self {
            EventType::ClockIn => "clock_in",
            EventType::ClockOut => "clock_out",
            EventType::BreakStart => "break_start",
            EventType::BreakEnd => "break_end",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "clock_in" => Some(EventType::ClockIn),
            "clock_out" => Some(EventType::ClockOut),
            "break_start" => Some(EventType::BreakStart),
            "break_end" => Some(EventType::BreakEnd),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Employee {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub department: Option<String>,
    pub role: Option<String>,
    pub active: bool,
    #[serde(skip)]
    pub pin_hash: String,
    pub has_pin: bool,
    pub qr_token: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateEmployee {
    pub code: String,
    pub name: String,
    pub department: Option<String>,
    pub role: Option<String>,
    pub pin: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateEmployee {
    pub name: String,
    pub department: Option<String>,
    pub role: Option<String>,
    pub active: bool,
    pub pin: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeEvent {
    pub id: i64,
    pub employee_id: i64,
    pub employee_name: Option<String>,
    pub event_type: EventType,
    pub occurred_at: String,
    pub occurred_at_epoch: i64,
    pub source: String,
    pub note: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClockInput {
    pub identifier: String,
    pub event_type: EventType,
    pub occurred_at: Option<String>,
    pub note: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClockResult {
    pub event: TimeEvent,
    pub employee: Employee,
    pub state: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmployeeStatus {
    pub employee: Employee,
    pub state: String,
    pub last_event_at: Option<String>,
    pub clock_in_at: Option<String>,
    pub open_minutes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shift {
    pub id: i64,
    pub name: String,
    pub start_time: String,
    pub end_time: String,
    pub days: Vec<i64>,
    pub break_minutes: i64,
    pub active: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShiftInput {
    pub name: String,
    pub start_time: String,
    pub end_time: String,
    pub days: Vec<i64>,
    pub break_minutes: i64,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShiftAssignment {
    pub id: i64,
    pub employee_id: i64,
    pub employee_name: Option<String>,
    pub shift_id: i64,
    pub shift_name: Option<String>,
    pub effective_from: String,
    pub effective_to: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssignmentInput {
    pub employee_id: i64,
    pub shift_id: i64,
    pub effective_from: String,
    pub effective_to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DutyRosterRecord {
    pub id: i64,
    pub employee_id: i64,
    pub employee_name: String,
    pub duty_date: String,
    pub shift_id: Option<i64>,
    pub shift_name: Option<String>,
    pub duty: String,
    pub notes: Option<String>,
    pub source: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DutyRosterEntry {
    pub id: i64,
    pub employee_id: i64,
    pub employee_name: String,
    pub duty_date: String,
    pub shift_id: Option<i64>,
    pub shift_name: Option<String>,
    pub duty: String,
    pub notes: Option<String>,
    pub source: String,
    pub created_at: String,
    pub planned_start: Option<String>,
    pub planned_end: Option<String>,
    pub planned_minutes: i64,
    pub actual_clock_in: Option<String>,
    pub actual_clock_out: Option<String>,
    pub worked_minutes: i64,
    pub late_minutes: i64,
    pub early_minutes: i64,
    pub state: String,
    pub attendance_status: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DutyRosterInput {
    pub employee_id: i64,
    pub duty_date: String,
    pub shift_id: Option<i64>,
    pub duty: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DutyRosterGenerateResult {
    pub created: i64,
    pub skipped: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleEntry {
    pub date: String,
    pub employee_id: i64,
    pub employee_name: String,
    pub shift_id: i64,
    pub shift_name: String,
    pub expected_start: String,
    pub expected_end: String,
    pub planned_minutes: i64,
    pub actual_clock_in: Option<String>,
    pub actual_clock_out: Option<String>,
    pub worked_minutes: i64,
    pub late_minutes: i64,
    pub early_minutes: i64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimesheetRow {
    pub employee_id: i64,
    pub employee_name: String,
    pub date: String,
    pub clock_in: Option<String>,
    pub clock_out: Option<String>,
    pub break_minutes: i64,
    pub worked_minutes: i64,
    pub overtime_minutes: i64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimesheetReport {
    pub rows: Vec<TimesheetRow>,
    pub total_worked_minutes: i64,
    pub total_overtime_minutes: i64,
    pub employee_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub company_name: String,
    pub overtime_daily_hours: f64,
    pub overtime_weekly_hours: f64,
    pub idle_lock_minutes: i64,
    pub pin_enabled: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            company_name: "My Organization".to_string(),
            overtime_daily_hours: 8.0,
            overtime_weekly_hours: 40.0,
            idle_lock_minutes: 15,
            pin_enabled: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SettingsInput {
    pub company_name: String,
    pub overtime_daily_hours: f64,
    pub overtime_weekly_hours: f64,
    pub idle_lock_minutes: i64,
}
