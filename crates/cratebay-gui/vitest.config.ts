/// <reference types="vitest" />
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import { resolve } from "path";

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": resolve(__dirname, "./src"),
    },
  },
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.{ts,tsx}"],
    globals: true,
    setupFiles: ["./src/__tests__/setup.ts"],
    testTimeout: 10_000,
    coverage: {
      provider: "v8",
      reporter: ["text", "html", "lcov"],
      reportsDirectory: "./coverage",
      // Global thresholds — baseline for CI gate.
      // Aspirational targets from testing-spec.md: 75% statements, 70% branches.
      // These will be raised as more tests are added.
      thresholds: {
        statements: 30,
        branches: 60,
        functions: 40,
        lines: 30,
      },
      include: ["src/**/*.{ts,tsx}"],
      exclude: [
        "src/components/ui/**",       // shadcn auto-generated
        "src/**/*.test.{ts,tsx}",     // test files
        "src/__tests__/**",           // test directory
        "src/__mocks__/**",           // mock files
        "src/types/**",              // type definitions
        "src/vite-env.d.ts",         // vite env types
        "src/main.tsx",              // entry point
      ],
    },
  },
});
