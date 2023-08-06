use std::f32::consts::PI;

use pipewire as pw;
use libspa_sys::*;

struct UserData {
    format: pw::spa::param::video::VideoInfoRaw,
}

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

// #[macro_export]
// macro_rules! __property__ {
//     ($key:expr, Choice, Enum, Id, $default:expr, $($alternative:expr),+ $(,)?) => {
//         pipewire::spa::pod::property!(
//             $key,
//             pipewire::spa::pod::Value::Choice(pipewire::spa::pod::ChoiceValue::Id(
//                 pipewire::spa::utils::Choice::<pipewire::spa::utils::Id>(
//                     pipewire::spa::utils::ChoiceFlags::empty(),
//                     pipewire::spa::utils::ChoiceEnum::<pipewire::spa::utils::Id>::Enum {
//                         default: pipewire::spa::utils::Id($default.as_raw()),
//                         alternatives: [ $( pipewire::spa::utils::Id($alternative.as_raw()), )+ ].to_vec()
//                     }
//                 )
//             ))
//         )
//     };
// }

fn main() {
    create_wav();

    let mut reader = hound::WavReader::open("sine.wav").unwrap();
    reader.samples::<i16>().for_each(|sample| match sample {
        Ok(s) => {
            print!("{}", s)
        }
        Err(e) => println!("{}", e),
    })
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

pub fn new_pipewire_microphone() -> Result<(), pw::Error> {
    let mainloop = pw::MainLoop::new()?;
    let context = pw::Context::new(&mainloop)?;
    let core = context.connect(None)?;

    let stream = pw::stream::Stream::new(
        &core,
        "alvr-mic",
        pw::properties! {
            *pw::keys::MEDIA_TYPE => "Audio",
            *pw::keys::MEDIA_CATEGORY => "Capture",
            *pw::keys::MEDIA_ROLE => "Music",
        }
    )?;

    let data = UserData {
        format: Default::default(),
    };

    let _listener = stream
        .add_local_listener_with_user_data(data)
        .state_changed(|old, new| {
            println!("State changed: {:?} -> {:?}", old, new);
        })
        .param_changed(|_, id, user_data, param| {
            if param.is_null() || id != pw::spa::param::ParamType::Format.as_raw() {
                return;
            }

            let (media_type, media_subtype) = unsafe {
                match pw::spa::param::format_utils::spa_parse_format(param) {
                    Ok(v) => v,
                    Err(_) => return,
                }
            };

            if media_type != pw::spa::format::MediaType::Video
                || media_subtype != pw::spa::format::MediaSubtype::Raw
            {
                return;
            }

            unsafe {
                user_data
                    .format
                    .parse(param)
                    .expect("Failed to parse param changed to VideoInfoRaw")
            };

            println!("got video format:");
            println!(
                "  format: {} ({:?})",
                user_data.format.format().as_raw(),
                user_data.format.format()
            );
            println!(
                "  size: {}x{}",
                user_data.format.size().width,
                user_data.format.size().height
            );
            println!(
                "  framerate: {}/{}",
                user_data.format.framerate().num,
                user_data.format.framerate().denom
            );

            // prepare to render video of this size
        })
        .process(|stream, _| {
            match stream.dequeue_buffer() {
                None => println!("out of buffers"),
                Some(mut buffer) => {
                    let datas = buffer.datas_mut();
                    if datas.is_empty() {
                        return;
                    }

                    // copy frame data to screen
                    let data = &mut datas[0];
                    println!("got a frame of size {}", data.chunk().size());
                }
            }
        })
        .register()?;

        pw::spa::param::video::VideoFormat;

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
            pw::spa::format::FormatProperties::AudioRate,
            Choice,
            Range,
            Fraction,
            pw::spa::utils::Fraction {
                num: 48000, denom: 1
            },
            pw::spa::utils::Fraction {
                num: 8000, denom: 1
            },
            pw::spa::utils::Fraction {
                num: 192000, denom: 1
            }
        ),
        pw::spa::pod::property!(
            pw::spa::format::FormatProperties::AudioFormat,
            Choice,
            Enum,
            Id,
            AudioFormat::S16LE,
            AudioFormat::S16BE
        ),
        pw::spa::pod::property!(
            pw::spa::format::FormatProperties::AudioChannels,
            Choice,
            pw::spa::utils::ChoiceEnum::<Int>::None(pw::pod:: ::Value::Int(1))
            
        ),
    );


    let values = pw::spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &pw::spa::pod::Value::Object(obj),
    )
    .unwrap()
    .0
    .into_inner();

    // let mut params = [values.as_ptr() as *const spa_sys::spa_pod];

    // stream.connect(
    //     spa::Direction::Input,
    //     opt.target,
    //     pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS,
    //     &mut params,
    // )?;


    Ok(())
}

fn process(sample: i16) {}
