import { Navigate, Route, Routes } from "react-router-dom";

import { Layout } from "@/components/Layout";
import { Overview } from "@/pages/Overview";
import { Pools } from "@/pages/Pools";
import { PoolDetail } from "@/pages/PoolDetail";
import { FailedTx } from "@/pages/FailedTx";
import { Traders } from "@/pages/Traders";

export function App() {
  return (
    <Routes>
      <Route element={<Layout />}>
        <Route index element={<Overview />} />
        <Route path="pools" element={<Pools />} />
        <Route path="pools/:address" element={<PoolDetail />} />
        <Route path="failed-tx" element={<FailedTx />} />
        <Route path="traders" element={<Traders />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Route>
    </Routes>
  );
}
