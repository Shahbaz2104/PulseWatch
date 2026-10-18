import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
} from "@/components/ui/command";
import {
  Heartbeat,
  Warning as AlertTriangle,
  ChartBar as BarChart3,
  BookOpen,
  SquaresFour as LayoutDashboard,
  Moon,
  Plus,
  Gear as Settings,
  Sun,
} from "@phosphor-icons/react";
import { useTheme } from "next-themes";
import { useMonitors } from "@/hooks/use-api";

/** ⌘K palette: navigate, run actions, jump to a monitor. */
export function CommandPalette() {
  const [open, setOpen] = useState(false);
  const navigate = useNavigate();
  const { setTheme } = useTheme();
  const [search, setSearch] = useState("");
  const { data } = useMonitors({ search: search || undefined, page_size: 8 });

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "k" && (event.metaKey || event.ctrlKey)) {
        event.preventDefault();
        setOpen((value) => !value);
      }
    };
    const onOpen = () => setOpen(true);
    window.addEventListener("keydown", onKey);
    window.addEventListener("open-command-palette", onOpen);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("open-command-palette", onOpen);
    };
  }, []);

  const go = (path: string) => {
    setOpen(false);
    navigate(path);
  };

  return (
    <CommandDialog open={open} onOpenChange={setOpen}>
      <CommandInput
        placeholder="Type a command or search…"
        value={search}
        onValueChange={setSearch}
      />
      <CommandList>
        <CommandEmpty>No results found.</CommandEmpty>
        <CommandGroup heading="Navigate">
          <CommandItem onSelect={() => go("/")}>
            <LayoutDashboard /> Overview
          </CommandItem>
          <CommandItem onSelect={() => go("/monitors")}>
            <Heartbeat /> Monitors
          </CommandItem>
          <CommandItem onSelect={() => go("/incidents")}>
            <AlertTriangle /> Incidents
          </CommandItem>
          <CommandItem onSelect={() => go("/reports")}>
            <BarChart3 /> Reports
          </CommandItem>
          <CommandItem onSelect={() => go("/settings")}>
            <Settings /> Settings
          </CommandItem>
        </CommandGroup>
        <CommandSeparator />
        <CommandGroup heading="Actions">
          <CommandItem onSelect={() => go("/monitors?new=1")}>
            <Plus /> Add monitor
          </CommandItem>
          <CommandItem onSelect={() => setTheme("dark")}>
            <Moon /> Switch to dark theme
          </CommandItem>
          <CommandItem onSelect={() => setTheme("light")}>
            <Sun /> Switch to light theme
          </CommandItem>
          <CommandItem
            onSelect={() => {
              setOpen(false);
              window.open("/api/v1/docs", "_blank");
            }}
          >
            <BookOpen /> Open API docs
          </CommandItem>
        </CommandGroup>
        {data && data.data.length > 0 && (
          <>
            <CommandSeparator />
            <CommandGroup heading="Monitors">
              {data.data.map((monitor) => (
                <CommandItem
                  key={monitor.id}
                  value={`${monitor.name} ${monitor.url}`}
                  onSelect={() => go(`/monitors/${monitor.id}`)}
                >
                  <Heartbeat />
                  <span>{monitor.name}</span>
                  <span className="text-muted-foreground ml-2 truncate font-mono text-xs">
                    {monitor.url}
                  </span>
                </CommandItem>
              ))}
            </CommandGroup>
          </>
        )}
      </CommandList>
    </CommandDialog>
  );
}
