import { invoke } from "@tauri-apps/api/core";
import { isEmptyValue } from "../../../../utils";
import { TTS_State } from "../schema";
import { ITTSReceiver, ITTSService } from "../types";
import { toast } from "react-toastify";

export class TTS_PiperService implements ITTSService {

    constructor(private bindings: ITTSReceiver) { }

    // we use a promise as a mutex to ensure that
    // concurrent calls to `play` resolve one after the other.
    private mutex = Promise.resolve();

    dispose(): void { }

    get state() {
        return window.ApiServer.state.services.tts.data.piper;
    }

    start(state: TTS_State): void {
        let i = 0;
        for (var val in Object.values(this.state)) {
            if (isEmptyValue(val))
                return this.bindings.onStop(`Option '${Object.keys(state)[i]}' is missing`);
            i += 1;
        }
        this.bindings.onStart();
    }

    async play(value: string) {
        this.mutex = this.mutex.then(async () => {
            await invoke<void>("plugin:piper-tts|speak", {
                args: {
                    device: this.state.device,
                    exe_path: this.state.exe_location,
                    voice_path: this.state.voice,
                    speaker_id: this.state.speaker_id,
                    value,
                },
            }).catch(err => {
                toast.error(err)
            })
        });
    }

    stop(): void {
        invoke<void>("plugin:piper_tts|stop").finally(() => {
            this.bindings.onStop();
        });
    }
}
