//! The speakers and the microphone. With the `audio` feature, the system's
//! default devices through cpal, each stream on a thread of its own (a cpal
//! stream can't always move between threads): the output plays the mixer,
//! resampled from 48 kHz to the device's rate; the input is mixed down to
//! mono and resampled to 48 kHz into the microphone buffer. Without it (and
//! in tests) there are no devices, and the mixer and the buffer are driven
//! by hand.

use crate::mic::MicBuffer;
use crate::mixer::Mixer;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// An open stream; dropping it closes it.
pub struct Stream {
    stop: Option<mpsc::Sender<()>>,
    thread: Option<JoinHandle<()>>,
    /// The device's name and format, for the settings.
    pub name: String,
}

impl Stream {
    /// A stream on its own thread: `open` builds it there and returns what
    /// keeps it alive (or why it couldn't), and the thread holds it until
    /// the stream is dropped.
    pub fn spawn<K: 'static>(
        what: &str,
        open: impl FnOnce() -> Result<(K, String), String> + Send + 'static,
    ) -> Result<Stream, String> {
        let (stop, stopped) = mpsc::channel::<()>();
        let (ready, opened) = mpsc::channel::<Result<String, String>>();
        let thread = std::thread::Builder::new()
            .name(format!("hstudio-voice-{what}"))
            .spawn(move || match open() {
                Ok((keep, name)) => {
                    let _ = ready.send(Ok(name));
                    let _ = stopped.recv();
                    drop(keep);
                }
                Err(e) => {
                    let _ = ready.send(Err(e));
                }
            })
            .map_err(|e| e.to_string())?;
        match opened.recv() {
            Ok(Ok(name)) => Ok(Stream {
                stop: Some(stop),
                thread: Some(thread),
                name,
            }),
            Ok(Err(e)) => {
                let _ = thread.join();
                Err(e)
            }
            Err(_) => Err(format!("the {what} stream's thread stopped")),
        }
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        self.stop.take();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Where voices are played and the microphone heard.
pub trait Devices: Send + Sync {
    /// Plays the mixer.
    fn output(&self, mixer: Arc<Mutex<Mixer>>) -> Result<Stream, String>;
    /// Captures into the buffer.
    fn input(&self, mic: Arc<MicBuffer>) -> Result<Stream, String>;
}

/// No devices: the mixer and the microphone buffer are only driven by hand.
pub struct NoDevices;

impl Devices for NoDevices {
    fn output(&self, _mixer: Arc<Mutex<Mixer>>) -> Result<Stream, String> {
        Err("no audio output in this build".into())
    }
    fn input(&self, _mic: Arc<MicBuffer>) -> Result<Stream, String> {
        Err("no microphone in this build".into())
    }
}

/// The system's default devices (with the `audio` feature), else none.
pub fn system() -> Arc<dyn Devices> {
    #[cfg(feature = "audio")]
    {
        Arc::new(system::Cpal)
    }
    #[cfg(not(feature = "audio"))]
    {
        Arc::new(NoDevices)
    }
}

#[cfg(feature = "audio")]
mod system {
    use super::{Devices, Stream};
    use crate::mic::MicBuffer;
    use crate::mixer::Mixer;
    use crate::proto::SAMPLE_RATE;
    use crate::resample::{Resampler, to_mono};
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::{Data, SampleFormat};
    use std::sync::{Arc, Mutex};

    pub struct Cpal;

    /// f32 samples into the device's format.
    fn write_out(data: &mut Data, src: &[f32]) {
        match data.sample_format() {
            SampleFormat::F32 => {
                if let Some(d) = data.as_slice_mut::<f32>() {
                    d.copy_from_slice(&src[..d.len()]);
                }
            }
            SampleFormat::I16 => {
                if let Some(d) = data.as_slice_mut::<i16>() {
                    for (o, s) in d.iter_mut().zip(src) {
                        *o = (s * 32767.0) as i16;
                    }
                }
            }
            SampleFormat::U16 => {
                if let Some(d) = data.as_slice_mut::<u16>() {
                    for (o, s) in d.iter_mut().zip(src) {
                        *o = ((s * 32767.0) as i32 + 32768) as u16;
                    }
                }
            }
            SampleFormat::I32 => {
                if let Some(d) = data.as_slice_mut::<i32>() {
                    for (o, s) in d.iter_mut().zip(src) {
                        *o = (f64::from(*s) * 2_147_483_647.0) as i32;
                    }
                }
            }
            _ => {}
        }
    }

    /// The device's samples as f32.
    fn read_in(data: &Data, out: &mut Vec<f32>) {
        out.clear();
        match data.sample_format() {
            SampleFormat::F32 => out.extend_from_slice(data.as_slice::<f32>().unwrap_or(&[])),
            SampleFormat::I16 => out.extend(
                data.as_slice::<i16>()
                    .unwrap_or(&[])
                    .iter()
                    .map(|&s| f32::from(s) / 32768.0),
            ),
            SampleFormat::U16 => out.extend(
                data.as_slice::<u16>()
                    .unwrap_or(&[])
                    .iter()
                    .map(|&s| (f32::from(s) - 32768.0) / 32768.0),
            ),
            SampleFormat::I32 => out.extend(
                data.as_slice::<i32>()
                    .unwrap_or(&[])
                    .iter()
                    .map(|&s| (f64::from(s) / 2_147_483_648.0) as f32),
            ),
            SampleFormat::U8 => out.extend(
                data.as_slice::<u8>()
                    .unwrap_or(&[])
                    .iter()
                    .map(|&s| (f32::from(s) - 128.0) / 128.0),
            ),
            _ => {}
        }
    }

    fn supported(format: SampleFormat, input: bool) -> bool {
        matches!(
            format,
            SampleFormat::F32 | SampleFormat::I16 | SampleFormat::U16 | SampleFormat::I32
        ) || (input && format == SampleFormat::U8)
    }

    impl Devices for Cpal {
        fn output(&self, mixer: Arc<Mutex<Mixer>>) -> Result<Stream, String> {
            Stream::spawn("output", move || {
                let host = cpal::default_host();
                let device = host.default_output_device().ok_or("no audio output device")?;
                let config = device.default_output_config().map_err(|e| e.to_string())?;
                let format = config.sample_format();
                if !supported(format, false) {
                    return Err(format!("the output's sample format {format} isn't supported"));
                }
                let channels = usize::from(config.channels()).max(1);
                let rate = config.sample_rate().0;
                let name = format!("{} ({rate} Hz)", device.name().unwrap_or_else(|_| "Output".into()));
                let mut left = Resampler::new(SAMPLE_RATE, rate);
                let mut right = Resampler::new(SAMPLE_RATE, rate);
                let (mut l_out, mut r_out) = (Vec::new(), Vec::new());
                let mut stereo = Vec::new();
                let mut buf: Vec<f32> = Vec::new();
                let step = f64::from(SAMPLE_RATE) / f64::from(rate.max(1));
                let stream = device
                    .build_output_stream_raw(
                        &config.config(),
                        format,
                        move |data: &mut Data, _| {
                            let frames = data.len() / channels;
                            // Enough 48 kHz frames for this many device frames, carried over.
                            while l_out.len() < frames {
                                let need = (((frames - l_out.len()) as f64) * step).ceil() as usize + 1;
                                stereo.resize(need * 2, 0.0);
                                if let Ok(mut m) = mixer.lock() {
                                    m.render(&mut stereo);
                                } else {
                                    stereo.fill(0.0);
                                }
                                let (l, r): (Vec<f32>, Vec<f32>) = stereo.chunks_exact(2).map(|f| (f[0], f[1])).unzip();
                                left.process(&l, &mut l_out);
                                right.process(&r, &mut r_out);
                            }
                            buf.clear();
                            // Both sides resample alike, so they are as long.
                            for (&l, &r) in l_out.iter().zip(r_out.iter()).take(frames) {
                                match channels {
                                    1 => buf.push((l + r) * 0.5),
                                    _ => {
                                        buf.push(l);
                                        buf.push(r);
                                        buf.extend(std::iter::repeat_n(0.0, channels - 2));
                                    }
                                }
                            }
                            l_out.drain(..frames);
                            r_out.drain(..frames.min(r_out.len()));
                            write_out(data, &buf);
                        },
                        |_| {},
                        None,
                    )
                    .map_err(|e| e.to_string())?;
                stream.play().map_err(|e| e.to_string())?;
                Ok((stream, name))
            })
        }

        fn input(&self, mic: Arc<MicBuffer>) -> Result<Stream, String> {
            Stream::spawn("input", move || {
                let host = cpal::default_host();
                let device = host.default_input_device().ok_or("no microphone")?;
                let config = device.default_input_config().map_err(|e| e.to_string())?;
                let format = config.sample_format();
                if !supported(format, true) {
                    return Err(format!("the microphone's sample format {format} isn't supported"));
                }
                let channels = usize::from(config.channels()).max(1);
                let rate = config.sample_rate().0;
                let name = format!("{} ({rate} Hz)", device.name().unwrap_or_else(|_| "Microphone".into()));
                let mut resampler = Resampler::new(rate, SAMPLE_RATE);
                let (mut raw, mut mono, mut out) = (Vec::new(), Vec::new(), Vec::new());
                let stream = device
                    .build_input_stream_raw(
                        &config.config(),
                        format,
                        move |data: &Data, _| {
                            read_in(data, &mut raw);
                            mono.clear();
                            to_mono(&raw, channels, &mut mono);
                            out.clear();
                            resampler.process(&mono, &mut out);
                            mic.push(&out);
                        },
                        |_| {},
                        None,
                    )
                    .map_err(|e| e.to_string())?;
                stream.play().map_err(|e| e.to_string())?;
                Ok((stream, name))
            })
        }
    }
}
