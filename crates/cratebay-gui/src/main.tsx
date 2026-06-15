import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { LocalizedErrorBoundary } from "./components/ErrorBoundary";
import { createI18n } from "./lib/i18n";
import { useAppStore } from "./stores/appStore";
import { useSettingsStore } from "./stores/settingsStore";
import App from "./App";
import "./app.css";

function commonText(key: "operationFailed" | "applicationError") {
  return createI18n(useSettingsStore.getState().settings.language).t("common", key);
}

// ─── Suppress WebView default error overlay ─────────────────────────────
// Tauri's WebView (and some Chromium builds) inject inline error banners
// for unhandled exceptions. We aggressively remove them.
function removeErrorOverlays() {
  // Target the classic red/crimson error bars injected by WebView
  document.querySelectorAll("body > div").forEach((el) => {
    const style = (el as HTMLElement).style;
    const bg = style.backgroundColor || style.background || "";
    if (
      bg.includes("red") || bg.includes("rgb(255, 0, 0") ||
      bg.includes("#ff0000") || bg.includes("#FF0000") ||
      bg.includes("crimson")
    ) {
      el.remove();
    }
  });
}

// MutationObserver to catch dynamically injected error overlays
const observer = new MutationObserver((mutations) => {
  for (const mutation of mutations) {
    for (const node of mutation.addedNodes) {
      if (node instanceof HTMLElement && node.parentElement === document.body) {
        const style = node.style;
        const bg = (style.backgroundColor || style.background || "").toLowerCase();
        if (
          bg.includes("red") || bg.includes("rgb(255, 0, 0") ||
          bg.includes("#ff0000") || bg.includes("crimson")
        ) {
          node.remove();
        }
      }
    }
  }
});
observer.observe(document.body, { childList: true });

// ─── Global error handlers ──────────────────────────────────────────────
window.addEventListener("unhandledrejection", (event) => {
  event.preventDefault();
  event.stopImmediatePropagation();

  const message =
    event.reason instanceof Error
      ? event.reason.message
      : String(event.reason ?? "Unknown error");

  console.warn("[CrateBay] Unhandled rejection:", message);

  useAppStore.getState().addNotification({
    type: "error",
    title: commonText("operationFailed"),
    message,
    dismissable: true,
  });

  // Clean up any error overlays that may have been injected
  requestAnimationFrame(removeErrorOverlays);
}, true); // Use capture phase to intercept before WebView handler

window.addEventListener("error", (event) => {
  event.preventDefault();
  console.warn("[CrateBay] Uncaught error:", event.message);
  useAppStore.getState().addNotification({
    type: "error",
    title: commonText("applicationError"),
    message: event.message || "Unknown error",
    dismissable: true,
  });
  requestAnimationFrame(removeErrorOverlays);
}, true);

// ─── App mount ──────────────────────────────────────────────────────────
const rootElement = document.getElementById("root");

if (!rootElement) {
  throw new Error("Root element not found. Check index.html for <div id='root'>.");
}

createRoot(rootElement).render(
  <StrictMode>
    <LocalizedErrorBoundary>
      <App />
    </LocalizedErrorBoundary>
  </StrictMode>,
);
