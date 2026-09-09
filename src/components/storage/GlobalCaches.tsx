import { Copy, HardDrive, RefreshCw } from "lucide-react";
import { Button, IconButton } from "@/components/common/Button";
import { Card } from "@/components/common/Card";
import { useAppStore } from "@/stores/app-store";
import { formatBytes } from "@/utils/format";

/** Global toolchain caches: shown, never touched. */
export function GlobalCaches() {
  const caches = useAppStore((s) => s.caches);
  const loading = useAppStore((s) => s.cachesLoading);
  const load = useAppStore((s) => s.loadCaches);
  const copyText = useAppStore((s) => s.copyText);
  const present = caches?.filter((c) => c.exists) ?? [];
  const total = present.reduce((s, c) => s + c.bytes, 0);

  return (
    <Card
      title="Toolchain caches"
      action={
        <Button variant="ghost" size="sm" icon={<RefreshCw size={13} className={loading ? "animate-spin" : ""} />} onClick={load} loading={loading && !caches}>
          {caches ? "Refresh" : "Measure"}
        </Button>
      }
    >
      {!caches ? (
        <div className="flex items-center gap-3 text-[12.5px] text-fg-muted">
          <HardDrive size={16} className="text-fg-subtle" />
          Shared caches such as the Cargo registry, npm cache and pnpm store live outside your projects. Measure them to see what the tools' own clean commands
          could free.
        </div>
      ) : present.length === 0 ? (
        <p className="text-[12.5px] text-fg-muted">No known toolchain caches found on this machine.</p>
      ) : (
        <div className="flex flex-col gap-1.5">
          <div className="text-[12px] text-fg-muted">
            <span className="font-semibold tabular text-fg">{formatBytes(total)}</span> outside your projects. Hibernate never removes these; use each tool's
            command.
          </div>
          <ul className="divide-y divide-border">
            {present.map((c) => (
              <li key={c.id} className="grid grid-cols-[1fr_auto_auto] items-center gap-x-3 py-1.5 text-[12.5px]">
                <div className="min-w-0">
                  <div className="truncate text-fg">{c.label}</div>
                  <div className="truncate font-mono text-[11px] text-fg-subtle" title={c.path}>
                    {c.path}
                  </div>
                </div>
                <span className="tabular text-fg">{formatBytes(c.bytes)}</span>
                {c.cleanCommand ? (
                  <IconButton title={`Copy: ${c.cleanCommand}`} onClick={() => copyText(c.cleanCommand!, "Command")}>
                    <Copy size={13} />
                  </IconButton>
                ) : (
                  <span className="w-7" />
                )}
              </li>
            ))}
          </ul>
        </div>
      )}
    </Card>
  );
}
