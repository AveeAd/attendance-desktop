import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { addDays, format } from "date-fns";
import {
  CalendarDays,
  ChevronLeft,
  ChevronRight,
  Clock3,
  Pencil,
  Plus,
  Trash2,
  UserPlus,
} from "lucide-react";
import { api, errorMessage } from "@/lib/ipc";
import { dateString, formatDuration, formatTime, mondayOf, weekLabel } from "@/lib/format";
import { Badge, Button, Card, EmptyState, Field, Input, Modal, Select, Spinner, cn } from "@/components/ui";
import { toast } from "@/store/toast";
import type { Shift, ShiftInput } from "@/types";

const dayNames = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const statusTone = {
  scheduled: "slate",
  completed: "green",
  late: "amber",
  missed: "red",
  in_progress: "blue",
} as const;

const emptyShift: ShiftInput = {
  name: "",
  start_time: "09:00",
  end_time: "17:00",
  days: [1, 2, 3, 4, 5],
  break_minutes: 30,
  active: true,
};

export function Schedule() {
  const queryClient = useQueryClient();
  const [weekStart, setWeekStart] = useState(() => mondayOf(new Date()));
  const [shiftForm, setShiftForm] = useState<ShiftInput | null>(null);
  const [editingShift, setEditingShift] = useState<Shift | null>(null);
  const [showShifts, setShowShifts] = useState(false);
  const [showAssign, setShowAssign] = useState(false);
  const [assignForm, setAssignForm] = useState({
    employee_id: 0,
    shift_id: 0,
    effective_from: dateString(),
    effective_to: "",
  });

  const scheduleQuery = useQuery({
    queryKey: ["schedule", dateString(weekStart)],
    queryFn: () => api.getSchedule(dateString(weekStart)),
  });
  const shiftsQuery = useQuery({ queryKey: ["shifts"], queryFn: () => api.listShifts(false) });
  const employeesQuery = useQuery({ queryKey: ["employees"], queryFn: () => api.listEmployees(true) });
  const assignmentsQuery = useQuery({ queryKey: ["assignments"], queryFn: () => api.listAssignments() });

  const saveShift = useMutation({
    mutationFn: () =>
      editingShift
        ? api.updateShift(editingShift.id, shiftForm!)
        : api.createShift(shiftForm!),
    onSuccess: () => {
      toast.success(editingShift ? "Shift updated" : "Shift created");
      setShiftForm(null);
      setEditingShift(null);
      void queryClient.invalidateQueries({ queryKey: ["shifts"] });
      void queryClient.invalidateQueries({ queryKey: ["schedule"] });
    },
    onError: (error) => toast.error(errorMessage(error)),
  });

  const deleteShift = useMutation({
    mutationFn: api.deleteShift,
    onSuccess: () => {
      toast.success("Shift deleted");
      void queryClient.invalidateQueries({ queryKey: ["shifts"] });
      void queryClient.invalidateQueries({ queryKey: ["schedule"] });
    },
    onError: (error) => toast.error(errorMessage(error)),
  });

  const saveAssignment = useMutation({
    mutationFn: api.createAssignment,
    onSuccess: () => {
      toast.success("Shift assigned");
      setShowAssign(false);
      void queryClient.invalidateQueries({ queryKey: ["assignments"] });
      void queryClient.invalidateQueries({ queryKey: ["schedule"] });
    },
    onError: (error) => toast.error(errorMessage(error)),
  });

  const deleteAssignment = useMutation({
    mutationFn: api.deleteAssignment,
    onSuccess: () => {
      toast.success("Assignment removed");
      void queryClient.invalidateQueries({ queryKey: ["assignments"] });
      void queryClient.invalidateQueries({ queryKey: ["schedule"] });
    },
    onError: (error) => toast.error(errorMessage(error)),
  });

  const entries = scheduleQuery.data ?? [];
  const dates = Array.from({ length: 7 }, (_, index) => addDays(weekStart, index));

  return (
    <div className="mx-auto max-w-[1300px] space-y-5">
      <div className="flex flex-wrap items-center justify-between gap-4">
        <div>
          <h2 className="text-xl font-semibold tracking-tight text-slate-900">Schedule</h2>
          <p className="mt-1 text-sm text-slate-500">
            Planned shifts compared with actual clock-in times.
          </p>
        </div>
        <div className="flex gap-2">
          <Button variant="secondary" onClick={() => setShowShifts(true)}>
            <Clock3 className="size-4" />
            Manage shifts
          </Button>
          <Button onClick={() => setShowAssign(true)}>
            <UserPlus className="size-4" />
            Assign shift
          </Button>
        </div>
      </div>

      <Card>
        <div className="flex items-center justify-between gap-3 border-b border-slate-100 px-5 py-3.5">
          <div className="flex items-center gap-2">
            <Button variant="ghost" size="sm" onClick={() => setWeekStart((value) => addDays(value, -7))}>
              <ChevronLeft className="size-4" />
            </Button>
            <Button variant="secondary" size="sm" onClick={() => setWeekStart(mondayOf(new Date()))}>
              Today
            </Button>
            <Button variant="ghost" size="sm" onClick={() => setWeekStart((value) => addDays(value, 7))}>
              <ChevronRight className="size-4" />
            </Button>
          </div>
          <span className="text-sm font-medium text-slate-700">{weekLabel(weekStart)}</span>
        </div>

        <div className="grid grid-cols-7 divide-x divide-slate-100 border-b border-slate-100">
          {dates.map((date) => (
            <div key={dateString(date)} className="px-3 py-2.5 text-center">
              <p className="text-[11px] uppercase tracking-wide text-slate-400">{format(date, "EEE")}</p>
              <p className={cn("text-sm font-semibold", dateString(date) === dateString() ? "text-indigo-600" : "text-slate-700")}>
                {format(date, "d")}
              </p>
            </div>
          ))}
        </div>

        {scheduleQuery.isLoading ? (
          <div className="flex justify-center py-16"><Spinner /></div>
        ) : entries.length === 0 ? (
          <EmptyState
            icon={<CalendarDays className="size-5" />}
            title="Nothing scheduled this week"
            description="Create a shift and assign it to an employee to see it here."
            action={<Button onClick={() => setShowAssign(true)}>Assign a shift</Button>}
          />
        ) : (
          <div className="divide-y divide-slate-100">
            {dates.map((date) => {
              const dayEntries = entries.filter((entry) => entry.date === dateString(date));
              return (
                <div key={dateString(date)} className="grid grid-cols-[100px_minmax(0,1fr)] gap-3 px-5 py-3">
                  <div className="pt-1 text-xs font-medium text-slate-400">{format(date, "EEE d")}</div>
                  <div className="space-y-2">
                    {dayEntries.length === 0 ? (
                      <p className="py-1 text-xs text-slate-300">—</p>
                    ) : (
                      dayEntries.map((entry, index) => (
                        <div
                          key={`${entry.date}-${entry.employee_id}-${entry.shift_id}-${index}`}
                          className="flex flex-wrap items-center gap-x-4 gap-y-1 rounded-lg border border-slate-100 bg-slate-50/60 px-3 py-2 text-sm"
                        >
                          <span className="font-medium text-slate-800">{entry.employee_name}</span>
                          <span className="text-xs text-slate-500">{entry.shift_name}</span>
                          <span className="text-xs tabular-nums text-slate-500">
                            {formatTime(entry.expected_start)}–{formatTime(entry.expected_end)}
                          </span>
                          <span className="ml-auto flex items-center gap-3 text-xs">
                            <span className="text-slate-500">
                              Worked {formatDuration(entry.worked_minutes)} / {formatDuration(entry.planned_minutes)}
                            </span>
                            {entry.late_minutes > 0 && <Badge tone="amber">+{entry.late_minutes}m late</Badge>}
                            {entry.early_minutes > 0 && <Badge tone="amber">−{entry.early_minutes}m early</Badge>}
                            <Badge tone={statusTone[entry.status]}>{entry.status.replace("_", " ")}</Badge>
                          </span>
                        </div>
                      ))
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </Card>

      <Card title="Current assignments" description="Active recurring shift assignments">
        {assignmentsQuery.data?.length ? (
          <div className="divide-y divide-slate-100">
            {assignmentsQuery.data.map((assignment) => (
              <div key={assignment.id} className="flex items-center gap-3 px-5 py-3 text-sm">
                <span className="font-medium text-slate-800">{assignment.employee_name}</span>
                <span className="text-slate-400">→</span>
                <span className="text-slate-600">{assignment.shift_name}</span>
                <span className="ml-auto text-xs text-slate-400">
                  From {assignment.effective_from}
                  {assignment.effective_to ? ` to ${assignment.effective_to}` : " onward"}
                </span>
                <Button variant="ghost" size="sm" onClick={() => deleteAssignment.mutate(assignment.id)} title="Remove assignment">
                  <Trash2 className="size-4 text-rose-500" />
                </Button>
              </div>
            ))}
          </div>
        ) : (
          <EmptyState icon={<UserPlus className="size-5" />} title="No assignments yet" description="Assign a shift to see expected hours." />
        )}
      </Card>

      <Modal
        open={showShifts}
        onClose={() => { setShowShifts(false); setShiftForm(null); setEditingShift(null); }}
        title="Manage shifts"
        description="Define recurring working hours."
        width="max-w-2xl"
        footer={
          shiftForm ? (
            <>
              <Button variant="secondary" onClick={() => { setShiftForm(null); setEditingShift(null); }}>Cancel</Button>
              <Button onClick={() => saveShift.mutate()} loading={saveShift.isPending} disabled={!shiftForm.name.trim()}>
                {editingShift ? "Save shift" : "Create shift"}
              </Button>
            </>
          ) : (
            <Button onClick={() => { setEditingShift(null); setShiftForm({ ...emptyShift }); }}>
              <Plus className="size-4" />
              New shift
            </Button>
          )
        }
      >
        {shiftForm ? (
          <div className="grid grid-cols-2 gap-4">
            <Field label="Shift name" className="col-span-2">
              <Input value={shiftForm.name} onChange={(event) => setShiftForm({ ...shiftForm, name: event.target.value })} placeholder="Morning shift" />
            </Field>
            <Field label="Start time">
              <Input type="time" value={shiftForm.start_time} onChange={(event) => setShiftForm({ ...shiftForm, start_time: event.target.value })} />
            </Field>
            <Field label="End time">
              <Input type="time" value={shiftForm.end_time} onChange={(event) => setShiftForm({ ...shiftForm, end_time: event.target.value })} />
            </Field>
            <Field label="Break (minutes)" className="col-span-2">
              <Input type="number" min={0} value={shiftForm.break_minutes} onChange={(event) => setShiftForm({ ...shiftForm, break_minutes: Number(event.target.value) })} />
            </Field>
            <Field label="Days of week" className="col-span-2">
              <div className="flex flex-wrap gap-1.5">
                {dayNames.map((name, index) => {
                  const day = index + 1;
                  const selected = shiftForm.days.includes(day);
                  return (
                    <button
                      key={name}
                      type="button"
                      onClick={() =>
                        setShiftForm({
                          ...shiftForm,
                          days: selected ? shiftForm.days.filter((value) => value !== day) : [...shiftForm.days, day].sort(),
                        })
                      }
                      className={cn(
                        "rounded-lg border px-3 py-1.5 text-xs font-medium transition-colors",
                        selected ? "border-indigo-200 bg-indigo-50 text-indigo-700" : "border-slate-200 text-slate-500 hover:bg-slate-50",
                      )}
                    >
                      {name}
                    </button>
                  );
                })}
              </div>
            </Field>
          </div>
        ) : (
          <div className="space-y-2">
            {shiftsQuery.data?.length ? shiftsQuery.data.map((shift) => (
              <div key={shift.id} className="flex items-center gap-3 rounded-lg border border-slate-100 px-3 py-2.5 text-sm">
                <div className="min-w-0 flex-1">
                  <p className="font-medium text-slate-800">{shift.name}</p>
                  <p className="text-[11px] text-slate-400">
                    {shift.start_time}–{shift.end_time} · {shift.days.map((day) => dayNames[day - 1]).join(", ")} · {shift.break_minutes}m break
                  </p>
                </div>
                <Badge tone={shift.active ? "green" : "slate"}>{shift.active ? "Active" : "Inactive"}</Badge>
                <Button variant="ghost" size="sm" onClick={() => { setEditingShift(shift); setShiftForm({ ...shift }); }}>
                  <Pencil className="size-4" />
                </Button>
                <Button variant="ghost" size="sm" onClick={() => deleteShift.mutate(shift.id)}>
                  <Trash2 className="size-4 text-rose-500" />
                </Button>
              </div>
            )) : (
              <EmptyState icon={<Clock3 className="size-5" />} title="No shifts defined" description="Create a shift to start building a schedule." />
            )}
          </div>
        )}
      </Modal>

      <Modal
        open={showAssign}
        onClose={() => setShowAssign(false)}
        title="Assign a shift"
        description="Recurring assignment for one employee."
        footer={
          <>
            <Button variant="secondary" onClick={() => setShowAssign(false)}>Cancel</Button>
            <Button
              onClick={() => saveAssignment.mutate(assignForm)}
              loading={saveAssignment.isPending}
              disabled={!assignForm.employee_id || !assignForm.shift_id || !assignForm.effective_from}
            >
              Assign
            </Button>
          </>
        }
      >
        <div className="space-y-4">
          <Field label="Employee">
            <Select value={assignForm.employee_id || ""} onChange={(event) => setAssignForm({ ...assignForm, employee_id: Number(event.target.value) })}>
              <option value="">Select employee</option>
              {employeesQuery.data?.map((employee) => (
                <option key={employee.id} value={employee.id}>{employee.name}</option>
              ))}
            </Select>
          </Field>
          <Field label="Shift">
            <Select value={assignForm.shift_id || ""} onChange={(event) => setAssignForm({ ...assignForm, shift_id: Number(event.target.value) })}>
              <option value="">Select shift</option>
              {shiftsQuery.data?.filter((shift) => shift.active).map((shift) => (
                <option key={shift.id} value={shift.id}>{shift.name} ({shift.start_time}–{shift.end_time})</option>
              ))}
            </Select>
          </Field>
          <div className="grid grid-cols-2 gap-4">
            <Field label="Effective from">
              <Input type="date" value={assignForm.effective_from} onChange={(event) => setAssignForm({ ...assignForm, effective_from: event.target.value })} />
            </Field>
            <Field label="Effective to (optional)">
              <Input type="date" value={assignForm.effective_to} onChange={(event) => setAssignForm({ ...assignForm, effective_to: event.target.value })} />
            </Field>
          </div>
        </div>
      </Modal>
    </div>
  );
}
