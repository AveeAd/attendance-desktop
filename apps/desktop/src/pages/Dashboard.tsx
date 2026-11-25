import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { format } from "date-fns";
import { QRCodeSVG } from "qrcode.react";
import {
  ArrowDownToLine,
  ArrowUpFromLine,
  Cloud,
  CloudOff,
  Coffee,
  Copy,
  Play,
  ScanLine,
  Smartphone,
  Square,
  Timer,
  UserRound,
  Wifi,
} from "lucide-react";
import { api, errorMessage } from "@/lib/ipc";
import { formatDuration, formatTime, initials, localIso } from "@/lib/format";
import { Badge, Button, Card, EmptyState, Input, cn } from "@/components/ui";
import { DutyRosterWidget } from "@/components/DutyRoster";
import { toast } from "@/store/toast";
import type { EventType } from "@/types";

const actions: Array<{
  type: EventType;
  label: string;
  icon: typeof ArrowDownToLine;
  className: string;
}> = [
  { type: "clock_in", label: "Clock in", icon: ArrowDownToLine, className: "bg-emerald-600 hover:bg-emerald-700" },
  { type: "clock_out", label: "Clock out", icon: ArrowUpFromLine, className: "bg-rose-600 hover:bg-rose-700" },
  { type: "break_start", label: "Start break", icon: Coffee, className: "bg-amber-600 hover:bg-amber-700" },
  { type: "break_end", label: "End break", icon: Play, className: "bg-indigo-600 hover:bg-indigo-700" },
];

const stateStyle = {
  in: { tone: "green" as const, label: "In" },
  on_break: { tone: "amber" as const, label: "On break" },
  out: { tone: "slate" as const, label: "Out" },
};

export function Dashboard() {
  const queryClient = useQueryClient();
  const inputRef = useRef<HTMLInputElement>(null);
  const [identifier, setIdentifier] = useState("");
  const [action, setAction] = useState<EventType>("clock_in");
  const [lastMessage, setLastMessage] = useState<string | null>(null);

  const statusesQuery = useQuery({ queryKey: ["statuses"], queryFn: api.getStatuses, refetchInterval: 30_000 });
  const eventsQuery = useQuery({
    queryKey: ["events", "today"],
    queryFn: () => {
      const start = new Date();
      start.setHours(0, 0, 0, 0);
      return api.listEvents({ from: localIso(start), limit: 100 });
    },
  });

  const mobileQuery = useQuery({
    queryKey: ["mobileServer"],
    queryFn: api.mobileServerStatus,
    refetchInterval: 10_000,
  });
  const cloudQuery = useQuery({
    queryKey: ["cloud"],
    queryFn: api.getCloudConfig,
    refetchInterval: 30_000,
  });
  const mobile = mobileQuery.data;
  const cloud = cloudQuery.data;
  const mobileUrl = mobile?.url ?? "";

  const startMobile = useMutation({
    mutationFn: api.startMobileServer,
    onSuccess: () => {
      toast.success("Mobile clock-in started");
      void queryClient.invalidateQueries({ queryKey: ["mobileServer"] });
    },
    onError: (error) => toast.error(errorMessage(error)),
  });

  const stopMobile = useMutation({
    mutationFn: api.stopMobileServer,
    onSuccess: () => {
      toast.info("Mobile clock-in stopped");
      void queryClient.invalidateQueries({ queryKey: ["mobileServer"] });
    },
    onError: (error) => toast.error(errorMessage(error)),
  });

  const clockMutation = useMutation({
    mutationFn: api.clockEvent,
    onSuccess: (result) => {
      setLastMessage(`${result.employee.name} · ${result.message}`);
      setIdentifier("");
      toast.success(`${result.employee.name}: ${result.message.toLowerCase()}`);
      inputRef.current?.focus();
      void queryClient.invalidateQueries({ queryKey: ["statuses"] });
      void queryClient.invalidateQueries({ queryKey: ["events"] });
      void queryClient.invalidateQueries({ queryKey: ["dutyRoster"] });
    },
    onError: (error) => {
      toast.error(errorMessage(error));
      inputRef.current?.focus();
    },
  });

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const submit = (type: EventType) => {
    const value = identifier.trim();
    if (!value) {
      inputRef.current?.focus();
      toast.info("Scan a badge or enter an employee ID first");
      return;
    }
    setAction(type);
    clockMutation.mutate({
      identifier: value,
      event_type: type,
      occurred_at: localIso(),
      source: "scan",
      note: null,
    });
  };

  const statuses = statusesQuery.data ?? [];
  const present = statuses.filter((status) => status.state !== "out");
  const events = eventsQuery.data ?? [];

  return (
    <div className="mx-auto grid max-w-[1400px] grid-cols-1 gap-5 xl:grid-cols-[minmax(0,1.35fr)_minmax(320px,0.65fr)]">
      <div className="space-y-5">
        <Card className="overflow-hidden">
          <div className="border-b border-slate-100 px-6 py-5">
            <div className="flex items-center gap-2">
              <ScanLine className="size-5 text-indigo-600" />
              <h2 className="text-base font-semibold text-slate-900">Clock station</h2>
            </div>
            <p className="mt-1 text-xs text-slate-500">
              Scan a badge or QR code, or type the employee ID and press Enter.
            </p>
          </div>

          <form
            className="px-6 py-6"
            onSubmit={(event) => {
              event.preventDefault();
              submit(action);
            }}
          >
            <div className="relative">
              <UserRound className="pointer-events-none absolute left-4 top-1/2 size-5 -translate-y-1/2 text-slate-400" />
              <Input
                ref={inputRef}
                value={identifier}
                onChange={(event) => setIdentifier(event.target.value)}
                placeholder="Employee ID or badge"
                className="h-16 rounded-xl border-2 border-slate-200 pl-12 text-lg shadow-inner shadow-slate-50 focus:border-indigo-400"
                disabled={clockMutation.isPending}
              />
              <div className="pointer-events-none absolute right-4 top-1/2 -translate-y-1/2 rounded-md bg-slate-100 px-2 py-1 text-[11px] font-medium text-slate-500">
                ↵ Enter
              </div>
            </div>

            <div className="mt-4 grid grid-cols-2 gap-3 sm:grid-cols-4">
              {actions.map((item) => (
                <Button
                  key={item.type}
                  type="button"
                  onClick={() => submit(item.type)}
                  disabled={clockMutation.isPending}
                  className={cn("h-12 text-white", item.className, action === item.type && "ring-2 ring-offset-2 ring-indigo-200")}
                >
                  <item.icon className="size-4" />
                  {item.label}
                </Button>
              ))}
            </div>

            {lastMessage && (
              <div className="mt-5 flex items-center gap-2 rounded-lg bg-emerald-50 px-3 py-2.5 text-sm text-emerald-800">
                <span className="size-2 rounded-full bg-emerald-500" />
                {lastMessage}
              </div>
            )}
          </form>
        </Card>

        <DutyRosterWidget />

        <Card title="Today's activity" description="Most recent scans across all employees">
          {events.length === 0 ? (
            <EmptyState icon={<Timer className="size-5" />} title="No activity yet" description="Scans will appear here as soon as someone clocks in." />
          ) : (
            <div className="divide-y divide-slate-100">
              {events.slice(0, 12).map((event) => (
                <div key={event.id} className="flex items-center gap-3 px-5 py-3">
                  <div className="flex size-8 items-center justify-center rounded-lg bg-slate-100 text-[11px] font-semibold text-slate-600">
                    {initials(event.employee_name ?? "?")}
                  </div>
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-sm font-medium text-slate-800">{event.employee_name}</p>
                    <p className="text-[11px] text-slate-400">
                      {event.source === "mobile"
                        ? "Phone clock-in"
                        : event.source === "manual"
                          ? "Manual entry"
                          : "Badge scan"}
                      {event.note ? ` · ${event.note}` : ""}
                    </p>
                  </div>
                  <Badge tone={event.event_type === "clock_in" ? "green" : event.event_type === "clock_out" ? "slate" : "amber"}>
                    {event.event_type.replaceAll("_", " ")}
                  </Badge>
                  <span className="w-20 text-right text-xs tabular-nums text-slate-500">
                    {formatTime(event.occurred_at)}
                  </span>
                </div>
              ))}
            </div>
          )}
        </Card>
      </div>

      <div className="space-y-5">
        <Card
          title="Cloud sync"
          description="Read-only status of the shared server connection"
          actions={
            <Badge tone={cloud?.configured ? "green" : "slate"}>
              {cloud?.configured ? <><Cloud className="size-3" /> Connected</> : <><CloudOff className="size-3" /> Local only</>}
            </Badge>
          }
        >
          <div className="space-y-2 px-5 py-4 text-xs text-slate-600">
            {cloud?.configured ? (
              <>
                <div className="flex justify-between gap-3">
                  <span className="text-slate-400">Last sync</span>
                  <span className="font-medium tabular-nums">
                    {cloud.lastSync ? format(new Date(cloud.lastSync), "MMM d, HH:mm") : "Never"}
                  </span>
                </div>
                <div className="flex justify-between gap-3">
                  <span className="text-slate-400">Pending uploads</span>
                  <span className="font-medium tabular-nums">{cloud.pending}</span>
                </div>
                <div className="flex justify-between gap-3">
                  <span className="text-slate-400">Conflicts</span>
                  <span className={cn("font-medium tabular-nums", cloud.conflicts > 0 && "text-rose-600")}>
                    {cloud.conflicts}
                  </span>
                </div>
                <div className="flex justify-between gap-3">
                  <span className="text-slate-400">Device</span>
                  <span className="truncate font-mono text-[11px]">{cloud.deviceId.slice(0, 8)}…</span>
                </div>
              </>
            ) : (
              <p className="leading-5 text-slate-500">
                This computer works offline. Connect a shared server in Settings to sync attendance across desktops.
              </p>
            )}
          </div>
        </Card>

        <Card
          title="Phone clock-in"
          description="One shared QR for the whole team"
          actions={
            mobile?.running ? (
              <Badge tone="green">
                <Wifi className="size-3" /> Live
              </Badge>
            ) : (
              <Badge tone="slate">Off</Badge>
            )
          }
        >
          {mobile?.running && mobileUrl ? (
            <div className="flex flex-col items-center px-5 py-5 text-center">
              <div className="rounded-2xl border border-slate-200 bg-white p-4 shadow-sm">
                <QRCodeSVG value={mobileUrl} size={168} level="M" />
              </div>
              <p className="mt-3 text-xs font-medium text-slate-700">Scan this with a phone camera</p>
              <p className="mt-1 text-[11px] leading-4 text-slate-500">
                Employees pick their name, enter their code and optional PIN, then clock in or out.
              </p>
              <div className="mt-3 flex w-full items-center gap-2 rounded-lg bg-slate-50 px-3 py-2">
                <code className="min-w-0 flex-1 truncate text-left text-[11px] text-slate-600">
                  {mobileUrl}
                </code>
                <button
                  type="button"
                  className="text-slate-400 hover:text-indigo-600"
                  title="Copy link"
                  onClick={() => {
                    void navigator.clipboard?.writeText(mobileUrl).then(
                      () => toast.success("Link copied"),
                      () => toast.error("Could not copy link"),
                    );
                  }}
                >
                  <Copy className="size-3.5" />
                </button>
              </div>
              <p className="mt-2 text-[11px] text-slate-400">
                Works only on the same Wi-Fi / LAN as this computer.
              </p>
              <Button
                variant="secondary"
                size="sm"
                className="mt-3"
                loading={stopMobile.isPending}
                onClick={() => stopMobile.mutate()}
              >
                <Square className="size-3.5" /> Stop mobile station
              </Button>
            </div>
          ) : (
            <div className="px-5 py-5 text-center">
              <div className="mx-auto mb-3 flex size-11 items-center justify-center rounded-xl bg-indigo-50 text-indigo-600">
                <Smartphone className="size-5" />
              </div>
              <p className="text-sm font-medium text-slate-800">Start the phone station</p>
              <p className="mt-1 text-[11px] leading-4 text-slate-500">
                Serves a clock-in page over your local network. A shared QR code will appear here.
              </p>
              <Button
                className="mt-4"
                loading={startMobile.isPending}
                onClick={() => startMobile.mutate()}
              >
                <Play className="size-4" /> Start mobile clock-in
              </Button>
            </div>
          )}
        </Card>

        <Card
          title={present.length > 0 ? `${present.length} currently in` : "Nobody is in"}
          description="Live status for all active employees"
          actions={<Badge tone="blue">Live</Badge>}
        >
          {statuses.length === 0 ? (
            <EmptyState icon={<UserRound className="size-5" />} title="No employees yet" description="Add employees to start tracking attendance." />
          ) : (
            <div className="divide-y divide-slate-100">
              {statuses
                .slice()
                .sort((a, b) => (a.state === "out" ? 1 : 0) - (b.state === "out" ? 1 : 0))
                .map((status) => (
                  <div key={status.employee.id} className="flex items-center gap-3 px-5 py-3.5">
                    <div className="flex size-9 items-center justify-center rounded-full bg-indigo-50 text-xs font-semibold text-indigo-700">
                      {initials(status.employee.name)}
                    </div>
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-sm font-medium text-slate-800">{status.employee.name}</p>
                      <p className="text-[11px] text-slate-400">
                        {status.clock_in_at
                          ? `In since ${formatTime(status.clock_in_at)} · ${formatDuration(status.open_minutes)}`
                          : status.last_event_at
                            ? `Last ${formatTime(status.last_event_at)}`
                            : "No scans today"}
                      </p>
                    </div>
                    <Badge tone={stateStyle[status.state].tone}>{stateStyle[status.state].label}</Badge>
                  </div>
                ))}
            </div>
          )}
        </Card>

        <Card title="Quick reference" description="Badge values accepted at the station">
          <div className="space-y-3 px-5 py-4 text-xs text-slate-600">
            <p className="flex items-start gap-2">
              <span className="mt-0.5 size-1.5 shrink-0 rounded-full bg-indigo-500" />
              Employee code (e.g. <span className="font-mono text-slate-800">EMP-001</span>) is typed or scanned.
            </p>
            <p className="flex items-start gap-2">
              <span className="mt-0.5 size-1.5 shrink-0 rounded-full bg-indigo-500" />
              Printed QR cards from the Employees page encode the same code.
            </p>
            <p className="flex items-start gap-2">
              <span className="mt-0.5 size-1.5 shrink-0 rounded-full bg-indigo-500" />
              Duplicate scans are ignored automatically.
            </p>
            <p className="text-[11px] text-slate-400">
              Today: {format(new Date(), "MMM d, yyyy")}
            </p>
          </div>
        </Card>
      </div>
    </div>
  );
}
