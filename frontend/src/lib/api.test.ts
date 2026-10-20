import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError, apiFetch } from "@/lib/api";
import { monitorsApi } from "@/lib/endpoints";

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("apiFetch", () => {
  it("calls the versioned API prefix and parses JSON", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ ok: true }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(apiFetch<{ ok: boolean }>("/health")).resolves.toEqual({ ok: true });
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/v1/health",
      expect.objectContaining({ headers: undefined }),
    );
  });

  it("maps the backend error envelope onto ApiError", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response(
          JSON.stringify({
            error: {
              code: "validation_failed",
              message: "Validation failed",
              details: [{ field: "url", message: "URL is required" }],
            },
          }),
          { status: 422, headers: { "Content-Type": "application/json" } },
        ),
      ),
    );

    const error = await apiFetch("/monitors").catch((value: unknown) => value);
    expect(error).toBeInstanceOf(ApiError);
    expect(error).toMatchObject({
      status: 422,
      code: "validation_failed",
      message: "Validation failed",
    });
    expect((error as ApiError).fieldError("url")).toBe("URL is required");
  });

  it("falls back to a status message when the body is not an envelope", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response(null, { status: 500 })));

    await expect(apiFetch("/monitors")).rejects.toMatchObject({
      status: 500,
      code: "unknown",
      message: "Request failed with status 500",
    });
  });
});

describe("monitorsApi.list", () => {
  it("forwards search, status, sort, order and pagination", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ data: [], pagination: {} }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await monitorsApi.list({
      search: "api",
      status: "up",
      sort: "uptime",
      order: "asc",
      window: "7d",
      page: 2,
      page_size: 20,
    });

    const url = String(fetchMock.mock.calls[0]?.[0]);
    expect(url).toContain("/api/v1/monitors?");
    expect(url).toContain("search=api");
    expect(url).toContain("status=up");
    expect(url).toContain("sort=uptime");
    expect(url).toContain("order=asc");
    expect(url).toContain("window=7d");
    expect(url).toContain("page=2");
    expect(url).toContain("page_size=20");
  });
});
