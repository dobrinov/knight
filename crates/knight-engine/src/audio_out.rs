//! Audio output: a cpal device stream fed by the software [`Mixer`] natively, WebAudio nodes on
//! the web. Both consume the [`Audio`] command queue once per frame.

use knight_core::Audio;

#[cfg(not(target_arch = "wasm32"))]
pub use native::AudioOut;
#[cfg(target_arch = "wasm32")]
pub use web::AudioOut;

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::*;
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use knight_core::Mixer;
    use std::sync::{Arc, Mutex};

    pub struct AudioOut {
        mixer: Arc<Mutex<Mixer>>,
        _stream: cpal::Stream,
    }

    impl AudioOut {
        pub fn new() -> Option<AudioOut> {
            let host = cpal::default_host();
            let device = host.default_output_device()?;
            let supported = device.default_output_config().ok()?;
            let format = supported.sample_format();
            let config = supported.config();
            let channels = config.channels as usize;
            let mixer = Arc::new(Mutex::new(Mixer::new(config.sample_rate)));
            let m = mixer.clone();
            let mut stereo: Vec<f32> = Vec::new();
            let mut fill = move |out_len: usize, write: &mut dyn FnMut(usize, f32)| {
                let frames = out_len / channels.max(1);
                stereo.resize(frames * 2, 0.0);
                if let Ok(mut mix) = m.lock() {
                    mix.render(&mut stereo);
                } else {
                    stereo.fill(0.0);
                }
                for f in 0..frames {
                    let (l, r) = (stereo[f * 2], stereo[f * 2 + 1]);
                    for c in 0..channels {
                        let v = match (channels, c) {
                            (1, _) => (l + r) * 0.5,
                            (_, 0) => l,
                            (_, 1) => r,
                            _ => 0.0,
                        };
                        write(f * channels + c, v);
                    }
                }
            };
            let err = |e| log::warn!("knight audio: {e}");
            let stream = match format {
                cpal::SampleFormat::F32 => device
                    .build_output_stream(
                        config,
                        move |data: &mut [f32], _| {
                            let n = data.len();
                            fill(n, &mut |i, v| data[i] = v);
                        },
                        err,
                        None,
                    )
                    .ok()?,
                cpal::SampleFormat::I16 => device
                    .build_output_stream(
                        config,
                        move |data: &mut [i16], _| {
                            let n = data.len();
                            fill(n, &mut |i, v| data[i] = (v.clamp(-1.0, 1.0) * i16::MAX as f32) as i16);
                        },
                        err,
                        None,
                    )
                    .ok()?,
                other => {
                    log::warn!("knight audio: unsupported sample format {other:?}");
                    return None;
                }
            };
            stream.play().ok()?;
            log::info!("knight audio: {} Hz, {channels} channels", config.sample_rate);
            Some(AudioOut { mixer, _stream: stream })
        }

        /// Desktop audio needs no user gesture.
        pub fn unlock(&mut self) {}

        pub fn submit(&mut self, audio: &mut Audio) {
            audio.unlocked = true;
            let cmds = audio.drain();
            if cmds.is_empty() {
                return;
            }
            if let Ok(mut m) = self.mixer.lock() {
                for c in &cmds {
                    m.apply(c, audio.sounds());
                }
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use super::*;
    use knight_core::AudioCmd;
    use wasm_bindgen::JsCast;
    use web_sys::{AudioBuffer, AudioBufferSourceNode, AudioContext, AudioContextState, GainNode};

    pub struct AudioOut {
        ctx: AudioContext,
        master: GainNode,
        buffers: Vec<Option<AudioBuffer>>,
        music: Option<(AudioBufferSourceNode, GainNode)>,
    }

    impl AudioOut {
        pub fn new() -> Option<AudioOut> {
            let ctx = AudioContext::new().ok()?;
            let master = ctx.create_gain().ok()?;
            master.connect_with_audio_node(&ctx.destination()).ok()?;
            Some(AudioOut { ctx, master, buffers: Vec::new(), music: None })
        }

        /// Browsers only allow audio after a user gesture; call from input events.
        pub fn unlock(&mut self) {
            if self.ctx.state() != AudioContextState::Running {
                let _ = self.ctx.resume();
            }
        }

        fn buffer(&mut self, audio: &Audio, id: usize) -> Option<AudioBuffer> {
            if self.buffers.len() <= id {
                self.buffers.resize(id + 1, None);
            }
            if self.buffers[id].is_none() {
                let s = audio.sounds().get(id)?;
                let buf = self.ctx.create_buffer(1, s.samples.len().max(1) as u32, s.rate as f32).ok()?;
                let _ = buf.copy_to_channel(&s.samples, 0);
                self.buffers[id] = Some(buf);
            }
            self.buffers[id].clone()
        }

        fn source(
            &mut self,
            audio: &Audio,
            id: usize,
            volume: f32,
            pan: f32,
            pitch: f32,
        ) -> Option<(AudioBufferSourceNode, GainNode)> {
            let buf = self.buffer(audio, id)?;
            let src = self.ctx.create_buffer_source().ok()?;
            src.set_buffer(Some(&buf));
            src.playback_rate().set_value(pitch);
            let gain = self.ctx.create_gain().ok()?;
            gain.gain().set_value(volume);
            let panner = self.ctx.create_stereo_panner().ok()?;
            panner.pan().set_value(pan);
            src.connect_with_audio_node(&gain).ok()?;
            gain.connect_with_audio_node(&panner).ok()?;
            panner.connect_with_audio_node(&self.master).ok()?;
            Some((src, gain))
        }

        pub fn submit(&mut self, audio: &mut Audio) {
            audio.unlocked = self.ctx.state() == AudioContextState::Running;
            for cmd in audio.drain() {
                match cmd {
                    AudioCmd::Play { sound, volume, pan, pitch } => {
                        if !audio.unlocked {
                            continue;
                        }
                        if let Some((src, _)) = self.source(audio, sound.0 as usize, volume, pan, pitch) {
                            let _ = src.start();
                        }
                    }
                    AudioCmd::Music { sound, volume } => {
                        if let Some((old, _)) = self.music.take() {
                            let _ = old.unchecked_ref::<web_sys::AudioScheduledSourceNode>().stop();
                        }
                        if let Some(id) = sound
                            && let Some((src, gain)) = self.source(audio, id.0 as usize, volume, 0.0, 1.0)
                        {
                            src.set_loop(true);
                            let _ = src.start();
                            self.music = Some((src, gain));
                        }
                    }
                    AudioCmd::Master { volume } => self.master.gain().set_value(volume),
                }
            }
        }
    }
}
