import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { QRCodeSVG } from "qrcode.react";
import { KeyRound, Pencil, Plus, Printer, QrCode, Search, UserRound, UserRoundX } from "lucide-react";
import { api, errorMessage } from "@/lib/ipc";
import { format } from "date-fns";
import { Badge, Button, Card, EmptyState, Field, Input, Modal, Select, Spinner } from "@/components/ui";
import { toast } from "@/store/toast";
import type { CreateEmployeeInput, Employee, UpdateEmployeeInput } from "@/types";

type FormState = {
  code: string;
  name: string;
  department: string;
  role: string;
  active: boolean;
  pin: string;
  clearPin: boolean;
};

const emptyForm: FormState = {
  code: "",
  name: "",
  department: "",
  role: "",
  active: true,
  pin: "",
  clearPin: false,
};

export function Employees() {
  const queryClient = useQueryClient();
  const [search, setSearch] = useState("");
  const [editing, setEditing] = useState<Employee | null>(null);
  const [creating, setCreating] = useState(false);
  const [form, setForm] = useState<FormState>(emptyForm);
  const [qrEmployee, setQrEmployee] = useState<Employee | null>(null);

  const employeesQuery = useQuery({ queryKey: ["employees"], queryFn: () => api.listEmployees(false) });

  const saveMutation = useMutation({
    mutationFn: () => {
      const normalized = {
        ...form,
        department: form.department.trim() || null,
        role: form.role.trim() || null,
      };
      if (editing) {
        const input: UpdateEmployeeInput = {
          name: normalized.name,
          department: normalized.department,
          role: normalized.role,
          active: normalized.active,
          pin: form.clearPin ? "" : form.pin.trim() ? form.pin.trim() : null,
        };
        return api.updateEmployee(editing.id, input).then(() => "Employee updated");
      }
      const input: CreateEmployeeInput = {
        code: normalized.code,
        name: normalized.name,
        department: normalized.department,
        role: normalized.role,
        pin: form.pin.trim() || null,
      };
      return api.createEmployee(input).then(() => "Employee created");
    },
    onSuccess: (message) => {
      toast.success(message);
      setEditing(null);
      setCreating(false);
      void queryClient.invalidateQueries({ queryKey: ["employees"] });
      void queryClient.invalidateQueries({ queryKey: ["statuses"] });
    },
    onError: (error) => toast.error(errorMessage(error)),
  });

  const toggleMutation = useMutation({
    mutationFn: ({ id, active }: { id: number; active: boolean }) =>
      api.setEmployeeActive(id, active),
    onSuccess: (_, variables) => {
      toast.success(variables.active ? "Employee activated" : "Employee deactivated");
      void queryClient.invalidateQueries({ queryKey: ["employees"] });
      void queryClient.invalidateQueries({ queryKey: ["statuses"] });
    },
    onError: (error) => toast.error(errorMessage(error)),
  });

  const openCreate = () => {
    setForm(emptyForm);
    setEditing(null);
    setCreating(true);
  };

  const openEdit = (employee: Employee) => {
    setForm({
      code: employee.code,
      name: employee.name,
      department: employee.department ?? "",
      role: employee.role ?? "",
      active: employee.active,
      pin: "",
      clearPin: false,
    });
    setEditing(employee);
    setCreating(false);
  };

  const closeModal = () => {
    setEditing(null);
    setCreating(false);
  };

  const term = search.trim().toLowerCase();
  const employees = (employeesQuery.data ?? []).filter((employee) =>
    [employee.name, employee.code, employee.department, employee.role].some((value) =>
      value?.toLowerCase().includes(term),
    ),
  );

  return (
    <div className="mx-auto max-w-[1200px] space-y-5">
      <div className="flex items-center justify-between gap-4">
        <div>
          <h2 className="text-xl font-semibold tracking-tight text-slate-900">Employees</h2>
          <p className="mt-1 text-sm text-slate-500">
            Manage staff records and printable QR badges.
          </p>
        </div>
        <Button onClick={openCreate}>
          <Plus className="size-4" />
          New employee
        </Button>
      </div>

      <Card>
        <div className="flex items-center gap-3 border-b border-slate-100 px-5 py-3.5">
          <div className="relative w-72">
            <Search className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-slate-400" />
            <Input
              value={search}
              onChange={(event) => setSearch(event.target.value)}
              placeholder="Search name, code, department..."
              className="pl-9"
            />
          </div>
          <span className="ml-auto text-xs text-slate-400">
            {employees.length} of {employeesQuery.data?.length ?? 0} employees
          </span>
        </div>

        {employeesQuery.isLoading ? (
          <div className="flex justify-center py-16"><Spinner /></div>
        ) : employees.length === 0 ? (
          <EmptyState
            icon={<UserRound className="size-5" />}
            title={term ? "No matching employees" : "No employees yet"}
            description={term ? "Try a different search term." : "Add your first employee to get started."}
            action={!term && <Button onClick={openCreate}>Add employee</Button>}
          />
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-left text-sm">
              <thead>
                <tr className="border-b border-slate-100 text-[11px] uppercase tracking-wide text-slate-400">
                  <th className="px-5 py-3 font-medium">Employee</th>
                  <th className="px-4 py-3 font-medium">Department</th>
                  <th className="px-4 py-3 font-medium">Role</th>
                  <th className="px-4 py-3 font-medium">Status</th>
                  <th className="px-4 py-3 font-medium">Added</th>
                  <th className="px-5 py-3 text-right font-medium">Actions</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-100">
                {employees.map((employee) => (
                  <tr key={employee.id} className="hover:bg-slate-50/60">
                    <td className="px-5 py-3.5">
                      <div className="flex items-center gap-3">
                        <div className="flex size-9 items-center justify-center rounded-full bg-indigo-50 text-xs font-semibold text-indigo-700">
                          {employee.name.split(/\s+/).slice(0, 2).map((part) => part[0]).join("").toUpperCase()}
                        </div>
                        <div>
                          <p className="font-medium text-slate-800">{employee.name}</p>
                          <p className="flex items-center gap-1 font-mono text-[11px] text-slate-400">
                            {employee.code}
                            {employee.has_pin && (
                              <span title="Mobile PIN set">
                                <KeyRound className="size-3 text-indigo-400" />
                              </span>
                            )}
                          </p>
                        </div>
                      </div>
                    </td>
                    <td className="px-4 py-3.5 text-slate-600">{employee.department ?? "—"}</td>
                    <td className="px-4 py-3.5 text-slate-600">{employee.role ?? "—"}</td>
                    <td className="px-4 py-3.5">
                      <Badge tone={employee.active ? "green" : "slate"}>
                        {employee.active ? "Active" : "Inactive"}
                      </Badge>
                    </td>
                    <td className="px-4 py-3.5 text-xs text-slate-500">
                      {format(new Date(employee.created_at), "MMM d, yyyy")}
                    </td>
                    <td className="px-5 py-3.5">
                      <div className="flex items-center justify-end gap-1">
                        <Button variant="ghost" size="sm" onClick={() => setQrEmployee(employee)} title="Show QR badge">
                          <QrCode className="size-4" />
                        </Button>
                        <Button variant="ghost" size="sm" onClick={() => openEdit(employee)} title="Edit employee">
                          <Pencil className="size-4" />
                        </Button>
                        <Button
                          variant="ghost"
                          size="sm"
                          title={employee.active ? "Deactivate" : "Activate"}
                          onClick={() => toggleMutation.mutate({ id: employee.id, active: !employee.active })}
                        >
                          <UserRoundX className="size-4" />
                        </Button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </Card>

      <Modal
        open={creating || editing !== null}
        onClose={closeModal}
        title={editing ? "Edit employee" : "New employee"}
        description={editing ? "Update details or deactivate this employee." : "Create a record and generate a badge."}
        footer={
          <>
            <Button variant="secondary" onClick={closeModal}>Cancel</Button>
            <Button
              onClick={() => saveMutation.mutate()}
              loading={saveMutation.isPending}
              disabled={!form.name.trim() || (!editing && !form.code.trim())}
            >
              {editing ? "Save changes" : "Create employee"}
            </Button>
          </>
        }
      >
        <div className="grid grid-cols-2 gap-4">
          <Field label="Employee code" hint="Used for badge scans and manual entry" className="col-span-2">
            <Input
              value={form.code}
              disabled={editing !== null}
              onChange={(event) => setForm({ ...form, code: event.target.value })}
              placeholder="EMP-001"
            />
          </Field>
          <Field label="Full name" className="col-span-2">
            <Input
              value={form.name}
              onChange={(event) => setForm({ ...form, name: event.target.value })}
              placeholder="Jane Doe"
            />
          </Field>
          <Field label="Department">
            <Input
              value={form.department}
              onChange={(event) => setForm({ ...form, department: event.target.value })}
              placeholder="Operations"
            />
          </Field>
          <Field label="Role">
            <Input
              value={form.role}
              onChange={(event) => setForm({ ...form, role: event.target.value })}
              placeholder="Technician"
            />
          </Field>
          <Field
            label={editing ? "New mobile PIN (optional)" : "Mobile PIN (optional)"}
            hint="4+ digits. Required on the phone clock-in page if set."
            className="col-span-2"
          >
            <Input
              type="password"
              inputMode="numeric"
              autoComplete="off"
              value={form.pin}
              disabled={form.clearPin}
              onChange={(event) => setForm({ ...form, pin: event.target.value.replace(/\D/g, "") })}
              placeholder={editing && editing.has_pin ? "Leave blank to keep current PIN" : "No PIN required"}
            />
          </Field>
          {editing && (
            <>
              <Field label="Status" className="col-span-2">
                <Select
                  value={form.active ? "active" : "inactive"}
                  onChange={(event) => setForm({ ...form, active: event.target.value === "active" })}
                >
                  <option value="active">Active</option>
                  <option value="inactive">Inactive</option>
                </Select>
              </Field>
              {editing.has_pin && (
                <label className="col-span-2 flex items-center gap-2 text-xs text-slate-600">
                  <input
                    type="checkbox"
                    checked={form.clearPin}
                    onChange={(event) => setForm({ ...form, clearPin: event.target.checked })}
                    className="size-4 accent-indigo-600"
                  />
                  Remove current PIN
                </label>
              )}
            </>
          )}
        </div>
      </Modal>

      <Modal
        open={qrEmployee !== null}
        onClose={() => setQrEmployee(null)}
        title="Badge QR code"
        description="Print and hand this card to the employee."
        width="max-w-md"
        footer={
          <>
            <Button variant="secondary" onClick={() => setQrEmployee(null)}>Close</Button>
            <Button onClick={() => window.print()}>
              <Printer className="size-4" />
              Print card
            </Button>
          </>
        }
      >
        {qrEmployee && (
          <div className="print-area flex flex-col items-center rounded-2xl border border-slate-200 bg-white p-8 text-center">
            <p className="text-[11px] font-semibold uppercase tracking-widest text-indigo-600">Attendance badge</p>
            <h3 className="mt-2 text-xl font-semibold text-slate-900">{qrEmployee.name}</h3>
            <p className="font-mono text-sm text-slate-500">{qrEmployee.code}</p>
            <div className="my-6 rounded-xl bg-white p-4 ring-1 ring-slate-200">
              <QRCodeSVG value={qrEmployee.code} size={200} level="M" />
            </div>
            <p className="text-xs text-slate-400">Scan at the clock station to record attendance.</p>
          </div>
        )}
      </Modal>
    </div>
  );
}
