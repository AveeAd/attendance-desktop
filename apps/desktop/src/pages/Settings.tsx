import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { save } from "@tauri-apps/plugin-dialog";
import { format } from "date-fns";
import {
  Check,
  Cloud,
  CloudOff,
  DatabaseBackup,
  KeyRound,
  LockKeyhole,
  RefreshCw,
  Save,
  ShieldCheck,
  TriangleAlert,
} from "lucide-react";
import { api, errorMessage } from "@/lib/ipc";
import { Badge, Button, Card, Field, Input, Spinner } from "@/components/ui";
import { toast } from "@/store/toast";
import { useAuthStore } from "@/store/auth";
import type { SettingsInput } from "@/types";

function formatWhen(value: string | null): string {
  if (!value) return "Never";
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? "Unknown" : format(date, "MMM d, HH:mm");
}

export function Settings() {
  const queryClient = useQueryClient();
  const lock = useAuthStore((state) => state.lock);
  const settingsQuery = useQuery({ queryKey: ["settings"], queryFn: api.getSettings });
  const cloudQuery = useQuery({
    queryKey: ["cloud"],
    queryFn: api.getCloudConfig,
    refetchInterval: 15_000,
  });
  const conflictsQuery = useQuery({
    queryKey: ["cloudConflicts"],
    queryFn: api.cloudConflicts,
    enabled: cloudQuery.data?.configured ?? false,
    refetchInterval: 30_000,
  });
  const [form, setForm] = useState<SettingsInput>({
    company_name: "",
    overtime_daily_hours: 8,
    overtime_weekly_hours: 40,
    idle_lock_minutes: 15,
  });
  const [pinForm, setPinForm] = useState({ current: "", next: "", confirm: "" });
  const [cloudForm, setCloudForm] = useState({ apiUrl: "", apiKey: "", importExisting: true });
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

  useEffect(() => {
    const cloud = cloudQuery.data;
    if (cloud?.apiUrl) {
      setCloudForm((previous) => ({ ...previous, apiUrl: previous.apiUrl || cloud.apiUrl }));
    }
  }, [cloudQuery.data]);

  const refreshAfterSync = () => {
    for (const key of [
      "settings", "cloud", "cloudConflicts", "statuses", "events", "employees",
      "shifts", "assignments", "dutyRoster", "schedule", "timesheet",
    ]) {
      void queryClient.invalidateQueries({ queryKey: [key] });
    }
  };

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

  const connectCloud = useMutation({
    mutationFn: () =>
      api.configureCloud(cloudForm.apiUrl, cloudForm.apiKey, cloudForm.importExisting),
    onSuccess: (result) => {
      setCloudForm((previous) => ({ ...previous, apiKey: "" }));
      toast.success(
        `Connected · ${result.pushed} uploaded, ${result.pulled} downloaded${result.conflicts ? `, ${result.conflicts} conflicts` : ""}`,
      );
      refreshAfterSync();
    },
    onError: (error) => {
      toast.error(errorMessage(error));
      void queryClient.invalidateQueries({ queryKey: ["cloud"] });
    },
  });

  const syncCloud = useMutation({
    mutationFn: api.syncCloud,
    onSuccess: (result) => {
      toast.success(
        `Sync complete · ${result.pushed} uploaded, ${result.pulled} downloaded${result.conflicts ? `, ${result.conflicts} conflicts` : ""}`,
      );
      refreshAfterSync();
    },
    onError: (error) => {
      toast.error(errorMessage(error));
      void queryClient.invalidateQueries({ queryKey: ["cloud"] });
    },
  });

  const disconnectCloud = useMutation({
    mutationFn: api.disconnectCloud,
    onSuccess: () => {
      toast.success("Cloud sync disconnected. Local data was kept.");
      refreshAfterSync();
    },
    onError: (error) => toast.error(errorMessage(error)),
  });

  const acknowledge = useMutation({
    mutationFn: (ids: number[]) => api.acknowledgeCloudConflicts(ids),
    onSuccess: () => {
      toast.success("Conflict notices cleared");
      refreshAfterSync();
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

  if (settingsQuery.isLoading || cloudQuery.isLoading) {
    return <div className="flex h-full items-center justify-center"><Spinner /></div>;
  }

  const pinEnabled = settingsQuery.data?.pin_enabled ?? false;
  const cloud = cloudQuery.data;
  const conflicts = conflictsQuery.data ?? [];

  return (
    <div className="mx-auto max-w-[1000px] space-y-5">
      <div>
        <h2 className="text-xl font-semibold tracking-tight text-slate-900">Settings</h2>
        <p className="mt-1 text-sm text-slate-500">
          Configure your organization, cloud sync, overtime rules and security.
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
        title="Cloud sync"
        description="Share one API key across desktops; the server is authoritative"
        actions={
          <Badge tone={cloud?.configured ? "green" : "slate"}>
            {cloud?.configured ? <><Cloud className="size-3" /> Connected</> : <><CloudOff className="size-3" /> Local only</>}
          </Badge>
        }
      >
        {cloud?.configured ? (
          <div className="space-y-4 px-5 py-5">
            <div className="grid grid-cols-2 gap-4 text-xs">
              <div>
                <p className="text-slate-400">Server</p>
                <p className="mt-0.5 truncate font-medium text-slate-700">{cloud.apiUrl}</p>
              </div>
              <div>
                <p className="text-slate-400">API key</p>
                <p className="mt-0.5 font-mono font-medium text-slate-700">{cloud.keyHint}</p>
              </div>
              <div>
                <p className="text-slate-400">Device ID</p>
                <p className="mt-0.5 truncate font-mono text-[11px] text-slate-700">{cloud.deviceId}</p>
              </div>
              <div>
                <p className="text-slate-400">Last sync</p>
                <p className="mt-0.5 font-medium text-slate-700">{formatWhen(cloud.lastSync)}</p>
              </div>
              <div>
                <p className="text-slate-400">Pending writes</p>
                <p className="mt-0.5 font-medium text-slate-700">{cloud.pending}</p>
              </div>
              <div>
                <p className="text-slate-400">Data imported</p>
                <p className="mt-0.5 font-medium text-slate-700">{cloud.imported ? "Yes" : "No"}</p>
              </div>
            </div>
            {cloud.lastError && (
              <p className="flex items-start gap-2 rounded-lg bg-rose-50 px-3 py-2 text-xs text-rose-700">
                <TriangleAlert className="mt-0.5 size-3.5 shrink-0" />
                {cloud.lastError}
              </p>
            )}
            <div className="flex flex-wrap items-center gap-2 border-t border-slate-100 pt-4">
              <Button variant="secondary" loading={syncCloud.isPending} onClick={() => syncCloud.mutate()}>
                <RefreshCw className="size-4" />
                Sync now
              </Button>
              <Button variant="ghost" loading={disconnectCloud.isPending} onClick={() => disconnectCloud.mutate()}>
                Disconnect
              </Button>
              <p className="text-[11px] text-slate-400">
                Automatic sync runs every minute and whenever this app starts.
              </p>
            </div>
          </div>
        ) : (
          <div className="grid grid-cols-2 gap-4 px-5 py-5">
            <Field label="Server URL" className="col-span-2" hint="HTTPS endpoint of the Attendance API on Render">
              <Input
                value={cloudForm.apiUrl}
                onChange={(event) => setCloudForm({ ...cloudForm, apiUrl: event.target.value })}
                placeholder="https://attendance-api.onrender.com"
              />
            </Field>
            <Field label="Shared API key" className="col-span-2" hint="Stored in the app's protected local database, never shown to users">
              <Input
                type="password"
                autoComplete="off"
                value={cloudForm.apiKey}
                onChange={(event) => setCloudForm({ ...cloudForm, apiKey: event.target.value })}
                placeholder="Paste the API key"
              />
            </Field>
            <label className="col-span-2 flex items-start gap-2.5 rounded-lg border border-slate-200 bg-slate-50 px-3 py-2.5 text-xs text-slate-600">
              <input
                type="checkbox"
                className="mt-0.5 size-4 accent-indigo-600"
                checked={cloudForm.importExisting}
                onChange={(event) => setCloudForm({ ...cloudForm, importExisting: event.target.checked })}
              />
              <span>
                <span className="font-medium text-slate-800">Import this computer's existing data</span>
                <span className="mt-0.5 block text-slate-500">
                  Upload current employees, shifts, duty roster and events before syncing. Leave unchecked to use only server data.
                </span>
              </span>
            </label>
            <div className="col-span-2 flex items-center gap-2">
              <Button
                loading={connectCloud.isPending}
                disabled={!cloudForm.apiUrl.trim() || !cloudForm.apiKey.trim()}
                onClick={() => connectCloud.mutate()}
              >
                <Cloud className="size-4" />
                Connect and sync
              </Button>
              <p className="text-[11px] text-slate-400">The key is shared by all desktops; each request includes a unique device ID.</p>
            </div>
          </div>
        )}

        {conflicts.length > 0 && (
          <div className="border-t border-slate-100 px-5 py-4">
            <p className="mb-2 flex items-center gap-1.5 text-xs font-medium text-rose-700">
              <TriangleAlert className="size-3.5" />
              {conflicts.length} change{conflicts.length === 1 ? "" : "s"} rejected by the server
            </p>
            <div className="space-y-2">
              {conflicts.slice(0, 8).map((conflict) => (
                <div key={conflict.id} className="flex items-start gap-2 rounded-lg bg-rose-50/70 px-3 py-2 text-xs text-rose-800">
                  <span className="flex-1">
                    <span className="font-medium">{conflict.kind}</span> · {conflict.message}
                  </span>
                  <button
                    type="button"
                    className="shrink-0 text-rose-600 hover:text-rose-800"
                    onClick={() => acknowledge.mutate([conflict.id])}
                  >
                    Dismiss
                  </button>
                </div>
              ))}
            </div>
            <Button
              variant="ghost"
              size="sm"
              className="mt-2"
              loading={acknowledge.isPending}
              onClick={() => acknowledge.mutate(conflicts.map((conflict) => conflict.id))}
            >
              <Check className="size-3.5" />
              Dismiss all
            </Button>
          </div>
        )}
      </Card>

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

      <Card title="Data" description="A local cache is always kept on this computer">
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
