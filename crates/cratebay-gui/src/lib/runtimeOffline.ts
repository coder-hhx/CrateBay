export function formatTauriError(err: unknown, fallback = ""): string {
  if (err instanceof Error) return err.message;
  if (typeof err === "string") return err;
  if (err === null || err === undefined) return fallback;
  return String(err);
}

export function isImplicitRuntimeStartDisabled(err: unknown): boolean {
  const message = formatTauriError(err, "");
  return (
    message.includes("CRATEBAY_DISABLE_RUNTIME_AUTO_START") ||
    message.includes("Implicit runtime start disabled")
  );
}
