import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import {
  CheckCircle2,
  Eye,
  Loader2,
  MoreHorizontal,
  Pause,
  Pencil,
  Play,
  Trash2,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { ApiError } from "@/lib/api";
import { useCheckNow, useDeleteMonitor, usePauseMonitor, useResumeMonitor } from "@/hooks/use-api";
import type { MonitorSummary } from "@/types/api";

interface MonitorActionsProps {
  monitor: MonitorSummary;
  onEdit: () => void;
}

export function MonitorActions({ monitor, onEdit }: MonitorActionsProps) {
  const navigate = useNavigate();
  const [confirmDelete, setConfirmDelete] = useState(false);
  const checkNow = useCheckNow();
  const pause = usePauseMonitor();
  const resume = useResumeMonitor();
  const remove = useDeleteMonitor();

  const paused = monitor.current_status === "paused";
  const actionPending = checkNow.isPending || pause.isPending || resume.isPending;

  const reportError = (error: unknown) => {
    toast.error(error instanceof ApiError ? error.message : "Action failed");
  };

  const runCheck = () => {
    checkNow.mutate(monitor.id, {
      onSuccess: (result) => {
        toast.success(result.check.success ? "Check succeeded" : "Check failed", {
          description: result.check.success
            ? `HTTP ${result.check.http_status_code} · ${result.check.response_time_ms} ms`
            : (result.check.error_message ?? "See check history for details"),
        });
      },
      onError: reportError,
    });
  };

  return (
    <>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button
            variant="ghost"
            size="icon"
            className="size-8"
            aria-label={`Actions for ${monitor.name}`}
            disabled={actionPending}
          >
            {actionPending ? (
              <Loader2 className="size-4 animate-spin" />
            ) : (
              <MoreHorizontal className="size-4" />
            )}
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-44">
          <DropdownMenuItem onSelect={() => navigate(`/monitors/${monitor.id}`)}>
            <Eye /> View details
          </DropdownMenuItem>
          <DropdownMenuItem onSelect={onEdit}>
            <Pencil /> Edit
          </DropdownMenuItem>
          <DropdownMenuItem onSelect={runCheck} disabled={paused}>
            <CheckCircle2 /> Check now
          </DropdownMenuItem>
          {paused ? (
            <DropdownMenuItem
              onSelect={() =>
                resume.mutate(monitor.id, {
                  onSuccess: () => toast.success(`Resumed ${monitor.name}`),
                  onError: reportError,
                })
              }
            >
              <Play /> Resume
            </DropdownMenuItem>
          ) : (
            <DropdownMenuItem
              onSelect={() =>
                pause.mutate(monitor.id, {
                  onSuccess: () => toast.success(`Paused ${monitor.name}`),
                  onError: reportError,
                })
              }
            >
              <Pause /> Pause
            </DropdownMenuItem>
          )}
          <DropdownMenuSeparator />
          <DropdownMenuItem
            variant="destructive"
            onSelect={() => setConfirmDelete(true)}
          >
            <Trash2 /> Delete
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>

      <AlertDialog open={confirmDelete} onOpenChange={setConfirmDelete}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete “{monitor.name}”?</AlertDialogTitle>
            <AlertDialogDescription>
              This permanently removes the monitor along with its check history and incidents.
              This cannot be undone.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
              onClick={() =>
                remove.mutate(monitor.id, {
                  onSuccess: () => toast.success(`Deleted ${monitor.name}`),
                  onError: reportError,
                })
              }
            >
              Delete monitor
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}
