mod ffmpeg;
use crate::ffmpeg::EncodeParams;
use clap::Parser;
use eframe::egui;
use ffmpeg::{AudioStream, Codec, FFmpeg, Resolution};
use hide_console;
use std::{path::PathBuf, process::exit};
use strum::IntoEnumIterator;
use win_msgbox;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    video_path: Option<PathBuf>,
}

fn main() -> eframe::Result {
    // Hides console when launching the executable from a double click
    hide_console::hide_console();

    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([768.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Clipinator",
        options,
        Box::new(|cc| {
            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);

            Ok(Box::<Gui>::default())
        }),
    )
}

struct Gui {
    audio_streams: Vec<ffmpeg::AudioStream>,
    ffmpeg: FFmpeg,
    selected_audio_stream: u32,
    selected_codec: u32,
    selected_resolution: u32,
    picked_path: Option<String>,
    start_time: f32,
    end_time: f32,
}

impl Default for Gui {
    fn default() -> Self {
        let args = Args::parse();
        let mut ffmpeg = FFmpeg::new();
        match &ffmpeg.check_executable_path() {
            Ok(_) => {}
            Err(error) => {
                eprintln!("Couldn't find ffmpeg path: {}", error);
                exit(1);
            }
        };
        let mut audio_streams: Vec<AudioStream> = Vec::new();
        let mut picked_path = None;
        match args.video_path {
            Some(video_path) => {
                picked_path = Some(video_path.to_str().unwrap().to_string());
                audio_streams = match ffmpeg.probe_audio_streams(video_path.to_path_buf()) {
                    Ok(audio_streams) => audio_streams,
                    Err(_) => {
                        eprintln!("Couldn't parse audio streams");
                        exit(1);
                    }
                };
            }
            None => {}
        }

        Self {
            picked_path,
            audio_streams,
            ffmpeg: ffmpeg.clone(),
            selected_audio_stream: 0,
            selected_codec: Codec::H264 as u32,
            selected_resolution: Resolution::FHD as u32,
            start_time: 0.0,
            end_time: ffmpeg.video_duration as f32,
        }
    }
}

impl eframe::App for Gui {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.set_pixels_per_point(1.2);
        egui_extras::install_image_loaders(ui);
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.image(egui::include_image!("../image/clipinator_logo.png"));
            ui.separator();

            // File picker prompt button
            ui.label(
                egui::RichText::new("1. Select the clip you want to trim and convert").strong(),
            );

            if ui.button("Open video file…").clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("Video file", &["mp4", "mkv"])
                    .pick_file()
            {
                self.picked_path = Some(path.display().to_string());
                self.audio_streams = self
                    .ffmpeg
                    .probe_audio_streams(PathBuf::from(path.display().to_string()))
                    .unwrap();
                self.end_time = self.ffmpeg.video_duration as f32;
            }

            // Filename
            if let Some(picked_path) = &self.picked_path {
                ui.horizontal(|ui| {
                    ui.label("Video:");
                    ui.monospace(picked_path);
                });
            }

            // Some prerequisites in case there isn't a video loaded
            ui.separator();
            ui.label(
                egui::RichText::new("2. Pick an audio stream, a video codec and a resolution")
                    .strong(),
            );
            let mut audio_stream_str = "None".to_string();
            if self.audio_streams.len() != 0 {
                audio_stream_str = format!(
                    "#{}: {}",
                    self.selected_audio_stream,
                    &self.audio_streams[self.selected_audio_stream as usize].name
                );
            }

            ui.horizontal(|ui| {
                // Audio Stream selector
                egui::ComboBox::from_label("Audio stream")
                    .selected_text(audio_stream_str)
                    .show_ui(ui, |ui| {
                        for stream in &self.audio_streams {
                            ui.selectable_value(
                                &mut self.selected_audio_stream,
                                stream.index as u32,
                                format!("#{}: {}", stream.index, stream.name),
                            );
                        }
                    });

                // Video codec selector
                let codec: Codec =
                    num::FromPrimitive::from_u32(self.selected_codec).expect("Codec not found");
                let video_codec_str = codec.pretty_str();
                egui::ComboBox::from_label("Target codec")
                    .selected_text(video_codec_str)
                    .show_ui(ui, |ui| {
                        for codec in Codec::iter() {
                            ui.selectable_value(
                                &mut self.selected_codec,
                                codec.clone() as u32,
                                format!("{}", codec.pretty_str()),
                            );
                        }
                    });

                // Resolution selector
                let resolution: Resolution = num::FromPrimitive::from_u32(self.selected_resolution)
                    .expect("Codec not found");
                let resolution_str = resolution.pretty_str();
                egui::ComboBox::from_label("Target resolution")
                    .selected_text(resolution_str)
                    .show_ui(ui, |ui| {
                        for resolution in Resolution::iter() {
                            ui.selectable_value(
                                &mut self.selected_resolution,
                                resolution.clone() as u32,
                                format!("{}", resolution.pretty_str()),
                            );
                        }
                    });
            });

            // Start and end time selectors
            ui.separator();
            ui.label(egui::RichText::new("3. Trim the video (time is in seconds)").strong());
            ui.horizontal(|ui| {
                ui.horizontal(|ui| {
                    ui.label("Start time");
                    ui.add(
                        egui::DragValue::new(&mut self.start_time)
                            .range(0.0..=self.ffmpeg.video_duration as f32),
                    );
                });

                ui.horizontal(|ui| {
                    ui.label("End time");
                    ui.add(
                        egui::DragValue::new(&mut self.end_time)
                            .range(self.start_time..=self.ffmpeg.video_duration as f32),
                    );
                });
            });

            ui.separator();
            ui.label(
                egui::RichText::new(
                    "4. Done, choose where you want to save the file and the encoding will start",
                )
                .strong(),
            );

            let save_button = egui::Button::new("Save and encode");
            if ui.add(save_button).clicked() {
                if self.ffmpeg.video_path.is_empty() {
                    let _ = win_msgbox::show::<win_msgbox::Okay>("Select a video first");
                } else if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Video file", &["mp4", "mkv"])
                    .save_file()
                {
                    let params = EncodeParams {
                        start_time: self.start_time as u32,
                        end_time: self.end_time as u32,
                        output_path: path.clone(),
                        audio_stream: self.selected_audio_stream,
                        codec: num::FromPrimitive::from_u32(self.selected_codec)
                            .expect("Codec not found!"),
                        resolution: num::FromPrimitive::from_u32(self.selected_resolution)
                            .expect("Resolution not found!"),
                    };

                    self.ffmpeg.encode(params);
                }
            }
        });
    }
}
