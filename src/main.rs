use std::f32::consts::PI;

use libspa_sys::*;
use pipewire as pw;

pub const DEFAULT_RATE: u32 = 44100;
pub const DEFAULT_CHANNELS: u32 = 2;
pub const DEFAULT_VOLUME: f64 = 0.7;
pub const PI_2: f64 = std::f64::consts::PI + std::f64::consts::PI;
pub const CHAN_SIZE: usize = std::mem::size_of::<i16>();

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct AudioFormat(pub u32);

#[allow(non_upper_case_globals)]
impl AudioFormat {
    pub const Unknown: Self = Self(libspa_sys::SPA_AUDIO_FORMAT_UNKNOWN);
    pub const S16LE: Self = Self(libspa_sys::SPA_AUDIO_FORMAT_S16_LE);
    pub const S16BE: Self = Self(libspa_sys::SPA_AUDIO_FORMAT_S16_BE);

    /// Obtain a [`VideoFormat`] from a raw `spa_video_format` variant.
    pub fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    /// Get the raw [`spa_sys::spa_video_format`] representing this `VideoFormat`.
    pub fn as_raw(&self) -> u32 {
        self.0
    }
}

fn main() -> Result<(), pw::Error> {
    // create_wav();

    let mainloop = new_pipewire_microphone();

    unsafe { pw::deinit() };

    // let mut reader = hound::WavReader::open("sine.wav").unwrap();
    // reader.samples::<i16>().for_each(|sample| match sample {
    //     Ok(s) => {
    //         print!("{}", s)
    //     }
    //     Err(e) => println!("{}", e),
    // })
    Ok(())
}

pub fn create_wav() {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 44100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create("sine.wav", spec).unwrap();
    for t in (0..44100).map(|x| x as f32 / 44100.0) {
        let sample = (t * 440.0 * 2.0 * PI).sin();
        let amplitude = i16::MAX as f32;
        writer.write_sample((sample * amplitude) as i16).unwrap();
    }
}

pub fn new_pipewire_microphone() -> Result<pw::MainLoop, pw::Error> {
    let mainloop = pw::MainLoop::new()?;
    let context = pw::Context::new(&mainloop)?;
    let core = context.connect(None)?;

    let stream = pw::stream::Stream::new(
        &core,
        "alvr-mic",
        pw::properties! {
            *pw::keys::MEDIA_TYPE => "Audio",
            *pw::keys::MEDIA_CATEGORY => "Playback",
            *pw::keys::MEDIA_CLASS => "Audio/Source",
            *pw::keys::MEDIA_ROLE => "Communication",
        },
    )?;

    let _listener: pw::stream::StreamListener<f64> = stream
        .add_local_listener_with_user_data(0.0)
        .state_changed(|old, new| {
            println!("State changed: {:?} -> {:?}", old, new);
        })
        .param_changed(|_, id, user_data, param| {})
        .process(|stream, acc| match stream.dequeue_buffer() {
            None => println!("out of buffers"),
            Some(mut buffer) => {
                let datas = buffer.datas_mut();
                let stride = CHAN_SIZE * DEFAULT_CHANNELS as usize;
                let data = &mut datas[0];
                let n_frames = if let Some(slice) = data.data() {
                    let n_frames = slice.len() / stride;
                    for i in 0..n_frames {
                        *acc += PI_2 * 440.0 / DEFAULT_RATE as f64;
                        if *acc >= PI_2 {
                            *acc -= PI_2
                        }
                        let val = (f64::sin(*acc) * DEFAULT_VOLUME * 16767.0) as i16;
                        for c in 0..DEFAULT_CHANNELS {
                            let start = i * stride + (c as usize * CHAN_SIZE);
                            let end = start + CHAN_SIZE;
                            let chan = &mut slice[start..end];
                            chan.copy_from_slice(&i16::to_le_bytes(val));
                        }
                    }
                    n_frames
                } else {
                    0
                };
                let chunk = data.chunk_mut();
                *chunk.offset_mut() = 0;
                *chunk.stride_mut() = stride as _;
                *chunk.size_mut() = (stride * n_frames) as _;
            }
        })
        .register()?;

    let obj = pw::spa::pod::object!(
        pw::spa::utils::SpaTypes::ObjectParamFormat,
        pw::spa::param::ParamType::EnumFormat,
        pw::spa::pod::property!(
            pw::spa::format::FormatProperties::MediaType,
            Id,
            pw::spa::format::MediaType::Audio
        ),
        pw::spa::pod::property!(
            pw::spa::format::FormatProperties::MediaSubtype,
            Id,
            pw::spa::format::MediaSubtype::Raw
        ),
        pw::spa::pod::property!(
            pw::spa::format::FormatProperties::AudioFormat,
            Choice,
            Enum,
            Id,
            AudioFormat::S16LE,
            AudioFormat::S16BE
        ),
        pw::spa::pod::property!(pw::spa::format::FormatProperties::AudioRate, Int, 44100),
        pw::spa::pod::property!(pw::spa::format::FormatProperties::AudioChannels, Int, 1),
    );

    let values = pw::spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &pw::spa::pod::Value::Object(obj),
    )
    .unwrap()
    .0
    .into_inner();

    let mut params = [values.as_ptr() as *const libspa_sys::spa_pod];

    stream.connect(
        pw::spa::Direction::Output,
        None,
        pw::stream::StreamFlags::AUTOCONNECT
            | pw::stream::StreamFlags::MAP_BUFFERS
            | pw::stream::StreamFlags::RT_PROCESS,
        &mut params,
    )?;
    mainloop.run();

    Ok(mainloop)
}
