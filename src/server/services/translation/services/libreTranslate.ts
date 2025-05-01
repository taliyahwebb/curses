import { TextEvent, TextEventType } from "@/types";
import { Translation_State } from "../schema";
import {
  ITranslationReceiver,
  ITranslationService
} from "../types";
import { toast } from "react-toastify";

type ResponseValid = { translatedText: string };
type ResponseError = { error: string };

export class Translation_LibreTranslateService implements ITranslationService {
  constructor(private receiver: ITranslationReceiver) {}

  dispose(): void {}

  start(state: Translation_State): void {
    this.receiver.onStart();
  }

  get state() {
    return window.ApiServer.state.services.translation.data.libreTranslate;
  }

  async translate(id: number, text: TextEvent) {
    if (text.type === TextEventType.interim && !this.state.interim)
      return;
    let key = this.state.key;

    const link = new URL(
      "https://translate.flossboxin.org.in/translate"
    );

    const resp = await fetch(link, {
      method: "POST",
      body: JSON.stringify({
        q: text.value,
        source: this.state.languageFrom,
        target: this.state.languageTo,
        alternatives: 0,
        api_key: this.state.key,
      }),
      headers: { "Content-Type": "application/json" },
    }).catch(toast.error) as Response;

    const data: ResponseValid | ResponseError = await resp.json();

    if ('error' in data) return toast.error(data.error);

    this.receiver.onTranslation(id, text, (data as ResponseValid).translatedText);
  }

  stop(): void {
    this.receiver.onStop();
  }
}
