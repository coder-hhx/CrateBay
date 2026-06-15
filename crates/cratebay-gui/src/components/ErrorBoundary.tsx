import { Component, type ErrorInfo, type ReactNode } from "react";
import { createI18n, useI18n } from "@/lib/i18n";

interface ErrorBoundaryText {
  title: string;
  description: string;
  details: string;
  reload: string;
}

interface ErrorBoundaryProps {
  children: ReactNode;
  fallback?: ReactNode;
  text?: ErrorBoundaryText;
}

interface ErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
  errorInfo: ErrorInfo | null;
}

/**
 * Global ErrorBoundary that catches React rendering errors.
 * Displays a user-friendly error message instead of a white screen.
 */
export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false, error: null, errorInfo: null };
  }

  static getDerivedStateFromError(error: Error): Partial<ErrorBoundaryState> {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error("[CrateBay] React error caught by ErrorBoundary:", error, errorInfo);
    this.setState({ errorInfo });
  }

  render() {
    if (this.state.hasError) {
      if (this.props.fallback) {
        return this.props.fallback;
      }

      const text = this.props.text ?? defaultErrorBoundaryText;

      return (
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            justifyContent: "center",
            height: "100vh",
            width: "100vw",
            backgroundColor: "#0a0a0b",
            color: "#e5e5e5",
            fontFamily: "system-ui, -apple-system, sans-serif",
            padding: "2rem",
          }}
        >
          <div
            style={{
              maxWidth: "600px",
              width: "100%",
              textAlign: "center",
            }}
          >
            <h1 style={{ fontSize: "1.5rem", fontWeight: 600, marginBottom: "0.5rem" }}>
              {text.title}
            </h1>
            <p style={{ color: "#a1a1aa", marginBottom: "1.5rem" }}>
              {text.description}
            </p>

            {this.state.error && (
              <details
                style={{
                  textAlign: "left",
                  backgroundColor: "#18181b",
                  border: "1px solid #27272a",
                  borderRadius: "0.5rem",
                  padding: "1rem",
                  marginBottom: "1rem",
                }}
              >
                <summary style={{ cursor: "pointer", color: "#ef4444", fontWeight: 500 }}>
                  {text.details}
                </summary>
                <pre
                  style={{
                    marginTop: "0.5rem",
                    fontSize: "0.75rem",
                    color: "#a1a1aa",
                    whiteSpace: "pre-wrap",
                    wordBreak: "break-word",
                    overflow: "auto",
                    maxHeight: "300px",
                  }}
                >
                  {this.state.error.toString()}
                  {this.state.errorInfo?.componentStack}
                </pre>
              </details>
            )}

            <button
              onClick={() => window.location.reload()}
              style={{
                padding: "0.5rem 1.5rem",
                backgroundColor: "#6366f1",
                color: "#fff",
                border: "none",
                borderRadius: "0.375rem",
                cursor: "pointer",
                fontSize: "0.875rem",
                fontWeight: 500,
              }}
            >
              {text.reload}
            </button>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}

const defaultCommonText = createI18n("en").t("common");
const defaultErrorBoundaryText: ErrorBoundaryText = {
  title: defaultCommonText.unexpectedErrorTitle,
  description: defaultCommonText.unexpectedErrorDesc,
  details: defaultCommonText.errorDetails,
  reload: defaultCommonText.reloadApplication,
};

export function LocalizedErrorBoundary({
  children,
  fallback,
}: Omit<ErrorBoundaryProps, "text">) {
  const { t } = useI18n();

  return (
    <ErrorBoundary
      fallback={fallback}
      text={{
        title: t("common", "unexpectedErrorTitle"),
        description: t("common", "unexpectedErrorDesc"),
        details: t("common", "errorDetails"),
        reload: t("common", "reloadApplication"),
      }}
    >
      {children}
    </ErrorBoundary>
  );
}
