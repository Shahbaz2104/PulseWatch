import { NavLink, Outlet, useLocation } from "react-router-dom";
import { useState } from "react";
import { motion } from "motion/react";
import {
  Activity,
  AlertTriangle,
  BarChart3,
  LayoutDashboard,
  Menu,
  Settings,
  X,
  BookOpen,
  Search,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { Sheet, SheetContent, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { ThemeToggle } from "@/components/theme-toggle";
import { CommandPalette } from "@/components/command-palette";
import { PageTransition } from "@/components/page-transition";
import { cn } from "@/lib/utils";

const NAV_ITEMS = [
  { to: "/", label: "Overview", icon: LayoutDashboard, end: true },
  { to: "/monitors", label: "Monitors", icon: Activity },
  { to: "/incidents", label: "Incidents", icon: AlertTriangle },
  { to: "/reports", label: "Reports", icon: BarChart3 },
  { to: "/settings", label: "Settings", icon: Settings },
] as const;

function Logo() {
  return (
    <div className="flex items-center gap-2.5 px-1">
      <span className="flex size-7 items-center justify-center rounded-md bg-primary/15 text-primary ring-1 ring-primary/25">
        <Activity className="size-4" aria-hidden />
      </span>
      <span className="text-sm font-semibold tracking-tight">PulseWatch</span>
    </div>
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
              "focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none",
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
                className="absolute top-1.5 bottom-1.5 -left-2 w-0.5 rounded-full bg-primary"
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
        className="flex items-center gap-2.5 rounded-md px-2.5 py-2 text-sm font-medium text-sidebar-foreground/70 transition-colors hover:bg-sidebar-accent hover:text-sidebar-accent-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
      >
        <BookOpen className="size-4" aria-hidden />
        API docs
      </a>
      <div className="flex items-center justify-between px-1.5 pt-1">
        <span className="px-1 text-[10px] font-medium tracking-wider text-muted-foreground/70 uppercase">
          v0.1.0
        </span>
        <ThemeToggle />
      </div>
    </div>
  );
}

export function AppLayout() {
  const [collapsed, setCollapsed] = useState(false);
  const [mobileOpen, setMobileOpen] = useState(false);

  return (
    <div className="flex min-h-svh">
      {/* Desktop sidebar */}
      <aside
        className={cn(
          "sticky top-0 hidden h-svh shrink-0 flex-col border-r bg-sidebar transition-[width] duration-200 md:flex",
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
            {collapsed ? <Menu className="size-4" /> : <X className="size-4" />}
          </Button>
        </div>
        <div className="mt-2 flex-1 overflow-y-auto">
          <NavLinks collapsed={collapsed} />
        </div>
        {!collapsed && <SidebarFooter />}
      </aside>

      {/* Mobile sidebar */}
      <Sheet open={mobileOpen} onOpenChange={setMobileOpen}>
        <SheetContent side="left" className="w-60 bg-sidebar p-0">
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
        <header className="sticky top-0 z-30 flex h-14 items-center gap-2 border-b bg-background/85 px-4 backdrop-blur-md supports-[backdrop-filter]:bg-background/70">
          <Button
            variant="ghost"
            size="icon"
            className="md:hidden"
            aria-label="Open navigation"
            onClick={() => setMobileOpen(true)}
          >
            <Menu className="size-5" />
          </Button>
          <Button
            variant="outline"
            size="sm"
            className="ml-auto gap-2 text-muted-foreground md:w-56 md:justify-start md:font-normal"
            onClick={() => window.dispatchEvent(new CustomEvent("open-command-palette"))}
          >
            <Search className="size-3.5" aria-hidden />
            <span className="hidden md:inline">Search or jump to…</span>
            <kbd className="pointer-events-none ml-auto hidden rounded border bg-muted px-1.5 font-mono text-[10px] md:inline">
              ⌘K
            </kbd>
          </Button>
        </header>
        <main className="mx-auto w-full max-w-6xl flex-1 px-4 py-6 md:px-8">
          <PageTransition>
            <Outlet />
          </PageTransition>
        </main>
      </div>

      <CommandPalette />
    </div>
  );
}
