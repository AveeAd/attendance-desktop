import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { HashRouter, Route, Routes } from "react-router-dom";
import { AppShell } from "@/components/AppShell";
import { PinGate } from "@/components/PinGate";
import { Dashboard } from "@/pages/Dashboard";
import { Employees } from "@/pages/Employees";
import { Schedule } from "@/pages/Schedule";
import { Reports } from "@/pages/Reports";
import { Settings } from "@/pages/Settings";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      refetchOnWindowFocus: false,
      staleTime: 10_000,
    },
  },
});

function Admin({ children }: { children: React.ReactNode }) {
  return <PinGate>{children}</PinGate>;
}

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <HashRouter>
        <Routes>
          <Route element={<AppShell />}>
            <Route index element={<Dashboard />} />
            <Route path="employees" element={<Admin><Employees /></Admin>} />
            <Route path="schedule" element={<Admin><Schedule /></Admin>} />
            <Route path="reports" element={<Admin><Reports /></Admin>} />
            <Route path="settings" element={<Admin><Settings /></Admin>} />
          </Route>
        </Routes>
      </HashRouter>
    </QueryClientProvider>
  );
}
