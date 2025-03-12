import { STT_Backends, STT_State } from "@/server/services/stt/schema";
import { ServiceNetworkState } from "@/types";
import { invoke } from "@tauri-apps/api/core";
import { FC } from "react";
import { RiCharacterRecognitionFill, RiUserVoiceFill } from "react-icons/ri";
import { SiGooglechrome, SiMicrosoftedge } from "react-icons/si";
import { useSnapshot } from "valtio";
import { azureLanguages, deepGramLangs, webspeechapiLangs } from "../../services/stt/stt_data";
import ServiceButton from "../service-button";
import Inspector from "./components";
import { InputCheckbox, InputMapObject, InputMappedGroupSelect, InputSelect, InputText, InputWebAudioInput, InputFilePath } from "./components/input";
import NiceModal from "@ebay/nice-modal-react";
import Modal from "../Modal";
import { useTranslation } from 'react-i18next';

const WebSpeechAPI: FC = () => {
  const {t} = useTranslation();
  const pr = useSnapshot(window.ApiServer.state.services.stt.data.webspeechapi);
  const updateLanguage = (value: { group: string, option: string }) => {
    window.ApiServer.state.services.stt.data.webspeechapi.language       = value.option;
    window.ApiServer.state.services.stt.data.webspeechapi.language_group = value.group;
  };
  return <>
    <Inspector.SubHeader>{t('stt.webspeechapi_title')}</Inspector.SubHeader>
    <InputMappedGroupSelect
      labelGroup="common.field_language"
      labelOption="common.field_dialect"
      value={{ option: pr.language, group: pr.language_group }}
      onChange={updateLanguage}
      library={webspeechapiLangs} />
    {/* <div className="p-2 border-2 border-error rounded-lg text-xs space-y-2 flex flex-col">
      <span className="font-bold text-error">Experimental. Might be unstable on some PCs.</span>
      <span className="font-bold text-error">Auto start is disabled.</span>
      <span className="font-bold">How to test</span>
      Just keep it on in the background for 10-15 minutes and say something every minute or two.</div> */}
  </>
}

const Browser: FC = () => {
  const {t} = useTranslation();
  const handleOpen = () => {
    invoke<void>("plugin:web|open_browser", {
      data: {
        browser: "chrome",
        url: `http://localhost:${window.Config.serverNetwork.port}/mic.html`
      }
    });
  };

  const handleOpenEdge = () => {
    invoke<void>("plugin:web|open_browser", {
      data: {
        browser: "msedge",
        url: `http://localhost:${window.Config.serverNetwork.port}/mic.html`
      }
    });
  };

  return <>
    <Inspector.SubHeader>{t('stt.browser_title')}</Inspector.SubHeader>
    <button className="btn btn-sm btn-neutral gap-2" onClick={handleOpen}><SiGooglechrome/> Open Chrome</button>
    <button className="btn btn-sm btn-neutral gap-2" onClick={handleOpenEdge}><SiMicrosoftedge/> Open Edge</button>
  </>
}


const Azure: FC = () => {
  const {t} = useTranslation();
  const pr = useSnapshot(window.ApiServer.state.services.stt.data.azure);
  const up = <K extends keyof STT_State["azure"]>(key: K, v: STT_State["azure"][K]) => window.ApiServer.state.services.stt.data.azure[key] = v;

  const updateLanguage = (value: { group: string, option: string }) => {
    window.ApiServer.state.services.stt.data.azure.language       = value.option;
    window.ApiServer.state.services.stt.data.azure.language_group = value.group;
  };

  const updateSecondaryLanguage = (value: { group: string, option: string }) => {
    window.ApiServer.state.services.stt.data.azure.secondary_language       = value.option;
    window.ApiServer.state.services.stt.data.azure.secondary_language_group = value.group;
  };

  return <>
    <Inspector.SubHeader>{t('stt.azure_title')}</Inspector.SubHeader>
    <InputText label="stt.azure_key" type="password" value={pr.key} onChange={e => up("key", e.target.value)} />
    <InputText label="stt.azure_location" value={pr.location} onChange={e => up("location", e.target.value)} />

    <InputWebAudioInput value={pr.device} onChange={e => up("device", e)} label="common.field_input_device"/>

    <div className=" divider"></div>
    <InputMappedGroupSelect
      labelGroup="common.field_language"
      labelOption="common.field_dialect"
      value={{ option: pr.language, group: pr.language_group }}
      onChange={updateLanguage}
      library={azureLanguages} />
    <InputCheckbox label="stt.azure_use_secondary_language" onChange={e => up("use_secondary_language", e)} value={pr.use_secondary_language} />
    <Inspector.Switchable visible={pr.use_secondary_language}>
      <InputMappedGroupSelect
        labelGroup="stt.azure_secondary_language"
        labelOption="common.field_dialect"
        value={{ option: pr.secondary_language, group: pr.secondary_language_group }}
        onChange={updateSecondaryLanguage}
        library={azureLanguages} />
    </Inspector.Switchable>

    <InputSelect 
      label="stt.azure_profanity"
      options={[
        { label: t('stt.azure_profanity_masked'), value: 'masked' },
        { label: t('stt.azure_profanity_removed'), value: 'removed' },
        { label: t('stt.azure_profanity_raw'), value: 'raw' },
      ]}
      value={pr.profanity}
      onValueChange={e => up("profanity", e)}
    />
    <InputText type="number" step="1" label="stt.azure_silence_timeout" value={pr.silenceTimeout} onChange={e => up("silenceTimeout", e.target.value)} />
    <InputCheckbox label="stt.field_enable_interim_results" onChange={e => up("interim", e)} value={pr.interim} />
  </>
}

const Deepgram: FC = () => {
  const {t} = useTranslation();
  const pr = useSnapshot(window.ApiServer.state.services.stt.data.deepgram);
  const up = <K extends keyof STT_State["deepgram"]>(key: K, v: STT_State["deepgram"][K]) => window.ApiServer.state.services.stt.data.deepgram[key] = v;

  const updateLanguage = (value: { group: string, option: string }) => {
    window.ApiServer.state.services.stt.data.deepgram.language       = value.option;
    window.ApiServer.state.services.stt.data.deepgram.language_group = value.group;
  };

  return <>
    <Inspector.SubHeader>{t('stt.deepgram_title')}</Inspector.SubHeader>
    <InputText label="stt.deepgram_key" type="password" value={pr.key} onChange={e => up("key", e.target.value)} />

    <InputWebAudioInput value={pr.device} onChange={e => up("device", e)} label="common.field_input_device"/>
    <InputMappedGroupSelect
      labelGroup="common.field_language"
      labelOption="common.field_dialect"
      value={{ option: pr.language, group: pr.language_group }}
      onChange={updateLanguage}
      library={deepGramLangs} />

    <InputSelect options={[
      { label: 'Base', value: 'base' },
      { label: 'Enhanced', value: 'enhanced' },
    ]} label="stt.deepgram_quality" value={pr.tier} onValueChange={e => up("tier", e)} />

    <span className="text-base-content/60 text-xs">
      {t('stt.deepgram_quality_notice')}
      <br />
      <a className="link link-primary link-hover" target="_blank" href="https://developers.deepgram.com/documentation/features/language/#language-options">{t('stt.deepgram_quality_notice_link')}</a>
    </span>

    <InputCheckbox label="stt.field_enable_interim_results" onChange={e => up("interim", e)} value={pr.interim} />
    <InputCheckbox label="stt.deepgram_profanity" onChange={e => up("profanity", e)} value={pr.profanity} />
    <InputCheckbox label="stt.deepgram_punctuate" onChange={e => up("punctuate", e)} value={pr.punctuate} />
  </>
}

const Speechly: FC = () => {
  const {t} = useTranslation();
  const pr = useSnapshot(window.ApiServer.state.services.stt.data.speechly);
  const up = <K extends keyof STT_State["speechly"]>(key: K, v: STT_State["speechly"][K]) => window.ApiServer.state.services.stt.data.speechly[key] = v;

  return <>
    <Inspector.SubHeader>{t('stt.speechly_title')}</Inspector.SubHeader>
    <InputWebAudioInput value={pr.device} onChange={e => up("device", e)} label="common.field_input_device"/>
    <InputText label="stt.speechly_appid" type="password" value={pr.key} onChange={e => up("key", e.target.value)} />
  </>
}

const Whisper: FC = () => {
  const {t} = useTranslation();
  const data = useSnapshot(window.ApiServer.state.services.stt.data.whisper);
  const handleUpdate = <K extends keyof STT_State["whisper"]>(key: K, v: STT_State["whisper"][K]) => window.ApiServer.state.services.stt.data.whisper[key] = v;

  return <>
    <Inspector.SubHeader>{t('stt.whisper_title')}</Inspector.SubHeader>
    <InputWebAudioInput value={data.device} onChange={e => handleUpdate("device", e)} label="common.field_input_device"/>
    <InputText label="stt.whisper_lang" type="text" value={data.lang} onChange={e => handleUpdate("lang", e.target.value)}/>
    <Inspector.Description>{t('stt.whisper_lang_desc')}</Inspector.Description>
    <Inspector.Description>{t('stt.whisper_lang_desc_note')}</Inspector.Description>
    <InputCheckbox label="stt.whisper_translate_to_english" onChange={e => handleUpdate("translateToEnglish", e)} value={data.translateToEnglish}/>
    <InputFilePath
      label="stt.whisper_model_path"
      value={data.modelPath}
      onChange={e => handleUpdate("modelPath", e.target.value)}
      dialogOptions={{ filters: [{ name: "ggml bins", extensions: ["bin"] }] }}
    />
  </>
}


const WordsReplacementModal: FC = () => {
  const {t} = useTranslation();
  const data = useSnapshot(window.ApiServer.state.services.stt);

  const up = <K extends keyof STT_State>(key: K, v: STT_State[K]) => window.ApiServer.patchService("stt", s => s.data[key] = v);

  return <Modal.Body width={420}>
    <Modal.Header>{t('word_replacements.title')}</Modal.Header>
    <Modal.Content>
      <div className="p-4 flex flex-col space-y-2">
        <InputCheckbox label="word_replacements.field_ignore_case" value={data.data.replaceWordsIgnoreCase} onChange={v => up("replaceWordsIgnoreCase", v)}/>
        {data.data.replaceWordsIgnoreCase && <>
          <InputCheckbox label="word_replacements.field_preserve_capitalization" value={data.data.replaceWordsPreserveCase} onChange={v => up("replaceWordsPreserveCase", v)}/>
          <Inspector.Description>{t('word_replacements.field_preserve_capitalization_desc')}</Inspector.Description>
        </>}
        <InputMapObject keyPlaceholder={t('word_replacements.label_dictionary_key')} valuePlaceholder={t('word_replacements.label_dictionary_value')} addLabel={t('common.btn_add')} value={{...data.data.replaceWords}} onChange={e => up("replaceWords", e)} label="" />
      </div>
    </Modal.Content>
  </Modal.Body>
}
NiceModal.register('stt-replacements', (props) => <Modal.Base {...props}><WordsReplacementModal /></Modal.Base>);

const Inspector_STT: FC = () => {
  const {t} = useTranslation();
  const data = useSnapshot(window.ApiServer.state.services.stt);
  const state = useSnapshot(window.ApiServer.stt.serviceState);

  const handleStart = (v: boolean) => window.ApiServer.state.services.stt.showActionButton = v;
  const up = <K extends keyof STT_State>(key: K, v: STT_State[K]) => window.ApiServer.patchService("stt", s => s.data[key] = v);

  const handleShowReplacements = () => {
    NiceModal.show('stt-replacements');
  }

  return <Inspector.Body>
    <Inspector.Header><RiUserVoiceFill /> {t('stt.title')}</Inspector.Header>
    <Inspector.Content>
      <InputCheckbox label="common.field_action_bar" onChange={handleStart} value={data.showActionButton} />
      <InputCheckbox label="common.field_auto_start" value={data.data.autoStart} onChange={e => up("autoStart", e)} />
      <InputCheckbox label="stt.field_stop_with_stream" value={data.data.stopWithStream} onChange={e => up("stopWithStream", e)} />
      <span className="link link-accent link-hover font-semibold flex items-center gap-2 text-sm" onClick={handleShowReplacements}><RiCharacterRecognitionFill/>{t('common.btn_edit_replacements')}</span>
      <Inspector.Deactivatable active={state.status === ServiceNetworkState.disconnected}>
        <InputSelect options={[
          { label: t("stt.webspeechapi_title"), value: STT_Backends.webspeechapi },
          { label: t("stt.browser_title"), value: STT_Backends.browser },
          { label: t("stt.azure_title"), value: STT_Backends.azure },
          { label: t("stt.deepgram_title"), value: STT_Backends.deepgram },
          { label: t("stt.speechly_title"), value: STT_Backends.speechly },
          { label: t("stt.whisper_title"), value: STT_Backends.whisper }
        ]} label="common.field_service" value={data.data.backend} onValueChange={e => up("backend", e as STT_Backends)} />

        {data.data.backend === STT_Backends.browser && <Browser />}
        {data.data.backend === STT_Backends.azure && <Azure />}
        {data.data.backend === STT_Backends.deepgram && <Deepgram />}
        {data.data.backend === STT_Backends.speechly && <Speechly />}
        {data.data.backend === STT_Backends.whisper && <Whisper />}
        {data.data.backend === STT_Backends.webspeechapi && <WebSpeechAPI />}
      </Inspector.Deactivatable>

      {data.data.backend !== STT_Backends.browser && <ServiceButton status={state.status} onStart={() => window.ApiServer.stt.start()} onStop={() => window.ApiServer.stt.stop()} />}
      <div className="pt-8">
        <InputCheckbox label='stt.field_uwu_filter' onChange={e => up("uwu", e)} value={data.data.uwu} />
        <Inspector.Description>{t('stt.field_uwu_filter_desc')}</Inspector.Description>
      </div>
    </Inspector.Content>
  </Inspector.Body>
}

export default Inspector_STT;
