import type { ReactNode } from "react";
import { Archive, CheckCircle2, AlertTriangle, Shield, Zap, Moon, GitBranch, GitCommitHorizontal, CircleDashed, CloudOff } from "lucide-react";
import type { GitState, ProjectStatus, Safety } from "@/types";
import { GIT_STATE_LABELS } from "@/types";

export type Tone = "safe" | "review" | "danger" | "info" | "inactive" | "neutral" | "accent";

const TONES: Record<Tone, string> = {
  safe: "bg-safe-soft text-safe",
  review: "bg-review-soft text-review",
  danger: "bg-danger-soft text-danger",
  info: "bg-info-soft text-info",
  inactive: "bg-inactive-soft text-fg-muted",
  neutral: "bg-surface-2 text-fg-muted",
  accent: "bg-accent-soft text-accent",
};

export function Badge({ tone = "neutral", icon, children, className = "", title }: { tone?: Tone; icon?: ReactNode; children: ReactNode; className?: string; title?: string }) {
  return (
    <span title={title} className={`inline-flex items-center gap-1 rounded-md px-1.5 py-0.5 text-[11px] font-medium leading-4 whitespace-nowrap ${TONES[tone]} ${className}`}>
      {icon}
      {children}
    </span>
  );
}

export function StatusBadge({ status }: { status: ProjectStatus }) {
  switch (status) {
    case "active":
      return <Badge tone="info" icon={<Zap size={11} />}>Active</Badge>;
    case "dormant":
      return <Badge tone="neutral" icon={<Moon size={11} />}>Dormant</Badge>;
    case "hibernated":
      return <Badge tone="inactive" icon={<Archive size={11} />}>Hibernated</Badge>;
    case "protected":
      return <Badge tone="danger" icon={<Shield size={11} />}>Protected</Badge>;
  }
}

export function SafetyBadge({ safety, reasons, compact }: { safety: Safety; reasons?: string[]; compact?: boolean }) {
  const title = reasons?.length ? reasons.join("\n") : undefined;
  switch (safety) {
    case "safe":
      return <Badge tone="safe" icon={<CheckCircle2 size={11} />} title={title ?? "Only regeneratable folders"}>{compact ? "" : "Safe"}</Badge>;
    case "review":
      return <Badge tone="review" icon={<AlertTriangle size={11} />} title={title}>{compact ? "" : "Review"}</Badge>;
    case "protected":
      return <Badge tone="danger" icon={<Shield size={11} />} title={title}>{compact ? "" : "Protected"}</Badge>;
  }
}

export function GitBadge({ state, branch }: { state: GitState; branch?: string | null }) {
  const label = GIT_STATE_LABELS[state];
  const suffix = branch ? ` · ${branch}` : "";
  switch (state) {
    case "clean":
      return <Badge tone="safe" icon={<GitCommitHorizontal size={11} />}>{label + suffix}</Badge>;
    case "modified":
    case "untracked":
      return <Badge tone="review" icon={<GitBranch size={11} />}>{label + suffix}</Badge>;
    case "remoteMissing":
      return <Badge tone="review" icon={<CloudOff size={11} />}>{label + suffix}</Badge>;
    case "noRepo":
      return <Badge tone="neutral" icon={<CircleDashed size={11} />}>{label}</Badge>;
    case "unknown":
      return <Badge tone="neutral" icon={<GitBranch size={11} />}>{"Git" + suffix}</Badge>;
  }
}
