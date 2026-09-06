import { useState } from "react";
import { Folder, FolderPlus, Search, Trash2 } from "lucide-react";
import { Button, IconButton } from "@/components/common/Button";
import { Dialog } from "@/components/common/Dialog";
import { TextInput } from "@/components/common/Controls";
import { useAppStore } from "@/stores/app-store";

const SUGGESTED = ["~/Projects", "~/Developer", "~/Code", "~/Documents/Projects"];

export function AddFoldersDialog() {
  const open = useAppStore((s) => s.addFoldersOpen);
  const setOpen = useAppStore((s) => s.setAddFoldersOpen);
  const roots = useAppStore((s) => s.settings.scanRoots);
  const addScanRoots = useAppStore((s) => s.addScanRoots);
  const removeScanRoot = useAppStore((s) => s.removeScanRoot);
  const startScan = useAppStore((s) => s.startScan);
  const backend = useAppStore((s) => s.backend);
  const home = useAppStore((s) => s.info?.homeDir);
  const [typed, setTyped] = useState("");
  const [busy, setBusy] = useState(false);

  const pick = async () => {
    if (!backend) return;
    setBusy(true);
    try {
      const picked = await backend.pickFolders();
      if (picked.length) await addScanRoots(picked);
    } finally {
      setBusy(false);
    }
  };
  const addTyped = async () => {
    const v = typed.trim();
    if (!v) return;
    const expanded = home && v.startsWith("~") ? home + v.slice(1) : v;
    await addScanRoots([expanded]);
    setTyped("");
  };

  return (
    <Dialog
      open={open}
      onClose={() => setOpen(false)}
      title="Project folders"
      subtitle="Folders that contain your repositories. Each project inside is detected automatically."
      footer={
        <>
          <Button variant="ghost" onClick={() => setOpen(false)}>
            Close
          </Button>
          <Button
            variant="primary"
            icon={<Search size={14} />}
            disabled={!roots.length}
            onClick={() => {
              setOpen(false);
              startScan();
            }}
          >
            Scan {roots.length ? `${roots.length} folder${roots.length === 1 ? "" : "s"}` : ""}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-3">
        {roots.length ? (
          <ul className="divide-y divide-border rounded-md border border-border">
            {roots.map((r) => (
              <li key={r} className="flex items-center gap-2 px-3 py-2">
                <Folder size={14} className="shrink-0 text-fg-muted" />
                <span className="flex-1 truncate font-mono text-[12px] text-fg selectable" title={r}>
                  {r}
                </span>
                <IconButton title="Remove folder" onClick={() => removeScanRoot(r)}>
                  <Trash2 size={14} />
                </IconButton>
              </li>
            ))}
          </ul>
        ) : (
          <p className="rounded-md border border-dashed border-border-strong px-3 py-4 text-center text-[12.5px] text-fg-muted">No folders yet. Pick the folder where you keep your projects.</p>
        )}
        <div className="flex gap-2">
          <Button variant="secondary" icon={<FolderPlus size={14} />} onClick={pick} loading={busy}>
            Choose folder…
          </Button>
          <TextInput value={typed} onChange={setTyped} placeholder="…or type a path and press Enter" mono onKeyDown={(e) => e.key === "Enter" && addTyped()} />
          <Button variant="outline" onClick={addTyped} disabled={!typed.trim()}>
            Add
          </Button>
        </div>
        <div className="flex flex-wrap items-center gap-1.5 text-[11.5px] text-fg-subtle">
          <span>Common:</span>
          {SUGGESTED.map((s) => (
            <button key={s} type="button" onClick={() => setTyped(s)} className="rounded border border-border bg-surface-2 px-1.5 py-0.5 font-mono text-[11px] text-fg-muted hover:text-fg">
              {s}
            </button>
          ))}
        </div>
      </div>
    </Dialog>
  );
}
