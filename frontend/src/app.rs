use std::{
    fmt::Display,
    net::{IpAddr, Ipv4Addr},
};

use egui::{Color32, RichText, Sense, Stroke, Vec2};
use wasm_timer::Instant;

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct HyperdeckMonitorApp {
    #[serde(skip)]
    blink: bool,
    #[serde(skip)]
    last_blink_change: Instant,
    #[serde(skip)]
    new_hyperdeck_ip: String,

    #[serde(skip)]
    new_hyperdeck_name: String,

    #[serde(skip)]
    hyperdecks: Vec<Hyperdeck>,
}

impl Default for HyperdeckMonitorApp {
    fn default() -> Self {
        Self {
            blink: false,
            last_blink_change: Instant::now(),
            new_hyperdeck_ip: "".to_owned(),
            new_hyperdeck_name: "".to_owned(),
            hyperdecks: vec![
                Hyperdeck {
                    name: "Test Hyperdeck 1".to_string(),
                    ip: IpAddr::V4(Ipv4Addr::new(192, 168, 10, 1)),
                    status: HyperdeckStatus::Connected,
                    recording_bays: vec![HyperdeckRecordBay {
                        status: RecordingStatus::NotRecording,
                        storage_capacity_mb: 500_000,
                        recording_time_remaining: TimeRemaining(60),
                    }],
                },
                Hyperdeck {
                    name: "Test Hyperdeck 2".to_string(),
                    ip: IpAddr::V4(Ipv4Addr::new(192, 168, 10, 2)),
                    status: HyperdeckStatus::Disconnected,
                    recording_bays: vec![HyperdeckRecordBay {
                        status: RecordingStatus::NotRecording,
                        storage_capacity_mb: 500_000,
                        recording_time_remaining: TimeRemaining(3600 * 5), // 5 Hours
                    }],
                },
            ],
        }
    }
}

impl HyperdeckMonitorApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Load previous app state (if any)
        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        }

        Default::default()
    }
}

impl eframe::App for HyperdeckMonitorApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                egui::widgets::global_dark_light_mode_buttons(ui);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            add_hyperdeck_panel(ui, &mut self.new_hyperdeck_ip, &mut self.new_hyperdeck_name);
            ui.separator();

            ui.vertical(|ui| {
                hyperdeck_list(ui, &self.hyperdecks, self.blink);
            });

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                connection_status(ui);
                egui::warn_if_debug_build(ui);
            });
        });

        if self.last_blink_change.elapsed().as_secs() >= 1 {
            self.blink = !self.blink;
            println!("BLINK");
            self.last_blink_change = Instant::now();
        }

        egui::Context::request_repaint(ctx);
    }
}

fn add_hyperdeck_panel(
    ui: &mut egui::Ui,
    new_hyperdeck_ip: &mut String,
    new_hyperdeck_name: &mut String,
) {
    ui.heading("Add hyperdeck");
    ui.horizontal(|ui| {
        ui.label("Name");
        ui.text_edit_singleline(new_hyperdeck_name);
        ui.label("IP");
        ui.text_edit_singleline(new_hyperdeck_ip);
        if ui.button("Add").clicked() {
            // Do Something
        }
    });
}

fn hyperdeck_list(ui: &mut egui::Ui, hyperdecks: &[Hyperdeck], blink: bool) {
    for hyperdeck in hyperdecks {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let status_colour = match hyperdeck.status {
                    HyperdeckStatus::Connected => Color32::GREEN,
                    HyperdeckStatus::Disconnected => Color32::RED,
                };
                let (response, painter) =
                    ui.allocate_painter(Vec2 { x: 16.0, y: 16.0 }, Sense::hover());
                let rect = response.rect;
                let c = rect.center();
                let r = (rect.width() / 2.0) * 0.8;
                painter.circle(c, r, status_colour, Stroke::NONE);
                let hyperdeck_heading: RichText =
                    format!("{} [{}]", hyperdeck.name, hyperdeck.ip).into();
                ui.heading(hyperdeck_heading.strong());
            });
            if !hyperdeck.recording_bays.is_empty()
                && matches!(hyperdeck.status, HyperdeckStatus::Connected)
            {
                let recording_bays_text: RichText = "Recording Bays".into();
                ui.label(recording_bays_text.size(16.0).strong());
                for (index, bay) in hyperdeck.recording_bays.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let bay_label: RichText = format!("Bay {}", index + 1).into();
                        ui.label(bay_label.strong());
                        match bay.status {
                            RecordingStatus::Recording => ui.label("Recording"),
                            RecordingStatus::NotRecording => ui.label("Not Recording"),
                        };
                        ui.label(format!(
                            "Total Storage Capacity: {}GB",
                            bay.storage_capacity_mb / 1000,
                        ));
                        let time_remaining_text: RichText =
                            format!("Time remaining: {}", bay.recording_time_remaining).into();

                        if bay.recording_time_remaining.0 > 15 * 60 || !blink {
                            ui.label(time_remaining_text);
                        } else {
                            ui.label(time_remaining_text.color(Color32::RED));
                        };
                    });
                }
            }
            ui.separator();
        });
    }
}

fn connection_status(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        // TODO: Make it real
        ui.label("Connected");
    });
}

#[derive(serde::Deserialize, serde::Serialize)]
struct Hyperdeck {
    name: String,
    ip: IpAddr,
    status: HyperdeckStatus,
    recording_bays: Vec<HyperdeckRecordBay>,
}

#[derive(serde::Deserialize, serde::Serialize)]
enum HyperdeckStatus {
    Connected,
    Disconnected,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct HyperdeckRecordBay {
    status: RecordingStatus,
    /// Storage capacity in MB.
    storage_capacity_mb: u64,
    /// Recording time available in seconds.
    recording_time_remaining: TimeRemaining,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct TimeRemaining(u64);

impl Display for TimeRemaining {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let time = hrtime::from_sec_padded(self.0);
        write!(f, "{time}")
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
enum RecordingStatus {
    Recording,
    NotRecording,
}
