import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { save } from "@tauri-apps/plugin-dialog";
import { Bar, BarChart, CartesianGrid, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";
import { Clock3, FileSpreadsheet, FileText, Plus, TrendingUp, Users } from "lucide-react";
import { api, errorMessage } from "@/lib/ipc";
import { dateString, formatDuration, formatHours, formatTime, localIso } from "@/lib/format";
import { Badge, Button, Card, EmptyState, Field, Input, Modal, Select, Spinner } from "@/components/ui";
import { toast } from "@/store/toast";
import type { EventType } from "@/types";

const eventTypes: Array<{ value: EventType; label: string }> = [
  { value: "clock_in", label: "Clock in" },
  { value: "clock_out", label: "Clock out" },
  { value: "break_start", label: "Break start" },
  { value: "break_end", label: "Break end" },
];

export function Reports() {
  const queryClient = useQueryClient();
  const [from, setFrom] = useState(() => {
    const date = new Date();
    date.setDate(1);
    return dateString(date);
  });
  const [to, setTo] = useState(() => dateString());
  const [employeeId, setEmployeeId] = useState<number | null>(null);
  const [showManual, setShowManual] = useState(false);
  const [manual, setManual] = useState({
    employee_id: 0,
    event_type: "clock_in" as EventType,
    occurred_at: `${dateString()}T09:00`,
    note: "",
  });
  const [exporting, setExporting] = useState<"csv" | "xlsx" | null>(null);

  const employeesQuery = useQuery({ queryKey: ["employees"], queryFn: () => api.listEmployees(false) });
  const reportQuery = useQuery({
    queryKey: ["timesheet", from, to, employeeId],
    queryFn: () => api.getTimesheet({ from, to, employeeId }),
  });

  const manualMutation = useMutation({
    mutationFn: () =>
      api.createManualEvent({
        employeeId: manual.employee_id,
        eventType: manual.event_type,
        occurredAt: localIso(new Date(manual.occurred_at)),
        note: manual.note || null,
      }),
    onSuccess: () => {
      toast.success("Manual entry added");
      setShowManual(false);
      void queryClient.invalidateQueries({ queryKey: ["timesheet"] });
      void queryClient.invalidateQueries({ queryKey: ["events"] });
      void queryClient.invalidateQueries({ queryKey: ["statuses"] });
    },
    onError: (error) => toast.error(errorMessage(error)),
  });

  const report = reportQuery.data;
  const chartData = useMemo(() => {
    const totals = new Map<string, number>();
    report?.rows.forEach((row) => {
      totals.set(row.employee_name, (totals.get(row.employee_name) ?? 0) + row.worked_minutes / 60);
    });
    return Array.from(totals, ([name, hours]) => ({
      name: name.split(" ")[0],
      hours: Number(hours.toFixed(2)),
    }));
  }, [report]);

  const exportReport = async (format: "csv" | "xlsx") => {
    setExporting(format);
    try {
      const path = await save({
        defaultPath: `attendance-${from}-to-${to}.${format}`,
        filters: [
          {
            name: format === "csv" ? "CSV file" : "Excel workbook",
            extensions: [format],
          },
        ],
      });
      if (!path) return;
      const count =
        format === "csv"
          ? await api.exportCsv({ path, from, to, employeeId })
          : await api.exportXlsx({ path, from, to, employeeId });
      toast.success(`Exported ${count} rows`);
    } catch (caught) {
      toast.error(errorMessage(caught));
    } finally {
      setExporting(null);
    }
  };

  return (
    <div className="mx-auto max-w-[1300px] space-y-5">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h2 className="text-xl font-semibold tracking-tight text-slate-900">Timesheet reports</h2>
          <p className="mt-1 text-sm text-slate-500">Daily worked hours, breaks and overtime.</p>
        </div>
        <div className="flex flex-wrap items-end gap-2">
          <Field label="From" className="w-36">
            <Input type="date" value={from} onChange={(event) => setFrom(event.target.value)} />
          </Field>
          <Field label="To" className="w-36">
            <Input type="date" value={to} onChange={(event) => setTo(event.target.value)} />
          </Field>
          <Field label="Employee" className="w-48">
            <Select value={employeeId ?? ""} onChange={(event) => setEmployeeId(event.target.value ? Number(event.target.value) : null)}>
              <option value="">All employees</option>
              {employeesQuery.data?.map((employee) => (
                <option key={employee.id} value={employee.id}>{employee.name}</option>
              ))}
            </Select>
          </Field>
          <Button variant="secondary" onClick={() => setShowManual(true)} title="Add a manual time entry">
            <Plus className="size-4" />
            Manual entry
          </Button>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
        {[
          { label: "Total hours", value: formatHours(report?.total_worked_minutes ?? 0), icon: Clock3, tone: "text-indigo-600 bg-indigo-50" },
          { label: "Overtime", value: formatHours(report?.total_overtime_minutes ?? 0), icon: TrendingUp, tone: "text-amber-600 bg-amber-50" },
          { label: "Employees", value: String(report?.employee_count ?? 0), icon: Users, tone: "text-emerald-600 bg-emerald-50" },
          { label: "Records", value: String(report?.rows.length ?? 0), icon: FileText, tone: "text-slate-600 bg-slate-100" },
        ].map((stat) => (
          <Card key={stat.label} className="flex items-center gap-3 px-5 py-4">
            <div className={`flex size-10 items-center justify-center rounded-xl ${stat.tone}`}>
              <stat.icon className="size-5" />
            </div>
            <div>
              <p className="text-xs text-slate-500">{stat.label}</p>
              <p className="text-xl font-semibold tabular-nums text-slate-900">{stat.value}</p>
            </div>
          </Card>
        ))}
      </div>

      {chartData.length > 0 && (
        <Card title="Hours by employee" description="Total worked hours in the selected range" className="h-[280px]">
          <ResponsiveContainer width="100%" height="100%" className="!h-[220px]">
            <BarChart data={chartData} margin={{ top: 16, right: 16, bottom: 4, left: -12 }}>
              <CartesianGrid strokeDasharray="3 3" vertical={false} stroke="#e2e8f0" />
              <XAxis dataKey="name" axisLine={false} tickLine={false} tick={{ fontSize: 11, fill: "#64748b" }} />
              <YAxis axisLine={false} tickLine={false} tick={{ fontSize: 11, fill: "#64748b" }} />
              <Tooltip cursor={{ fill: "#f1f5f9" }} contentStyle={{ borderRadius: 10, border: "1px solid #e2e8f0", fontSize: 12 }} />
              <Bar dataKey="hours" fill="#6366f1" radius={[6, 6, 0, 0]} maxBarSize={48} />
            </BarChart>
          </ResponsiveContainer>
        </Card>
      )}

      <Card
        title="Daily records"
        description={`${report?.rows.length ?? 0} entries in range`}
        actions={
          <div className="flex gap-2">
            <Button variant="secondary" size="sm" onClick={() => void exportReport("csv")} loading={exporting === "csv"}>
              <FileText className="size-4" />
              CSV
            </Button>
            <Button variant="secondary" size="sm" onClick={() => void exportReport("xlsx")} loading={exporting === "xlsx"}>
              <FileSpreadsheet className="size-4" />
              Excel
            </Button>
          </div>
        }
      >
        {reportQuery.isLoading ? (
          <div className="flex justify-center py-16"><Spinner /></div>
        ) : report?.rows.length ? (
          <div className="overflow-x-auto">
            <table className="w-full text-left text-sm">
              <thead>
                <tr className="border-b border-slate-100 text-[11px] uppercase tracking-wide text-slate-400">
                  <th className="px-5 py-3 font-medium">Date</th>
                  <th className="px-4 py-3 font-medium">Employee</th>
                  <th className="px-4 py-3 font-medium">Clock in</th>
                  <th className="px-4 py-3 font-medium">Clock out</th>
                  <th className="px-4 py-3 font-medium">Break</th>
                  <th className="px-4 py-3 font-medium">Worked</th>
                  <th className="px-4 py-3 font-medium">Overtime</th>
                  <th className="px-5 py-3 font-medium">Status</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-100">
                {report.rows.map((row, index) => (
                  <tr key={`${row.employee_id}-${row.date}-${index}`} className="hover:bg-slate-50/60">
                    <td className="px-5 py-3 text-slate-600">{row.date}</td>
                    <td className="px-4 py-3 font-medium text-slate-800">{row.employee_name}</td>
                    <td className="px-4 py-3 tabular-nums text-slate-600">{formatTime(row.clock_in)}</td>
                    <td className="px-4 py-3 tabular-nums text-slate-600">{formatTime(row.clock_out)}</td>
                    <td className="px-4 py-3 tabular-nums text-slate-500">{formatDuration(row.break_minutes)}</td>
                    <td className="px-4 py-3 font-medium tabular-nums text-slate-800">{formatDuration(row.worked_minutes)}</td>
                    <td className="px-4 py-3 tabular-nums text-slate-600">
                      {row.overtime_minutes > 0 ? formatDuration(row.overtime_minutes) : "—"}
                    </td>
                    <td className="px-5 py-3">
                      <Badge tone={row.status === "complete" ? "green" : "amber"}>{row.status}</Badge>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <EmptyState icon={<FileText className="size-5" />} title="No records in this range" description="Adjust the date range or record a clock-in." />
        )}
      </Card>

      <Modal
        open={showManual}
        onClose={() => setShowManual(false)}
        title="Add manual entry"
        description="Correct a missed scan or add an offline record."
        footer={
          <>
            <Button variant="secondary" onClick={() => setShowManual(false)}>Cancel</Button>
            <Button
              onClick={() => manualMutation.mutate()}
              loading={manualMutation.isPending}
              disabled={!manual.employee_id || !manual.occurred_at}
            >
              Add entry
            </Button>
          </>
        }
      >
        <div className="space-y-4">
          <Field label="Employee">
            <Select value={manual.employee_id || ""} onChange={(event) => setManual({ ...manual, employee_id: Number(event.target.value) })}>
              <option value="">Select employee</option>
              {employeesQuery.data?.map((employee) => (
                <option key={employee.id} value={employee.id}>{employee.name}</option>
              ))}
            </Select>
          </Field>
          <Field label="Event type">
            <Select value={manual.event_type} onChange={(event) => setManual({ ...manual, event_type: event.target.value as EventType })}>
              {eventTypes.map((type) => (
                <option key={type.value} value={type.value}>{type.label}</option>
              ))}
            </Select>
          </Field>
          <Field label="Date and time">
            <Input type="datetime-local" value={manual.occurred_at} onChange={(event) => setManual({ ...manual, occurred_at: event.target.value })} />
          </Field>
          <Field label="Note (optional)">
            <Input value={manual.note} onChange={(event) => setManual({ ...manual, note: event.target.value })} placeholder="Reason for manual entry" />
          </Field>
        </div>
      </Modal>
    </div>
  );
}
