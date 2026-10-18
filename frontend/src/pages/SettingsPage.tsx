import { useEffect, useRef } from "react";
import { useForm, type Resolver } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { toast } from "sonner";
import {
  Warning as AlertTriangle,
  ArrowCounterClockwise as RotateCcw,
  SlidersHorizontal as Settings2,
} from "@phosphor-icons/react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
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
import { Skeleton } from "@/components/ui/skeleton";
import { EmptyState } from "@/components/empty-state";
import { PageHeader } from "@/components/page-header";
import { ApiError } from "@/lib/api";
import { useSettings, useUpdateSettings } from "@/hooks/use-api";

const settingsSchema = z
  .object({
    default_check_interval_seconds: z.coerce
      .number()
      .int("Must be a whole number of seconds")
      .min(10, "Minimum interval is 10 seconds")
      .max(86_400, "Maximum interval is 24 hours"),
    default_timeout_ms: z.coerce
      .number()
      .int("Must be whole milliseconds")
      .min(1_000, "Timeout must be at least 1000 ms")
      .max(60_000, "Timeout must be at most 60000 ms"),
    default_retry_max_attempts: z.coerce
      .number()
      .int("Must be a whole number")
      .min(1, "At least 1 attempt")
      .max(5, "At most 5 attempts"),
    default_expected_status_min: z.coerce
      .number()
      .int("Whole HTTP status")
      .min(100, "Minimum status is 100")
      .max(599, "Maximum status is 599"),
    default_expected_status_max: z.coerce
      .number()
      .int("Whole HTTP status")
      .min(100, "Minimum status is 100")
      .max(599, "Maximum status is 599"),
    max_concurrent_checks: z.coerce
      .number()
      .int("Must be a whole number")
      .min(1, "At least 1 concurrent check")
      .max(200, "At most 200 concurrent checks"),
  })
  .refine((v) => v.default_expected_status_min <= v.default_expected_status_max, {
    message: "Range start must be ≤ range end",
    path: ["default_expected_status_max"],
  });

type SettingsFormValues = z.output<typeof settingsSchema>;

const DEFAULTS: SettingsFormValues = {
  default_check_interval_seconds: 30,
  default_timeout_ms: 10_000,
  default_retry_max_attempts: 3,
  default_expected_status_min: 200,
  default_expected_status_max: 399,
  max_concurrent_checks: 8,
};

export function SettingsPage() {
  const settingsQuery = useSettings();
  const update = useUpdateSettings();

  const form = useForm<SettingsFormValues>({
    resolver: zodResolver(settingsSchema) as Resolver<SettingsFormValues>,
    defaultValues: DEFAULTS,
  });

  // Seed from the server once per data load. `form` gets a new identity each
  // render, so listing it as an effect dep would re-run `form.reset()` forever
  // and wipe whatever the user was typing.
  const formRef = useRef(form);
  useEffect(() => {
    formRef.current = form;
  });
  const seededFrom = useRef<SettingsFormValues | null>(null);

  useEffect(() => {
    const data = settingsQuery.data;
    if (!data || seededFrom.current === data) return;
    seededFrom.current = data;
    formRef.current.reset({
      default_check_interval_seconds: data.default_check_interval_seconds,
      default_timeout_ms: data.default_timeout_ms,
      default_retry_max_attempts: data.default_retry_max_attempts,
      default_expected_status_min: data.default_expected_status_min,
      default_expected_status_max: data.default_expected_status_max,
      max_concurrent_checks: data.max_concurrent_checks,
    });
  }, [settingsQuery.data]);

  const onSubmit = (values: SettingsFormValues) => {
    update.mutate(values, {
      onSuccess: (data) => {
        toast.success("Settings saved", {
          description:
            data.restart_required.length > 0
              ? "Max concurrent checks takes effect after a backend restart."
              : "New monitors will use these defaults immediately.",
        });
      },
      onError: (error) => {
        if (error instanceof ApiError) {
          const fieldError = error.details?.[0];
          if (fieldError?.field) {
            form.setError(fieldError.field as keyof SettingsFormValues, {
              message: fieldError.message,
            });
          }
          toast.error(error.message);
        } else {
          toast.error("Couldn't save settings");
        }
      },
    });
  };

  if (settingsQuery.isLoading) {
    return (
      <div className="space-y-4">
        <Skeleton className="h-10 w-64" />
        <Skeleton className="h-80" />
      </div>
    );
  }

  if (settingsQuery.isError) {
    return (
      <EmptyState
        icon={Settings2}
        title="Couldn't load settings"
        description="The API didn't respond."
        action={{ label: "Retry", onClick: () => void settingsQuery.refetch() }}
      />
    );
  }

  return (
    <>
      <PageHeader
        title="Settings"
        description="Defaults applied to newly created monitors. Existing monitors keep their own overrides."
      />

      <Alert className="mb-6">
        <AlertTriangle className="size-4" />
        <AlertTitle>Restart to apply concurrency changes</AlertTitle>
        <AlertDescription>
          Changing <span className="font-mono text-xs">Max concurrent checks</span> requires a
          backend restart — the scheduler reads it once at boot.
        </AlertDescription>
      </Alert>

      <Card className="max-w-2xl">
        <CardHeader>
          <CardTitle>Check defaults</CardTitle>
          <CardDescription>
            Inherited by monitors at creation; edit a monitor to override per-URL.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <Form {...form}>
            <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-5" noValidate>
              <div className="grid gap-4 sm:grid-cols-2">
                <FormField
                  control={form.control}
                  name="default_check_interval_seconds"
                  render={({ field }) => (
                    <FormItem>
                      <FormLabel>Check interval (seconds)</FormLabel>
                      <FormControl>
                        <Input type="number" inputMode="numeric" className="font-mono" {...field} />
                      </FormControl>
                      <FormDescription>10 – 86400</FormDescription>
                      <FormMessage />
                    </FormItem>
                  )}
                />
                <FormField
                  control={form.control}
                  name="default_timeout_ms"
                  render={({ field }) => (
                    <FormItem>
                      <FormLabel>Timeout (milliseconds)</FormLabel>
                      <FormControl>
                        <Input type="number" inputMode="numeric" className="font-mono" {...field} />
                      </FormControl>
                      <FormDescription>1000 – 60000</FormDescription>
                      <FormMessage />
                    </FormItem>
                  )}
                />
                <FormField
                  control={form.control}
                  name="default_retry_max_attempts"
                  render={({ field }) => (
                    <FormItem>
                      <FormLabel>Max attempts</FormLabel>
                      <FormControl>
                        <Input type="number" inputMode="numeric" className="font-mono" {...field} />
                      </FormControl>
                      <FormDescription>Retries before a failed check</FormDescription>
                      <FormMessage />
                    </FormItem>
                  )}
                />
                <FormField
                  control={form.control}
                  name="max_concurrent_checks"
                  render={({ field }) => (
                    <FormItem>
                      <FormLabel>Max concurrent checks</FormLabel>
                      <FormControl>
                        <Input type="number" inputMode="numeric" className="font-mono" {...field} />
                      </FormControl>
                      <FormDescription>1 – 200 · restart required</FormDescription>
                      <FormMessage />
                    </FormItem>
                  )}
                />
                <FormField
                  control={form.control}
                  name="default_expected_status_min"
                  render={({ field }) => (
                    <FormItem>
                      <FormLabel>Expected status min</FormLabel>
                      <FormControl>
                        <Input type="number" inputMode="numeric" className="font-mono" {...field} />
                      </FormControl>
                      <FormMessage />
                    </FormItem>
                  )}
                />
                <FormField
                  control={form.control}
                  name="default_expected_status_max"
                  render={({ field }) => (
                    <FormItem>
                      <FormLabel>Expected status max</FormLabel>
                      <FormControl>
                        <Input type="number" inputMode="numeric" className="font-mono" {...field} />
                      </FormControl>
                      <FormMessage />
                    </FormItem>
                  )}
                />
              </div>

              <div className="flex items-center gap-2 border-t pt-4">
                <Button type="submit" disabled={update.isPending}>
                  <RotateCcw className={update.isPending ? "animate-spin" : undefined} />
                  Save settings
                </Button>
                <Button
                  type="button"
                  variant="ghost"
                  disabled={update.isPending}
                  onClick={() => form.reset()}
                >
                  Reset
                </Button>
              </div>
            </form>
          </Form>
        </CardContent>
      </Card>
    </>
  );
}
