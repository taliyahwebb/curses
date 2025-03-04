import { ISTTReceiver, ISTTService } from "../types";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { isEmptyValue } from "../../../../utils";
import { STT_State } from "../schema";
import { toast } from "react-toastify";

export class STT_WhisperService implements ISTTService {
  constructor(private bindings: ISTTReceiver) {}

  #initialized: boolean = false;

  dispose(): void {}

  get state() {
    return window.ApiServer.state.services.stt.data.whisper
  }

  async start(state: STT_State) {
    if (this.#initialized) {
      toast.warn("whisper stt is already running");
    }

    for (const [key, value] of Object.entries(state.whisper)) {
        if (isEmptyValue(value) && !(key === "binary_path"))
            return this.bindings.onStop(`Option '${key}' is missing`);
    }

    this.#initialized = true;
    this.bindings.onStart();
    const stop_final_callback = await listen<string>("whisper_stt_final", (event) => {
      this.bindings.onFinal(event.payload);
    });
    const stop_interim_callback = await listen<string>("whisper_stt_interim", (event) => {
      this.bindings.onInterim(event.payload);
    });
    /// the rust backend function will only return when an error occured or stop() was issued
    await invoke<void>("plugin:whisper-stt|start", {
        args: {
            inputDevice: this.state.device,
            modelPath: this.state.modelPath,
            lang: this.state.lang,
            translateToEnglish: this.state.translateToEnglish,
        },
    }).catch(err => {
        this.#initialized = false;
        toast.error(JSON.stringify(err));
        // needed as the rust part can't reset itself when it errored
        invoke<void>("plugin:whisper-stt|stop");
    }).finally(() => {
      this.#initialized = false;
      this.bindings.onStop();
      stop_final_callback();
      stop_interim_callback();
    });
  }

  stop(): void {
    invoke<void>("plugin:whisper-stt|stop");
  }
}
