use chrono::DateTime;
use eframe::{
    egui::{self, RichText, ScrollArea, Spinner, TextEdit, TextStyle},
    emath::Align,
    epaint::Color32,
    epi,
};
use flume::{unbounded, Receiver};
use reqwest::header::HeaderValue;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    thread,
};
use tungstenite::{client::IntoClientRequest, connect};

#[derive(Deserialize)]
struct Account {
    id: String,
    username: String,
    token: String,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Guild {
    name: String,
    id: String,
    created_at: String,
    owner_id: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]

struct SocketTypePayload {
    msg_type: String,
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
    open_windows: HashSet<String>,
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
    fn update(&mut self, ctx: &egui::Context, frame: &epi::Frame) {
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

                        let frame = frame.0.clone();

                        thread::spawn(move || loop {
                            let socket_msg = sock.read_message().unwrap().into_text().unwrap();

                            frame.lock().unwrap().repaint_signal.request_repaint();

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

        let msg = socket.try_recv().unwrap_or_else(|_| "".to_owned());

        let type_payload: SocketTypePayload =
            serde_json::from_str(&msg).unwrap_or(SocketTypePayload {
                msg_type: "".to_string(),
            });

        let account = self.account.as_ref().unwrap();

        if type_payload.msg_type == "user_guild_data_update" {
            println!("user guild update");
            *guilds = http
                .get(format!("{}/{}", INSTANCE_URL, "users/me/guilds"))
                .header(
                    "Authorization",
                    format!("Bearer {}", self.account.as_ref().unwrap().token),
                )
                .send()
                .unwrap()
                .json()
                .unwrap();
        }

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("Guilds", |ui| {
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
                            *current_channel = None;
                        }
                    }
                    if ui.button("Join guild...").clicked() {
                        open_windows.insert("join_guild".to_owned());
                    };

                    if ui.button("Create guild...").clicked() {
                        open_windows.insert("create_guild".to_owned());
                    };
                });

                if current_guild.is_some() && current_guild.as_ref().unwrap().owner_id == account.id
                {
                    ui.menu_button("Manage", |ui| {
                        if ui.button("Create Channel").clicked() {
                            open_windows.insert("create_channel".to_owned());
                        }
                        if ui.button("Delete Guild").clicked() {
                            open_windows.insert("delete_channel".to_owned());
                        }
                    });
                }
            });
        });

        if open_windows.contains("create_channel") {
            egui::Window::new("Create Channel").show(ctx, |ui| {
                ui.add(
                    TextEdit::singleline(inputs.get_mut("channel_name").unwrap())
                        .desired_width(f32::INFINITY)
                        .desired_rows(1)
                        .hint_text("Channel Name"),
                );
                ui.horizontal(|ui| {
                    if ui.button("Create").clicked() {
                        let mut data = HashMap::new();
                        data.insert("name", inputs.get("channel_name").unwrap());

                        http.post(format!(
                            "{}/guilds/{}/channels/create",
                            INSTANCE_URL,
                            current_guild.as_ref().unwrap().id
                        ))
                        .json(&data)
                        .header("Authorization", format!("Bearer {}", account.token))
                        .send()
                        .unwrap();

                        let guild = current_guild.as_ref().unwrap();

                        let mut data = HashMap::new();
                        data.insert("guild_id", guild.clone().id);

                        open_windows.remove("create_channel");
                        inputs.get_mut("channel_name").unwrap().clear();
                    }
                    if ui.button("Cancel").clicked() {
                        open_windows.remove("create_channel");
                        inputs.get_mut("channel_name").unwrap().clear();
                    }
                });
            });
        }

        if open_windows.contains("join_guild") {
            egui::Window::new("Join Guild")
                .show(ctx, |ui| {
                    ui.add(
                        TextEdit::singleline(inputs.get_mut("invite_code").unwrap())
                            .desired_width(f32::INFINITY)
                            .desired_rows(1)
                            .hint_text("Invite Code"),
                    );

                    ui.horizontal(|ui| {
                        if ui.button("Join").clicked() {}

                        if ui.button("Cancel").clicked() {
                            open_windows.remove("join_guild");
                            inputs.get_mut("invite_code").unwrap().clear();
                        }
                    });
                })
                .unwrap();
        }

        if open_windows.contains("create_guild") {
            egui::Window::new("Create Guild")
                .show(ctx, |ui| {
                    ui.add(
                        TextEdit::singleline(inputs.get_mut("guild_name").unwrap())
                            .desired_width(f32::INFINITY)
                            .desired_rows(1)
                            .hint_text("Name"),
                    );

                    ui.horizontal(|ui| {
                        if ui.button("Create").clicked() {
                            let mut data = HashMap::new();
                            data.insert("name", inputs.get("guild_name").unwrap());

                            let res = http
                                .post(format!("{}/guilds/create", INSTANCE_URL))
                                .header("Authorization", format!("Bearer {}", account.token))
                                .json(&data)
                                .send()
                                .unwrap();

                            println!("{:?}", res);

                            open_windows.remove("create_guild");
                            inputs.get_mut("guild_name").unwrap().clear();
                        }

                        if ui.button("Cancel").clicked() {
                            open_windows.remove("create_guild");
                            inputs.get_mut("guild_name").unwrap().clear();
                        }
                    });
                })
                .unwrap();
        }

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
                if ui.button(format!("#{}", channel.name.to_owned())).clicked() {
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

            if type_payload.msg_type == "new_message" {
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

        if type_payload.msg_type == "guild_data_update" {
            println!("guild_data_update");
            let res: Vec<Guild> = http
                .get(format!("{}/{}", INSTANCE_URL, "users/me/guilds"))
                .header("Authorization", format!("Bearer {}", account.token))
                .send()
                .unwrap()
                .json()
                .unwrap();

            *current_guild = Some(res.iter().find(|g| g.id == guild.id).unwrap().clone());

            self.guilds = res;

            ctx.request_repaint();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            let guild_name = current_guild.as_ref().unwrap().name.to_owned();

            if current_channel.is_none() {
                ui.heading(format!("{}: Select a channel", guild_name));
                return;
            }

            let current_channel = current_channel.as_ref().unwrap();

            ui.heading(format!(
                "{}: #{}",
                guild_name,
                current_channel.name.to_owned()
            ));

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
                    if num_rows < row_range.start {
                        ui.scroll_to_cursor(Some(Align::TOP));
                        return;
                    }
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

                http.post(format!(
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
        frame: &epi::Frame,
        storage: Option<&dyn epi::Storage>,
    ) {
        self.inputs.insert("username".to_owned(), "".to_owned());
        self.inputs.insert("password".to_owned(), "".to_owned());
        self.inputs.insert("chatbox".to_owned(), "".to_owned());
        self.inputs.insert("invite_code".to_owned(), "".to_owned());
        self.inputs.insert("guild_name".to_owned(), "".to_owned());
        self.inputs.insert("channel_name".to_owned(), "".to_owned());

        self.http = reqwest::blocking::Client::new();

        let stored_token = storage.unwrap().get_string("token");

        if stored_token.is_some() && stored_token.unwrap() != *"" {
            let token = storage.unwrap().get_string("token").unwrap();
            let user_req = self
                .http
                .get(format!("{}/users/me", INSTANCE_URL))
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .unwrap()
                .json::<User>();

            if user_req.is_err() {
                return;
            }

            let user = user_req.unwrap();

            self.account = Some(Account {
                id: user.id,
                username: user.username,
                token,
            });

            self.guilds = self
                .http
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

            self.socket_channel = Some(r);

            let frame = frame.0.clone();

            thread::spawn(move || loop {
                let socket_msg = sock.read_message().unwrap().into_text().unwrap();

                frame.lock().unwrap().repaint_signal.request_repaint();

                s.send(socket_msg).unwrap();
            });
        }
    }

    fn save(&mut self, storage: &mut dyn epi::Storage) {
        println!("Saving data...");
        if let Some(account) = &self.account {
            storage.set_string("token", account.token.to_owned());
        }
    }

    fn name(&self) -> &str {
        "rustcord"
    }
}
