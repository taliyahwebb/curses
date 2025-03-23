<p align="center">
<img height="60" src="https://user-images.githubusercontent.com/3977499/218317016-0ebd9936-4065-4b6b-a0a0-1199d43c0024.svg">
</p>
<p align="center">
  <a href="https://github.com/taliyahwebb/curses/releases/"><img src="https://img.shields.io/github/release/taliyahwebb/curses.svg?color=FC6471&label" alt="Node.js CI"></a>
</p>
<p align="center">Speech to Text Captions for OBS, VRChat, Twitch chat and Discord</p>

<p align="center">
  <!-- TODO change image location to master -->
  <img width="600" src="https://github.com/taliyahwebb/curses/blob/dev/.github/assets/readme-sugar.gif">
</p>

<p align="center"><b>Repo Stats</b></p>
<p align="center">
  <img alt="GitHub repo size" src="https://img.shields.io/github/repo-size/taliyahwebb/curses?color=2EB87D"/>
  <img alt="GitHub language count" src="https://img.shields.io/github/languages/count/taliyahwebb/curses?color=2EB87D"/>
  <img alt="GitHub top language" src="https://img.shields.io/github/languages/top/taliyahwebb/curses?color=2EB87D"/>
  <img alt="GitHub last commit" src="https://img.shields.io/github/last-commit/taliyahwebb/curses?color=2EB87D"/>
</p>

<!--toc:start-->
- [Features](#features)
  - [Roadmap](#roadmap)
- [Community](#community)
- [Usage](#usage)
  - [Runtime Dependencies](#runtime-dependencies)
    - [Web renderer](#web-renderer)
    - [Whisper STT](#whisper-stt)
  - [STT services](#stt-services)
    - [Web Speech API (STT)](#web-speech-api-stt)
    - [Whisper](#whisper)
    - [Browser](#browser)
    - [Azure (STT)](#azure-stt)
    - [Deepgram](#deepgram)
    - [Speechly](#speechly)
  - [TTS services](#tts-services)
    - [Web Speech API (TTS)](#web-speech-api-tts)
      - [Changing output device](#changing-output-device)
    - [Piper](#piper)
    - [Windows (TTS)](#windows-tts)
    - [Azure (TTS)](#azure-tts)
    - [TikTok](#tiktok)
    - [Uberduck](#uberduck)
    - [Custom TTS](#custom-tts)
- [Building](#building)
  - [Prerequisites](#prerequisites)
    - [NixOS](#nixos)
    - [Other Linux](#other-linux)
    - [Windows](#windows)
  - [Build](#build)
<!--toc:end-->

# Features
[Instructions and details](#usage)
- **[Speech to Text](#stt-services)**
- **[Text to Speech](#tts-services)**
- **OBS Captions customization**: Colors, fonts, shadows, background textures, text typing animation, sound effects, particle effects and CSS
- **Native OBS stream captions**
- **Google Fonts**: more than 1000 free fonts for OBS captions
- **VRChat**: [KillFrenzy Avatar text](https://github.com/killfrenzy96/KillFrenzyAvatarText), vrchat's chatbox
- **Twitch**:
  - Use 7TV/FFZ/BTTV emotes in OBS captions
  - Post your STT to chat
  - Use your chat messages as a source for captions and TTS
  - native captions
- **Discord**: Send your STT to specified channel
- **Scenes**:
  - Save multiple designs and freely switch between them
  - Automatically switch design when OBS changes scene

## Roadmap
- see [Github Milestones](https://github.com/taliyahwebb/curses/milestones)

# Community
For help, feature requests, bug reports, release notifications, design templates, [Join Discord](https://discord.gg/Sw6pw8fGYS).

<a href="https://discord.gg/Sw6pw8fGYS"><img src="https://discordapp.com/api/guilds/856500849815060500/widget.png?style=banner2"/></a>

# Usage
## Runtime Dependencies
### Web renderer
**On Windows, Edge WebView2 is required to render the app**. This is done to make the app smaller on disk.

If you're not running an old Windows version and didn't accidentally remove it trying to debloat your computer, **it should already be installed**.
Otherwise, you can download it from [here](https://developer.microsoft.com/en-us/microsoft-edge/webview2/#download).

### Whisper STT
If you want to use the STT module [Whisper](#whisper) you will need a Vulkan ready graphics driver installed.

- NixOS: if you are using a recent NixOS version and have a graphical user environment enabled, it will likely ✨just work✨ if your hardware supports Vulkan
- Other Linux: check your distributions documentation or see [Arch Linux Wiki](https://wiki.archlinux.org/title/Vulkan) for more information
- Windows: having up to date Graphics drivers should suffice if the hardware supports it

Here is a list of [Vulkan ready devices](https://vulkan.gpuinfo.org/). Most modern Graphics drivers should support Vulkan.

## STT services
**Every service has its pros and cons. I'd advice to read about them all before making your choice.**

### Web Speech API (STT)
Web Speech API is a general specification for web browsers to support both speech synthesis and recognition. Its implementation and voices available change depending on your operating system.

<details>
<summary>Windows</summary>
We get the Web Speech API through Edge WebView2.

Edge WebView2 (probably) uses cloud services to provide Speech-To-Text to the Web Speech API (can't be sure because it's closed-source).
</details>

<details>
<summary>Linux</summary>
We get the Web Speech API through WebKitGTK.

> **WebKitGTK does not support the speech recognition of Web Speech API yet**, but everything should work as soon as the feature gets released.
> There have been experimentations by the WebKitGTK team to use Whisper.cpp, but ["that is much farther down the roadmap"](https://matrix.to/#/#webkitgtk:matrix.org/$PQpUpl13RWnMzowuj9Ylk_zJ_0-5uajLDa20n0vCs1o) (2025/03/08).
</details>

### Whisper
[`whisper.cpp`](https://github.com/ggerganov/whisper.cpp) is a port OpenAI's Whisper.

It works locally, without going through OpenAI's servers, and also supports GPU acceleration, with a pretty small performance cost.
You can also automatically translate to english at the same time.

You're going to need to [download a model (`.bin`)](https://github.com/ggerganov/whisper.cpp) (or [learn how to download more models](https://github.com/ggerganov/whisper.cpp)), and select it in the *Whisper Model* field.

Smaller models have a smaller performance impact, but larger models are more accurate. There are also english-only models (files with `.en`), all others being multilingual. `-q5_0` models take less memory and disk space and *can* be more efficient. `-tdrz` models can detect speaker changes but are more resource-intensive.

> [!TIP]
> The `base.en-q5_1` ([ggml-base.en-q5_1.bin](https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en-q5_1.bin?download=true)) gives pretty decent results when speaking clear english and is near instant on GPU and even works with acceptable performance on integrated graphics.

### Browser
Browser allows you to open a browser (Chrome or Edge for now), and use the page it opens on as an input. It also uses the [Web Speech API](#web-speech-api-stt), but the provider is the web browser.

> [!NOTE]
> Chrome uses Google's cloud computing services, and Edge probably does something similar.

### Azure (STT)
Azure is Microsoft's cloud computing service. It uses [per second billing](https://azure.microsoft.com/en-us/pricing/details/cognitive-services/speech-services/).

You will need to find how to create an API key and paste it in the *Key* field.

### Deepgram
Deepgram is a cloud service. It uses [per minute billing](https://deepgram.com/pricing) for free accounts.

You will need to find how to create an API key and paste it in the *Key* field.

### Speechly
> [!WARNING]
> Speechly was aquired by Roblox and it seems its Speech To Text API was shut down.
> This service may be removed in the future.

## TTS services
**Every service has its pros and cons. I'd advice to read about them all before making your choice.**

### Web Speech API (TTS)
Web Speech API is a general specification for web browsers to support both speech synthesis and recognition. Its implementation and voices available change depending on your operating system.

<details>
<summary>Windows</summary>
We get the Web Speech API through Edge WebView2.

Edge WebView2 only supports local voices ([due to the cost constraints](https://github.com/MicrosoftEdge/WebView2Feedback/issues/2660#issuecomment-1212616745)). Afaik, it only uses the Windows voice packs for now, so here's [how to add new voice packs to Windows](https://support.microsoft.com/en-us/topic/download-languages-and-voices-for-immersive-reader-read-mode-and-read-aloud-4c83a8d8-7486-42f7-8e46-2b0fdf753130) (you might need to reboot after following these instructions).

#### Changing output device
You can't change the output device of this service inside Curses, but you change the system-wide output device of Edge WebView2 somewhere in your Windows settings. The instructions differ a bit on Windows 10/11 but you should be able to find instructions online. 
</details>

<details>
<summary>Linux</summary>
We get the Web Speech API through WebKitGTK.

>**WebKitGTK does not officially support the speech synthesis part of Web Speech API yet**, but everything should work as soon as the feature gets released.
</details>

### Piper
Piper is a Free and Open Source Text to Speech synthesizer. It generates the sound locally, and the voices are usually Public Domain (do check the license when downloading voices though).

You will need to follow these few steps to get it up and running, but don't be scared!

> [!NOTE]
> On Linux, Piper might be in your package manager of choice. Make sure you install the TTS executable, and not the mouse configuration app! (e.g. [`piper-tts-{bin,git}`](https://aur.archlinux.org/packages?K=piper-tts-) from the AUR on Arch and not `piper` from extra)

- Download the [latest release of Piper](https://github.com/rhasspy/piper/releases/latest), un-zip it and select it in Curses in the *Executable* field.
- Create a directory (folder) where you will put your voices and select it in Curses in the *Voice directory* field.
- Find a voice you like on https://rhasspy.github.io/piper-samples/, and download both the `.onnx` and `.onnx.json` files into the directory you created. Make sure both files have the same name (e.g. `en_US-kristin-medium.onnx` and `en_US-kristin-medium.onnx.json`).
- Select said voice in Curses and you're good to go :)

### Windows (TTS)
Windows provides the Microsoft Speech API (SAPI) which can be used for Text to Speech using the voices installed in your Windows instance.

### Azure (TTS)
Azure is Microsoft's cloud computing service.
It uses AI-powered voices, and usually uses per character billing ([learn more](https://azure.microsoft.com/en-us/pricing/details/cognitive-services/speech-services/)).

You will need to find [how to create an API key](https://ttsvoicewizard.com/docs/TTSMethods/AzureTTS) and paste it in the *Key* field.

### TikTok
Fast and high quality voices obtained through an unofficial TikTok TTS API.

> [!WARNING]
> Not recommended for anything important (anything non-joke tbh), since **TikTok might shutdown the API at any point** ([learn more](https://github.com/agusibrahim/tiktok-tts-api?tab=readme-ov-file#important-notice-use-of-private-tiktok-api)).

### Uberduck
AI voices paid with a [subscription](https://www.uberduck.ai/pricing). **API access is needed to use Uberduck through Curses**.

You will need to find [how to create an API key](https://ttsvoicewizard.com/docs/TTSMethods/Uberduck) and paste it in the *Api key* field.

### Custom TTS
Custom TTS isn't a service, but it allows you to plug in pretty much any TTS service.

You will probably need to create a wrapper script to make it work though.

It executes the given file as a command and passes 2 arguments:
- the path to a file containing the text to synthesize in UTF-8 format.
- the path to an output file that should containing the audio to play back once the executable finishes.

<details open>
<summary>Windows</summary>
There are more advanced options for Windows users depending on the extension of the file.

| Extension     | Command executed                                    |
| ------------- | --------------------------------------------------- |
| .exe or .com  | `%script%`                                          |
| .py           | `python %script%`                                   |
| .ps1          | `powershell -ExecutionPolicy Bypass -File %script%` |
| .*            | `cmd /c %script%`                                   |

(where `%script%` is the absolute path to the script)
</details>

## Twitch
### Using custom ports
If you are using a custom port (i.e. running `curses --port {your port}`), Twitch authentication might not work. This is because Twitch only allows a few static URLs as redirects, ports included.

If you are unable to use the default port (3030) for this one-time operation, you can try with any of 45561-45569.

If this is not an option for you, you can [create your own app](https://dev.twitch.tv/docs/authentication/register-app/). Set the OAuth Redirect URL to `http://localhost:{your port}/oauth_twitch.html`, and the client type to `Public`. Then, pass the `CURSES_TWITCH_CLIENT_ID` env variable with the newly-generated client ID when running Curses.

# Building
## Prerequisites
**Application framework dependencies**:
- [Tauri](https://v2.tauri.app/start/prerequisites/)
> [!NOTE]
> You can skip this step when using [NixOS](#nixos) with the included Nix Flake.

### NixOS
This repository provides a [Nix flake](https://nixos.wiki/wiki/flakes) which provides:
- [Development Environment](https://nixos.wiki/wiki/Development_environment_with_nix-shell) via `nix develop`
- Nix Package as the default flake package output
  - can be built with `nix build` (binary will be available as `./result/bin/curses`)

> [!WARNING]
> The flake is not made for non-NixOS environments.

The Development Environment provides all needed libraries to build the project.

Note: [Runtime Dependencies](#runtime-dependencies)

### Other Linux
Additionally the following are required for building:
- [cmake](https://cmake.org/)
- [shaderc](https://github.com/google/shaderc)
- [clang](https://clang.llvm.org/)
- [alsa-lib](https://github.com/alsa-project/alsa-lib)
- [vulkan-headers](https://github.com/KhronosGroup/Vulkan-Headers)
- [vulkan-loader](https://wiki.archlinux.org/title/Vulkan) (`vulkan-icd-loader` on arch-linux)

List of additional packages for arch linux: `cmake shaderc alsa-lib vulkan-headers vulkan-icd-loader`

Note: [Runtime Dependencies](#runtime-dependencies)

### Windows
- [rust](https://www.rust-lang.org/tools/install) (or `winget install rustup`)
- [nodejs](https://nodejs.org/en/download) (or `winget install nodejs`)
- [pnpm](https://pnpm.io/installation) (or `winget install pnpm.pnpm`)
- [vulkansdk](https://vulkan.lunarg.com/) (or `winget install vulkansdk`)
- [msvc](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2019)
  - get the community visual studio installer (if you have it installed already, open the installer again and press 'modify' on your installed instance and make sure the shown below are checked)
  - [x] 'Desktop development with C++'
- [clang lib](https://clang.llvm.org/get_started.html) (or `winget install llvm`)
- [cmake](https://cmake.org/) (or `winget install cmake`)

Note: [Runtime Dependencies](#runtime-dependencies)

## Build
1. Setup pnpm local dependencies
  - `pnpm i --frozen-lockfile`
2. Choose from the following the action you want to perform
  - `pnpm tauri dev` build and run a local development version that restarts on code changes
  - `pnpm tauri dev --release` build and run the dev version with release settings
  - `pnpm tauri build --no-bundle --debug` to create a development build
    - binary will be produced at `./src-tauri/target/debug/<curses-bin>`
  - `pnpm tauri build --no-bundle` to create a final build
    - binary will be produced at `./src-tauri/target/build/<curses-bin>`
