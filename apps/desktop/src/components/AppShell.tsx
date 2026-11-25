import { useEffect, useRef, useState } from "react";
import { NavLink, Outlet } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import {
  BarChart3,
  CalendarDays,
  Clock3,
  LayoutDashboard,
  LockKeyhole,
  ScanLine,
  Settings,
  Users,
} from "lucide-react";
import { api } from "@/lib/ipc";
import { format } from "date-fns";
import { useAuthStore } from "@/store/auth";
import { Toaster, cn } from "@/components/ui";

const navigation = [
  { to: "/", label: "Dashboard", icon: LayoutDashboard },
  { to: "/employees", label: "Employees", icon: Users },
  { to: "/schedule", label: "Schedule", icon: CalendarDays },
  { to: "/reports", label: "Reports", icon: BarChart3 },
  { to: "/settings", label: "Settings", icon: Settings },
];

export function AppShell() {
  const [now, setNow] = useState(new Date());
  const settingsQuery = useQuery({ queryKey: ["settings"], queryFn: api.getSettings });
  const cloudQuery = useQuery({
    queryKey: ["cloud"],
    queryFn: api.getCloudConfig,
    refetchInterval: 60_000,
  });
  const lock = useAuthStore((state) => state.lock);
  const unlocked = useAuthStore((state) => state.unlocked);
  const idleTimer = useRef<number | null>(null);

  useEffect(() => {
    const interval = window.setInterval(() => setNow(new Date()), 15_000);
    return () => window.clearInterval(interval);
  }, []);

  useEffect(() => {
    const settings = settingsQuery.data;
    if (!settings?.pin_enabled || !unlocked || settings.idle_lock_minutes <= 0) return;

    const reset = () => {
      if (idleTimer.current) window.clearTimeout(idleTimer.current);
      idleTimer.current = window.setTimeout(() => lock(), settings.idle_lock_minutes * 60_000);
    };
    const events: Array<keyof WindowEventMap> = ["mousemove", "keydown", "click", "scroll"];
    events.forEach((event) => window.addEventListener(event, reset, { passive: true }));
    reset();
    return () => {
      events.forEach((event) => window.removeEventListener(event, reset));
      if (idleTimer.current) window.clearTimeout(idleTimer.current);
    };
  }, [settingsQuery.data, unlocked, lock]);

  return (
    <div className="flex h-full bg-[#f7f8fb]">
      <aside className="flex w-[228px] shrink-0 flex-col border-r border-slate-200/80 bg-white">
        <div className="flex items-center gap-2.5 px-5 py-5">
          <div className="flex size-9 items-center justify-center rounded-xl bg-indigo-600 text-white shadow-sm shadow-indigo-600/25">
            <ScanLine className="size-5" />
          </div>
          <div>
            <p className="text-sm font-semibold tracking-tight text-slate-900">Attendance</p>
            <p className="text-[11px] text-slate-400">Workforce time tracking</p>
          </div>
        </div>

        <nav className="flex-1 space-y-0.5 px-3 py-2">
          {navigation.map((item) => (
            <NavLink
              key={item.to}
              to={item.to}
              end={item.to === "/"}
              className={({ isActive }) =>
                cn(
                  "flex items-center gap-3 rounded-lg px-3 py-2.5 text-sm font-medium transition-colors",
                  isActive
                    ? "bg-indigo-50 text-indigo-700"
                    : "text-slate-600 hover:bg-slate-50 hover:text-slate-900",
                )
              }
            >
              <item.icon className="size-[18px]" />
              {item.label}
            </NavLink>
          ))}
        </nav>

        <div className="border-t border-slate-100 px-5 py-4">
          <div className="flex items-center gap-2 text-[11px] text-slate-500">
            <LockKeyhole className="size-3.5" />
            {settingsQuery.data?.pin_enabled ? "Admin PIN enabled" : "No admin PIN set"}
          </div>
          <p className="mt-1 text-[11px] text-slate-400">
            {cloudQuery.data?.configured ? "Synced with server" : "Local data"} · v0.1.0
          </p>
        </div>
      </aside>

      <div className="flex min-w-0 flex-1 flex-col">
        <header className="flex h-16 shrink-0 items-center justify-between border-b border-slate-200/80 bg-white px-7">
          <div>
            <h1 className="text-sm font-semibold text-slate-900">
              {settingsQuery.data?.company_name ?? "Attendance"}
            </h1>
            <p className="text-[11px] text-slate-400">
              {format(now, "EEEE, MMMM d, yyyy")}
            </p>
          </div>
          <div className="flex items-center gap-2 rounded-lg bg-slate-50 px-3 py-2 text-sm text-slate-600">
            <Clock3 className="size-4 text-slate-400" />
            <span className="tabular-nums">{format(now, "h:mm:ss a")}</span>
          </div>
        </header>

        <main className="min-h-0 flex-1 overflow-y-auto px-7 py-6">
          <Outlet />
        </main>
      </div>
      <Toaster />
    </div>
  );
}
