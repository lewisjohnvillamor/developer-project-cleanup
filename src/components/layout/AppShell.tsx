import type { ReactNode } from "react";
import { AddFoldersDialog } from "./AddFoldersDialog";
import { Sidebar } from "./Sidebar";
import { Toasts } from "./Toasts";
import { TopBar } from "./TopBar";

export function AppShell({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-full w-full overflow-hidden bg-bg">
      <Sidebar />
      <div className="flex min-w-0 flex-1 flex-col">
        <TopBar />
        <main className="relative min-h-0 flex-1 overflow-hidden">{children}</main>
      </div>
      <Toasts />
      <AddFoldersDialog />
    </div>
  );
}
