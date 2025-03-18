import { IServiceInterface } from "@/types";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  BaseDirectory,
  mkdir,
  exists,
  readFile,
  writeFile,
} from "@tauri-apps/plugin-fs";
import { appDataDir } from '@tauri-apps/api/path';
import { Binder, bind } from "immer-yjs";
import debounce from "lodash/debounce";
import { toast } from "react-toastify";
import * as Y from "yjs";
import { ElementType } from "../../elements/schema";
import { DocumentSchema, DocumentState } from "../../schema";

class Service_Document implements IServiceInterface {
  #file: Y.Doc = new Y.Doc();
  fileBinder!: Binder<DocumentState>;

  get template() {
    return this.#file?.getMap("template");
  }

  get fileArray() {
    return this.#file.getArray<Uint8Array>("files");
  }

  get file() {
    return this.#file;
  }

  createNewState() {
    this.patch(state => {
      const newState = DocumentSchema.parse({});
      let k: keyof DocumentState;
      for (k in newState)
        this.#patchField(state, newState, k);
    });
    const canvas = this.fileBinder.get().canvas;
    // add default text element
    const eleWidth = canvas.w - 100;
    window.ApiClient.elements.addElement(ElementType.text, "main", {
      w: eleWidth,
      h: 65,
      x: (canvas.w - eleWidth) / 2,
      y: (canvas.h - 65) / 2,
      r: 0
    });
  }
  
  // i hate this
  #patchField<Key extends keyof DocumentState>(og: DocumentState, patch: DocumentState, key: Key) {
    og[key] = patch[key];
  }

  patchState(immerState: DocumentState, newState: DocumentState){
    // trigger immer-yjs generator
    let k: keyof DocumentState;
    for (k in newState)
      this.#patchField(immerState, newState, k);
    // remove fields
    for (let k in immerState) {
      if (!(k in newState))
        delete immerState[k as keyof DocumentState]
    }
  }

  async init() {
    this.#file.getArray<Uint8Array>("files");
    this.fileBinder = bind<DocumentState>(this.#file.getMap("template"));
    
    if (window.Config.isClient()) {
      // wait for initial push from server
      await new Promise((res, rej) => {
        this.#file.once("update", res);
      });
      return;
    }

    const loadState = await this.loadDocument();
    if (loadState) {
      Y.applyUpdate(this.#file, loadState);
      this.patch((state) => {
        const patchState = DocumentSchema.safeParse(state);
        if (patchState.success) {
          this.patchState(state, patchState.data);
        }
        else {
          toast.error("Invalid template");
          this.createNewState();
        }
      });
    }
    else {
      this.createNewState();
    }
    this.saveDocument();
    this.#file.on("afterTransaction", (e) => {
      this.saveDocument();
    });
  }

  async importDocument() {
    const path = await open({
      filters: [
        {
          name: "Curses template",
          extensions: ["cursestmp"],
        },
      ],
      defaultPath: await appDataDir(),
    });
    if (!path || Array.isArray(path)) return;
    const data = await readFile(path);
    const tempDoc = new Y.Doc();
    let binder: Binder<DocumentState> = bind<DocumentState>(tempDoc.getMap("template"));
    try {
      // try import and patch state
      Y.applyUpdate(tempDoc, data);
      tempDoc.transact(() => {
        binder.update(state => {
          const patchState = DocumentSchema.safeParse(state);
          if (patchState.success) {
            this.patchState(state, patchState.data);
            this.#saveDocumentNative(tempDoc).then(() => window.location.reload())
          }
          else
            toast.error("Couldn't parse template");
        })
      });
    } catch (error) {
      toast.error(`Invalid template: ${error}`);
    }
  }
  async exportDocument(authorName: string) {
    // clone doc
    const tempDoc = new Y.Doc();
    const encodedUpdate = Y.encodeStateAsUpdate(this.#file);
    Y.applyUpdate(tempDoc, encodedUpdate);
    // apply author name to temp
    tempDoc.getMap("template").set("author", authorName);

    const tempEncodedUpdate = Y.encodeStateAsUpdate(tempDoc);
    let path = await save({
      filters: [
        {
          name: "Curses template",
          extensions: ["cursestmp"],
        },
      ],
      defaultPath: await appDataDir(),
    });
    if (path) try {
      if (!path.endsWith(".cursestmp"))
        path += ".cursestmp";

      await writeFile(path, tempEncodedUpdate, {append: false});
      // write author to original doc on success
      this.fileBinder.update(a => {a.author = authorName});
    } catch (error) {
      toast.error(`Error writing file: ${error}`);
    }
  }

  async loadDocument(): Promise<Uint8Array | undefined> {
    if (window.Config.isClient()) {
      return;
    }

    const bExists = await exists("user/template", {
      baseDir: BaseDirectory.AppConfig,
    });
    if (bExists) try {
      const data = await readFile("user/template", {
        baseDir: BaseDirectory.AppConfig,
      });
      return data;
    } catch (error) {
      toast("Error loading template", { type: "error" });
    }
  }

  async #saveDocumentNative(doc: Y.Doc) {
    const bExists = await exists("user", { baseDir: BaseDirectory.AppConfig });
    if (!bExists)
      await mkdir("user", { baseDir: BaseDirectory.AppConfig, recursive: true });
    const data = Y.encodeStateAsUpdate(doc);
    await writeFile("user/template", data, {append: false, baseDir: BaseDirectory.AppConfig});
  }

  saveDocument = debounce(() => {
    if (window.Config.isClient())
      return;
    this.#saveDocumentNative(this.#file);
  }, 2000);

  patch(patchFn: (state: DocumentState) => void) {
    this.file.transact((_) => {
      this.fileBinder.update(patchFn);
    });
  }
}
export default Service_Document;
