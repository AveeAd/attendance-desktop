export type EventType = "clock_in" | "clock_out" | "break_start" | "break_end";

export interface Employee {
  id: number;
  code: string;
  name: string;
  department: string | null;
  role: string | null;
  active: boolean;
  has_pin: boolean;
  qr_token: string;
  created_at: string;
}

export interface CreateEmployeeInput {
  code: string;
  name: string;
  department: string | null;
  role: string | null;
  pin: string | null;
}

export interface UpdateEmployeeInput {
  name: string;
  department: string | null;
  role: string | null;
  active: boolean;
  pin: string | null;
}

export interface MobileServerState {
  running: boolean;
  url: string | null;
  port: number | null;
}

export interface TimeEvent {
  id: number;
  employee_id: number;
  employee_name: string | null;
  event_type: EventType;
  occurred_at: string;
  occurred_at_epoch: number;
  source: string;
  note: string | null;
  created_at: string;
}

export interface ClockInput {
  identifier: string;
  event_type: EventType;
  occurred_at?: string | null;
  note?: string | null;
  source?: string | null;
}

export interface ClockResult {
  event: TimeEvent;
  employee: Employee;
  state: "in" | "out" | "on_break";
  message: string;
}

export interface EmployeeStatus {
  employee: Employee;
  state: "in" | "out" | "on_break";
  last_event_at: string | null;
  clock_in_at: string | null;
  open_minutes: number;
}

export interface Shift {
  id: number;
  name: string;
  start_time: string;
  end_time: string;
  days: number[];
  break_minutes: number;
  active: boolean;
}

export interface ShiftInput {
  name: string;
  start_time: string;
  end_time: string;
  days: number[];
  break_minutes: number;
  active: boolean;
}

export interface ShiftAssignment {
  id: number;
  employee_id: number;
  employee_name: string | null;
  shift_id: number;
  shift_name: string | null;
  effective_from: string;
  effective_to: string | null;
}

export interface AssignmentInput {
  employee_id: number;
  shift_id: number;
  effective_from: string;
  effective_to: string | null;
}

export interface ScheduleEntry {
  date: string;
  employee_id: number;
  employee_name: string;
  shift_id: number;
  shift_name: string;
  expected_start: string;
  expected_end: string;
  planned_minutes: number;
  actual_clock_in: string | null;
  actual_clock_out: string | null;
  worked_minutes: number;
  late_minutes: number;
  early_minutes: number;
  status: "scheduled" | "completed" | "late" | "missed" | "in_progress";
}

export interface TimesheetRow {
  employee_id: number;
  employee_name: string;
  date: string;
  clock_in: string | null;
  clock_out: string | null;
  break_minutes: number;
  worked_minutes: number;
  overtime_minutes: number;
  status: "complete" | "open";
}

export interface TimesheetReport {
  rows: TimesheetRow[];
  total_worked_minutes: number;
  total_overtime_minutes: number;
  employee_count: number;
}

export interface AppSettings {
  company_name: string;
  overtime_daily_hours: number;
  overtime_weekly_hours: number;
  idle_lock_minutes: number;
  pin_enabled: boolean;
}

export interface SettingsInput {
  company_name: string;
  overtime_daily_hours: number;
  overtime_weekly_hours: number;
  idle_lock_minutes: number;
}
