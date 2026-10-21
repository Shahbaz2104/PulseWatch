import { NavLink, Outlet, useLocation } from "react-router-dom";
import { Suspense, useRef, useState } from "react";
import { motion } from "motion/react";
import {
  Heartbeat,
  BookOpen,
  ChartBar,
  Gear,
  List,
  MagnifyingGlass,
  SquaresFour,
  Warning,
  X,
} from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { Sheet, SheetContent, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { AmbientCanvas } from "@/components/ambient-canvas";
import { RouteFallback } from "@/components/route-fallback";
import { BootOverlay } from "@/components/boot-overlay";
import { ThemeToggle } from "@/components/theme-toggle";
import { AuthAction } from "@/features/auth/auth-action";
import { CommandPalette } from "@/components/command-palette";
import { PageTransition } from "@/components/page-transition";
import { cn } from "@/lib/utils";

const NAV_ITEMS = [
  { to: "/", label: "Overview", icon: SquaresFour, end: true },
  { to: "/monitors", label: "Monitors", icon: Heartbeat },
  { to: "/incidents", label: "Incidents", icon: Warning },
  { to: "/reports", label: "Reports", icon: ChartBar },
  { to: "/settings", label: "Settings", icon: Gear },
] as const;

const CRUMB_LABEL: Record<string, string> = {
  "": "Overview",
  monitors: "Monitors",
  incidents: "Incidents",
  reports: "Reports",
  settings: "Settings",
};

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

function crumbLabel(segment: string): string {
  if (CRUMB_LABEL[segment]) return CRUMB_LABEL[segment];
  // Monitor detail routes carry a UUID — never show the raw id in the trail.
  if (UUID_RE.test(segment) || Number.isFinite(Number(segment))) return "Monitor";
  return segment;
}

function Logo() {
  return (
    <div className="flex items-center gap-2.5 px-1">
      <span className="bg-primary/15 text-primary ring-primary/25 relative flex size-7 items-center justify-center rounded-md ring-1">
        <Heartbeat className="size-4" aria-hidden />
        <span
          aria-hidden
          className="bg-success animate-ring-pulse text-success absolute -top-0.5 -right-0.5 size-1.5 rounded-full"
        />
      </span>
      <span className="text-sm font-semibold tracking-tight">PulseWatch</span>
    </div>
  );
}

function Breadcrumbs() {
  const { pathname } = useLocation();
  const segments = pathname.split("/").filter(Boolean);
  const crumbs = segments.map(crumbLabel);
  if (crumbs.length === 0) return null;
  const trail = ["Overview", ...crumbs];
  // "Overview / Overview" would read as a stutter on the dashboard.
  const cleaned = trail[0] === trail[1] ? trail.slice(1) : trail;
  if (cleaned.length <= 1) {
    return <span className="text-foreground text-sm font-medium">{cleaned[0]}</span>;
  }
  return (
    <nav aria-label="Breadcrumb" className="flex min-w-0 items-center gap-1.5 text-sm">
      {cleaned.map((crumb, index) => {
        const last = index === cleaned.length - 1;
        return (
          <span key={`${crumb}-${index}`} className="flex min-w-0 items-center gap-1.5">
            {index > 0 && (
              <span aria-hidden className="text-muted-foreground/50">
                /
              </span>
            )}
            {last ? (
              <span className="text-foreground truncate font-medium">{crumb}</span>
            ) : (
              <NavLink
                to={index === 0 ? "/" : `/${crumb.toLowerCase()}`}
                className="text-muted-foreground hover:text-foreground truncate transition-colors"
              >
                {crumb}
              </NavLink>
            )}
          </span>
        );
      })}
    </nav>
  );
}

function NavLinks({ collapsed, onNavigate }: { collapsed?: boolean; onNavigate?: () => void }) {
  const { pathname } = useLocation();
  return (
    <nav className="flex flex-col gap-0.5 px-2" aria-label="Primary">
      {NAV_ITEMS.map((item) => {
        const { to, label, icon: Icon } = item;
        const end = "end" in item ? item.end : false;
        const active = end ? pathname === to : pathname === to || pathname.startsWith(`${to}/`);
        return (
          <NavLink
            key={to}
            to={to}
            end={end}
            onClick={onNavigate}
            title={collapsed ? label : undefined}
            className={cn(
              "group relative flex items-center gap-2.5 rounded-md px-2.5 py-2 text-sm font-medium transition-colors",
              "hover:bg-sidebar-accent hover:text-sidebar-accent-foreground",
              "focus-visible:ring-ring focus-visible:ring-2 focus-visible:outline-none",
              active
                ? "bg-sidebar-accent text-sidebar-accent-foreground"
                : "text-sidebar-foreground/70",
              collapsed && "justify-center px-0",
            )}
            aria-current={active ? "page" : undefined}
          >
            {active && (
              <motion.span
                aria-hidden
                layoutId="nav-active-indicator"
                className="bg-primary absolute top-1.5 bottom-1.5 -left-2 w-0.5 rounded-full"
                transition={{ type: "spring", stiffness: 420, damping: 34 }}
              />
            )}
            <Icon className="size-4 shrink-0" aria-hidden />
            {!collapsed && <span>{label}</span>}
          </NavLink>
        );
      })}
    </nav>
  );
}

function SidebarFooter() {
  return (
    <div className="mt-auto flex flex-col gap-1 px-2 pb-3">
      <Separator className="mb-2" />
      <a
        href="/api/v1/docs"
        target="_blank"
        rel="noreferrer"
        className="text-sidebar-foreground/70 hover:bg-sidebar-accent hover:text-sidebar-accent-foreground focus-visible:ring-ring flex items-center gap-2.5 rounded-md px-2.5 py-2 text-sm font-medium transition-colors focus-visible:ring-2 focus-visible:outline-none"
      >
        <BookOpen className="size-4" aria-hidden />
        API docs
      </a>
      <div className="px-1.5 pt-1">
        <span className="label-cap text-muted-foreground/80 px-1">v0.1.0</span>
      </div>
    </div>
  );
}

const BOOT_KEY = "pulsewatch-booted";

/** Session-once boot curtain: first visit only, never in tests, never under
 * reduced-motion. The flag is written in `onDone` (not here) so React
 * StrictMode's double-invoked initializer can't swallow the sequence. */
function canBoot(): boolean {
  if (import.meta.env.MODE === "test") return false;
  if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return false;
  try {
    return sessionStorage.getItem(BOOT_KEY) === null;
  } catch {
    return false;
  }
}

export function AppLayout() {
  const [collapsed, setCollapsed] = useState(false);
  const [mobileOpen, setMobileOpen] = useState(false);
  const [booting, setBooting] = useState(canBoot);
  const shellRef = useRef<HTMLDivElement>(null);

  return (
    <div ref={shellRef} className="app-shell flex min-h-svh">
      {booting && (
        <BootOverlay
          onDone={() => {
            try {
              sessionStorage.setItem(BOOT_KEY, "1");
            } catch {
              /* private mode — boot will replay next visit, harmless */
            }
            setBooting(false);
          }}
        />
      )}
      {/* Signature move: slow aurora wash behind the entire console. */}
      <div className="ambient-layer" aria-hidden="true">
        <AmbientCanvas />
      </div>
      {/* Desktop sidebar — glass rail over the layered ground. */}
      <aside
        className={cn(
          "sticky top-0 hidden h-svh shrink-0 flex-col border-r backdrop-blur-xl transition-[width] duration-200 md:flex",
          collapsed ? "w-14" : "w-56",
        )}
      >
        <div className="flex h-14 items-center justify-between px-3">
          {!collapsed && <Logo />}
          <Button
            variant="ghost"
            size="icon"
            className="size-7"
            aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
            onClick={() => setCollapsed((value) => !value)}
          >
            {collapsed ? <List className="size-4" /> : <X className="size-4" />}
          </Button>
        </div>
        <div className="mt-2 flex-1 overflow-y-auto">
          <NavLinks collapsed={collapsed} />
        </div>
        {!collapsed && <SidebarFooter />}
      </aside>

      {/* Mobile sidebar */}
      <Sheet open={mobileOpen} onOpenChange={setMobileOpen}>
        <SheetContent side="left" className="bg-sidebar w-60 p-0 backdrop-blur-xl">
          <SheetHeader className="h-14 justify-center border-b px-4">
            <SheetTitle>
              <Logo />
            </SheetTitle>
          </SheetHeader>
          <div className="mt-3 flex h-[calc(100%-3.5rem)] flex-col">
            <NavLinks onNavigate={() => setMobileOpen(false)} />
            <SidebarFooter />
          </div>
        </SheetContent>
      </Sheet>

      {/* Main column */}
      <div className="flex min-w-0 flex-1 flex-col">
        <header className="bg-background/70 supports-[backdrop-filter]:bg-background/70 sticky top-0 z-30 flex h-14 items-center gap-3 border-b px-4 backdrop-blur-xl">
          <Button
            variant="ghost"
            size="icon"
            className="md:hidden"
            aria-label="Open navigation"
            onClick={() => setMobileOpen(true)}
          >
            <List className="size-5" />
          </Button>
          <div className="min-w-0 truncate text-sm">
            <Breadcrumbs />
          </div>
          <div className="ml-auto flex items-center gap-2">
            <Button
              variant="outline"
              size="sm"
              className="text-muted-foreground gap-2 md:w-52 md:justify-start md:font-normal"
              onClick={() => window.dispatchEvent(new CustomEvent("open-command-palette"))}
            >
              <MagnifyingGlass className="size-3.5" aria-hidden />
              <span className="hidden md:inline">Search or jump to…</span>
              <kbd className="bg-muted pointer-events-none ml-auto hidden rounded border px-1.5 font-mono text-[11px] md:inline">
                ⌘K
              </kbd>
            </Button>
            <ThemeToggle />
            <AuthAction />
          </div>
        </header>
        <main className="mx-auto w-full max-w-6xl flex-1 px-4 py-6 md:px-8">
          <PageTransition>
            <Suspense fallback={<RouteFallback />}>
              <Outlet />
            </Suspense>
          </PageTransition>
        </main>
      </div>

      <CommandPalette />
    </div>
  );
}
