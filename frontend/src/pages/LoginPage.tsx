import { useEffect, useState, type FormEvent } from "react";
import { Link, useNavigate } from "react-router-dom";
import { ArrowRight, Heartbeat, SignIn } from "@phosphor-icons/react";
import { AmbientCanvas } from "@/components/ambient-canvas";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { useAuthStatus, useLogin } from "@/hooks/use-api";
import { ApiError } from "@/lib/api";

export function LoginPage() {
  const navigate = useNavigate();
  const status = useAuthStatus();
  const login = useLogin();
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (status.data?.authenticated) navigate("/", { replace: true });
  }, [navigate, status.data?.authenticated]);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);
    try {
      await login.mutateAsync(password);
      navigate("/", { replace: true });
    } catch (cause) {
      setError(cause instanceof ApiError ? cause.message : "Couldn't sign in. Try again.");
    }
  }

  return (
    <div className="relative flex min-h-svh items-center justify-center overflow-hidden px-4">
      <div className="ambient-layer" aria-hidden="true">
        <AmbientCanvas />
      </div>
      <div className="relative w-full max-w-sm">
        <Link
          to="/"
          className="mb-6 flex items-center justify-center gap-2.5 rounded-md focus-visible:ring-ring focus-visible:ring-2 focus-visible:outline-none"
        >
          <span className="bg-primary/15 text-primary ring-primary/25 relative flex size-8 items-center justify-center rounded-md ring-1">
            <Heartbeat className="size-4" aria-hidden />
          </span>
          <span className="text-base font-semibold tracking-tight">PulseWatch</span>
        </Link>

        <Card className="backdrop-blur-xl">
          {status.data?.demo_mode ? (
            <>
              <CardHeader>
                <CardTitle>Read-only demo</CardTitle>
                <CardDescription>
                  This deployment runs with seeded data and live checks. Sign-in is disabled and
                  every change is blocked.
                </CardDescription>
              </CardHeader>
              <CardContent>
                <Button asChild className="w-full">
                  <Link to="/">
                    View dashboard
                    <ArrowRight aria-hidden />
                  </Link>
                </Button>
              </CardContent>
            </>
          ) : (
            <>
              <CardHeader>
                <CardTitle>Admin sign in</CardTitle>
                <CardDescription>
                  Reads are public. Sign in to add monitors, run checks, and change settings.
                </CardDescription>
              </CardHeader>
              <CardContent>
                <form onSubmit={handleSubmit} className="flex flex-col gap-3" noValidate>
                  <label htmlFor="admin-password" className="text-sm font-medium">
                    Password
                  </label>
                  <Input
                    id="admin-password"
                    name="password"
                    type="password"
                    autoComplete="current-password"
                    autoFocus
                    value={password}
                    onChange={(event) => {
                      setPassword(event.target.value);
                      setError(null);
                    }}
                    aria-invalid={error !== null}
                    aria-describedby={error ? "login-error" : undefined}
                    disabled={login.isPending}
                  />
                  {error && (
                    <p id="login-error" role="alert" className="text-destructive text-sm">
                      {error}
                    </p>
                  )}
                  <Button type="submit" className="w-full" disabled={login.isPending}>
                    <SignIn aria-hidden />
                    {login.isPending ? "Signing in…" : "Sign in"}
                  </Button>
                </form>
              </CardContent>
            </>
          )}
        </Card>

        <p className="text-muted-foreground mt-4 text-center text-xs">
          Single-admin access · Sessions expire after 7 days
        </p>
      </div>
    </div>
  );
}
