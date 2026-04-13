import { useCallback, useState } from "react";
import {
  useSettingsStore,
  normalizeProvider,
  type LlmProviderCreateRequest,
  type ApiFormat,
} from "@/stores/settingsStore";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Eye, EyeOff, Plus } from "lucide-react";

const API_FORMAT_OPTIONS: { value: ApiFormat; label: string }[] = [
  { value: "anthropic", label: "Anthropic Messages" },
  { value: "openai_responses", label: "OpenAI Responses" },
  { value: "openai_completions", label: "OpenAI Chat Completions" },
];

export function ProviderForm() {
  const createProvider = useSettingsStore((s) => s.createProvider);
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [apiBase, setApiBase] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [apiFormat, setApiFormat] = useState<ApiFormat>("openai_completions");
  const [showKey, setShowKey] = useState(false);
  const [saving, setSaving] = useState(false);
  const [formatAutoDetected, setFormatAutoDetected] = useState(false);

  const resetForm = useCallback(() => {
    setName("");
    setApiBase("");
    setApiKey("");
    setApiFormat("openai_completions");
    setShowKey(false);
    setFormatAutoDetected(false);
  }, []);

  const canSave = name.trim().length > 0 && apiBase.trim().length > 0;

  const handleBaseUrlBlur = useCallback(() => {
    if (apiBase.trim().length === 0) return;
    const normalized = normalizeProvider(apiBase);
    if (normalized.baseUrl !== apiBase.trim()) {
      setApiBase(normalized.baseUrl);
    }
    // Auto-detect format from URL suffix
    if (normalized.apiFormat !== apiFormat) {
      setApiFormat(normalized.apiFormat);
      setFormatAutoDetected(true);
    }
  }, [apiBase, apiFormat]);

  const handleSave = useCallback(async () => {
    if (!canSave) return;
    setSaving(true);
    try {
      const normalized = normalizeProvider(apiBase);
      const request: LlmProviderCreateRequest = {
        name: name.trim(),
        apiBase: normalized.baseUrl,
        apiKey,
        apiFormat: apiFormat,
      };
      await createProvider(request);
      resetForm();
      setOpen(false);
    } finally {
      setSaving(false);
    }
  }, [canSave, name, apiBase, apiKey, apiFormat, createProvider, resetForm]);

  return (
    <Dialog open={open} onOpenChange={(v) => { setOpen(v); if (!v) resetForm(); }}>
      <DialogTrigger asChild>
        <Button size="sm">
          <Plus className="h-4 w-4" />
          Add Provider
        </Button>
      </DialogTrigger>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Add LLM Provider</DialogTitle>
          <DialogDescription>
            Configure a new LLM provider. API keys are encrypted and stored securely on the backend.
          </DialogDescription>
        </DialogHeader>

        <div className="flex flex-col gap-4 py-4">
          {/* Provider Name */}
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="provider-name">Provider Name</Label>
            <Input
              id="provider-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g., OpenAI, Anthropic, DeepSeek"
            />
          </div>

          {/* Base URL */}
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="provider-url">Base URL</Label>
            <Input
              id="provider-url"
              value={apiBase}
              onChange={(e) => setApiBase(e.target.value)}
              onBlur={handleBaseUrlBlur}
              placeholder="https://api.openai.com"
            />
            {formatAutoDetected && (
              <p className="text-[10px] text-muted-foreground">
                API format auto-detected from URL
              </p>
            )}
          </div>

          {/* API Key */}
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="provider-key">API Key</Label>
            <div className="relative">
              <Input
                id="provider-key"
                type={showKey ? "text" : "password"}
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
                placeholder="sk-..."
                className="pr-10"
              />
              <Button
                type="button"
                variant="ghost"
                size="icon-xs"
                className="absolute right-2 top-1/2 -translate-y-1/2"
                onClick={() => setShowKey(!showKey)}
                aria-label={showKey ? "Hide API key" : "Show API key"}
              >
                {showKey ? (
                  <EyeOff className="h-3.5 w-3.5" />
                ) : (
                  <Eye className="h-3.5 w-3.5" />
                )}
              </Button>
            </div>
          </div>

          {/* API Format */}
          <div className="flex flex-col gap-1.5">
            <Label>API Format</Label>
            <Select
              value={apiFormat}
              onValueChange={(v) => {
                setApiFormat(v as ApiFormat);
                setFormatAutoDetected(false);
              }}
            >
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {API_FORMAT_OPTIONS.map((opt) => (
                  <SelectItem key={opt.value} value={opt.value}>
                    {opt.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => setOpen(false)}>
            Cancel
          </Button>
          <Button onClick={handleSave} disabled={!canSave || saving}>
            {saving ? "Saving..." : "Save"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
