import { useState } from "react";
import { Loader2, Trash2 } from "lucide-react";
import { useI18n } from "@/lib/i18n";
import { useContainerStore, type ContainerInfo } from "@/stores/containerStore";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

type ContainerDeleteDialogProps = {
  container: ContainerInfo | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onDeleted?: () => void;
};

export function ContainerDeleteDialog({
  container,
  open,
  onOpenChange,
  onDeleted,
}: ContainerDeleteDialogProps) {
  const { t } = useI18n();
  const deleteContainer = useContainerStore((s) => s.deleteContainer);
  const [deleting, setDeleting] = useState(false);
  const [forceDelete, setForceDelete] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleDelete = async () => {
    if (container === null || deleting) return;
    setDeleting(true);
    setError(null);
    try {
      await deleteContainer(container.id, forceDelete);
      onOpenChange(false);
      setForceDelete(false);
      onDeleted?.();
    } catch (err) {
      setError(formatDeleteError(err, t("containers", "deleteFailed")));
    } finally {
      setDeleting(false);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(nextOpen) => {
        if (!deleting) {
          if (!nextOpen) {
            setForceDelete(false);
            setError(null);
          }
          onOpenChange(nextOpen);
        }
      }}
    >
      <DialogContent className="sm:max-w-[420px]">
        <DialogHeader>
          <DialogTitle>{t("containers", "delete")}</DialogTitle>
          <DialogDescription>
            {t("containers", "confirmDelete").replace("{name}", container?.name ?? "")}
          </DialogDescription>
        </DialogHeader>
        {container !== null && (
          <div className="rounded-md border border-border bg-muted/40 px-3 py-2">
            <div className="truncate text-sm font-medium">{container.name}</div>
            <div className="mt-0.5 truncate font-mono text-xs text-muted-foreground">
              {container.image}
            </div>
            <label className="mt-3 flex items-center gap-2 text-xs text-muted-foreground">
              <Checkbox
                checked={forceDelete}
                onCheckedChange={(checked) => setForceDelete(checked === true)}
              />
              {t("containers", "forceDelete")}
            </label>
          </div>
        )}
        {error !== null && (
          <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
            {error}
          </div>
        )}
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)} disabled={deleting}>
            {t("common", "cancel")}
          </Button>
          <Button
            variant="destructive"
            onClick={() => void handleDelete()}
            disabled={deleting || container === null}
            data-testid="container-delete-confirm"
          >
            {deleting ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <Trash2 className="h-3.5 w-3.5" />
            )}
            {t("common", "delete")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function formatDeleteError(err: unknown, fallback: string): string {
  if (err instanceof Error) return err.message;
  if (typeof err === "string") return err;
  if (err === null || err === undefined) return fallback;
  try {
    return JSON.stringify(err);
  } catch {
    return String(err);
  }
}
