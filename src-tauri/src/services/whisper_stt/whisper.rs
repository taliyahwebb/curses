use std::path::Path;

use serde::Serialize;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState};

/// Whisper expects this sample rate
pub const SAMPLE_RATE: usize = 16000;
/// Wait for at most 30s before dispatching to whisper
pub const MAX_WHISPER_FRAME: usize = (SAMPLE_RATE * 30) - WHISPER_PREPEND_SILENCE;
/// prepend 700ms of silence to each whisper frame so the first word gets picked up better
const WHISPER_PREPEND_SILENCE: usize = 1600 * 7;

pub struct Whisper {
    state: WhisperState,
    params: FullParams<'static, 'static>,
    buf: Box<[i16; WHISPER_PREPEND_SILENCE + MAX_WHISPER_FRAME]>,
    samples_in_buf: usize,
}

#[derive(Debug, Serialize)]
pub enum WhisperSetupError {
    ModelFileNotFound,
    ModelInvalid,
}

impl Whisper {
    pub fn new(model: impl AsRef<Path>) -> Result<Whisper, WhisperSetupError> {
        let params = WhisperContextParameters::default();
        let ctx = WhisperContext::new_with_params(
            model
                .as_ref()
                .to_str()
                .ok_or(WhisperSetupError::ModelFileNotFound)?,
            params,
        )
        .map_err(|_| WhisperSetupError::ModelInvalid)?;
        // now we can run the model
        let state = ctx
            .create_state()
            .map_err(|_| WhisperSetupError::ModelInvalid)?;

        // create a params object
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_n_threads(4);
        params.set_no_timestamps(true);
        params.set_suppress_non_speech_tokens(true);
        params.set_single_segment(true);

        Ok(Whisper {
            state,
            params,
            buf: Box::new([0i16; MAX_WHISPER_FRAME + WHISPER_PREPEND_SILENCE]),
            samples_in_buf: 0,
        })
    }

    /// Obtain access to `sample_count` audio samples of the internal buffer
    ///
    /// # Note
    /// - make sure to write the entire len of the slice, otherwise it COULD contain junk audio
    /// - panics if sample count is bigger then MAX_WHISPER_FRAME
    pub fn audio_buf(&mut self, sample_count: usize) -> &mut [i16] {
        assert!(sample_count < MAX_WHISPER_FRAME);
        self.samples_in_buf = sample_count;
        &mut self.buf[WHISPER_PREPEND_SILENCE..WHISPER_PREPEND_SILENCE + sample_count]
    }

    /// Transcribes the registered audio
    pub fn transcribe(&mut self) -> Option<String> {
        if self.samples_in_buf < SAMPLE_RATE - WHISPER_PREPEND_SILENCE {
            // save some processing since whisper will reject <1s audio anyway
            return None;
        }
        let samples = &self.buf[0..WHISPER_PREPEND_SILENCE + self.samples_in_buf];
        let mut float_samples = Box::new([0f32; WHISPER_PREPEND_SILENCE + MAX_WHISPER_FRAME]);
        whisper_rs::convert_integer_to_float_audio(samples, &mut float_samples[..samples.len()]).expect("should be able to de-quantize data");

        self.state
            .full(self.params.clone(), &float_samples[..samples.len()])
            .expect("failed to run model");

        // fetch the results
        let num_segments = self
            .state
            .full_n_segments()
            .expect("failed to get number of segments");
        if num_segments == 0 {
            return None;
        }
        if num_segments > 1 {
            // this should not happen as we have enabled the single segment option
            eprintln!("more then one text segment received from whisper, dropping others");
        }
        return self
            .state
            .full_get_segment_text(0)
            .ok()
            // filter hallucination
            .and_then(|text| if text.eq_ignore_ascii_case(" you") { None } else { Some(text) });
    }
}
