use core::panic;
use std::mem::{self, MaybeUninit};

use earshot::{VoiceActivityDetector, VoiceActivityModel, VoiceActivityProfile};
use ringbuf::{
    storage::Heap,
    traits::{Consumer, Observer, Producer},
    LocalRb,
};
use rodio::cpal::{BufferSize, StreamConfig};

/// ~30ms of audio
pub const VAD_FRAME: usize = 480; // sample count

pub type NSamples = usize;
pub enum VadStatus {
    Silence,
    SpeechStart,
    Speech,
    SpeechEnd(NSamples),
}

pub struct Vad {
    vad: VoiceActivityDetector,
    ring: LocalRb<Heap<i16>>,
    // TODO: build controll structure
    /// reading this while `last_speech_frame = None` is undefined behavior
    current_frame: usize,
    last_speech_frame: Option<usize>,
    /// reading this while `last_speech_frame = None` is undefined behavior
    current_speech_samples: NSamples,
}

impl Vad {
    pub fn try_new(config: &StreamConfig) -> Result<Vad, &'static str> {
        let BufferSize::Fixed(buffer_size) = config.buffer_size else {
            return Err("config doesnt allow safe vad setup");
        };
        let ring = LocalRb::new(buffer_size.max(VAD_FRAME as u32 * 2) as usize);
        Ok(Vad {
            vad: VoiceActivityDetector::new_with_model(VoiceActivityModel::ES_ALPHA, VoiceActivityProfile::VERY_AGGRESSIVE),
            ring,
            current_frame: 0,
            last_speech_frame: None,
            current_speech_samples: 0,
        })
    }

    pub fn input(&mut self, samples: &[i16]) {
        if self.ring.push_slice(samples) != samples.len() {
            eprintln!("warning: internal buffer full, some audio was dropped");
        }
    }

    pub fn output_to(&mut self, final_ring: &mut impl Producer<Item = i16>) -> VadStatus {
        while self.ring.occupied_len() >= VAD_FRAME {
            let mut frame: [MaybeUninit<i16>; VAD_FRAME] = [const { MaybeUninit::uninit() }; VAD_FRAME];
            if VAD_FRAME != self.ring.pop_slice_uninit(&mut frame) {
                panic!("vad ring should have enough data for at least one frame");
            }
            // SAFETY: this is safe because the panic above makes sure that all i16 were initialized
            let frame = unsafe { mem::transmute::<[MaybeUninit<i16>; VAD_FRAME], [i16; VAD_FRAME]>(frame) };

            let is_speech = self
                .vad
                .predict_16khz(&frame)
                .expect("frame should have valid length");

            let Some(last_speech_frame) = self.last_speech_frame.as_mut() else {
                // we are inside a silence window
                if !is_speech {
                    continue;
                }
                // speech just started
                let n = final_ring.push_slice(&frame);
                if n != frame.len() {
                    eprintln!("transcription audio ring was full, dropped some audio");
                }

                self.last_speech_frame = Some(0);
                self.current_speech_samples = n;
                self.current_frame = 0;
                return VadStatus::SpeechStart; // it's ok to return here since the upper level will poll again until `Speech`
            };
            // we are inside a speech window
            self.current_frame += 1;
            let silence_frames = self.current_frame - *last_speech_frame;
            if !is_speech && silence_frames >= 8 {
                // if silence for 240ms
                self.last_speech_frame = None;
                return VadStatus::SpeechEnd(self.current_speech_samples);
            }

            if is_speech {
                *last_speech_frame = self.current_frame;
            }
            if is_speech || silence_frames <= 3 {
                // if speech or silence <= 90ms record audio
                let n = final_ring.push_slice(&frame);
                if n != frame.len() {
                    eprintln!("transcription audio ring was full, dropped some audio");
                }

                self.current_speech_samples += n;
            }
        }
        match self.last_speech_frame {
            Some(_) => VadStatus::Speech,
            None => VadStatus::Silence,
        }
    }
}
