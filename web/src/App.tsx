import { Navigate, Route, Routes } from "react-router-dom";

import { Layout } from "@/components/Layout";
import { Alerts } from "@/pages/Alerts";
import { Overview } from "@/pages/Overview";
import { Pools } from "@/pages/Pools";
import { PoolDetail } from "@/pages/PoolDetail";
import { FailedTx } from "@/pages/FailedTx";
import { Traders } from "@/pages/Traders";
import { ApiKeyProvider } from "@/state/apiKey";

export function App() {
  // ApiKeyProvider wraps all routes so the `/alerts` page can read/write the
  // session-memory admin key, and the @/api/client module slot stays in sync
  // for the writes it makes on the user's behalf (S18 / M006 / D024).
  return (
    <ApiKeyProvider>
      <Routes>
        <Route element={<Layout />}>
          <Route index element={<Overview />} />
          <Route path="pools" element={<Pools />} />
          <Route path="pools/:address" element={<PoolDetail />} />
          <Route path="failed-tx" element={<FailedTx />} />
          <Route path="traders" element={<Traders />} />
          <Route path="alerts" element={<Alerts />} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Route>
      </Routes>
    </ApiKeyProvider>
  );
}
