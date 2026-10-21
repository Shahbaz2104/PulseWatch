import { Link, useNavigate } from "react-router-dom";
import { LockSimple, SignIn, SignOut } from "@phosphor-icons/react";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { useAuthStatus, useLogout } from "@/hooks/use-api";
import { ApiError } from "@/lib/api";

/** Header-level session controls. Reads stay available either way; the action
 * only reflects whether mutations are currently allowed. */
export function AuthAction() {
  const status = useAuthStatus();
  const logout = useLogout();
  const navigate = useNavigate();

  if (status.isLoading) {
    return <Skeleton className="h-8 w-20" />;
  }

  if (status.data?.demo_mode) {
    return (
      <Badge variant="outline" title="Public demo: authentication is disabled and changes are blocked">
        <LockSimple aria-hidden />
        <span className="hidden sm:inline">Read-only demo</span>
      </Badge>
    );
  }

  if (status.data?.authenticated) {
    return (
      <Button
        variant="ghost"
        size="sm"
        disabled={logout.isPending}
        onClick={() =>
          logout.mutate(undefined, {
            onSuccess: () => {
              toast.success("Signed out");
              navigate("/");
            },
            onError: (error) =>
              toast.error(error instanceof ApiError ? error.message : "Couldn't sign out"),
          })
        }
      >
        <SignOut aria-hidden />
        <span className="hidden sm:inline">{logout.isPending ? "Signing out…" : "Sign out"}</span>
      </Button>
    );
  }

  return (
    <Button variant="outline" size="sm" asChild>
      <Link to="/login">
        <SignIn aria-hidden />
        <span className="hidden sm:inline">Sign in</span>
      </Link>
    </Button>
  );
}
