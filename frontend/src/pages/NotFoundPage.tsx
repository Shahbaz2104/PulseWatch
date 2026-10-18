import { useNavigate } from "react-router-dom";
import { Compass } from "lucide-react";
import { EmptyState } from "@/components/empty-state";

export function NotFoundPage() {
  const navigate = useNavigate();
  return (
    <div className="flex min-h-[60vh] items-center justify-center">
      <EmptyState
        icon={Compass}
        title="Page not found"
        description="That route doesn't exist in this console."
        action={{ label: "Back to Overview", onClick: () => navigate("/") }}
      />
    </div>
  );
}
