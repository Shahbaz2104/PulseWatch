import { useEffect, useRef } from "react";
import { useForm, type Resolver } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Form,
  FormControl,
  FormDescription,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from "@/components/ui/form";
import { Input } from "@/components/ui/input";
import { ApiError } from "@/lib/api";
import {
  monitorDefaults,
  monitorSchema,
  type MonitorFormValues,
} from "@/features/monitors/monitor-schema";
import { useCreateMonitor, useSettings, useUpdateMonitor } from "@/hooks/use-api";
import type { MonitorSummary } from "@/types/api";

interface MonitorFormDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** When set, the dialog edits this monitor instead of creating one. */
  editing?: MonitorSummary | null;
}

function applyServerFieldErrors(
  error: unknown,
  setError: <K extends keyof MonitorFormValues>(
    name: K,
    error: { message: string },
  ) => void,
): boolean {
  if (!(error instanceof ApiError)) return false;
  let mapped = false;
  for (const detail of error.details) {
    if (detail.field in monitorDefaults) {
      setError(detail.field as keyof MonitorFormValues, { message: detail.message });
      mapped = true;
    }
  }
  return mapped;
}

export function MonitorFormDialog({ open, onOpenChange, editing }: MonitorFormDialogProps) {
  const isEdit = editing != null;
  const { data: settings } = useSettings();
  const create = useCreateMonitor();
  const update = useUpdateMonitor(editing?.id ?? "");

  const form = useForm<MonitorFormValues>({
    resolver: zodResolver(monitorSchema) as Resolver<MonitorFormValues>,
    defaultValues: monitorDefaults,
  });

  // Seed once per dialog open. The previous effect listed the mutation
  // objects (`create`/`update`) and `form` in its deps — those get new
  // identities on every render, so the effect re-ran continuously and
  // `form.reset()` wiped whatever the user had typed (and could lock the
  // tab in a reset → re-render loop). Latest-ref pattern keeps the
  // callbacks fresh without re-triggering the seed.
  const formRef = useRef(form);
  const createRef = useRef(create);
  const updateRef = useRef(update);
  useEffect(() => {
    formRef.current = form;
    createRef.current = create;
    updateRef.current = update;
  });
  const seededFor = useRef<string | null>(null);

  useEffect(() => {
    if (!open) {
      seededFor.current = null;
      return;
    }
    // For create, wait until settings have loaded so defaults come from the API.
    if (!editing && !settings) return;
    const seedKey = editing?.id ?? "create";
    if (seededFor.current === seedKey) return;
    seededFor.current = seedKey;

    createRef.current.reset();
    updateRef.current.reset();
    if (editing) {
      formRef.current.reset({
        name: editing.name,
        url: editing.url,
        check_interval_seconds: editing.check_interval_seconds,
        timeout_ms: editing.timeout_ms,
        retry_max_attempts: editing.retry_max_attempts,
        expected_status_min: editing.expected_status_min,
        expected_status_max: editing.expected_status_max,
        consecutive_failures_threshold: editing.consecutive_failures_threshold,
        consecutive_successes_threshold: editing.consecutive_successes_threshold,
      });
    } else if (settings) {
      formRef.current.reset({
        ...monitorDefaults,
        check_interval_seconds: settings.default_check_interval_seconds,
        timeout_ms: settings.default_timeout_ms,
        retry_max_attempts: settings.default_retry_max_attempts,
        expected_status_min: settings.default_expected_status_min,
        expected_status_max: settings.default_expected_status_max,
      });
    }
  }, [open, editing, settings]);

  const onSubmit = (values: MonitorFormValues) => {
    const trimmedName = (values.name ?? "").trim();
    const payload = {
      ...values,
      name: trimmedName === "" ? undefined : trimmedName,
    };
    const mutation = isEdit ? update : create;
    mutation.mutate(payload, {
      onSuccess: () => {
        toast.success(isEdit ? "Monitor updated" : "Monitor created", {
          description: isEdit
            ? undefined
            : "The first check runs within a few seconds.",
        });
        onOpenChange(false);
      },
      onError: (error) => {
        if (!applyServerFieldErrors(error, form.setError)) {
          toast.error(error instanceof ApiError ? error.message : "Something went wrong");
        }
      },
    });
  };

  const pending = create.isPending || update.isPending;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[90svh] overflow-y-auto sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{isEdit ? "Edit monitor" : "Add monitor"}</DialogTitle>
          <DialogDescription>
            {isEdit
              ? "Changes apply to future checks."
              : "PulseWatch begins checking as soon as the monitor is created."}
          </DialogDescription>
        </DialogHeader>
        <Form {...form}>
          <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-4">
            <FormField
              control={form.control}
              name="url"
              render={({ field }) => (
                <FormItem>
                  <FormLabel>URL</FormLabel>
                  <FormControl>
                    <Input
                      placeholder="https://example.com/health"
                      spellCheck={false}
                      autoComplete="off"
                      {...field}
                    />
                  </FormControl>
                  <FormMessage />
                </FormItem>
              )}
            />
            <FormField
              control={form.control}
              name="name"
              render={({ field }) => (
                <FormItem>
                  <FormLabel>Name</FormLabel>
                  <FormControl>
                    <Input placeholder="Optional — defaults to the hostname" {...field} />
                  </FormControl>
                  <FormMessage />
                </FormItem>
              )}
            />
            <div className="grid grid-cols-2 gap-4">
              <FormField
                control={form.control}
                name="check_interval_seconds"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>Interval (s)</FormLabel>
                    <FormControl>
                      <Input type="number" min={10} {...field} />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />
              <FormField
                control={form.control}
                name="timeout_ms"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>Timeout (ms)</FormLabel>
                    <FormControl>
                      <Input type="number" min={100} {...field} />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />
              <FormField
                control={form.control}
                name="retry_max_attempts"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>Attempts</FormLabel>
                    <FormControl>
                      <Input type="number" min={1} max={5} {...field} />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />
              <FormField
                control={form.control}
                name="consecutive_failures_threshold"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>Fail → down after</FormLabel>
                    <FormControl>
                      <Input type="number" min={1} {...field} />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />
              <FormField
                control={form.control}
                name="expected_status_min"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>Status min</FormLabel>
                    <FormControl>
                      <Input type="number" min={100} max={599} {...field} />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />
              <FormField
                control={form.control}
                name="expected_status_max"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>Status max</FormLabel>
                    <FormControl>
                      <Input type="number" min={100} max={599} {...field} />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />
              <FormField
                control={form.control}
                name="consecutive_successes_threshold"
                render={({ field }) => (
                  <FormItem className="col-span-2">
                    <FormLabel>Recover → up after</FormLabel>
                    <FormControl>
                      <Input type="number" min={1} {...field} />
                    </FormControl>
                    <FormDescription>
                      Consecutive successful checks before an incident resolves.
                    </FormDescription>
                    <FormMessage />
                  </FormItem>
                )}
              />
            </div>
            <DialogFooter>
              <Button
                type="button"
                variant="outline"
                onClick={() => onOpenChange(false)}
                disabled={pending}
              >
                Cancel
              </Button>
              <Button type="submit" disabled={pending}>
                {pending ? "Saving…" : isEdit ? "Save changes" : "Create monitor"}
              </Button>
            </DialogFooter>
          </form>
        </Form>
      </DialogContent>
    </Dialog>
  );
}
