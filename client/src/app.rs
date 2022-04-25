use chrono::DateTime;
use crossbeam_channel::{unbounded, Receiver};
use eframe::{
    egui::{self, RichText, ScrollArea, Spinner, TextEdit, TextStyle},
    epaint::Color32,
    epi,
};
use reqwest::header::HeaderValue;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, thread};
use tungstenite::{client::IntoClientRequest, connect};

#[derive(Deserialize)]
struct Account {
    id: String,
    username: String,
    token: String,
}

#[derive(Deserialize, Clone)]
struct Guild {
    name: String,
    id: String,
    // createdAt: String,
    channels: Vec<Channel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SocketMessagePayload {
    content: String,
    author: User,
    channel_id: String,
    created_at: String,
    id: String,
}

#[derive(Deserialize, Clone)]
struct Channel {
    name: String,
    id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    id: String,
    username: String,
}

#[derive(Debug, Deserialize, Clone)]
struct Message {
    id: String,
    content: String,
    created_at: String,
    author: User,
}

#[derive(Default)]
pub struct RustCord {
    inputs: HashMap<String, String>,
    open_windows: HashMap<String, String>,
    current_guild: Option<Guild>,
    current_channel: Option<Channel>,
    message_cache: HashMap<String, Vec<Message>>,
    guilds: Vec<Guild>,
    account: Option<Account>,
    http: reqwest::blocking::Client,
    socket_channel: Option<Receiver<String>>,
}

const INSTANCE_URL: &str = "http://localhost:3000";

impl epi::App for RustCord {
    fn update(&mut self, ctx: &egui::Context, _frame: &epi::Frame) {
        let Self {
            inputs,
            open_windows,
            current_guild,
            current_channel,
            message_cache,
            guilds,
            account,
            http,
            socket_channel,
        } = self;

        if account.is_none() {
            egui::Window::new("Login or Register").show(ctx, |ui| {
                ui.add(
                    TextEdit::singleline(inputs.get_mut("username").unwrap())
                        .desired_width(f32::INFINITY)
                        .desired_rows(1)
                        .hint_text("Username"),
                );
                ui.add(
                    TextEdit::singleline(inputs.get_mut("password").unwrap())
                        .desired_width(f32::INFINITY)
                        .desired_rows(1)
                        .hint_text("Password")
                        .password(true),
                );
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Login").clicked() {
                        let mut data = HashMap::new();
                        data.insert("username", inputs.get("username").unwrap());
                        data.insert("password", inputs.get("password").unwrap());

                        ui.add(Spinner::new());

                        let req = http
                            .post(format!("{}/{}", INSTANCE_URL, "login"))
                            .json(&data)
                            .send();

                        self.account = Some(req.unwrap().json::<Account>().unwrap());

                        self.guilds = http
                            .get(format!("{}/{}", INSTANCE_URL, "users/me/guilds"))
                            .header(
                                "Authorization",
                                format!("Bearer {}", self.account.as_ref().unwrap().token),
                            )
                            .send()
                            .unwrap()
                            .json()
                            .unwrap();

                        let mut request = "ws://localhost:3000/ws".into_client_request().unwrap();

                        request.headers_mut().append(
                            "Authorization",
                            HeaderValue::from_str(&self.account.as_ref().unwrap().token).unwrap(),
                        );

                        let (mut sock, _response) = connect(request).unwrap();

                        let (s, r) = unbounded();

                        *socket_channel = Some(r);

                        thread::spawn(move || loop {
                            let socket_msg = sock.read_message().unwrap().into_text().unwrap();

                            s.send(socket_msg).unwrap();
                        });
                    }

                    if ui.button("Register").clicked() {
                        let mut data = HashMap::new();
                        data.insert("username", inputs.get("username"));
                        data.insert("password", inputs.get("password"));

                        ui.add(Spinner::new());

                        http.post(format!("{}/{}", INSTANCE_URL, "register"))
                            .json(&data)
                            .send()
                            .unwrap();
                    }
                });
            });
            return;
        }

        let socket = socket_channel.as_ref().unwrap();

        let account = self.account.as_ref().unwrap();

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("Servers", |ui| {
                    for guild in guilds {
                        if ui
                            .radio(
                                current_guild.is_some()
                                    && current_guild.as_ref().unwrap().id == guild.id,
                                guild.name.to_owned(),
                            )
                            .clicked()
                        {
                            *current_guild = Some(guild.clone());
                        }
                    }
                });
            });
        });

        if current_guild.is_none() {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.with_layout(
                    egui::Layout::centered_and_justified(egui::Direction::TopDown),
                    |ui| {
                        ui.heading("Select a guild");
                    },
                );
            });
            return;
        }

        let guild = current_guild.as_ref().unwrap();

        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.heading("Channels");

            for channel in &guild.channels {
                if ui.button(channel.name.to_owned()).clicked() {
                    *current_channel = Some(channel.clone());

                    let mut messages: Vec<Message> = http
                        .get(format!("{}/channels/{}/messages", INSTANCE_URL, channel.id))
                        .header("Authorization", format!("Bearer {}", account.token))
                        .send()
                        .unwrap()
                        .json()
                        .unwrap();

                    messages.sort_by(|a, b| {
                        DateTime::parse_from_rfc3339(&a.created_at)
                            .unwrap()
                            .cmp(&DateTime::parse_from_rfc3339(&b.created_at).unwrap())
                    });

                    message_cache.insert(channel.id.to_owned(), messages);
                };
            }

            let msg = socket.try_recv();

            if let Ok(msg) = msg {
                let data: SocketMessagePayload = serde_json::from_str(&msg).unwrap();

                if message_cache.contains_key(&data.channel_id) {
                    let messages = message_cache.get_mut(&data.channel_id).unwrap();

                    messages.push(Message {
                        author: User {
                            id: data.author.id,
                            username: data.author.username,
                        },
                        content: data.content,
                        created_at: data.created_at,
                        id: data.id,
                    });

                    messages.sort_by(|a, b| {
                        DateTime::parse_from_rfc3339(&a.created_at)
                            .unwrap()
                            .cmp(&DateTime::parse_from_rfc3339(&b.created_at).unwrap())
                    });
                }
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label("Logged in as ");
                    ui.label(RichText::new(account.username.to_owned()).color(Color32::WHITE));
                });
                egui::warn_if_debug_build(ui);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if current_channel.is_none() {
                ui.heading("Select a channel");
                return;
            }

            let current_channel = current_channel.as_ref().unwrap();

            ui.heading(current_channel.name.to_owned());

            ui.add_space(4.0);

            let current_message_cache = message_cache.get(current_channel.id.as_str()).unwrap();

            let text_style = TextStyle::Body;
            let row_height = ui.text_style_height(&text_style);
            let num_rows = current_message_cache.len();

            ScrollArea::vertical()
                .stick_to_bottom()
                .auto_shrink([false; 2])
                .max_height(ui.max_rect().height() - row_height * 3.5)
                .show_rows(ui, row_height, num_rows, |ui, row_range| {
                    for message in &current_message_cache[row_range] {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("[{}]", message.author.username.to_owned()))
                                    .color(Color32::WHITE),
                            );
                            ui.label(message.content.to_owned());
                        });
                    }
                });

            let textbox = ui.add(
                TextEdit::multiline(inputs.get_mut("chatbox").unwrap())
                    .desired_width(f32::INFINITY)
                    .desired_rows(1)
                    .hint_text(format!("Message #{}", current_channel.name)),
            );
            // enter sends message, shift+enter creates a new line
            if textbox.has_focus()
                && ctx.input().key_pressed(egui::Key::Enter)
                && !ctx.input().modifiers.shift
            {
                let mut data = HashMap::new();
                data.insert("content", inputs.get("chatbox").unwrap().trim());

                ui.with_layout(egui::Layout::right_to_left(), |ui| ui.add(Spinner::new()));

                let res = http
                    .post(format!(
                        "{}/channels/{}/messages",
                        INSTANCE_URL, current_channel.id
                    ))
                    .header("Authorization", format!("Bearer {}", account.token))
                    .json(&data)
                    .send()
                    .unwrap();

                *inputs.get_mut("chatbox").unwrap() = "".to_owned();
            }
        });
    }

    fn setup(
        &mut self,
        _ctx: &egui::Context,
        _frame: &epi::Frame,
        _storage: Option<&dyn epi::Storage>,
    ) {
        self.inputs.insert("username".to_owned(), "".to_owned());
        self.inputs.insert("password".to_owned(), "".to_owned());
        self.inputs.insert("chatbox".to_owned(), "".to_owned());

        self.http = reqwest::blocking::Client::new();

        // re-add when token is stored
        // self.guilds = self
        //     .http
        //     .get(format!("{}/{}", INSTANCE_URL, "users/me/guilds"))
        //     .header(
        //         "Authorization",
        //         format!("Bearer {}", self.account.as_ref().unwrap().token),
        //     )
        //     .send()
        //     .unwrap()
        //     .json()
        //     .unwrap();
    }

    fn name(&self) -> &str {
        "rustcord"
    }
}
