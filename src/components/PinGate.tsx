import { useEffect, useRef, useState, type ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { KeyRound, LockKeyhole } from "lucide-react";
import { api, errorMessage } from "@/lib/ipc";
import { useAuthStore } from "@/store/auth";
import { Button, Card, Input, Spinner } from "@/components/ui";

export function PinGate({ children }: { children: ReactNode }) {
  const settingsQuery = useQuery({ queryKey: ["settings"], queryFn: api.getSettings });
  const unlocked = useAuthStore((state) => state.unlocked);
  const unlock = useAuthStore((state) => state.unlock);
  const [pin, setPin] = useState("");
  const [error, setError] = useState("");
  const [checking, setChecking] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (settingsQuery.data && !settingsQuery.data.pin_enabled) unlock();
  }, [settingsQuery.data, unlock]);

  useEffect(() => {
    if (settingsQuery.data?.pin_enabled && !unlocked) inputRef.current?.focus();
  }, [settingsQuery.data, unlocked]);

  if (settingsQuery.isLoading) {
    return (
      <div className="flex h-full items-center justify-center">
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
      const valid = await api.verifyAdminPin(pin);
      if (valid) {
        unlock();
        setPin("");
      } else {
        setError("Incorrect PIN. Try again.");
        setPin("");
        inputRef.current?.focus();
      }
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setChecking(false);
    }
  };

  return (
    <div className="flex h-full items-center justify-center">
      <Card className="w-full max-w-sm p-8 text-center">
        <div className="mx-auto mb-4 flex size-12 items-center justify-center rounded-2xl bg-indigo-50 text-indigo-600">
          <LockKeyhole className="size-6" />
        </div>
        <h2 className="text-lg font-semibold text-slate-900">Admin area locked</h2>
        <p className="mt-1.5 text-sm text-slate-500">
          Enter the admin PIN to manage employees, schedules and reports.
        </p>
        <form
          className="mt-6 space-y-3 text-left"
          onSubmit={(event) => {
            event.preventDefault();
            void submit();
          }}
        >
          <div className="relative">
            <KeyRound className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-slate-400" />
            <Input
              ref={inputRef}
              type="password"
              inputMode="numeric"
              pattern="[0-9]*"
              autoComplete="off"
              placeholder="Enter PIN"
              value={pin}
              onChange={(event) => setPin(event.target.value.replace(/\D/g, ""))}
              className="pl-9 text-center text-lg tracking-[0.3em]"
            />
          </div>
          {error && <p className="text-xs text-rose-600">{error}</p>}
          <Button type="submit" className="w-full" loading={checking} disabled={!pin}>
            Unlock
          </Button>
        </form>
      </Card>
    </div>
  );
}
