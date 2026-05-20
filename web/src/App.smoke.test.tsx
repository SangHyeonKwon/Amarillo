import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

import { App } from "@/App";

vi.mock("@/api/hooks", () => ({
  useLatestBlock: () => ({
    isSuccess: true,
    isLoading: false,
    data: 123,
  }),
}));

vi.mock("@/pages/Overview", () => ({ Overview: () => <h1>overview-page</h1> }));
vi.mock("@/pages/Pools", () => ({ Pools: () => <h1>pools-page</h1> }));
vi.mock("@/pages/PoolDetail", () => ({
  PoolDetail: () => <h1>pool-detail-page</h1>,
}));
vi.mock("@/pages/FailedTx", () => ({ FailedTx: () => <h1>failedtx-page</h1> }));
vi.mock("@/pages/Traders", () => ({ Traders: () => <h1>traders-page</h1> }));
vi.mock("@/pages/Alerts", () => ({ Alerts: () => <h1>alerts-page</h1> }));

describe("App routes", () => {
  it("renders failed-tx page route", () => {
    render(
      <MemoryRouter initialEntries={["/failed-tx"]}>
        <App />
      </MemoryRouter>,
    );

    expect(screen.getByRole("heading", { name: "failedtx-page" })).toBeInTheDocument();
  });

  it("renders alerts page route", () => {
    render(
      <MemoryRouter initialEntries={["/alerts"]}>
        <App />
      </MemoryRouter>,
    );

    expect(screen.getByRole("heading", { name: "alerts-page" })).toBeInTheDocument();
  });

  it("redirects unknown routes to overview", () => {
    render(
      <MemoryRouter initialEntries={["/unknown"]}>
        <App />
      </MemoryRouter>,
    );

    expect(screen.getByRole("heading", { name: "overview-page" })).toBeInTheDocument();
  });
});
