import { z } from "zod";

/** Mirror of the backend's documented validation bounds. */
export const monitorSchema = z
  .object({
    name: z.string().trim().max(100, "Must be at most 100 characters").optional().or(z.literal("")),
    url: z
      .string()
      .trim()
      .min(1, "URL is required")
      .url("Must be a valid URL (http:// or https://)"),
    check_interval_seconds: z.coerce
      .number()
      .int("Whole seconds")
      .min(10, "Minimum interval is 10 seconds")
      .max(86400, "Maximum interval is 24 hours"),
    timeout_ms: z.coerce
      .number()
      .int("Whole milliseconds")
      .min(100, "Minimum timeout is 100 ms")
      .max(60000, "Maximum timeout is 60 s"),
    retry_max_attempts: z.coerce
      .number()
      .int("Whole number")
      .min(1, "At least 1 attempt")
      .max(5, "At most 5 attempts"),
    expected_status_min: z.coerce
      .number()
      .int("Whole number")
      .min(100, "HTTP status minimum is 100")
      .max(599, "HTTP status maximum is 599"),
    expected_status_max: z.coerce
      .number()
      .int("Whole number")
      .min(100, "HTTP status minimum is 100")
      .max(599, "HTTP status maximum is 599"),
    consecutive_failures_threshold: z.coerce
      .number()
      .int("Whole number")
      .min(1, "At least 1")
      .max(100, "At most 100"),
    consecutive_successes_threshold: z.coerce
      .number()
      .int("Whole number")
      .min(1, "At least 1")
      .max(100, "At most 100"),
  })
  .refine((value) => value.expected_status_min <= value.expected_status_max, {
    message: "Range start must be ≤ range end",
    path: ["expected_status_max"],
  });

export type MonitorFormValues = z.output<typeof monitorSchema>;

export const monitorDefaults: MonitorFormValues = {
  name: "",
  url: "",
  check_interval_seconds: 60,
  timeout_ms: 5000,
  retry_max_attempts: 2,
  expected_status_min: 200,
  expected_status_max: 399,
  consecutive_failures_threshold: 1,
  consecutive_successes_threshold: 1,
};
