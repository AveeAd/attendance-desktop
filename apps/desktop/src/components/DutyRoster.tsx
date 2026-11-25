import { useEffect, useState, type ReactNode } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { save } from "@tauri-apps/plugin-dialog";
import { addDays, format, parseISO } from "date-fns";
import {
  CalendarCheck2,
  ChevronLeft,
  ChevronRight,
  ClipboardList,
  Copy,
  Download,
  KeyRound,
  LockKeyhole,
  Pencil,
  Plus,
  Printer,
  Sparkles,
  Trash2,
} from "lucide-react";
import { api, errorMessage } from "@/lib/ipc";
import {
  dateString,
  formatDuration,
  formatTime,
  initials,
  mondayOf,
  weekDates,
  weekLabel,
} from "@/lib/format";
import { useAuthStore } from "@/store/auth";
import { toast } from "@/store/toast";
import {
  Badge,
  Button,
  Card,
  EmptyState,
  Field,
  Input,
  Modal,
  Select,
  Spinner,
  Textarea,
  cn,
} from "@/components/ui";
import type { DutyRosterEntry, DutyRosterInput } from "@/types";

const attendanceTone = {
  scheduled: "slate",
  completed: "green",
  late: "amber",
  missed: "red",
  in_progress: "blue",
} as const;

const stateTone = {
  in: "green",
  on_break: "amber",
  out: "slate",
} as const;

function entryKey(entry: DutyRosterEntry): string {
  return `${entry.duty_date}:${entry.employee_id}:${entry.shift_id ?? "none"}`;
}

function RosterAdminGate({ children }: { children: ReactNode }) {
  const settingsQuery = useQuery({ queryKey: ["settings"], queryFn: api.getSettings });
  const unlocked = useAuthStore((state) => state.unlocked);
  const unlock = useAuthStore((state) => state.unlock);
  const [pin, setPin] = useState("");
  const [error, setError] = useState("");
  const [checking, setChecking] = useState(false);

  useEffect(() => {
    if (settingsQuery.data && !settingsQuery.data.pin_enabled) unlock();
  }, [settingsQuery.data, unlock]);

  if (settingsQuery.isLoading) {
    return (
      <div className="flex justify-center py-16">
        <Spinner />
      </div>
    );
  }

  if (!settingsQuery.data?.pin_enabled || unlocked) {
    return <>{children}</>;
  }

  const submit = async () => {
    setError("");
    setChecking(true);
    try {
      if (await api.verifyAdminPin(pin)) {
        unlock();
        setPin("");
      } else {
        setError("Incorrect PIN. Try again.");
        setPin("");
      }
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setChecking(false);
    }
  };

  return (
    <form
      className="mx-auto max-w-sm py-10 text-center"
      onSubmit={(event) => {
        event.preventDefault();
        void submit();
      }}
    >
      <div className="mx-auto mb-4 flex size-12 items-center justify-center rounded-2xl bg-indigo-50 text-indigo-600">
        <LockKeyhole className="size-5" />
      </div>
      <h3 className="text-base font-semibold text-slate-900">Manage duties? Enter admin PIN</h3>
      <p className="mt-1 text-xs text-slate-500">
        The roster itself stays visible on this dashboard.
      </p>
      <div className="relative mt-5">
        <KeyRound className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-slate-400" />
        <Input
          autoFocus
          type="password"
          inputMode="numeric"
          autoComplete="off"
          placeholder="Enter PIN"
          value={pin}
          onChange={(event) => setPin(event.target.value.replace(/\D/g, ""))}
          className="pl-9 text-center text-lg tracking-[0.3em]"
        />
      </div>
      {error && <p className="mt-2 text-xs text-rose-600">{error}</p>}
      <Button type="submit" className="mt-4 w-full" loading={checking} disabled={!pin}>
        Unlock
      </Button>
    </form>
  );
}

export function DutyRosterWidget() {
  const [selectedDate, setSelectedDate] = useState(() => new Date());
  const [showManager, setShowManager] = useState(false);
  const [exporting, setExporting] = useState(false);

  const weekStart = mondayOf(selectedDate);
  const from = dateString(weekStart);
  const to = dateString(addDays(weekStart, 6));
  const selected = dateString(selectedDate);

  const rosterQuery = useQuery({
    queryKey: ["dutyRoster", from],
    queryFn: () => api.listDutyRoster(from),
    refetchInterval: 30_000,
  });

  const allEntries = rosterQuery.data ?? [];
  const entries = allEntries.filter((entry) => entry.duty_date === selected);

  const exportCsv = async () => {
    setExporting(true);
    try {
      const path = await save({
        defaultPath: `duty-roster-${from}-to-${to}.csv`,
        filters: [{ name: "CSV file", extensions: ["csv"] }],
      });
      if (!path) return;
      const count = await api.exportDutyRosterCsv({ path, from, to });
      toast.success(`Exported ${count} duty assignments`);
    } catch (caught) {
      toast.error(errorMessage(caught));
    } finally {
      setExporting(false);
    }
  };

  return (
    <Card
      title={`${format(selectedDate, "EEE, MMM d")} · duty roster`}
      description="Who is assigned where today"
      actions={
        <div className="flex items-center gap-1.5">
          <Button variant="ghost" size="sm" onClick={() => setSelectedDate((value) => addDays(value, -1))} title="Previous day">
            <ChevronLeft className="size-4" />
          </Button>
          <Button variant="secondary" size="sm" onClick={() => setSelectedDate(new Date())}>
            Today
          </Button>
          <Button variant="ghost" size="sm" onClick={() => setSelectedDate((value) => addDays(value, 1))} title="Next day">
            <ChevronRight className="size-4" />
          </Button>
          <Button variant="secondary" size="sm" onClick={() => setShowManager(true)}>
            <ClipboardList className="size-4" />
            Manage
          </Button>
        </div>
      }
    >
      {rosterQuery.isLoading ? (
        <div className="flex justify-center py-12">
          <Spinner />
        </div>
      ) : entries.length === 0 ? (
        <EmptyState
          icon={<CalendarCheck2 className="size-5" />}
          title="No duties assigned"
          description="Open the roster manager to generate this week from the schedule or add a duty."
          action={
            <Button size="sm" onClick={() => setShowManager(true)}>
              Open roster manager
            </Button>
          }
        />
      ) : (
        <div className="divide-y divide-slate-100">
          {entries.map((entry) => (
            <div key={entry.id} className="flex items-center gap-3 px-5 py-3">
              <div className="flex size-9 shrink-0 items-center justify-center rounded-full bg-indigo-50 text-xs font-semibold text-indigo-700">
                {initials(entry.employee_name)}
              </div>
              <div className="min-w-0 flex-1">
                <p className="truncate text-sm font-medium text-slate-800">
                  {entry.employee_name}
                  <span className="font-normal text-slate-400"> · {entry.duty}</span>
                </p>
                <p className="truncate text-[11px] text-slate-400">
                  {entry.shift_name ?? "No shift"}
                  {entry.planned_start
                    ? ` · ${formatTime(entry.planned_start)}–${formatTime(entry.planned_end)}`
                    : " · No fixed time"}
                  {entry.actual_clock_in ? ` · In ${formatTime(entry.actual_clock_in)}` : ""}
                </p>
              </div>
              <span className="hidden text-xs tabular-nums text-slate-500 sm:block">
                {formatDuration(entry.worked_minutes)} / {formatDuration(entry.planned_minutes)}
              </span>
              <Badge tone={stateTone[entry.state]}>{entry.state.replace("_", " ")}</Badge>
              <Badge tone={attendanceTone[entry.attendance_status]}>
                {entry.attendance_status.replace("_", " ")}
              </Badge>
            </div>
          ))}
        </div>
      )}

      <div className="flex flex-wrap items-center justify-between gap-2 border-t border-slate-100 px-5 py-3">
        <p className="text-[11px] text-slate-400">
          {allEntries.length} assignment{allEntries.length === 1 ? "" : "s"} this week · {weekLabel(weekStart)}
        </p>
        <div className="flex gap-2">
          <Button variant="secondary" size="sm" onClick={() => window.print()}>
            <Printer className="size-4" />
            Print
          </Button>
          <Button variant="secondary" size="sm" onClick={() => void exportCsv()} loading={exporting}>
            <Download className="size-4" />
            CSV
          </Button>
        </div>
      </div>

      <div className="print-area hidden">
        <h1 className="mb-1 text-lg font-semibold">Duty roster</h1>
        <p className="mb-4 text-xs text-slate-500">{weekLabel(weekStart)}</p>
        <table className="w-full border-collapse text-left text-xs">
          <thead>
            <tr>
              <th className="border-b border-slate-300 px-2 py-2">Date</th>
              <th className="border-b border-slate-300 px-2 py-2">Employee</th>
              <th className="border-b border-slate-300 px-2 py-2">Duty</th>
              <th className="border-b border-slate-300 px-2 py-2">Shift</th>
              <th className="border-b border-slate-300 px-2 py-2">Time</th>
              <th className="border-b border-slate-300 px-2 py-2">Status</th>
              <th className="border-b border-slate-300 px-2 py-2">Notes</th>
            </tr>
          </thead>
          <tbody>
            {allEntries.map((entry) => (
              <tr key={entry.id}>
                <td className="border-b border-slate-200 px-2 py-2">{entry.duty_date}</td>
                <td className="border-b border-slate-200 px-2 py-2">{entry.employee_name}</td>
                <td className="border-b border-slate-200 px-2 py-2">{entry.duty}</td>
                <td className="border-b border-slate-200 px-2 py-2">{entry.shift_name ?? "—"}</td>
                <td className="border-b border-slate-200 px-2 py-2">
                  {entry.planned_start ? `${formatTime(entry.planned_start)}–${formatTime(entry.planned_end)}` : "—"}
                </td>
                <td className="border-b border-slate-200 px-2 py-2">{entry.attendance_status.replace("_", " ")}</td>
                <td className="border-b border-slate-200 px-2 py-2">{entry.notes ?? ""}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <DutyRosterManager
        open={showManager}
        onClose={() => setShowManager(false)}
        weekStart={weekStart}
        onWeekStartChange={setSelectedDate}
      />
    </Card>
  );
}

export function DutyRosterManager({
  open,
  onClose,
  weekStart,
  onWeekStartChange,
}: {
  open: boolean;
  onClose: () => void;
  weekStart: Date;
  onWeekStartChange: (date: Date) => void;
}) {
  const queryClient = useQueryClient();
  const [form, setForm] = useState<DutyRosterInput | null>(null);
  const [editingId, setEditingId] = useState<number | null>(null);

  const from = dateString(weekStart);
  const dates = weekDates(weekStart);

  const rosterQuery = useQuery({
    queryKey: ["dutyRoster", from],
    queryFn: () => api.listDutyRoster(from),
    enabled: open,
  });
  const shiftsQuery = useQuery({
    queryKey: ["shifts"],
    queryFn: () => api.listShifts(false),
    enabled: open,
  });
  const employeesQuery = useQuery({
    queryKey: ["employees"],
    queryFn: () => api.listEmployees(true),
    enabled: open,
  });

  const entries = rosterQuery.data ?? [];
  const dutyOptions = Array.from(
    new Set([
      ...entries.map((entry) => entry.duty),
      ...(shiftsQuery.data ?? []).map((shift) => shift.name),
    ]),
  ).sort((a, b) => a.localeCompare(b));

  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: ["dutyRoster"] });
  };

  const saveMutation = useMutation({
    mutationFn: () =>
      editingId
        ? api.updateDutyRoster(editingId, form!)
        : api.createDutyRoster(form!),
    onSuccess: () => {
      toast.success(editingId ? "Duty updated" : "Duty added");
      setForm(null);
      setEditingId(null);
      invalidate();
    },
    onError: (error) => toast.error(errorMessage(error)),
  });

  const deleteMutation = useMutation({
    mutationFn: api.deleteDutyRoster,
    onSuccess: () => {
      toast.success("Duty removed");
      invalidate();
    },
    onError: (error) => toast.error(errorMessage(error)),
  });

  const generateMutation = useMutation({
    mutationFn: () => api.generateDutyRoster(from),
    onSuccess: (result) => {
      toast.success(`Generated ${result.created} duties · ${result.skipped} already existed`);
      invalidate();
    },
    onError: (error) => toast.error(errorMessage(error)),
  });

  const copyMutation = useMutation({
    mutationFn: async () => {
      const existing = new Set(entries.map(entryKey));
      const previous = await api.listDutyRoster(dateString(addDays(weekStart, -7)));
      let created = 0;
      for (const entry of previous) {
        const shiftedDate = dateString(addDays(parseISO(entry.duty_date), 7));
        if (existing.has(`${shiftedDate}:${entry.employee_id}:${entry.shift_id ?? "none"}`)) {
          continue;
        }
        await api.createDutyRoster({
          employee_id: entry.employee_id,
          duty_date: shiftedDate,
          shift_id: entry.shift_id,
          duty: entry.duty,
          notes: entry.notes,
        });
        created += 1;
      }
      return created;
    },
    onSuccess: (count) => {
      toast.success(count > 0 ? `Copied ${count} duties from last week` : "Nothing new to copy");
      invalidate();
    },
    onError: (error) => toast.error(errorMessage(error)),
  });

  const openForm = (entry?: DutyRosterEntry) => {
    setEditingId(entry?.id ?? null);
    setForm(
      entry
        ? {
            employee_id: entry.employee_id,
            duty_date: entry.duty_date,
            shift_id: entry.shift_id,
            duty: entry.duty,
            notes: entry.notes,
          }
        : {
            employee_id: 0,
            duty_date: from,
            shift_id: null,
            duty: "",
            notes: null,
          },
    );
  };

  const close = () => {
    setForm(null);
    setEditingId(null);
    onClose();
  };

  return (
    <Modal
      open={open}
      onClose={close}
      title="Manage duty roster"
      description="Daily assignments, independent of the recurring schedule."
      width="max-w-5xl"
      footer={
        form ? (
          <>
            <Button
              variant="secondary"
              onClick={() => {
                setForm(null);
                setEditingId(null);
              }}
            >
              Cancel
            </Button>
            <Button
              onClick={() => saveMutation.mutate()}
              loading={saveMutation.isPending}
              disabled={!form.employee_id || !form.duty.trim() || !form.duty_date}
            >
              {editingId ? "Save duty" : "Add duty"}
            </Button>
          </>
        ) : (
          <Button onClick={close}>Done</Button>
        )
      }
    >
      <RosterAdminGate>
        {form ? (
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <Field label="Employee">
              <Select
                value={form.employee_id || ""}
                onChange={(event) => setForm({ ...form, employee_id: Number(event.target.value) })}
              >
                <option value="">Select employee</option>
                {employeesQuery.data?.map((employee) => (
                  <option key={employee.id} value={employee.id}>
                    {employee.name}
                  </option>
                ))}
              </Select>
            </Field>
            <Field label="Date">
              <Input
                type="date"
                value={form.duty_date}
                onChange={(event) => setForm({ ...form, duty_date: event.target.value })}
              />
            </Field>
            <Field label="Shift (optional)" hint="Leaving this blank removes planned time.">
              <Select
                value={form.shift_id ?? ""}
                onChange={(event) =>
                  setForm({ ...form, shift_id: event.target.value ? Number(event.target.value) : null })
                }
              >
                <option value="">No fixed shift</option>
                {shiftsQuery.data?.map((shift) => (
                  <option key={shift.id} value={shift.id}>
                    {shift.name} ({shift.start_time}–{shift.end_time})
                  </option>
                ))}
              </Select>
            </Field>
            <Field label="Duty / post">
              <Input
                list="duty-options"
                value={form.duty}
                onChange={(event) => setForm({ ...form, duty: event.target.value })}
                placeholder="Front desk, patrol, training…"
              />
              <datalist id="duty-options">
                {dutyOptions.map((duty) => (
                  <option key={duty} value={duty} />
                ))}
              </datalist>
            </Field>
            <Field label="Notes (optional)" className="sm:col-span-2">
              <Textarea
                value={form.notes ?? ""}
                onChange={(event) => setForm({ ...form, notes: event.target.value || null })}
                placeholder="Handover, backup, or anything the team should know"
              />
            </Field>
          </div>
        ) : (
          <div className="space-y-4">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <div className="flex items-center gap-1.5">
                <Button variant="ghost" size="sm" onClick={() => onWeekStartChange(addDays(weekStart, -7))}>
                  <ChevronLeft className="size-4" />
                </Button>
                <Button variant="secondary" size="sm" onClick={() => onWeekStartChange(mondayOf(new Date()))}>
                  This week
                </Button>
                <Button variant="ghost" size="sm" onClick={() => onWeekStartChange(addDays(weekStart, 7))}>
                  <ChevronRight className="size-4" />
                </Button>
                <span className="ml-2 text-sm font-medium text-slate-700">{weekLabel(weekStart)}</span>
              </div>
              <div className="flex flex-wrap gap-2">
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={() => generateMutation.mutate()}
                  loading={generateMutation.isPending}
                >
                  <Sparkles className="size-4" />
                  Generate from schedule
                </Button>
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={() => copyMutation.mutate()}
                  loading={copyMutation.isPending}
                >
                  <Copy className="size-4" />
                  Copy last week
                </Button>
                <Button size="sm" onClick={() => openForm()}>
                  <Plus className="size-4" />
                  Add duty
                </Button>
              </div>
            </div>

            {rosterQuery.isLoading ? (
              <div className="flex justify-center py-16">
                <Spinner />
              </div>
            ) : (
              <div className="overflow-hidden rounded-xl border border-slate-200">
                <div className="grid grid-cols-7 divide-x divide-slate-100 border-b border-slate-100 bg-slate-50/70">
                  {dates.map((date) => (
                    <div key={dateString(date)} className="px-2 py-2 text-center">
                      <p className="text-[11px] uppercase tracking-wide text-slate-400">{format(date, "EEE")}</p>
                      <p
                        className={cn(
                          "text-sm font-semibold",
                          dateString(date) === dateString() ? "text-indigo-600" : "text-slate-700",
                        )}
                      >
                        {format(date, "d")}
                      </p>
                      <p className="text-[10px] text-slate-400">
                        {entries.filter((entry) => entry.duty_date === dateString(date)).length} duties
                      </p>
                    </div>
                  ))}
                </div>

                {dates.map((date) => {
                  const dayEntries = entries.filter((entry) => entry.duty_date === dateString(date));
                  return (
                    <div
                      key={dateString(date)}
                      className="grid grid-cols-[92px_minmax(0,1fr)] gap-3 border-t border-slate-100 px-4 py-3"
                    >
                      <div className="pt-1 text-xs font-medium text-slate-400">{format(date, "EEE d")}</div>
                      <div className="space-y-2">
                        {dayEntries.length === 0 ? (
                          <p className="py-1 text-xs text-slate-300">—</p>
                        ) : (
                          dayEntries.map((entry) => (
                            <div
                              key={entry.id}
                              className="flex flex-wrap items-center gap-x-3 gap-y-1 rounded-lg border border-slate-100 bg-slate-50/60 px-3 py-2 text-sm"
                            >
                              <span className="font-medium text-slate-800">{entry.employee_name}</span>
                              <Badge tone="blue">{entry.duty}</Badge>
                              <span className="text-xs text-slate-500">
                                {entry.planned_start
                                  ? `${formatTime(entry.planned_start)}–${formatTime(entry.planned_end)}`
                                  : "No fixed time"}
                              </span>
                              <span className="text-xs tabular-nums text-slate-500">
                                {formatDuration(entry.worked_minutes)} / {formatDuration(entry.planned_minutes)}
                              </span>
                              <Badge tone={attendanceTone[entry.attendance_status]}>
                                {entry.attendance_status.replace("_", " ")}
                              </Badge>
                              {entry.source === "generated" && <Badge tone="slate">generated</Badge>}
                              <div className="ml-auto flex items-center gap-1">
                                <Button variant="ghost" size="sm" onClick={() => openForm(entry)} title="Edit duty">
                                  <Pencil className="size-3.5" />
                                </Button>
                                <Button
                                  variant="ghost"
                                  size="sm"
                                  onClick={() => deleteMutation.mutate(entry.id)}
                                  title="Remove duty"
                                >
                                  <Trash2 className="size-3.5 text-rose-500" />
                                </Button>
                              </div>
                            </div>
                          ))
                        )}
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        )}
      </RosterAdminGate>
    </Modal>
  );
}
