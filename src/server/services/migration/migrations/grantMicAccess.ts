import IMigration from "../IMigration";
import { invoke } from "@tauri-apps/api/core";

export default {
  isApplicable: () => true, // worst case scenario we set the permission twice

  apply: async () => {
    await invoke<void>("grant_mic_access", { origin: "https://tauri.localhost" });
    // dev url
    await invoke<void>("grant_mic_access", { origin: "http://localhost:1420" });
  }
} as IMigration;
