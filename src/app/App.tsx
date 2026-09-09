import { Loader2 } from "lucide-react";
import { useEffect } from "react";
import { CommandPalette } from "@/components/common/CommandPalette";
import { AppShell } from "@/components/layout/AppShell";
import { HibernateDialog } from "@/components/projects/HibernateDialog";
import { WakeDialog } from "@/components/projects/WakeDialog";
import { useShortcuts } from "@/hooks/useShortcuts";
import { useTheme } from "@/hooks/useTheme";
import { HistoryPage } from "@/pages/HistoryPage";
import { OverviewPage } from "@/pages/OverviewPage";
import { ProjectsPage } from "@/pages/ProjectsPage";
import { SettingsPage } from "@/pages/SettingsPage";
import { useAppStore } from "@/stores/app-store";

export function App() {
  const ready = useAppStore((s) => s.ready);
  const init = useAppStore((s) => s.init);
  const page = useAppStore((s) => s.page);
  useTheme();
  useShortcuts();

  useEffect(() => {
    init().catch((err) => console.error("init failed", err));
  }, [init]);

  useEffect(() => {
    // Keep the desktop feel: no browser context menu, no drag-drop navigation.
    const block = (e: Event) => e.preventDefault();
    window.addEventListener("dragover", block);
    window.addEventListener("drop", block);
    return () => {
      window.removeEventListener("dragover", block);
      window.removeEventListener("drop", block);
    };
  }, []);

  if (!ready) {
    return (
      <div className="flex h-full items-center justify-center bg-bg text-fg-muted">
        <Loader2 size={18} className="animate-spin" />
      </div>
    );
  }

  return (
    <AppShell>
      {page === "overview" && <OverviewPage />}
      {page === "projects" && <ProjectsPage />}
      {page === "history" && <HistoryPage />}
      {page === "settings" && <SettingsPage />}
      <HibernateDialog />
      <WakeDialog />
      <CommandPalette />
    </AppShell>
  );
}
