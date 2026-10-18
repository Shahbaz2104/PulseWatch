/** Typed API client for the PulseWatch backend.
 *
 * The backend wraps errors as `{ error: { code, message, details } }`; this
 * module surfaces that shape so forms can map `details[].field` onto inputs.
 */

export interface ApiFieldError {
  field: string;
  message: string;
}

export class ApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly details: ApiFieldError[];

  constructor(status: number, code: string, message: string, details: ApiFieldError[]) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.code = code;
    this.details = details;
  }

  /** Field-level message for a form input, if the server reported one. */
  fieldError(field: string): string | undefined {
    return this.details.find((detail) => detail.field === field)?.message;
  }
}

async function parseBody(response: Response): Promise<unknown> {
  if (response.status === 204) return null;
  const text = await response.text();
  if (!text) return null;
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

export async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`/api/v1${path}`, {
    headers:
      init?.body != null
        ? { "Content-Type": "application/json", ...init?.headers }
        : init?.headers,
    ...init,
  });
  const body = await parseBody(response);
  if (!response.ok) {
    const envelope = body as
      | { error?: { code?: string; message?: string; details?: ApiFieldError[] } }
      | null;
    throw new ApiError(
      response.status,
      envelope?.error?.code ?? "unknown",
      envelope?.error?.message ?? `Request failed with status ${response.status}`,
      envelope?.error?.details ?? [],
    );
  }
  return body as T;
}

export const api = {
  get: <T>(path: string) => apiFetch<T>(path),
  post: <T>(path: string, body?: unknown) =>
    apiFetch<T>(path, {
      method: "POST",
      body: body != null ? JSON.stringify(body) : undefined,
    }),
  put: <T>(path: string, body: unknown) =>
    apiFetch<T>(path, { method: "PUT", body: JSON.stringify(body) }),
  delete: <T>(path: string) => apiFetch<T>(path, { method: "DELETE" }),
};
