import { invoke } from "@tauri-apps/api/core";
import type {
  AppSettings,
  AssignmentInput,
  ClockInput,
  ClockResult,
  CreateEmployeeInput,
  DutyRosterEntry,
  DutyRosterGenerateResult,
  DutyRosterInput,
  Employee,
  EmployeeStatus,
  MobileServerState,
  ScheduleEntry,
  SettingsInput,
  Shift,
  ShiftAssignment,
  ShiftInput,
  TimeEvent,
  TimesheetReport,
  UpdateEmployeeInput,
  EventType,
} from "@/types";

export const api = {
  listEmployees: (activeOnly = false) =>
    invoke<Employee[]>("list_employees", { activeOnly }),
  getEmployee: (id: number) => invoke<Employee>("get_employee", { id }),
  createEmployee: (input: CreateEmployeeInput) =>
    invoke<Employee>("create_employee", { input }),
  updateEmployee: (id: number, input: UpdateEmployeeInput) =>
    invoke<Employee>("update_employee", { id, input }),
  setEmployeeActive: (id: number, active: boolean) =>
    invoke<void>("set_employee_active", { id, active }),

  clockEvent: (input: ClockInput) => invoke<ClockResult>("clock_event", { input }),
  listEvents: (params: {
    employeeId?: number | null;
    from?: string | null;
    to?: string | null;
    limit?: number;
  }) =>
    invoke<TimeEvent[]>("list_events", {
      employeeId: params.employeeId ?? null,
      from: params.from ?? null,
      to: params.to ?? null,
      limit: params.limit ?? 200,
    }),
  createManualEvent: (params: {
    employeeId: number;
    eventType: EventType;
    occurredAt: string;
    note?: string | null;
  }) =>
    invoke<TimeEvent>("create_manual_event", {
      employeeId: params.employeeId,
      eventType: params.eventType,
      occurredAt: params.occurredAt,
      note: params.note ?? null,
    }),
  deleteEvent: (id: number) => invoke<void>("delete_event", { id }),
  getStatuses: () => invoke<EmployeeStatus[]>("get_employee_statuses"),

  listShifts: (activeOnly = false) => invoke<Shift[]>("list_shifts", { activeOnly }),
  createShift: (input: ShiftInput) => invoke<Shift>("create_shift", { input }),
  updateShift: (id: number, input: ShiftInput) =>
    invoke<Shift>("update_shift", { id, input }),
  deleteShift: (id: number) => invoke<void>("delete_shift", { id }),
  listAssignments: (employeeId?: number | null) =>
    invoke<ShiftAssignment[]>("list_assignments", {
      employeeId: employeeId ?? null,
    }),
  createAssignment: (input: AssignmentInput) =>
    invoke<ShiftAssignment>("create_assignment", { input }),
  deleteAssignment: (id: number) => invoke<void>("delete_assignment", { id }),
  getSchedule: (weekStart: string) =>
    invoke<ScheduleEntry[]>("get_schedule", { weekStart }),

  listDutyRoster: (weekStart: string) =>
    invoke<DutyRosterEntry[]>("list_duty_roster", { weekStart }),
  createDutyRoster: (input: DutyRosterInput) =>
    invoke<DutyRosterEntry>("create_duty_roster", { input }),
  updateDutyRoster: (id: number, input: DutyRosterInput) =>
    invoke<DutyRosterEntry>("update_duty_roster", { id, input }),
  deleteDutyRoster: (id: number) => invoke<void>("delete_duty_roster", { id }),
  generateDutyRoster: (weekStart: string) =>
    invoke<DutyRosterGenerateResult>("generate_duty_roster", { weekStart }),
  exportDutyRosterCsv: (params: { path: string; from: string; to: string }) =>
    invoke<number>("export_duty_roster_csv", {
      path: params.path,
      from: params.from,
      to: params.to,
    }),

  getTimesheet: (params: {
    from: string;
    to: string;
    employeeId?: number | null;
  }) =>
    invoke<TimesheetReport>("get_timesheet", {
      from: params.from,
      to: params.to,
      employeeId: params.employeeId ?? null,
    }),
  exportCsv: (params: {
    path: string;
    from: string;
    to: string;
    employeeId?: number | null;
  }) =>
    invoke<number>("export_csv", {
      path: params.path,
      from: params.from,
      to: params.to,
      employeeId: params.employeeId ?? null,
    }),
  exportXlsx: (params: {
    path: string;
    from: string;
    to: string;
    employeeId?: number | null;
  }) =>
    invoke<number>("export_xlsx", {
      path: params.path,
      from: params.from,
      to: params.to,
      employeeId: params.employeeId ?? null,
    }),

  getSettings: () => invoke<AppSettings>("get_settings"),
  updateSettings: (input: SettingsInput) =>
    invoke<AppSettings>("update_settings", { input }),
  setAdminPin: (current: string | null, newPin: string) =>
    invoke<void>("set_admin_pin", { current, newPin }),
  verifyAdminPin: (pin: string) => invoke<boolean>("verify_admin_pin", { pin }),
  clearAdminPin: (current: string) => invoke<void>("clear_admin_pin", { current }),
  backupDatabase: (path: string) => invoke<void>("backup_database", { path }),

  startMobileServer: () =>
    invoke<MobileServerState>("start_mobile_server"),
  stopMobileServer: () => invoke<MobileServerState>("stop_mobile_server"),
  mobileServerStatus: () => invoke<MobileServerState>("mobile_server_status"),
};

export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "Something went wrong";
}
