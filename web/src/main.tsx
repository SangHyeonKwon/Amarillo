import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter } from "react-router-dom";

import { ApiError } from "@/api/client";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import { App } from "./App";
import "./index.css";

function normalizeBasePath(path: string): string {
  const trimmed = path.trim();
  if (!trimmed || trimmed === "/") return "/";
  return `/${trimmed.replace(/^\/+|\/+$/g, "")}`;
}

const ROUTER_BASENAME = normalizeBasePath(
  import.meta.env.VITE_APP_BASE_PATH ?? "/",
);

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      retry: (failureCount, error) => {
        if (error instanceof ApiError && error.status < 500) return false;
        return failureCount < 2;
      },
      refetchOnWindowFocus: true,
      refetchOnReconnect: true,
    },
  },
});

const rootEl = document.getElementById("root");
if (!rootEl) throw new Error("#root element not found");

createRoot(rootEl).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <ErrorBoundary>
        <BrowserRouter basename={ROUTER_BASENAME}>
          <App />
        </BrowserRouter>
      </ErrorBoundary>
    </QueryClientProvider>
  </StrictMode>,
);
