import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { save } from "@tauri-apps/plugin-dialog";
import { DatabaseBackup, KeyRound, LockKeyhole, Save, ShieldCheck } from "lucide-react";
import { api, errorMessage } from "@/lib/ipc";
import { Badge, Button, Card, Field, Input, Spinner } from "@/components/ui";
import { toast } from "@/store/toast";
import { useAuthStore } from "@/store/auth";
import type { SettingsInput } from "@/types";

export function Settings() {
  const queryClient = useQueryClient();
  const lock = useAuthStore((state) => state.lock);
  const settingsQuery = useQuery({ queryKey: ["settings"], queryFn: api.getSettings });
  const [form, setForm] = useState<SettingsInput>({
    company_name: "",
    overtime_daily_hours: 8,
    overtime_weekly_hours: 40,
    idle_lock_minutes: 15,
  });
  const [pinForm, setPinForm] = useState({ current: "", next: "", confirm: "" });
  const [backingUp, setBackingUp] = useState(false);

  useEffect(() => {
    const data = settingsQuery.data;
    if (!data) return;
    setForm({
      company_name: data.company_name,
      overtime_daily_hours: data.overtime_daily_hours,
      overtime_weekly_hours: data.overtime_weekly_hours,
      idle_lock_minutes: data.idle_lock_minutes,
    });
  }, [settingsQuery.data]);

  const saveSettings = useMutation({
    mutationFn: api.updateSettings,
    onSuccess: () => {
      toast.success("Settings saved");
      void queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
    onError: (error) => toast.error(errorMessage(error)),
  });

  const savePin = useMutation({
    mutationFn: () => api.setAdminPin(pinForm.current || null, pinForm.next),
    onSuccess: () => {
      toast.success("Admin PIN updated");
      setPinForm({ current: "", next: "", confirm: "" });
      void queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
    onError: (error) => toast.error(errorMessage(error)),
  });

  const removePin = useMutation({
    mutationFn: () => api.clearAdminPin(pinForm.current),
    onSuccess: () => {
      toast.success("Admin PIN removed");
      setPinForm({ current: "", next: "", confirm: "" });
      void queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
    onError: (error) => toast.error(errorMessage(error)),
  });

  const backup = async () => {
    setBackingUp(true);
    try {
      const path = await save({
        defaultPath: `attendance-backup-${new Date().toISOString().slice(0, 10)}.db`,
        filters: [{ name: "SQLite database", extensions: ["db"] }],
      });
      if (!path) return;
      await api.backupDatabase(path);
      toast.success("Backup created");
    } catch (caught) {
      toast.error(errorMessage(caught));
    } finally {
      setBackingUp(false);
    }
  };

  if (settingsQuery.isLoading) {
    return <div className="flex h-full items-center justify-center"><Spinner /></div>;
  }

  const pinEnabled = settingsQuery.data?.pin_enabled ?? false;

  return (
    <div className="mx-auto max-w-[1000px] space-y-5">
      <div>
        <h2 className="text-xl font-semibold tracking-tight text-slate-900">Settings</h2>
        <p className="mt-1 text-sm text-slate-500">
          Configure your organization, overtime rules and security.
        </p>
      </div>

      <form
        onSubmit={(event) => {
          event.preventDefault();
          saveSettings.mutate(form);
        }}
      >
        <Card title="General" description="Shown in the app header and reports">
          <div className="grid grid-cols-2 gap-4 px-5 py-5">
            <Field label="Organization name" className="col-span-2">
              <Input value={form.company_name} onChange={(event) => setForm({ ...form, company_name: event.target.value })} />
            </Field>
            <Field label="Daily overtime threshold (hours)" hint="Hours per day before overtime is counted">
              <Input type="number" min={0} step={0.5} value={form.overtime_daily_hours} onChange={(event) => setForm({ ...form, overtime_daily_hours: Number(event.target.value) })} />
            </Field>
            <Field label="Weekly overtime threshold (hours)" hint="Applied to each ISO week">
              <Input type="number" min={0} step={0.5} value={form.overtime_weekly_hours} onChange={(event) => setForm({ ...form, overtime_weekly_hours: Number(event.target.value) })} />
            </Field>
            <Field label="Auto-lock admin area (minutes)" hint="Set to 0 to disable idle locking">
              <Input type="number" min={0} value={form.idle_lock_minutes} onChange={(event) => setForm({ ...form, idle_lock_minutes: Number(event.target.value) })} />
            </Field>
          </div>
          <div className="flex justify-end border-t border-slate-100 bg-slate-50/60 px-5 py-3">
            <Button type="submit" loading={saveSettings.isPending}>
              <Save className="size-4" />
              Save settings
            </Button>
          </div>
        </Card>
      </form>

      <Card
        title="Admin PIN"
        description="Restrict employee, schedule and report screens"
        actions={<Badge tone={pinEnabled ? "green" : "slate"}>{pinEnabled ? "Enabled" : "Disabled"}</Badge>}
      >
        <div className="grid grid-cols-2 gap-4 px-5 py-5">
          {pinEnabled && (
            <Field label="Current PIN" className="col-span-2">
              <Input type="password" inputMode="numeric" value={pinForm.current} onChange={(event) => setPinForm({ ...pinForm, current: event.target.value.replace(/\D/g, "") })} placeholder="Enter current PIN" />
            </Field>
          )}
          <Field label={pinEnabled ? "New PIN" : "PIN"}>
            <Input type="password" inputMode="numeric" value={pinForm.next} onChange={(event) => setPinForm({ ...pinForm, next: event.target.value.replace(/\D/g, "") })} placeholder="At least 4 digits" />
          </Field>
          <Field label="Confirm new PIN">
            <Input type="password" inputMode="numeric" value={pinForm.confirm} onChange={(event) => setPinForm({ ...pinForm, confirm: event.target.value.replace(/\D/g, "") })} placeholder="Repeat PIN" />
          </Field>
          <div className="col-span-2 flex flex-wrap items-center gap-2">
            <Button
              onClick={() => savePin.mutate()}
              loading={savePin.isPending}
              disabled={pinForm.next.length < 4 || pinForm.next !== pinForm.confirm || (pinEnabled && !pinForm.current)}
            >
              <KeyRound className="size-4" />
              {pinEnabled ? "Change PIN" : "Set PIN"}
            </Button>
            {pinEnabled && (
              <Button variant="secondary" onClick={() => removePin.mutate()} loading={removePin.isPending} disabled={!pinForm.current}>
                Remove PIN
              </Button>
            )}
            {pinEnabled && (
              <Button variant="ghost" onClick={lock}>
                <LockKeyhole className="size-4" />
                Lock now
              </Button>
            )}
          </div>
          <p className="col-span-2 flex items-center gap-1.5 text-[11px] text-slate-400">
            <ShieldCheck className="size-3.5" />
            PINs are stored as salted Argon2 hashes, never as plain text.
          </p>
        </div>
      </Card>

      <Card title="Data" description="All records are stored locally on this computer">
        <div className="flex flex-wrap items-center gap-3 px-5 py-5">
          <Button variant="secondary" onClick={() => void backup()} loading={backingUp}>
            <DatabaseBackup className="size-4" />
            Back up database
          </Button>
          <p className="text-xs text-slate-500">
            Creates a snapshot of your SQLite database. Keep it somewhere safe.
          </p>
        </div>
      </Card>
    </div>
  );
}
