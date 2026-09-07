import { BulkActionBar } from "@/components/projects/BulkActionBar";
import { ProjectDetailsDrawer } from "@/components/projects/ProjectDetailsDrawer";
import { ProjectTable } from "@/components/projects/ProjectTable";
import { ProjectToolbar } from "@/components/projects/ProjectToolbar";
import { useProjectList } from "@/hooks/useProjectList";
import { useAppStore } from "@/stores/app-store";
import { formatBytes, pluralize } from "@/utils/format";
import { useRef } from "react";

export function ProjectsPage() {
  const visible = useProjectList();
  const total = useAppStore((s) => s.projects.length);
  const scrollRef = useRef<HTMLDivElement>(null);
  const reclaimable = visible.reduce((s, p) => s + p.reclaimableBytes, 0);
  return (
    <div className="flex h-full flex-col">
      <ProjectToolbar visible={visible} />
      <div className="flex items-center justify-between border-b border-border bg-surface px-4 py-1.5 text-[11.5px] text-fg-subtle">
        <span>
          {visible.length === total ? pluralize(total, "project") : `${visible.length} of ${pluralize(total, "project")}`}
          {" · "}
          <span className="tabular">{formatBytes(reclaimable)}</span> reclaimable in view
        </span>
        <span>Click a row for details · Checkbox to select</span>
      </div>
      <div className="relative min-h-0 flex-1">
        <div ref={scrollRef} className="h-full overflow-auto">
          <ProjectTable projects={visible} scrollRef={scrollRef} />
          <div className="h-20" />
        </div>
        <BulkActionBar />
        <ProjectDetailsDrawer />
      </div>
    </div>
  );
}
