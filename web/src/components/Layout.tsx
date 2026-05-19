import { NavLink, Outlet } from "react-router-dom";

import { useLatestBlock } from "@/api/hooks";
import { formatNumber } from "@/lib/format";

const NAV = [
  { to: "/", label: "Overview", icon: "01", end: true },
  { to: "/pools", label: "Pools", icon: "[]" },
  { to: "/failed-tx", label: "Failed Tx", icon: "!" },
  { to: "/traders", label: "Top Traders", icon: "$" },
];

/** App chrome: sidebar nav + sticky topbar with live API status. */
export function Layout() {
  const latest = useLatestBlock();

  const online = latest.isSuccess;
  const statusText = latest.isLoading
    ? "Connecting…"
    : online
      ? latest.data != null
        ? `Block ${formatNumber(latest.data)}`
        : "Connected · no blocks indexed"
      : "API unreachable";

  return (
    <div className="app-shell">
      <a href="#main-content" className="skip-link">
        Skip to content
      </a>
      <aside className="sidebar">
        <div className="brand">
          <img
            className="brand-logo"
            src="/logo-mark-01.svg"
            alt=""
            aria-hidden="true"
          />
          <div className="brand-name">defi-tx-indexer</div>
        </div>

        <nav className="nav" aria-label="Primary">
          <div className="nav-section">Dashboard</div>
          {NAV.map((item) => (
            <NavLink
              key={item.to}
              to={item.to}
              end={item.end}
              className={({ isActive }) =>
                isActive ? "nav-link active" : "nav-link"
              }
            >
              <span className="nav-icon" aria-hidden>
                {item.icon}
              </span>
              {item.label}
            </NavLink>
          ))}
        </nav>
      </aside>

      <div className="main">
        <header className="topbar">
          <div className="topbar-title">DeFi Analytics</div>
          <div className="topbar-meta">
            <span>{statusText}</span>
          </div>
        </header>
        <main id="main-content" className="content" tabIndex={-1}>
          <Outlet />
        </main>
      </div>
    </div>
  );
}
