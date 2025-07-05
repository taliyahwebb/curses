import { BaseDirectory, exists, readFile, writeFile } from "@tauri-apps/plugin-fs";
import IMigration from "../IMigration";

const SETTINGS_FILE = {
  PATH: "user/settings",
  BASE_DIR: BaseDirectory.AppConfig,
} as const;
const NATIVE = "native";
const WEBSPEECHAPI = "webspeechapi";

export default {
  isApplicable: async () => await exists(SETTINGS_FILE.PATH, { baseDir: SETTINGS_FILE.BASE_DIR }),

  isStillValid: (..._) => true,

  apply: async () => {
    // Load
    const data = await readFile(SETTINGS_FILE.PATH, { baseDir: SETTINGS_FILE.BASE_DIR });
    const state = JSON.parse(new TextDecoder().decode(data));

    // Replace
    state.services.stt.data[WEBSPEECHAPI] = state.services.stt.data[NATIVE];
    delete state.services.stt.data[NATIVE];

    state.services.tts.data[WEBSPEECHAPI] = state.services.tts.data[NATIVE];
    delete state.services.tts.data[NATIVE];

    // If currently selected
    if (state.services.stt.data.backend === NATIVE)
      state.services.stt.data.backend = WEBSPEECHAPI;

    if (state.services.tts.data.backend === NATIVE)
      state.services.tts.data.backend = WEBSPEECHAPI;

    // Save
    const json = JSON.stringify(state, null, 4);
    await writeFile(
      SETTINGS_FILE.PATH,
      new TextEncoder().encode(json),
      { baseDir: SETTINGS_FILE.BASE_DIR },
    );

    return new Object();
  }
} as IMigration;
