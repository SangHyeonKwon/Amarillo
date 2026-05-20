import type { ReactNode } from "react";

interface KpiCardProps {
  label: string;
  value: ReactNode;
  hint?: ReactNode;
  icon?: string;
  loading?: boolean;
}

/** Single headline metric tile used on the Overview grid. */
export function KpiCard({ label, value, hint, icon, loading }: KpiCardProps) {
  return (
    <div className="kpi">
      <div className="kpi-label">
        {icon && <span aria-hidden>{icon}</span>}
        {label}
      </div>
      {loading ? (
        <div className="skeleton" style={{ height: 28, marginTop: 12, width: "60%" }} />
      ) : (
        <div className="kpi-value">{value}</div>
      )}
      {hint && !loading && <div className="kpi-hint">{hint}</div>}
    </div>
  );
}
