/* This file's goal will be to check if ffmpeg is present in the system and launch encoding commands */

use num_derive::FromPrimitive;
use serde_json::{self, Value};
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use strum_macros::EnumIter;
use which;
use win_msgbox;

#[derive(Clone)]
pub struct FFmpeg {
    ffmpeg_path: String,
    pub video_path: String,
    pub video_duration: f64,
}

#[derive(Clone)]
pub struct AudioStream {
    pub index: u64,
    pub name: String,
}

#[derive(Clone)]
pub struct EncodeParams {
    pub start_time: u32,
    pub end_time: u32,
    pub output_path: PathBuf,
    pub audio_stream: u32,
    pub codec: Codec,
    pub resolution: Resolution,
}

#[derive(Clone, FromPrimitive, EnumIter)]
pub enum Codec {
    H264 = 0,
    HEVC = 1,
    AV1 = 2,
    H264amf = 3,
    HEVCamf = 4,
    AV1amf = 5,
}

impl Codec {
    pub fn as_str(&self) -> &'static str {
        match self {
            Codec::H264 => "h264",
            Codec::HEVC => "libx265",
            Codec::AV1 => "av1",
            Codec::H264amf => "h264_amf",
            Codec::HEVCamf => "hevc_amf",
            Codec::AV1amf => "av1_amf",
        }
    }

    pub fn pretty_str(&self) -> &'static str {
        match self {
            Codec::H264 => "H264",
            Codec::HEVC => "HEVC",
            Codec::AV1 => "AV1",
            Codec::H264amf => "H264 (AMF)",
            Codec::HEVCamf => "HEVC (AMF)",
            Codec::AV1amf => "AV1 (AMF)",
        }
    }
}

#[derive(Clone, FromPrimitive, EnumIter)]
pub enum Resolution {
    SD,
    HDReady,
    FHD,
    QHD,
    UHD,
}

impl Resolution {
    pub fn as_args(&self) -> [&str; 2] {
        match self {
            Resolution::SD => ["-vf", "scale=854:480"],
            Resolution::HDReady => ["-vf", "scale=1280:720"],
            Resolution::FHD => ["-vf", "scale=1920:1080"],
            Resolution::QHD => ["-vf", "scale=2560:1440"],
            Resolution::UHD => ["-vf", "scale=3840:2160"],
        }
    }

    pub fn pretty_str(&self) -> &'static str {
        match self {
            Resolution::SD => "480p",
            Resolution::HDReady => "720p",
            Resolution::FHD => "1080p",
            Resolution::QHD => "1440p",
            Resolution::UHD => "2160p",
        }
    }
}

impl FFmpeg {
    pub fn new() -> FFmpeg {
        return FFmpeg {
            ffmpeg_path: "".to_string(),
            video_path: "".to_string(),
            video_duration: 0.0,
        };
    }

    pub fn check_executable_path(&mut self) -> Result<String, String> {
        let _ = match which::which("ffmpeg") {
            Ok(path) => {
                self.ffmpeg_path = path.to_str().unwrap().to_string();
                return Ok(path.into_os_string().into_string().unwrap());
            }
            Err(error) => return Err(error.to_string()),
        };
    }

    pub fn probe_audio_streams(
        &mut self,
        video_file_path: PathBuf,
    ) -> Result<Vec<AudioStream>, ()> {
        let output_res = Command::new("ffprobe")
            .args(["-v", "error"])
            .args(["-show_streams", "-select_streams", "a"])
            .args(["-of", "json"])
            .args(video_file_path.to_str())
            .output();

        let output = match output_res {
            Ok(output) => output,
            Err(error) => {
                eprintln!("couldn't run ffprobe: {}", error);
                return Err(());
            }
        };

        let probe_result = String::from_utf8(output.stdout).unwrap();
        let probe_json: Value = match serde_json::from_str(probe_result.as_str()) {
            Ok(parsed) => parsed,
            Err(_) => {
                eprintln!("couldn't parse json");
                return Err(());
            }
        };

        self.video_path = video_file_path.to_str().unwrap().to_string();
        let mut audio_streams: Vec<AudioStream> = Vec::new();
        let streams = probe_json["streams"].as_array().unwrap();

        //set video duration from stream duration (they're the same)
        self.video_duration = streams[0]["duration"]
            .as_str()
            .unwrap()
            .parse::<f64>()
            .unwrap();

        self.video_duration = self.video_duration.round();

        for stream in streams {
            let index = stream["index"]
                .as_u64()
                .expect("couldn't convert id to u64");

            let mut audio_stream = AudioStream {
                index,
                name: match stream["tags"]["name"].as_str() {
                    Some(name) => name.to_string(),
                    None => format!("Audio track {}", index - 1),
                },
            };
            audio_stream.index -= 1; // This is done because we'll use it to single out an audio stream with the map argument on ffmpeg, and they're offset by one
            audio_streams.push(audio_stream);
        }

        return Ok(audio_streams);
    }

    pub fn encode(&self, params: EncodeParams) {
        let ffmpeg_path = self.ffmpeg_path.clone();
        let video_path = self.video_path.clone();
        thread::spawn(move || {
            let _ = Command::new(ffmpeg_path.as_str())
                .args(["-v", "quiet"])
                .args(["-i", &video_path.as_str()])
                .args(["-ss", format!("{}", &params.start_time).as_str()])
                .args(["-to", format!("{}", &params.end_time).as_str()])
                .args(["-c:v", &params.codec.as_str()])
                .args(params.resolution.as_args())
                .args([
                    "-map",
                    "0:v:0",
                    "-map",
                    format!("0:a:{}", params.audio_stream).as_str(),
                ])
                .arg(&params.output_path)
                .output();

            let msg = format!(
                "Clip successfully converted at: {}",
                &params.output_path.to_str().unwrap()
            );

            let _ = win_msgbox::show::<win_msgbox::Okay>(msg.as_str());
        });
    }
}
