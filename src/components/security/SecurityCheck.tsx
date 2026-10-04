import { RefreshCw, Settings2, ShieldAlert, ShieldCheck } from "lucide-react";
import { SeverityBadge } from "@/components/common/Badge";
import { Button } from "@/components/common/Button";
import { Card } from "@/components/common/Card";
import { useAppStore } from "@/stores/app-store";
import type { ProjectAudit, Severity } from "@/types";
import { pluralize } from "@/utils/format";

const ORDER: Severity[] = ["unknown", "low", "moderate", "high", "critical"];

export function worstOf(audit: ProjectAudit): Severity | null {
  let worst: Severity | null = null;
  for (const f of audit.findings) if (worst === null || ORDER.indexOf(f.severity) > ORDER.indexOf(worst)) worst = f.severity;
  return worst;
}

/**
 * Known-vulnerable dependencies across every scanned project — including
 * hibernated ones, since only the lockfile is read. A project with no
 * lockfile we can read is counted as not checked, never as clean.
 */
export function SecurityCheck() {
  const advisoryDb = useAppStore((s) => s.settings.advisoryDb);
  const audit = useAppStore((s) => s.audit);
  const running = useAppStore((s) => s.auditRunning);
  const runAudit = useAppStore((s) => s.runAudit);
  const setPage = useAppStore((s) => s.setPage);
  const openDrawer = useAppStore((s) => s.openDrawer);

  if (!advisoryDb) {
    return (
      <Card title="Security check">
        <div className="flex items-start gap-3 text-[12.5px] text-fg-muted">
          <ShieldAlert size={16} className="mt-0.5 shrink-0 text-fg-subtle" />
          <div className="flex-1">
            Find known-vulnerable dependencies in projects nobody is watching — including hibernated ones, from their lockfiles. Point the app at OSV advisory
            data in Settings. Nothing is downloaded and nothing leaves this machine.
          </div>
          <Button variant="ghost" size="sm" icon={<Settings2 size={13} />} onClick={() => setPage("settings")}>
            Set up
          </Button>
        </div>
      </Card>
    );
  }

  const action = (
    <Button variant="ghost" size="sm" icon={<RefreshCw size={13} className={running ? "animate-spin" : ""} />} onClick={runAudit} loading={running && !audit}>
      {audit ? "Check again" : "Check now"}
    </Button>
  );

  if (!audit) {
    return (
      <Card title="Security check" action={action}>
        <div className="flex items-center gap-3 text-[12.5px] text-fg-muted">
          <ShieldCheck size={16} className="text-fg-subtle" />
          Check every scanned project's lockfile against your advisory data. Hibernated projects are included.
        </div>
      </Card>
    );
  }

  const checked = audit.projects.filter((p) => p.packages > 0);
  const notChecked = audit.projects.length - checked.length;
  const affected = checked
    .filter((p) => p.findings.length > 0)
    .sort((a, b) => ORDER.indexOf(worstOf(b) ?? "unknown") - ORDER.indexOf(worstOf(a) ?? "unknown") || b.findings.length - a.findings.length);
  const critical = affected.filter((p) => worstOf(p) === "critical").length;

  return (
    <Card title="Security check" action={action} padded={affected.length === 0}>
      {affected.length === 0 ? (
        <div className="flex items-center gap-3 text-[12.5px] text-fg-muted">
          <ShieldCheck size={16} className="text-safe" />
          <span>
            No known vulnerabilities in the {pluralize(checked.length, "project")} that could be checked, against {audit.advisories.toLocaleString()}{" "}
            advisories.
            {notChecked > 0 && ` ${pluralize(notChecked, "project")} had no supported lockfile and ${notChecked === 1 ? "was" : "were"} not checked.`}
          </span>
        </div>
      ) : (
        <>
          <div className="border-b border-border px-4 py-2.5 text-[12.5px] text-fg-muted">
            <span className="font-semibold text-fg">
              {affected.length} of {pluralize(checked.length, "checked project")}
            </span>{" "}
            pin known-vulnerable dependencies
            {critical > 0 && (
              <>
                , <span className="font-semibold text-danger">{critical} critical</span>
              </>
            )}
            .{notChecked > 0 && ` ${notChecked} not checked: no supported lockfile.`} Waking one reinstalls exactly these versions.
          </div>
          <ul className="divide-y divide-border">
            {affected.slice(0, 6).map((p) => (
              <li key={p.projectId}>
                <button
                  type="button"
                  onClick={() => {
                    setPage("projects");
                    openDrawer(p.projectId);
                  }}
                  className="grid w-full grid-cols-[1fr_auto_auto] items-center gap-x-3 px-4 py-2 text-left text-[12.5px] hover:bg-surface-2"
                >
                  <div className="min-w-0">
                    <div className="truncate font-medium text-fg">{p.name}</div>
                    <div className="truncate text-[11.5px] text-fg-subtle">
                      {p.findings
                        .slice(0, 3)
                        .map((f) => `${f.package.name}@${f.package.version}`)
                        .join(" · ")}
                      {p.findings.length > 3 && ` · +${p.findings.length - 3} more`}
                    </div>
                  </div>
                  <span className="tabular text-fg-muted">{pluralize(p.findings.length, "finding")}</span>
                  <SeverityBadge severity={worstOf(p) ?? "unknown"} />
                </button>
              </li>
            ))}
          </ul>
          {affected.length > 6 && <div className="px-4 py-2 text-[11.5px] text-fg-subtle">and {affected.length - 6} more</div>}
        </>
      )}
    </Card>
  );
}
