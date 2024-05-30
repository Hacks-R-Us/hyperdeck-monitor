use std::{
    collections::{HashMap, VecDeque},
    mem,
    net::{IpAddr, Ipv4Addr},
};

use egui::{Button, Color32, RichText, Rounding, Sense, Stroke, Vec2};
use ewebsock::{Options, WsReceiver, WsSender};
use wasm_timer::Instant;

use crate::websocket::{
    AddHyperdeckRequest, ClientRequest, HyperdeckConnectionState, HyperdeckRecordBay,
    RecordingState, RemoveHyperdeckRequest, ServerEvent,
};

pub struct HyperdeckMonitorApp {
    blink: bool,
    last_blink_change: Instant,
    new_hyperdeck_ip: String,
    new_hyperdeck_name: String,
    new_hyperdeck_port: String,
    hyperdecks: Vec<Hyperdeck>,
    websocket_message_queue: VecDeque<ClientRequest>,
    ws_sender: WsSender,
    ws_receiver: WsReceiver,
}

impl Default for HyperdeckMonitorApp {
    fn default() -> Self {
        let api_websocket_location = format!("ws://voc.emf.camp:9681/ws");
        let (ws_sender, ws_receiver) =
            ewebsock::connect(api_websocket_location, Options::default()).unwrap();
        Self {
            blink: false,
            last_blink_change: Instant::now(),
            new_hyperdeck_ip: "".to_owned(),
            new_hyperdeck_name: "".to_owned(),
            new_hyperdeck_port: 9993.to_string(),
            hyperdecks: vec![
                Hyperdeck {
                    id: "test-1".to_string(),
                    name: "Description: Connected Hyperdeck - Not Recording - Not Much Time"
                        .to_string(),
                    ip: IpAddr::V4(Ipv4Addr::new(192, 168, 10, 1)),
                    status: HyperdeckStatus::Connected,
                    recording_status: RecordingState::NotRecording,
                    slots: vec![(
                        0usize,
                        HyperdeckRecordBay {
                            recording_time_remaining: 60,
                        },
                    )]
                    .into_iter()
                    .collect::<HashMap<usize, HyperdeckRecordBay>>(),
                },
                Hyperdeck {
                    id: "test-2".to_string(),
                    name: "Description: Connected Hyperdeck - Recording - Not Much Time"
                        .to_string(),
                    ip: IpAddr::V4(Ipv4Addr::new(192, 168, 10, 2)),
                    status: HyperdeckStatus::Connected,
                    recording_status: RecordingState::Recording,
                    slots: vec![(
                        0usize,
                        HyperdeckRecordBay {
                            recording_time_remaining: 60,
                        },
                    )]
                    .into_iter()
                    .collect::<HashMap<usize, HyperdeckRecordBay>>(),
                },
                Hyperdeck {
                    id: "test-3".to_string(),
                    name: "Description: Connected Hyperdeck - Recording - Plenty of Time"
                        .to_string(),
                    ip: IpAddr::V4(Ipv4Addr::new(192, 168, 10, 3)),
                    status: HyperdeckStatus::Connected,
                    recording_status: RecordingState::Recording,
                    slots: vec![(
                        0usize,
                        HyperdeckRecordBay {
                            recording_time_remaining: 60 * 30,
                        },
                    )]
                    .into_iter()
                    .collect::<HashMap<usize, HyperdeckRecordBay>>(),
                },
                Hyperdeck {
                    id: "test-4".to_string(),
                    name: "Description: Disconnected Hyperdeck".to_string(),
                    ip: IpAddr::V4(Ipv4Addr::new(192, 168, 10, 4)),
                    status: HyperdeckStatus::Disconnected,
                    recording_status: RecordingState::NotRecording,
                    slots: vec![(
                        0usize,
                        HyperdeckRecordBay {
                            recording_time_remaining: 3600 * 5, // 5 Hours
                        },
                    )]
                    .into_iter()
                    .collect::<HashMap<usize, HyperdeckRecordBay>>(),
                },
            ],
            websocket_message_queue: VecDeque::new(),
            ws_sender,
            ws_receiver,
        }
    }
}

impl eframe::App for HyperdeckMonitorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(message) = self.websocket_message_queue.pop_front() {
            self.ws_sender.send(ewebsock::WsMessage::Text(
                serde_json::to_string(&message).expect("Could not serialize message"),
            ));
        }
        if let Some(ewebsock::WsEvent::Message(ewebsock::WsMessage::Text(event))) =
            self.ws_receiver.try_recv()
        {
            if let Ok(received) = serde_json::from_str::<ServerEvent>(&event) {
                match received {
                    ServerEvent::HyperdeckMonitorState(state) => {
                        self.hyperdecks = Default::default();
                        for (id, hyperdeck) in state.hyperdecks {
                            self.hyperdecks.push(Hyperdeck {
                                id,
                                name: hyperdeck.name,
                                ip: hyperdeck.ip.parse().unwrap(),
                                status: hyperdeck.connection_state.into(),
                                recording_status: hyperdeck.recording_status,
                                slots: hyperdeck.slots,
                            })
                        }
                    }
                }
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            add_hyperdeck_panel(
                ui,
                &mut self.new_hyperdeck_name,
                &mut self.new_hyperdeck_ip,
                &mut self.new_hyperdeck_port,
                &mut self.websocket_message_queue,
            );
            ui.separator();

            ui.vertical(|ui| {
                hyperdeck_list(
                    ui,
                    &self.hyperdecks,
                    self.blink,
                    &mut self.websocket_message_queue,
                );
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
    new_hyperdeck_name: &mut String,
    new_hyperdeck_ip: &mut String,
    new_hyperdeck_port: &mut String,
    message_queue: &mut VecDeque<ClientRequest>,
) {
    ui.heading("Add hyperdeck");
    ui.horizontal(|ui| {
        ui.label("Name");
        ui.text_edit_singleline(new_hyperdeck_name);
        ui.label("IP");
        ui.text_edit_singleline(new_hyperdeck_ip);
        ui.label("Port");
        ui.text_edit_singleline(new_hyperdeck_port);
        let button_enabled = new_hyperdeck_ip.parse::<IpAddr>().is_ok()
            && !new_hyperdeck_name.is_empty()
            && new_hyperdeck_port.parse::<u16>().is_ok();
        if ui.add_enabled(button_enabled, Button::new("Add")).clicked() {
            message_queue.push_back(ClientRequest::AddHyperdeck(AddHyperdeckRequest {
                name: mem::take(new_hyperdeck_name),
                ip: mem::take(new_hyperdeck_ip),
                port: mem::replace(new_hyperdeck_port, "9993".to_string())
                    .parse::<u16>()
                    .unwrap(),
            }));
        }
    });
}

fn hyperdeck_list(
    ui: &mut egui::Ui,
    hyperdecks: &[Hyperdeck],
    blink: bool,
    message_queue: &mut VecDeque<ClientRequest>,
) {
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
                if ui.button("Remove").clicked() {
                    message_queue.push_back(ClientRequest::RemoveHyperdeck(
                        RemoveHyperdeckRequest {
                            id: hyperdeck.id.clone(),
                        },
                    ));
                }
            });
            if matches!(hyperdeck.status, HyperdeckStatus::Connected) {
                ui.horizontal(|ui| {
                    match hyperdeck.recording_status {
                        RecordingState::Recording => {
                            let (response, painter) =
                                ui.allocate_painter(Vec2 { x: 16.0, y: 16.0 }, Sense::hover());
                            let rect = response.rect;
                            painter.rect(
                                rect,
                                Rounding::ZERO,
                                Color32::from_rgb(255, 255, 255),
                                Stroke::NONE,
                            );
                            let recording_text: RichText = "[Recording]".into();
                            ui.label(
                                recording_text
                                    .color(Color32::from_rgb(255, 255, 255))
                                    .strong(),
                            );
                        }
                        RecordingState::NotRecording => {
                            ui.label("[Not Recording]");
                        }
                    };
                });

                for (index, slot) in hyperdeck.slots.iter() {
                    ui.horizontal(|ui| {
                        let slot_label: RichText = format!("Slot {}", index + 1).into();
                        ui.label(slot_label.strong());

                        let time_remaining_text: RichText =
                            format!("Time remaining: {}", slot.recording_time_remaining).into();

                        if slot.recording_time_remaining > 15 * 60 || !blink {
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

#[derive(serde::Deserialize, serde::Serialize)]
struct Hyperdeck {
    id: String,
    name: String,
    ip: IpAddr,
    status: HyperdeckStatus,
    recording_status: RecordingState,
    slots: HashMap<usize, HyperdeckRecordBay>,
}

#[derive(serde::Deserialize, serde::Serialize)]
enum HyperdeckStatus {
    Connected,
    Disconnected,
}

impl From<HyperdeckConnectionState> for HyperdeckStatus {
    fn from(value: HyperdeckConnectionState) -> Self {
        match value {
            HyperdeckConnectionState::Connected => HyperdeckStatus::Connected,
            HyperdeckConnectionState::Disconnected => HyperdeckStatus::Disconnected,
        }
    }
}
