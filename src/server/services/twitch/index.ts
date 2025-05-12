import { IServiceInterface, ServiceNetworkState, TextEventType } from "@/types";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { Event, UnlistenFn } from "@tauri-apps/api/event";
import { ApiClient, HelixUser } from "@twurple/api";
import { StaticAuthProvider } from "@twurple/auth";
import { proxy } from "valtio";
import { subscribeKey } from "valtio/utils";
import {
  serviceSubscribeToInput,
  serviceSubscribeToSource,
} from "../../../utils";
import TwitchChatApi from "./chat";
import TwitchEmotesApi from "./emotes";
import { toast } from "react-toastify";
import { t } from "i18next";
const scope = ["chat:read", "chat:edit", "channel:read:subscriptions"];

class Service_Twitch implements IServiceInterface {
  authProvider?: StaticAuthProvider;
  constructor() {}

  emotes!: TwitchEmotesApi;
  chat!: TwitchChatApi;

  liveCheckInterval?: any = null;

  apiClient?: ApiClient;

  unlistener: UnlistenFn = () => {};

  state = proxy<{
    user: HelixUser | null;
    liveStatus: ServiceNetworkState;
  }>({
    liveStatus: ServiceNetworkState.disconnected,
    user: null,
  });

  get #state() {
    return window.ApiServer.state.services.twitch;
  }

  async init() {
    this.emotes = new TwitchEmotesApi();
    this.chat = new TwitchChatApi();
    // check live status
    setInterval(() => this.#checkLive(), 4000);

    // login with token
    this.connect();

    subscribeKey(this.#state.data, "chatEnable", (value) => {
      if (value) {
        if (this.state.user && this.authProvider)
          this.chat.connect(this.state.user.name, this.authProvider);
      } else this.chat.disconnect();
    });

    serviceSubscribeToSource(this.#state.data, "chatPostSource", (data) => {
      if (
        this.#state.data.chatPostLive &&
        this.state.liveStatus !== ServiceNetworkState.connected
      )
        return;
      this.#state.data.chatPostEnable &&
        data?.value &&
        data?.type === TextEventType.final &&
        this.chat.post(data.value);
    });

    serviceSubscribeToInput(this.#state.data, "chatPostInput", (data) => {
      if (
        this.#state.data.chatPostLive &&
        this.state.liveStatus !== ServiceNetworkState.connected
      )
        return;
      this.#state.data.chatPostEnable &&
        data?.textFieldType !== "twitchChat" &&
        data?.value &&
        data?.type === TextEventType.final &&
        this.chat.post(data.value);
    });
  }

  async login() {
    // Using [Implicit grant flow](https://dev.twitch.tv/docs/authentication/getting-tokens-oauth/#implicit-grant-flow).
    const link = new URL("https://id.twitch.tv/oauth2/authorize");
    link.searchParams.set(
      "client_id",
      import.meta.env.CURSES_TWITCH_CLIENT_ID
    );
    link.searchParams.set(
      "redirect_uri",
      `http://localhost:${window.Config.serverNetwork.port}/oauth_twitch.html`
    );
    link.searchParams.set("response_type", "token");
    link.searchParams.set("scope", scope.join("+"));
    link.search = decodeURIComponent(link.search);

    const auth_window = new WebviewWindow("oauth_twitch", {
      url: link.toString(),
      width: 600,
      height: 600,
      title: t('twitch.auth_window_title'),
      devtools: import.meta.env.DEV, // true if vite is in dev mode
    });
    auth_window.once("tauri://created", () => {});
    auth_window.once("tauri://error", (err) => toast.error(`Error creating window: ${err.payload}`));

    const thisRef = this;
    const handleEvent = (event: Event<string>) => {
      thisRef.unlistener();

      thisRef.#state.data.token = event.payload;
      thisRef.connect();
    };

    this.unlistener(); // in case there was somehow an uncalled unlistener
    this.unlistener = await auth_window.listen<string>("twitch_token", handleEvent);
  }

  logout() {
    window.ApiServer.state.services.twitch.data.token = "";
    this.chat.dispose();
    delete this.apiClient;
    delete this.authProvider;
    this.emotes.dispose();
    this.state.user = null;
    this.state.liveStatus = ServiceNetworkState.disconnected;
  }

  async #checkLive() {
    if (!this.state.user?.name) {
      this.state.liveStatus = ServiceNetworkState.disconnected;
      return;
    }
    try {
      const resp = await this.apiClient?.streams.getStreamByUserName(
        this.state.user.name
      );
      // window.ApiShared.pubsub.publishLocally({topic: "stream.on_started"});
      const prevStatus = this.state.liveStatus;
      this.state.liveStatus = !!resp
        ? ServiceNetworkState.connected
        : ServiceNetworkState.disconnected;
      // stream ended
      if (prevStatus === ServiceNetworkState.connected && this.state.liveStatus == ServiceNetworkState.disconnected) {
        window.ApiShared.pubsub.publishLocally({topic: "stream.on_ended"});
      }
    } catch (error) {
      this.state.liveStatus = ServiceNetworkState.disconnected;
    }
  }

  async connect() {
    try {
      if (!this.#state.data.token) return this.logout();

      this.authProvider = new StaticAuthProvider(
        import.meta.env.CURSES_TWITCH_CLIENT_ID,
        this.#state.data.token,
        scope
      );

      this.apiClient = new ApiClient({ authProvider: this.authProvider });
      const tokenInfo = await this.apiClient.getTokenInfo();
      if (!tokenInfo.userId) return this.logout();

      const me = await this.apiClient?.users.getUserById({
        id: tokenInfo.userId,
      });

      if (!me) return this.logout();

      this.state.user = me;

      // initial live check
      this.#checkLive();

      this.emotes.loadEmotes(me.id, this.apiClient);
      if (this.#state.data.chatEnable)
        this.chat.connect(me.name, this.authProvider);
    } catch (error) {
      this.logout();
    }
  }
}

export default Service_Twitch;
