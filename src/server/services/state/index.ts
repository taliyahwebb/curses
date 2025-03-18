import { IServiceInterface } from "@/types";
import { BaseDirectory, mkdir, exists, readFile, writeFile } from "@tauri-apps/plugin-fs";
import debounce from "lodash/debounce";
import { proxy, subscribe } from "valtio";
import { BackendState, BackendSchema } from "../../schema";

class Service_State implements IServiceInterface {
  state!: BackendState;

  async init() {
    let data = await this.#load_state();
    const hasData = !!data;
    // create new state
    if (!hasData) {
      data = {};
    }
    
    this.state = proxy(BackendSchema.parse(data));
    !hasData && this.#save_state();
    subscribe(this.state, () => this.#save_state());
  }

  private tryParseState(str: string | null) {
    if (str) try {
      const parse = JSON.parse(str);
      if (typeof parse !== "object") return;
      return parse;
    } catch (error) {
      console.error("invalid state data");
    }
  }

  async #load_state(): Promise<Record<string, any> | undefined> {
    const decoder = new TextDecoder();
    const fileExists = await exists("user/settings", {baseDir: BaseDirectory.AppConfig});
    if (!fileExists)
      return;
    try {
      const data = await readFile("user/settings", {baseDir: BaseDirectory.AppConfig});
      return this.tryParseState(decoder.decode(data));
    } catch (error) {
      return;
    }
  }

  #save_state = debounce(async () => {
    const encoder = new TextEncoder();
    const bExists = await exists("user", { baseDir: BaseDirectory.AppConfig });
    if (!bExists)
      await mkdir("user", { baseDir: BaseDirectory.AppConfig, recursive: true });
    const value = JSON.stringify(this.state, null, 4);
    await writeFile("user/settings", encoder.encode(value), {append: false, baseDir: BaseDirectory.AppConfig});
  }, 1000);
}
export default Service_State;
