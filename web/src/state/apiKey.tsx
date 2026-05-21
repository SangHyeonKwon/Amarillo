/**
 * Admin API key — session-memory store + module-helper sync (S18 / M006).
 *
 * Two coordinated halves:
 *
 * 1. **React Context** — `ApiKeyProvider` holds `apiKey: string | null` in
 *    `useState`. `useApiKey()` exposes the value plus `setApiKey(k)`. The
 *    state is *plain React memory* — no localStorage / sessionStorage / cookie
 *    (D024 — XSS surface minimized; refresh clears the key on purpose, that's
 *    the intended operational safety signal).
 *
 * 2. **Module-level mirror** — `setAdminApiKey()` in `@/api/client` holds a
 *    mutable `_apiKey` slot that `apiPost` / `apiDelete` read on every write
 *    call. The Provider syncs the slot whenever the state changes
 *    (`useEffect`), so the existing helper API stays untouched. D024 A-pattern
 *    — zero call-site changes in `hooks.ts` or pages.
 *
 * Refresh = lose key = re-enter. Don't paper over that — it's the deal.
 */
import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";

import { setAdminApiKey } from "@/api/client";

interface ApiKeyContextValue {
  /** Current key, or `null` if none has been applied this session. */
  apiKey: string | null;
  /**
   * Replace the active key. Pass `null` to clear (e.g. on a "Clear" button).
   * Empty / whitespace-only inputs are clamped to `null` to keep callers from
   * accidentally enabling write buttons with an effectively empty key.
   */
  setApiKey: (k: string | null) => void;
}

const ApiKeyContext = createContext<ApiKeyContextValue | null>(null);

/**
 * Provider — wrap the app (or a sub-tree) once. Children may then call
 * {@link useApiKey} to read or change the session key.
 */
export function ApiKeyProvider({ children }: { children: ReactNode }) {
  const [apiKey, setApiKeyState] = useState<string | null>(null);

  const setApiKey = useCallback((k: string | null) => {
    if (k == null) {
      setApiKeyState(null);
      return;
    }
    const trimmed = k.trim();
    setApiKeyState(trimmed === "" ? null : trimmed);
  }, []);

  // Mirror the React state into the api/client module slot — that's how
  // existing `apiPost` / `apiDelete` helpers learn about the new key without
  // every caller threading an option through (D024 / A-pattern).
  useEffect(() => {
    setAdminApiKey(apiKey);
    // On unmount, drop the key from the module slot too — guards against a
    // page navigation leaving the slot live after the provider is gone.
    return () => {
      setAdminApiKey(null);
    };
  }, [apiKey]);

  const value = useMemo<ApiKeyContextValue>(
    () => ({ apiKey, setApiKey }),
    [apiKey, setApiKey],
  );

  return <ApiKeyContext.Provider value={value}>{children}</ApiKeyContext.Provider>;
}

/**
 * Read the current admin API key and a setter from context. Throws if used
 * outside an `ApiKeyProvider` — the error is a programmer mistake, not a
 * runtime user-input issue.
 */
export function useApiKey(): ApiKeyContextValue {
  const ctx = useContext(ApiKeyContext);
  if (ctx == null) {
    throw new Error("useApiKey() called outside <ApiKeyProvider>");
  }
  return ctx;
}
