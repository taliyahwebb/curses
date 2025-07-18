use std::sync::Arc;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{BufferSize, Device, SampleRate, StreamConfig};
use earshot::{VoiceActivityDetector, VoiceActivityModel, VoiceActivityProfile};
use futures::channel::mpsc::UnboundedSender;
use ringbuf::storage::Heap;
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::{CachingCons, CachingProd, SharedRb};
use rodio::cpal;
use rubato::{FftFixedOut, Resampler};
use thiserror::Error;
use tracing::{debug, error, warn};

use super::whisper::SAMPLE_RATE;

/// ~30ms of audio
pub const VAD_FRAME: usize = 480; // sample count
pub const SPEECH_DETECTION_LINGER: Duration = Duration::from_millis(90);
pub const DEFAULT_SEGMENT_SEPARATOR_SILENCE: Duration = Duration::from_millis(240);

pub const LINGER_FRAMES: usize = to_frames(SPEECH_DETECTION_LINGER);

pub const fn to_frames(duration: Duration) -> usize {
    (duration.as_millis() as usize * SAMPLE_RATE) / 1000 / VAD_FRAME
}

/// selectable alsa buffer sizes follow a weird pattern 32 seems to work as a
/// quantum over a wide range of buffer sizes
const ALSA_BUFFER_QAUANTUM: u32 = 32;
const ALSA_BUFFER_MIN: u32 = 32;

pub type NSamples = usize;
pub enum VadStatus {
    Silence,
    SpeechStart,
    Speech,
    SpeechEnd(NSamples),
}

#[derive(Error, Debug)]
pub enum InputDeviceError {
    #[error("input device no longer valid: '{0}'")]
    Invalid(String),
    #[error("input device has no valid configuration options")]
    NoConfig,
}

#[derive(Error, Debug)]
pub enum ResamplingVadSetupError {
    #[error("error setting up VAD: '{0}'")]
    Vad(#[from] VadSetupError),
    #[error("error setting up Resampling: '{0}'")]
    Resampler(#[from] ResamplerSetupError),
}

#[derive(Error, Debug)]
pub enum VadSetupError {}

#[derive(Error, Debug)]
#[error("{0}")]
pub struct ResamplerSetupError(String);

pub enum VadActivity {
    SpeechStart,
    SpeechEnd(NSamples),
}

type Cons = CachingCons<Arc<SharedRb<Heap<f32>>>>;
type Prod = CachingProd<Arc<SharedRb<Heap<f32>>>>;

pub struct ResamplingVad {
    vad: VoiceActivityDetector,
    ring: Cons,
    resample_with: Option<(FftFixedOut<f32>, Vec<Vec<f32>>)>,
    channels: u16,
    input_buff: Vec<f32>,
    // TODO: build control structure
    /// reading this while `last_speech_frame = None` is undefined behavior
    current_frame: usize,
    last_speech_frame: Option<usize>,
    /// reading this while `last_speech_frame = None` is undefined behavior
    current_speech_samples: NSamples,
    // number of audio frames to wait before dispatching to whisper
    silence_frames: usize,
}

impl ResamplingVad {
    pub fn with_silence_interval(
        config: &StreamConfig,
        silence_interval: Option<Duration>,
    ) -> Result<(Prod, ResamplingVad), ResamplingVadSetupError> {
        let resample_with = get_resampler(config.sample_rate.0)?;
        let BufferSize::Fixed(buffer_size) = config.buffer_size else {
            panic!("config doesnt allow safe vad setup");
        };
        let max_vad_input = match &resample_with {
            Some(resampler) => resampler.0.input_frames_max(),
            None => VAD_FRAME,
        };
        let ring = SharedRb::new(
            (config.sample_rate.0 as usize * config.channels as usize) // 1s of audio
                .max((buffer_size * 2).max(max_vad_input as u32 * 2) as usize), /* or 2x biggest needed buffer */
        );
        let (producer, ring) = ring.split();
        let input_buff = match &resample_with {
            Some((resampler, _)) => {
                let max_mono_input = resampler.input_frames_max();
                vec![0.; max_mono_input * config.channels as usize]
            }
            None => vec![0.; VAD_FRAME * config.channels as usize],
        };
        Ok((
            producer,
            ResamplingVad {
                vad: VoiceActivityDetector::new_with_model(
                    VoiceActivityModel::ES_ALPHA,
                    VoiceActivityProfile::VERY_AGGRESSIVE,
                ),
                resample_with,
                channels: config.channels,
                input_buff,
                ring,
                current_frame: 0,
                last_speech_frame: None,
                current_speech_samples: 0,
                silence_frames: to_frames(
                    silence_interval.unwrap_or(DEFAULT_SEGMENT_SEPARATOR_SILENCE),
                ),
            },
        ))
    }

    /// # Returns
    /// the amount of raw audio frames missing for the next VAD action
    pub fn missing_frames(&self) -> usize {
        let raw_needed = self.input_frames_next();
        raw_needed.saturating_sub(self.ring.occupied_len())
    }

    /// # Returns
    /// the amount of frames total for the next VAD action
    pub fn input_frames_next(&self) -> usize {
        let mono_needed = match &self.resample_with {
            Some((resampler, _)) => resampler.input_frames_next(),
            None => VAD_FRAME,
        };
        mono_needed * self.channels as usize
    }

    pub fn output_to(&mut self, final_ring: &mut impl Producer<Item = i16>) -> VadStatus {
        loop {
            if self.missing_frames() > 0 {
                match self.last_speech_frame {
                    Some(_) => return VadStatus::Speech,
                    None => return VadStatus::Silence,
                };
            }
            let raw_needed = self.input_frames_next();
            let raw_samples = &mut self.input_buff[..raw_needed];
            if raw_needed != self.ring.pop_slice(raw_samples) {
                panic!("vad ring should have enough data for at least one resample");
            }
            let mono_samples = condense_in_place(raw_samples, self.channels);
            let vad_input = match &mut self.resample_with {
                Some((resampler, out)) => {
                    let (consumed, produced) = resampler
                        .process_into_buffer(&[&mono_samples], out, None)
                        .expect("resampler started with invalid buffers");
                    assert!(
                        consumed == mono_samples.len(),
                        "resampler did not consume all frames"
                    );
                    assert!(
                        produced == VAD_FRAME,
                        "resampler did not produce a vad frame"
                    );
                    convert_samples_f32_to_i16(&out[0][..VAD_FRAME])
                }
                None => convert_samples_f32_to_i16(mono_samples),
            };
            let is_speech = self
                .vad
                .predict_16khz(&vad_input)
                .expect("frame should have valid length");

            let Some(last_speech_frame) = self.last_speech_frame.as_mut() else {
                // we are inside a silence window
                if !is_speech {
                    continue;
                }
                // speech just started
                let n = final_ring.push_slice(&vad_input);
                if n != vad_input.len() {
                    eprintln!("transcription audio ring was full, dropped some audio");
                }

                self.last_speech_frame = Some(0);
                self.current_speech_samples = n;
                self.current_frame = 0;
                return VadStatus::SpeechStart; // it's ok to return here since
                // the upper level will poll
                // again until `Speech`
            };
            // we are inside a speech window
            self.current_frame += 1;
            let silence_frames = self.current_frame - *last_speech_frame;
            if !is_speech && silence_frames >= self.silence_frames {
                // if silence for 240ms
                self.last_speech_frame = None;
                return VadStatus::SpeechEnd(self.current_speech_samples);
            }

            if is_speech {
                *last_speech_frame = self.current_frame;
            }
            if is_speech || silence_frames <= LINGER_FRAMES {
                // if speech or silence <= 90ms record audio
                let n = final_ring.push_slice(&vad_input);
                if n != vad_input.len() {
                    eprintln!("transcription audio ring was full, dropped some audio");
                }

                self.current_speech_samples += n;
            }
        }
    }
}

/// nop when there are not enough frames
/// executes inner multiple times if sufficient frames are present
pub fn audio_loop(
    ring_buffer: &mut impl Producer<Item = i16>,
    vad: &mut ResamplingVad,
    activity: &mut UnboundedSender<VadActivity>,
) {
    loop {
        let status = vad.output_to(ring_buffer);
        match status {
            VadStatus::Silence => (),
            VadStatus::Speech => (),
            VadStatus::SpeechEnd(samples) => {
                // can safely drop the error case here as it only happens when the receiver has
                // hung up (which means the stream is bound to stop soon too)
                let _ = activity.unbounded_send(VadActivity::SpeechEnd(samples));
                continue; // make sure we run this input to completion
            }
            VadStatus::SpeechStart => {
                // can safely drop the error case here as it only happens when the receiver has
                // hung up (which means the stream is bound to stop soon too)
                let _ = activity.unbounded_send(VadActivity::SpeechStart);
                continue; // make sure we run this input to completion
            }
        }
        break;
    }
}

pub fn get_microphone_by_name(name: &str) -> Result<(Device, StreamConfig), InputDeviceError> {
    let host = cpal::default_host();
    let mut devices = host.input_devices().unwrap();
    if let Some(device) = devices.find(|device| device.name().unwrap() == name) {
        let config = device
            .supported_input_configs()
            .map_err(|err| InputDeviceError::Invalid(format!("{err}",)))?
            .next()
            .ok_or(InputDeviceError::NoConfig)?;
        let config = config
            .try_with_sample_rate(SampleRate(SAMPLE_RATE as u32))
            .unwrap_or_else(|| {
                let dev_rate = if config.min_sample_rate().0 > SAMPLE_RATE as u32 {
                    config.min_sample_rate()
                } else {
                    config.max_sample_rate()
                };
                config.with_sample_rate(dev_rate)
            });
        let buffer_size = BufferSize::Fixed(match config.buffer_size() {
            cpal::SupportedBufferSize::Range { min, max } => ((config.sample_rate().0 / 30)
                .next_multiple_of(ALSA_BUFFER_QAUANTUM)
                .max(ALSA_BUFFER_MIN))
            .max(*min)
            .min(*max),
            cpal::SupportedBufferSize::Unknown => (config.sample_rate().0 / 30)
                .next_multiple_of(ALSA_BUFFER_QAUANTUM)
                .max(ALSA_BUFFER_MIN),
        });
        println!("using buffer size {buffer_size:?}");
        let sample_rate = config.sample_rate();
        let channels = config.channels();
        if channels > 1 {
            warn!(
                "input device uses more then one channel ({channels}), some spacial audio setups may cause issues when their audio is compressed to mono"
            );
        }
        let config = StreamConfig {
            channels,
            sample_rate,
            buffer_size,
        };
        Ok((device, config))
    } else {
        Err(InputDeviceError::Invalid("not found".into()))
    }
}

type BufferedResampler = Option<(FftFixedOut<f32>, Vec<Vec<f32>>)>;

pub fn get_resampler(src_rate: u32) -> Result<BufferedResampler, ResamplerSetupError> {
    if src_rate != SAMPLE_RATE as u32 {
        eprintln!(
            "running with resampling src{:?}->dest{SAMPLE_RATE}",
            src_rate
        );
        let resampler = FftFixedOut::<f32>::new(
            src_rate as usize,
            SAMPLE_RATE,
            VAD_FRAME, // output exactly a vad frame we can't even be sure that it's not an issso we don't need to buffer the output again
            2, /* 2 was suggested as default value here https://github.com/HEnquist/rubato/issues/38 */
            1,
        ).map_err(|err| ResamplerSetupError(err.to_string()))?;
        let out = resampler.output_buffer_allocate(true);
        Ok(Some((resampler, out)))
    } else {
        debug!("running at native {SAMPLE_RATE}Hz sample rate");
        Ok(None)
    }
}

/// convert f32 to i16 samples
pub fn convert_samples_f32_to_i16(samples: &[f32]) -> Vec<i16> {
    let mut samples_i16 = Vec::with_capacity(samples.len());
    for v in samples {
        samples_i16.push((*v * i16::MAX as f32) as i16);
    }
    samples_i16
}

/// condenses all `samples` of interleaved audio with `channels` into single
/// channel audio
///
/// # Note
/// - returned array has size `samples.len()/channels`, samples in
///   `samples[new_len..]` remain untouched
///
/// # Panics
/// - when `samples` does not have a multiple of `channels` elements
pub fn condense_in_place(samples: &mut [f32], channels: u16) -> &mut [f32] {
    let channels = channels as usize;
    if channels == 1 {
        // mono fastpath
        return samples;
    }
    assert_eq!(
        samples.len() % channels,
        0,
        "invalid samples for {channels} channels"
    );
    let new_size = samples.len() / channels;
    for i in 0..new_size {
        let current_pos = i * channels;
        samples[i] = samples[current_pos..current_pos + channels]
            .iter()
            .sum::<f32>()
            / channels as f32;
    }
    &mut samples[..new_size]
}
