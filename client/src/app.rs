use std::collections::HashMap;

use eframe::{
    egui::{self, RichText, ScrollArea, TextEdit, TextStyle},
    epaint::Color32,
    epi,
};

struct Account {
    name: String,
    discriminator: String,
    id: String,
}

struct Guild {
    name: String,
    id: String,
    channels: Vec<Channel>,
}

struct Channel {
    name: String,
    id: String,
    messages: Vec<Message>,
}

struct Message {
    content: String,
    author_id: String,
    author_name: String,
    author_discriminator: String,
    timestamp: String,
}

#[derive(Default)]
pub struct RustCord {
    inputs: HashMap<String, String>,
    current_guild: Option<Guild>,
    current_channel: Option<Channel>,
    account: Option<Account>,
}

impl epi::App for RustCord {
    fn update(&mut self, ctx: &egui::Context, _frame: &epi::Frame) {
        let Self {
            inputs,
            current_guild,
            current_channel,
            account,
        } = self;

        let account = self.account.as_ref().unwrap();

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("Servers", |ui| {
                    if ui.radio(true, "jayden's server").clicked() {};
                    if ui.radio(false, "server 2").clicked() {};
                });
            });
        });

        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.heading("Channels");

            if ui.button("#general").clicked() {}
            if ui.button("#general2").clicked() {}

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label("Logged in as ");
                    ui.label(
                        RichText::new(format!("{}#{}", account.name, account.discriminator))
                            .color(Color32::WHITE),
                    );
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

            ui.heading("#general");

            ui.add_space(4.0);

            let text_style = TextStyle::Body;
            let row_height = ui.text_style_height(&text_style);
            let num_rows = 2;
            ScrollArea::vertical().auto_shrink([false; 2]).show_rows(
                ui,
                row_height,
                num_rows,
                |ui, _row_range| {
                    ui.label(RichText::new("jayden#0000").color(Color32::WHITE));
                    ui.label("hello world");
                    ui.label(RichText::new("bruh#0000").color(Color32::WHITE));
                    ui.label("hello world");
                },
            );

            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                ui.add(
                    TextEdit::multiline(inputs.get_mut("chatbox").unwrap())
                        .desired_width(f32::INFINITY)
                        .desired_rows(1)
                        .hint_text(format!("Message #{}", current_channel.name)),
                );
            });
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

        // dummy data
        self.current_channel = Some(Channel {
            id: "123".to_owned(),
            name: "general".to_owned(),
            messages: vec![
                Message {
                    content: "hello world".to_owned(),
                    author_id: "123".to_owned(),
                    author_name: "jayden".to_owned(),
                    author_discriminator: "0000".to_owned(),
                    timestamp: "2020-01-01T00:00:00.000Z".to_owned(),
                },
                Message {
                    content: "what's up".to_owned(),
                    author_id: "123".to_owned(),
                    author_name: "jayden".to_owned(),
                    author_discriminator: "0000".to_owned(),
                    timestamp: "2020-01-01T00:00:00.000Z".to_owned(),
                },
            ],
        });

        self.account = Some(Account {
            name: "jayden".to_owned(),
            discriminator: "0000".to_owned(),
            id: "123".to_owned(),
        });
    }

    fn name(&self) -> &str {
        "RustCord"
    }
}
