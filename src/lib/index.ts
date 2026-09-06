import type { Backend } from "./backend";
import { isTauri } from "./backend";

let instance: Backend | null = null;

/** Lazily pick the backend so the mock never loads inside the desktop app. */
export async function getBackend(): Promise<Backend> {
  if (instance) return instance;
  if (isTauri()) {
    const { tauriBackend } = await import("./tauri-backend");
    instance = tauriBackend;
  } else {
    const { createMockBackend } = await import("./mock-backend");
    instance = createMockBackend();
  }
  return instance;
}

export type { Backend } from "./backend";
