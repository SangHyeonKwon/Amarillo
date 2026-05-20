import { defineConfig, loadEnv } from "vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath, URL } from "node:url";

function normalizeBasePath(path: string): string {
  const trimmed = path.trim();
  if (!trimmed || trimmed === "/") return "/";
  return `/${trimmed.replace(/^\/+|\/+$/g, "")}/`;
}

// https://vite.dev/config/
export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), "");
  const base = normalizeBasePath(env.VITE_APP_BASE_PATH ?? "/");

  return {
    base,
    plugins: [react()],
    resolve: {
      alias: {
        "@": fileURLToPath(new URL("./src", import.meta.url)),
      },
    },
    server: {
      port: 5173,
    },
    build: {
      rollupOptions: {
        output: {
          // Split heavy vendors so the charting lib doesn't bloat the entry
          // chunk and stays cacheable across deploys.
          manualChunks: {
            charts: ["recharts"],
            vendor: [
              "react",
              "react-dom",
              "react-router-dom",
              "@tanstack/react-query",
            ],
          },
        },
      },
    },
    test: {
      environment: "jsdom",
      setupFiles: "./src/test/setup.ts",
      css: true,
      include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
    },
  };
});
