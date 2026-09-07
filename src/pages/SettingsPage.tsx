import { useEffect, useState } from "react";
import { ClipboardCopy, Download, Eraser, Folder, FolderPlus, Loader2, Plus, Search, Trash2 } from "lucide-react";
import { Button, IconButton } from "@/components/common/Button";
import { Card } from "@/components/common/Card";
import { NumberInput, RadioGroup, Select, TextInput, Toggle } from "@/components/common/Controls";
import { Badge, SafetyBadge } from "@/components/common/Badge";
import { useAppStore } from "@/stores/app-store";
import { CATEGORY_LABELS, STACK_LABELS, type CleanupRule, type Disposition, type RuleMatch, type Stack, type Theme } from "@/types";
import { formatBytes } from "@/utils/format";

const BUILTIN_RULES: { pattern: string; stacks: string; safety: "safe" | "review"; why: string }[] = [
  { pattern: "node_modules/", stacks: "Node", safety: "safe", why: "Installed packages generated from the lockfile." },
  { pattern: ".next/ .nuxt/ .output/ .svelte-kit/", stacks: "Node", safety: "safe", why: "Framework build output." },
  { pattern: "dist/ build/ coverage/ storybook-static/", stacks: "Node · Python · Gradle · Dart", safety: "safe", why: "Build and test output." },
  { pattern: ".turbo/ .vite/ .parcel-cache/ .angular/ .webpack/ .cache/", stacks: "Node · any", safety: "safe", why: "Tool caches." },
  { pattern: "target/", stacks: "Rust · Maven", safety: "safe", why: "Compiler output and incremental caches." },
  { pattern: "__pycache__/ .pytest_cache/ .mypy_cache/ .ruff_cache/ .tox/ .nox/ *.egg-info/", stacks: "Python", safety: "safe", why: "Bytecode, test and lint caches." },
  { pattern: ".venv/ venv/", stacks: "Python", safety: "review", why: "Virtual environments may contain hand-installed packages." },
  { pattern: "bin/ obj/", stacks: ".NET", safety: "safe", why: "MSBuild output." },
  { pattern: ".dart_tool/", stacks: "Dart", safety: "safe", why: "Pub tooling cache." },
  { pattern: "Pods/ vendor/ .gradle/ deps/ .terraform/", stacks: "CocoaPods · Go · PHP · Ruby · Gradle · Elixir · Terraform", safety: "review", why: "Sometimes committed or patched on purpose." },
  { pattern: ".build/ _build/ dist-newstyle/ .stack-work/ zig-out/ zig-cache/", stacks: "Swift · Elixir · Haskell · Zig", safety: "safe", why: "Compiler output." },
  { pattern: "Library/ Temp/ Logs/", stacks: "Unity", safety: "safe", why: "Editor caches, regenerated when the project opens." },
];

function Section({ title, children, description }: { title: string; description?: string; children: React.ReactNode }) {
  return (
    <Card title={title}>
      {description && <p className="mb-2 text-[12px] text-fg-muted">{description}</p>}
      <div className="divide-y divide-border">{children}</div>
    </Card>
  );
}

function ListEditor({ items, onChange, placeholder, mono = true, validate }: { items: string[]; onChange: (v: string[]) => void; placeholder: string; mono?: boolean; validate?: (v: string) => string | null }) {
  const [draft, setDraft] = useState("");
  const [error, setError] = useState<string | null>(null);
  const add = () => {
    const v = draft.trim();
    if (!v) return;
    const err = validate?.(v) ?? null;
    if (err) {
      setError(err);
      return;
    }
    if (!items.includes(v)) onChange([...items, v]);
    setDraft("");
    setError(null);
  };
  return (
    <div className="flex flex-col gap-2 py-2">
      {items.length > 0 && (
        <ul className="divide-y divide-border rounded-md border border-border">
          {items.map((it) => (
            <li key={it} className="flex items-center gap-2 px-3 py-1.5">
              <span className={`flex-1 truncate text-[12px] text-fg selectable ${mono ? "font-mono" : ""}`}>{it}</span>
              <IconButton title="Remove" onClick={() => onChange(items.filter((x) => x !== it))}>
                <Trash2 size={13} />
              </IconButton>
            </li>
          ))}
        </ul>
      )}
      <div className="flex gap-2">
        <TextInput value={draft} onChange={setDraft} placeholder={placeholder} mono={mono} onKeyDown={(e) => e.key === "Enter" && add()} />
        <Button variant="outline" icon={<Plus size={13} />} onClick={add} disabled={!draft.trim()}>
          Add
        </Button>
      </div>
      {error && <div className="text-[11.5px] text-danger">{error}</div>}
    </div>
  );
}

const PROTECTED_NAMES = ["src", "app", "pages", "public", "assets", "uploads", "data", "migrations", ".git", ".env", "package.json", "Cargo.toml", "Cargo.lock"];

export function SettingsPage() {
  const settings = useAppStore((s) => s.settings);
  const save = useAppStore((s) => s.saveSettings);
  const info = useAppStore((s) => s.info);
  const setAddFoldersOpen = useAppStore((s) => s.setAddFoldersOpen);
  const removeScanRoot = useAppStore((s) => s.removeScanRoot);
  const backend = useAppStore((s) => s.backend);
  const cacheEntries = useAppStore((s) => s.cacheEntries);
  const clearCache = useAppStore((s) => s.clearCache);
  const exportProjects = useAppStore((s) => s.exportProjects);
  const copyDiagnostics = useAppStore((s) => s.copyDiagnostics);
  const projectCount = useAppStore((s) => s.projects.length);
  const [ruleDraft, setRuleDraft] = useState<{ pattern: string; safety: "safe" | "review"; stacks: Stack | "any"; explanation: string }>({ pattern: "", safety: "safe", stacks: "any", explanation: "" });
  const [preview, setPreview] = useState<{ matches: RuleMatch[]; loading: boolean; pattern: string }>({ matches: [], loading: false, pattern: "" });

  // Preview which folders a draft rule would match, debounced.
  useEffect(() => {
    const pattern = ruleDraft.pattern.trim();
    if (!backend || !pattern || pattern.includes("/") || pattern.includes("\\") || !projectCount) {
      setPreview({ matches: [], loading: false, pattern });
      return;
    }
    setPreview((p) => ({ ...p, loading: true, pattern }));
    const t = setTimeout(async () => {
      try {
        const matches = await backend.previewRule(pattern, ruleDraft.stacks === "any" ? [] : [ruleDraft.stacks]);
        setPreview({ matches, loading: false, pattern });
      } catch {
        setPreview({ matches: [], loading: false, pattern });
      }
    }, 400);
    return () => clearTimeout(t);
  }, [backend, ruleDraft.pattern, ruleDraft.stacks, projectCount]);

  const addRule = () => {
    const pattern = ruleDraft.pattern.trim();
    if (!pattern || pattern.includes("/") || pattern.includes("\\")) return;
    if (PROTECTED_NAMES.includes(pattern)) return;
    const rule: CleanupRule = {
      id: `custom-${pattern.replace(/[^a-z0-9]/gi, "-").toLowerCase()}-${Date.now().toString(36)}`,
      pattern,
      ecosystems: ruleDraft.stacks === "any" ? [] : [ruleDraft.stacks],
      safety: ruleDraft.safety,
      regeneratable: true,
      category: "other",
      explanation: ruleDraft.explanation.trim() || "Custom rule.",
      restoreHint: null,
      builtin: false,
    };
    save({ customRules: [...settings.customRules, rule] });
    setRuleDraft({ pattern: "", safety: "safe", stacks: "any", explanation: "" });
  };

  return (
    <div className="h-full overflow-y-auto p-5">
      <div className="mx-auto flex max-w-3xl flex-col gap-4">
        <Section title="General">
          <div className="flex items-center justify-between py-2">
            <div>
              <div className="text-[13px] font-medium text-fg">Theme</div>
              <div className="text-[12px] text-fg-muted">Follows the operating system by default.</div>
            </div>
            <Select<Theme> value={settings.theme} onChange={(theme) => save({ theme })} options={[{ value: "system", label: "System" }, { value: "light", label: "Light" }, { value: "dark", label: "Dark" }]} className="w-32" />
          </div>
          <Toggle checked={settings.rememberFolders} onChange={(v) => save({ rememberFolders: v })} label="Remember previous folders" description="Reopen with the last scan results and folders." />
          <div className="py-2">
            <div className="mb-1.5 flex items-center justify-between">
              <div>
                <div className="text-[13px] font-medium text-fg">Project folders</div>
                <div className="text-[12px] text-fg-muted">Scanned for projects. Nothing outside these folders is ever touched.</div>
              </div>
              <Button size="sm" variant="outline" icon={<FolderPlus size={13} />} onClick={() => setAddFoldersOpen(true)}>
                Add folder
              </Button>
            </div>
            {settings.scanRoots.length ? (
              <ul className="divide-y divide-border rounded-md border border-border">
                {settings.scanRoots.map((r) => (
                  <li key={r} className="flex items-center gap-2 px-3 py-1.5">
                    <Folder size={13} className="text-fg-muted" />
                    <span className="flex-1 truncate font-mono text-[12px] text-fg selectable">{r}</span>
                    <IconButton title="Remove" onClick={() => removeScanRoot(r)}>
                      <Trash2 size={13} />
                    </IconButton>
                  </li>
                ))}
              </ul>
            ) : (
              <div className="text-[12px] text-fg-subtle">No folders yet.</div>
            )}
          </div>
        </Section>

        <Section title="Scanning">
          <div className="flex items-center justify-between py-2">
            <div>
              <div className="text-[13px] font-medium text-fg">Maximum scan concurrency</div>
              <div className="text-[12px] text-fg-muted">Projects measured in parallel. 0 uses one thread per core (max 8).</div>
            </div>
            <NumberInput value={settings.maxConcurrency} onChange={(v) => save({ maxConcurrency: Math.max(0, Math.min(64, Math.round(v))) })} min={0} max={64} suffix="threads" />
          </div>
          <div className="flex items-center justify-between py-2">
            <div>
              <div className="text-[13px] font-medium text-fg">Dormant after</div>
              <div className="text-[12px] text-fg-muted">Projects with no commits or source edits for this long count as dormant.</div>
            </div>
            <NumberInput value={settings.dormantAfterDays} onChange={(v) => save({ dormantAfterDays: Math.max(1, Math.min(3650, Math.round(v))) })} min={1} max={3650} suffix="days" />
          </div>
          <Toggle checked={settings.followSymlinks} onChange={(v) => save({ followSymlinks: v })} label="Follow symbolic links" description="Off by default. Every folder is still visited at most once." />
          <Toggle checked={settings.scanHidden} onChange={(v) => save({ scanHidden: v })} label="Scan hidden folders for projects" description="Hidden artifact folders like .next/ are always measured." />
          <Toggle checked={settings.inspectGit} onChange={(v) => save({ inspectGit: v })} label="Check Git working tree status" description={info?.gitAvailable === false ? "git was not found on this machine." : "Runs `git status` per repository to flag uncommitted changes."} disabled={info?.gitAvailable === false} />
          <Toggle
            checked={settings.incrementalScans}
            onChange={(v) => save({ incrementalScans: v })}
            label="Incremental scans"
            description={`Reuse the size of node_modules/, target/ and other big folders when their contents have not changed. ${cacheEntries ? `${cacheEntries} folder${cacheEntries === 1 ? "" : "s"} cached.` : ""}`}
          />
          <div className="flex items-center justify-between py-2">
            <div>
              <div className="text-[13px] font-medium text-fg">Cached folder sizes</div>
              <div className="text-[12px] text-fg-muted">Clear them if a size looks wrong; the next scan measures everything again.</div>
            </div>
            <Button size="sm" variant="outline" icon={<Eraser size={13} />} onClick={clearCache} disabled={!cacheEntries}>
              Clear cache
            </Button>
          </div>
          <div className="flex items-center justify-between py-2">
            <div>
              <div className="text-[13px] font-medium text-fg">Scheduled scans</div>
              <div className="text-[12px] text-fg-muted">
                Rescan in the background while the app is open and notify when at least {formatBytes(settings.notifyThresholdBytes)} is reclaimable.
                {info?.platform === "browser" ? " (Not available in the browser preview.)" : ""}
              </div>
            </div>
            <Select<string>
              value={String(settings.scheduledScanHours)}
              onChange={(v) => save({ scheduledScanHours: Number(v) })}
              options={[
                { value: "0", label: "Off" },
                { value: "24", label: "Daily" },
                { value: "168", label: "Weekly" },
                { value: "720", label: "Monthly" },
              ]}
              className="w-32"
            />
          </div>
          {settings.scheduledScanHours > 0 && (
            <div className="flex items-center justify-between py-2">
              <div>
                <div className="text-[13px] font-medium text-fg">Notify when reclaimable is at least</div>
              </div>
              <Select<string>
                value={String(settings.notifyThresholdBytes)}
                onChange={(v) => save({ notifyThresholdBytes: Number(v) })}
                options={[
                  { value: "500000000", label: "500 MB" },
                  { value: "1000000000", label: "1 GB" },
                  { value: "5000000000", label: "5 GB" },
                  { value: "20000000000", label: "20 GB" },
                ]}
                className="w-32"
              />
            </div>
          )}
        </Section>

        <Section title="Safety" description="Hibernate never removes source files. This controls where the regeneratable folders go.">
          <div className="py-2">
            <RadioGroup<Disposition>
              value={settings.disposition}
              onChange={(disposition) => save({ disposition })}
              options={[
                { value: "trash", label: "Use Recycle Bin / Trash", description: info?.trashAvailable === false ? "The system Trash does not seem to be available here." : "Removed folders can be restored with the operating system's Trash." },
                { value: "quarantine", label: "Use quarantine", description: `Moved into ${info?.quarantineDir ?? "the app's data folder"} and restorable from History until they expire.` },
                { value: "permanent", label: "Permanent delete", description: "Removed immediately. Not reversible.", tone: "danger" },
              ]}
            />
          </div>
          <div className="flex items-center justify-between py-2">
            <div>
              <div className="text-[13px] font-medium text-fg">Quarantine retention</div>
              <div className="text-[12px] text-fg-muted">
                Expired quarantine batches are deleted when the app starts.
                {info && info.quarantineBytes > 0 ? ` Currently holding ${formatBytes(info.quarantineBytes)}.` : ""}
              </div>
            </div>
            <NumberInput value={settings.quarantineRetentionDays} onChange={(v) => save({ quarantineRetentionDays: Math.max(1, Math.min(365, Math.round(v))) })} min={1} max={365} suffix="days" />
          </div>
          <Toggle checked={settings.includeReviewItems} onChange={(v) => save({ includeReviewItems: v })} label="Include review items in bulk cleanup" description="Amber folders such as .venv/, Pods/ and vendor/. Off by default; you can also opt in per cleanup." />
        </Section>

        <Section title="Rules">
          <div className="py-2">
            <div className="mb-1.5 text-[13px] font-medium text-fg">Built-in cleanup rules</div>
            <ul className="divide-y divide-border rounded-md border border-border">
              {BUILTIN_RULES.map((r) => (
                <li key={r.pattern} className="grid grid-cols-[1fr_auto] gap-x-3 px-3 py-1.5 text-[12px]">
                  <div>
                    <div className="font-mono text-fg">{r.pattern}</div>
                    <div className="text-fg-subtle">
                      {r.stacks} · {r.why}
                    </div>
                  </div>
                  <SafetyBadge safety={r.safety} />
                </li>
              ))}
            </ul>
          </div>
          <div className="py-2">
            <div className="text-[13px] font-medium text-fg">Custom cleanup rules</div>
            <div className="mb-1.5 text-[12px] text-fg-muted">Folder names (with optional * wildcards) that are safe to remove in your projects.</div>
            {settings.customRules.length > 0 && (
              <ul className="mb-2 divide-y divide-border rounded-md border border-border">
                {settings.customRules.map((r) => (
                  <li key={r.id} className="flex items-center gap-2 px-3 py-1.5 text-[12px]">
                    <span className="font-mono text-fg">{r.pattern}/</span>
                    <Badge tone="neutral">{r.ecosystems.length ? r.ecosystems.map((s) => STACK_LABELS[s]).join(", ") : "All projects"}</Badge>
                    <SafetyBadge safety={r.safety} />
                    <span className="flex-1 truncate text-fg-subtle">{r.explanation}</span>
                    <Badge tone="neutral">{CATEGORY_LABELS[r.category]}</Badge>
                    <IconButton title="Remove rule" onClick={() => save({ customRules: settings.customRules.filter((x) => x.id !== r.id) })}>
                      <Trash2 size={13} />
                    </IconButton>
                  </li>
                ))}
              </ul>
            )}
            <div className="grid grid-cols-[1fr_120px_110px_1fr_auto] gap-2">
              <TextInput value={ruleDraft.pattern} onChange={(v) => setRuleDraft({ ...ruleDraft, pattern: v })} placeholder=".storybook-cache" mono />
              <Select<Stack | "any"> value={ruleDraft.stacks} onChange={(v) => setRuleDraft({ ...ruleDraft, stacks: v })} options={[{ value: "any", label: "All projects" }, ...(Object.keys(STACK_LABELS) as Stack[]).map((s) => ({ value: s, label: STACK_LABELS[s] }))]} />
              <Select<"safe" | "review"> value={ruleDraft.safety} onChange={(v) => setRuleDraft({ ...ruleDraft, safety: v })} options={[{ value: "safe", label: "Safe" }, { value: "review", label: "Review" }]} />
              <TextInput value={ruleDraft.explanation} onChange={(v) => setRuleDraft({ ...ruleDraft, explanation: v })} placeholder="Why is it safe?" />
              <Button variant="outline" icon={<Plus size={13} />} onClick={addRule} disabled={!ruleDraft.pattern.trim() || PROTECTED_NAMES.includes(ruleDraft.pattern.trim())}>
                Add
              </Button>
            </div>
            {ruleDraft.pattern.trim() && (
              <div className="mt-2 rounded-md border border-dashed border-border px-3 py-2 text-[12px]">
                <div className="mb-1 flex items-center gap-1.5 text-fg-muted">
                  {preview.loading ? <Loader2 size={12} className="animate-spin" /> : <Search size={12} />}
                  {!projectCount
                    ? "Scan your projects to preview what this rule would match."
                    : preview.loading
                      ? "Looking for matches in the last scan…"
                      : preview.matches.length
                        ? `Would match ${preview.matches.length}${preview.matches.length >= 40 ? "+" : ""} folder${preview.matches.length === 1 ? "" : "s"} · ${formatBytes(preview.matches.reduce((s, m) => s + m.bytes, 0))}`
                        : "No folder in the last scan matches this name."}
                </div>
                {preview.matches.slice(0, 8).map((m) => (
                  <div key={m.path} className="flex items-center justify-between gap-3 py-0.5">
                    <span className="truncate">
                      <span className="text-fg">{m.projectName}</span> <span className="font-mono text-fg-subtle">{m.relativePath}</span>
                    </span>
                    <span className="tabular text-fg-muted">{formatBytes(m.bytes)}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
          <div className="py-2">
            <div className="text-[13px] font-medium text-fg">Protected names</div>
            <div className="text-[12px] text-fg-muted">
              Never removed, on top of the built-in list ({PROTECTED_NAMES.slice(0, 6).join(", ")}, lockfiles, *.db, README*, …).
            </div>
            <ListEditor items={settings.protectedPatterns} onChange={(v) => save({ protectedPatterns: v })} placeholder="generated-sdk or *.secrets" />
          </div>
          <div className="py-2">
            <div className="text-[13px] font-medium text-fg">Ignored folders</div>
            <div className="text-[12px] text-fg-muted">Absolute paths the scanner never enters.</div>
            <ListEditor items={settings.ignoredPaths} onChange={(v) => save({ ignoredPaths: v })} placeholder={info?.platform === "windows" ? "C:\\Users\\me\\Projects\\archive" : "/home/me/Projects/archive"} />
          </div>
        </Section>

        <Section title="Data">
          <div className="flex items-center justify-between py-2">
            <div>
              <div className="text-[13px] font-medium text-fg">Export project table</div>
              <div className="text-[12px] text-fg-muted">Every scanned project with sizes, activity, Git state and artifacts.</div>
            </div>
            <div className="flex gap-2">
              <Button size="sm" variant="outline" icon={<Download size={13} />} onClick={() => exportProjects("csv")} disabled={!projectCount}>
                CSV
              </Button>
              <Button size="sm" variant="outline" icon={<Download size={13} />} onClick={() => exportProjects("json")} disabled={!projectCount}>
                JSON
              </Button>
            </div>
          </div>
          <div className="flex items-center justify-between py-2">
            <div>
              <div className="text-[13px] font-medium text-fg">Diagnostics</div>
              <div className="text-[12px] text-fg-muted">Version, platform, settings and scan warnings for bug reports. No file contents or project names.</div>
            </div>
            <Button size="sm" variant="outline" icon={<ClipboardCopy size={13} />} onClick={copyDiagnostics}>
              Copy diagnostics
            </Button>
          </div>
        </Section>

        <Card title="About">
          <dl className="grid grid-cols-[auto_1fr] gap-x-6 gap-y-1 text-[12.5px]">
            <dt className="text-fg-muted">Version</dt>
            <dd className="text-fg">{info?.version ?? "…"}</dd>
            <dt className="text-fg-muted">Data folder</dt>
            <dd className="truncate font-mono text-[12px] text-fg selectable">{info?.dataDir ?? "…"}</dd>
            <dt className="text-fg-muted">Command line</dt>
            <dd className="font-mono text-[12px] text-fg selectable">hibernate scan ~/Projects</dd>
          </dl>
          <p className="mt-3 text-[11.5px] text-fg-subtle">Local-first. No account, no telemetry, no file contents leave this machine.</p>
        </Card>
      </div>
    </div>
  );
}
