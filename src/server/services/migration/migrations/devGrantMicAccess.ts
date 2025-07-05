import IMigration from "../IMigration";
import { invoke } from "@tauri-apps/api/core";

type Data = { origin: string };

export default {
  // worst case scenario we set the permission twice
  isApplicable: () => Promise.resolve(import.meta.env.DEV),

  isStillValid: (_version, data) => (<Data>data).origin === window.origin,

  apply: async () => {
    await invoke<void>("grant_mic_access", { origin: window.origin });
    return { origin: window.origin };
  },
} as IMigration;
