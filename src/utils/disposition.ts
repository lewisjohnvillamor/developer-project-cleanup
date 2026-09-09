import type { Disposition } from "@/types";

/**
 * What actually happened to the bytes. Only a permanent delete frees space
 * immediately: items in the Trash or in quarantine still occupy the disk
 * until they are emptied or expire, so calling that "freed" would be a
 * claim the user can check and find false.
 */
export const DISPOSITION_VERB: Record<Disposition, string> = {
  trash: "moved to Trash",
  quarantine: "moved to quarantine",
  permanent: "freed",
};

/** The step still needed before the space is actually available, if any. */
export const DISPOSITION_CAVEAT: Record<Disposition, string | null> = {
  trash: "Empty your Trash to free this space.",
  quarantine: "Held in quarantine so you can restore it; it frees up when the batch expires.",
  permanent: null,
};
