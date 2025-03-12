import { zSafe, zStringNumber } from "@/utils";
import { z } from "zod";

export enum STT_Backends {
  webspeechapi = "webspeechapi",
  browser = "browser",
  azure = "azure",
  deepgram = "deepgram",
  speechly = "speechly",
  whisper = "whisper",
}

export const zodSTT_Backends = z.nativeEnum(STT_Backends);

export const Service_STT_Schema = z.object({
  backend: zSafe(zodSTT_Backends, STT_Backends.webspeechapi),
  autoStart: zSafe(z.coerce.boolean(), false),
  uwu: zSafe(z.coerce.boolean(), false),
  stopWithStream: zSafe(z.coerce.boolean(), false),
  replaceWords: zSafe(z.record(z.coerce.string(), z.coerce.string()), {}),
  replaceWordsIgnoreCase: zSafe(z.coerce.boolean(), false),
  replaceWordsPreserveCase: zSafe(z.coerce.boolean(), false),
  webspeechapi: z.object({
    language_group: zSafe(z.coerce.string(), ""),
    language: zSafe(z.coerce.string(), ""),
  }).default({}),
  azure: z.object({
    device: zSafe(z.coerce.string(), "default"),
    language_group: zSafe(z.coerce.string(), ""),
    language: zSafe(z.coerce.string(), ""),
    secondary_language_group: zSafe(z.coerce.string(), ""),
    secondary_language: zSafe(z.coerce.string(), ""),
    use_secondary_language: zSafe(z.coerce.boolean(), true),
    key: zSafe(z.coerce.string(), ""),
    location: zSafe(z.coerce.string(), ""),
    profanity: zSafe(z.coerce.string(), "masked"),
    silenceTimeout: zSafe(zStringNumber(), "20"),
    interim: zSafe(z.coerce.boolean(), true),
  }).default({}),
  speechly: z.object({
    device: zSafe(z.coerce.string(), ""),
    key: zSafe(z.coerce.string(), ""),
  }).default({}),
  whisper: z.object({
    device: zSafe(z.coerce.string(), "default"),
    modelPath: zSafe(z.coerce.string(), ""),
    lang: zSafe(z.coerce.string(), "auto"),
    translateToEnglish: zSafe(z.coerce.boolean(), false),
  }).default({}),
  deepgram: z.object({
    device: zSafe(z.coerce.string(), "default"),
    language_group: zSafe(z.coerce.string(), ""),
    language: zSafe(z.coerce.string(), ""),
    tier: zSafe(z.coerce.string(), ""),
    key: zSafe(z.coerce.string(), ""),
    punctuate: zSafe(z.coerce.boolean(), true),
    profanity: zSafe(z.coerce.boolean(), true),
    interim: zSafe(z.coerce.boolean(), true),
  }).default({})
}).default({});

export type STT_State = z.infer<typeof Service_STT_Schema>;
