use eframe::egui::{self, Color32, FontFamily, FontId, RichText, Vec2};
use nokiri::{
    model::Settings,
    platform::Vault,
    worker::{Action, Handle, Snapshot},
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Dashboard,
    Servers,
    Subscriptions,
    Settings,
}
pub struct Nokiri {
    worker: Handle,
    snapshot: Snapshot,
    page: Page,
    search: String,
    import_open: bool,
    subscription_mode: bool,
    input: String,
    name: String,
    renames: BTreeMap<String, String>,
    port: u16,
    initialized: bool,
    pending: bool,
    logo: egui::TextureHandle,
}
impl Nokiri {
    pub fn new(cc: &eframe::CreationContext<'_>, vault: Vault) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "Roboto".into(),
            egui::FontData::from_static(include_bytes!("../assets/fonts/Roboto.ttf")).into(),
        );
        fonts
            .families
            .get_mut(&FontFamily::Proportional)
            .unwrap()
            .insert(0, "Roboto".into());
        fonts.font_data.insert(
            "Symbols".into(),
            egui::FontData::from_static(include_bytes!("../assets/fonts/MaterialSymbols.ttf"))
                .into(),
        );
        fonts
            .families
            .insert(FontFamily::Name("Symbols".into()), vec!["Symbols".into()]);
        cc.egui_ctx.set_fonts(fonts);
        let rgba = image::load_from_memory(include_bytes!("../assets/icons/nokiri.png"))
            .unwrap()
            .into_rgba8();
        let logo = cc.egui_ctx.load_texture(
            "Nokiri mark",
            egui::ColorImage::from_rgba_unmultiplied(
                [rgba.width() as usize, rgba.height() as usize],
                rgba.as_raw(),
            ),
            Default::default(),
        );
        Self {
            worker: Handle::spawn(vault, cc.egui_ctx.clone()),
            snapshot: Snapshot::default(),
            page: Page::Dashboard,
            search: String::new(),
            import_open: false,
            subscription_mode: true,
            input: String::new(),
            name: String::new(),
            renames: BTreeMap::new(),
            port: 7890,
            initialized: false,
            pending: false,
            logo,
        }
    }
    fn send(&mut self, action: Action) {
        if !self.pending && !self.snapshot.busy && !self.snapshot.locked {
            self.pending = self.worker.tx.send(action).is_ok();
        }
    }
    fn accent(&self) -> Color32 {
        if self.snapshot.state.settings.dark {
            Color32::from_rgb(255, 180, 91)
        } else {
            Color32::from_rgb(133, 72, 0)
        }
    }
    fn surface(&self) -> Color32 {
        if self.snapshot.state.settings.dark {
            Color32::from_rgb(38, 35, 31)
        } else {
            Color32::from_rgb(245, 238, 230)
        }
    }
    fn muted(&self) -> Color32 {
        if self.snapshot.state.settings.dark {
            Color32::from_rgb(193, 183, 170)
        } else {
            Color32::from_rgb(104, 91, 75)
        }
    }
    fn card(&self) -> egui::Frame {
        egui::Frame::new()
            .fill(self.surface())
            .corner_radius(20)
            .inner_margin(24)
    }
    fn available(&self) -> bool {
        self.initialized && !self.snapshot.busy && !self.pending && !self.snapshot.locked
    }
    fn style(&self, ctx: &egui::Context) {
        let dark = self.snapshot.state.settings.dark;
        let mut style = (*ctx.style()).clone();
        style.visuals = if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        style.visuals.panel_fill = if dark {
            Color32::from_rgb(25, 23, 20)
        } else {
            Color32::from_rgb(255, 248, 240)
        };
        style.visuals.window_fill = self.surface();
        style.visuals.extreme_bg_color = if dark {
            Color32::from_rgb(20, 18, 16)
        } else {
            Color32::from_rgb(255, 253, 250)
        };
        style.visuals.selection.bg_fill = if dark {
            Color32::from_rgb(100, 65, 25)
        } else {
            Color32::from_rgb(255, 219, 181)
        };
        style.visuals.selection.stroke.color = self.accent();
        style.visuals.widgets.active.bg_fill = style.visuals.selection.bg_fill;
        style.visuals.widgets.active.fg_stroke.color = self.accent();
        style.visuals.widgets.hovered.fg_stroke.color = self.accent();
        style.visuals.widgets.inactive.corner_radius = 10.into();
        style.visuals.widgets.hovered.corner_radius = 10.into();
        style.visuals.widgets.active.corner_radius = 10.into();
        style.spacing.item_spacing = Vec2::new(12.0, 12.0);
        style.spacing.button_padding = Vec2::new(16.0, 10.0);
        style.spacing.interact_size.y = 38.0;
        style
            .text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(15.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(15.0));
        style
            .text_styles
            .insert(egui::TextStyle::Heading, FontId::proportional(26.0));
        style
            .text_styles
            .insert(egui::TextStyle::Small, FontId::proportional(12.0));
        ctx.set_style(style);
    }
    fn heading(&self, ui: &mut egui::Ui, title: &str, subtitle: &str) {
        ui.heading(title);
        ui.label(RichText::new(subtitle).color(self.muted()));
        ui.add_space(18.0);
    }
    fn primary(&self, ui: &mut egui::Ui, text: &str) -> egui::Response {
        ui.add(
            egui::Button::new(
                RichText::new(text).color(if self.snapshot.state.settings.dark {
                    Color32::from_rgb(67, 35, 0)
                } else {
                    Color32::WHITE
                }),
            )
            .fill(self.accent())
            .corner_radius(24),
        )
    }
    fn navigation(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("navigation")
            .exact_width(180.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(self.surface()).inner_margin(18))
            .show(ctx, |ui| {
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.image((self.logo.id(), Vec2::splat(34.0)));
                    ui.label(RichText::new("Nokiri").size(23.0).strong());
                });
                ui.add_space(34.0);
                for (page, symbol, label) in [
                    (Page::Dashboard, '\u{e88a}', "Dashboard"),
                    (Page::Servers, '\u{e1b1}', "Servers"),
                    (Page::Subscriptions, '\u{e2c7}', "Subscriptions"),
                    (Page::Settings, '\u{e8b8}', "Settings"),
                ] {
                    let active = self.page == page;
                    let response = egui::Frame::new()
                        .fill(if active {
                            ui.visuals().selection.bg_fill
                        } else {
                            Color32::TRANSPARENT
                        })
                        .corner_radius(24)
                        .inner_margin(egui::Margin::symmetric(12, 4))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(symbol.to_string())
                                        .font(FontId::new(21.0, FontFamily::Name("Symbols".into())))
                                        .color(if active { self.accent() } else { self.muted() }),
                                );
                                ui.selectable_label(active, label)
                            })
                            .inner
                        })
                        .inner;
                    if response.clicked() {
                        self.page = page;
                    }
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.label(
                        RichText::new("Your server. Your choice.")
                            .size(12.0)
                            .color(self.muted()),
                    );
                    ui.label(
                        RichText::new(format!("Nokiri {}", env!("CARGO_PKG_VERSION")))
                            .small()
                            .color(self.muted()),
                    );
                });
            });
    }
    fn dashboard(&mut self, ui: &mut egui::Ui) {
        self.heading(
            ui,
            "Your connection",
            "One selected server. Every proxied request.",
        );
        self.card().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                let color = if self.snapshot.running {
                    self.accent()
                } else {
                    self.muted()
                };
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(12.0), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), 5.0, color);
                ui.label(
                    RichText::new(if self.snapshot.running {
                        "Proxy running"
                    } else {
                        "Proxy stopped"
                    })
                    .color(color),
                );
            });
            ui.add_space(10.0);
            let selected = self.snapshot.state.selected_server();
            ui.label(
                RichText::new(selected.map_or(
                    if self.snapshot.state.selected.is_some() {
                        "Selected server is missing"
                    } else {
                        "Choose your server"
                    },
                    |s| &s.proxy.name,
                ))
                .size(30.0),
            );
            ui.label(
                RichText::new(
                    selected.map_or("Import a subscription or VLESS URL to get started.", |s| {
                        self.snapshot.state.source_name(&s.source)
                    }),
                )
                .color(self.muted()),
            );
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                ui.add_enabled_ui(self.available(), |ui| {
                    if self
                        .primary(
                            ui,
                            if self.snapshot.running {
                                "Stop proxy"
                            } else {
                                "Start proxy"
                            },
                        )
                        .clicked()
                    {
                        self.send(if self.snapshot.running {
                            Action::Stop
                        } else {
                            Action::Start
                        });
                    }
                    if ui.button("Choose server").clicked() {
                        self.page = Page::Servers;
                    }
                });
            });
        });
        ui.add_space(8.0);
        self.card().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new("Local proxy").size(19.0));
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("127.0.0.1:{}", self.snapshot.state.settings.port))
                        .size(23.0),
                );
                if ui.small_button("Copy URL").clicked() {
                    ui.ctx().copy_text(format!(
                        "http://127.0.0.1:{}",
                        self.snapshot.state.settings.port
                    ));
                }
            });
            ui.label(RichText::new("HTTP and SOCKS5 on the same port").color(self.muted()));
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("This device").strong());
                ui.label("to");
                ui.colored_label(self.accent(), "Your selected server");
                ui.label("to");
                ui.label("Internet");
            });
            ui.label(
                RichText::new("System proxy stays unchanged. TUN is off.")
                    .small()
                    .color(self.muted()),
            );
        });
        ui.add_space(8.0);
        self.card().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new("Check the connection").size(19.0));
            ui.label(
                RichText::new("Tests make an HTTPS request through your selected proxy.")
                    .color(self.muted()),
            );
            ui.horizontal(|ui| {
                ui.add_enabled_ui(self.available() && self.snapshot.running, |ui| {
                    if ui.button("Test HTTPS").clicked() {
                        self.send(Action::Connectivity(false));
                    }
                    if ui.button("Check exit IP").clicked() {
                        self.send(Action::Connectivity(true));
                    }
                });
                if let Some(ip) = &self.snapshot.exit_ip {
                    ui.label(RichText::new(ip).strong().color(self.accent()));
                }
            });
        });
    }
    fn servers(&mut self, ui: &mut egui::Ui) {
        self.heading(ui, "Servers", "Selection stays fixed until you change it.");
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("Search servers or sources")
                    .desired_width(260.0),
            );
            if ui.button("Import").clicked() {
                self.import_open = true;
                self.subscription_mode = false;
            }
            ui.add_enabled_ui(
                self.available() && !self.snapshot.state.servers.is_empty(),
                |ui| {
                    if ui.button("Test all").clicked() {
                        self.send(Action::Test(
                            self.snapshot
                                .state
                                .servers
                                .iter()
                                .map(|s| s.id.clone())
                                .collect(),
                        ));
                    }
                },
            );
        });
        ui.add_space(8.0);
        if self.snapshot.state.servers.is_empty() {
            self.empty(
                ui,
                "No servers yet",
                "Import a subscription or paste a VLESS URL.",
            );
            return;
        }
        let query = self.search.to_lowercase();
        let servers = self.snapshot.state.servers.clone();
        let mut matched = 0;
        for server in servers {
            let source = self.snapshot.state.source_name(&server.source).to_string();
            if !format!("{} {source}", server.proxy.name)
                .to_lowercase()
                .contains(&query)
            {
                continue;
            }
            matched += 1;
            let selected = self.snapshot.state.selected.as_ref() == Some(&server.id);
            egui::Frame::new()
                .fill(if selected {
                    ui.visuals().selection.bg_fill
                } else {
                    self.surface()
                })
                .corner_radius(16)
                .inner_margin(18)
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&server.proxy.name).size(18.0).strong());
                            ui.label(
                                RichText::new(format!(
                                    "{source}  /  VLESS {}{}  /  {}",
                                    server.proxy.network.to_uppercase(),
                                    if server.proxy.reality_opts.is_some() {
                                        " + REALITY"
                                    } else if server.proxy.tls {
                                        " + TLS"
                                    } else {
                                        ""
                                    },
                                    &server.id[..6]
                                ))
                                .small()
                                .color(self.muted()),
                            );
                        });
                    });
                    if let Some(check) = self.snapshot.checks.get(&server.id) {
                        let status = if check.testing {
                            "Testing...".into()
                        } else if let Some(ms) = check.latency {
                            format!("Available  /  {ms} ms")
                        } else {
                            "Unavailable".into()
                        };
                        ui.colored_label(
                            if check.latency.is_some() {
                                self.accent()
                            } else {
                                self.muted()
                            },
                            status,
                        );
                        if let Some(last) = check.last_success {
                            ui.label(
                                RichText::new(format!("Last successful test: {}", age(last)))
                                    .small()
                                    .color(self.muted()),
                            );
                        }
                        if let Some(error) = &check.error {
                            ui.label(RichText::new(error).small());
                        }
                    } else {
                        ui.label(RichText::new("Not tested").small().color(self.muted()));
                    }
                    ui.horizontal(|ui| {
                        ui.add_enabled_ui(self.available(), |ui| {
                            if ui
                                .selectable_label(
                                    selected,
                                    if selected {
                                        "Selected"
                                    } else {
                                        "Select server"
                                    },
                                )
                                .clicked()
                                && !selected
                            {
                                self.send(Action::Select(server.id.clone()));
                            }
                            if ui.small_button("Test").clicked() {
                                self.send(Action::Test(vec![server.id.clone()]));
                            }
                            if server.source == "manual" && ui.small_button("Remove").clicked() {
                                self.send(Action::RemoveServer(server.id.clone()));
                            }
                        });
                    });
                });
        }
        if matched == 0 {
            self.empty(
                ui,
                "No matching servers",
                "Try another server or subscription name.",
            );
        }
    }
    fn subscriptions(&mut self, ui: &mut egui::Ui) {
        self.heading(
            ui,
            "Subscriptions",
            "Updates are manual. Credentials stay private.",
        );
        ui.horizontal(|ui| {
            if self.primary(ui, "Add subscription").clicked() {
                self.import_open = true;
                self.subscription_mode = true;
            }
            if ui.button("Import VLESS / YAML").clicked() {
                self.import_open = true;
                self.subscription_mode = false;
            }
        });
        ui.add_space(8.0);
        if self.snapshot.state.subscriptions.is_empty() {
            self.empty(
                ui,
                "Keep your servers together",
                "Add an HTTPS subscription from your provider.",
            );
        }
        for sub in self.snapshot.state.subscriptions.clone() {
            self.card().show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.label(RichText::new(&sub.name).size(20.0));
                let count = self
                    .snapshot
                    .state
                    .servers
                    .iter()
                    .filter(|s| s.source == sub.id)
                    .count();
                ui.label(
                    RichText::new(format!("{count} servers  /  URL hidden")).color(self.muted()),
                );
                ui.label(&sub.status);
                if let Some(updated) = sub.updated {
                    ui.label(
                        RichText::new(format!("Last updated {}", age(updated)))
                            .small()
                            .color(self.muted()),
                    );
                }
                ui.add_enabled_ui(self.available(), |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Update").clicked() {
                            self.send(Action::Update(sub.id.clone()));
                        }
                        if ui.button("Rename").clicked() {
                            self.renames.insert(sub.id.clone(), sub.name.clone());
                        }
                        if ui.button("Remove").clicked() {
                            self.send(Action::RemoveSubscription(sub.id.clone()));
                        }
                    });
                    if let Some(name) = self.renames.get_mut(&sub.id) {
                        let mut save = false;
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(name);
                            save = ui.button("Save name").clicked();
                        });
                        if save {
                            let name = self.renames.remove(&sub.id).unwrap();
                            self.send(Action::Rename(sub.id.clone(), name));
                        }
                    }
                });
            });
        }
    }
    fn settings(&mut self, ui: &mut egui::Ui) {
        self.heading(ui, "Settings", "Small by design. Explicit by default.");
        self.card().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new("Local endpoint").size(20.0));
            ui.horizontal(|ui| {
                ui.label("Mixed proxy port");
                ui.add(egui::DragValue::new(&mut self.port).range(1..=65535));
            });
            ui.label(
                RichText::new("Bound to 127.0.0.1. Changing the port restarts a running proxy.")
                    .small()
                    .color(self.muted()),
            );
            let mut dark = self.snapshot.state.settings.dark;
            let mut start = self.snapshot.state.settings.start_on_launch;
            ui.checkbox(&mut dark, "Dark theme");
            ui.checkbox(&mut start, "Start the selected proxy when Nokiri opens");
            ui.add_enabled_ui(self.available(), |ui| {
                if self.primary(ui, "Save settings").clicked() {
                    self.send(Action::Settings(Settings {
                        port: self.port,
                        dark,
                        start_on_launch: start,
                    }));
                }
            });
            // Checkbox edits survive until the Save settings action reaches the worker.
            self.snapshot.state.settings.dark = dark;
            self.snapshot.state.settings.start_on_launch = start;
        });
        ui.add_space(8.0);
        self.card().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new("Proxy core").size(20.0));
            ui.label(format!(
                "Mihomo {} / SHA-256 verified before every launch",
                nokiri::engine::manifest().version
            ));
            if !self.snapshot.core_version.is_empty() {
                ui.label(RichText::new(&self.snapshot.core_version).small());
            }
            ui.label(
                RichText::new("Managed by Nokiri. No administrator access required.")
                    .small()
                    .color(self.muted()),
            );
            ui.add_enabled_ui(self.available() && self.snapshot.running, |ui| {
                if ui.button("Restart core").clicked() {
                    self.send(Action::Restart);
                }
            });
        });
        ui.add_space(8.0);
        ui.collapsing("Diagnostics", |ui| {
            ui.label(format!(
                "Core process: {}",
                self.snapshot
                    .pid
                    .map_or("Stopped".into(), |p| p.to_string())
            ));
            ui.label(
                "Captured core output is kept in memory. Known server credentials are redacted.",
            );
            if self.snapshot.diagnostics.is_empty() {
                ui.label("No core diagnostics.");
            } else {
                ui.label(
                    RichText::new(&self.snapshot.diagnostics)
                        .monospace()
                        .size(12.0),
                );
            }
        });
    }
    fn empty(&self, ui: &mut egui::Ui, title: &str, body: &str) {
        self.card().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new(title).size(21.0));
            ui.label(RichText::new(body).color(self.muted()));
        });
    }
    fn import_dialog(&mut self, ctx: &egui::Context) {
        if !self.import_open {
            return;
        }
        let mut open = true;
        egui::Window::new(if self.subscription_mode { "Add subscription" } else { "Import servers" }).open(&mut open).collapsible(false).resizable(false).default_width(470.0).anchor(egui::Align2::CENTER_CENTER,Vec2::ZERO).show(ctx,|ui| {
            if self.subscription_mode {
                ui.label("Subscription name"); ui.text_edit_singleline(&mut self.name);
                ui.label("HTTPS subscription URL"); ui.add(egui::TextEdit::singleline(&mut self.input).password(true).hint_text("https://provider.example/sub/...").desired_width(f32::INFINITY));
                ui.label(RichText::new("Fetched directly from the provider. Redirects are refused.").small().color(self.muted()));
            } else {
                ui.label("Paste VLESS URLs, Base64 URLs, or Clash YAML");
                ui.add(egui::TextEdit::multiline(&mut self.input).password(true).desired_rows(7).desired_width(f32::INFINITY));
                ui.label(RichText::new("VLESS over TCP, WebSocket or gRPC. TLS and REALITY supported. Other protocols and transport extensions are rejected.").small().color(self.muted()));
            }
            ui.label(RichText::new("Your catalog is encrypted with a key in the system credential store.").small().color(self.muted()));
            ui.add_enabled_ui(self.available() && !self.input.trim().is_empty(),|ui| {
                if self.primary(ui,"Import").clicked() {
                    let action=if self.subscription_mode { Action::AddSubscription(self.name.trim().into(),self.input.trim().into()) } else { Action::Import(self.input.trim().into()) };
                    self.send(action); self.input.clear(); self.name.clear(); self.import_open=false;
                }
            });
        });
        if !open {
            self.import_open = false;
            self.input.clear();
            self.name.clear();
        }
    }
}
impl eframe::App for Nokiri {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(snapshot) = self.worker.rx.try_recv() {
            if !self.initialized {
                self.port = snapshot.state.settings.port;
                self.initialized = true;
            }
            if !snapshot.busy {
                self.pending = false;
            }
            self.snapshot = snapshot;
        }
        self.style(ctx);
        self.navigation(ctx);
        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(self.surface())
                    .inner_margin(egui::Margin::symmetric(22, 12)),
            )
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if self.snapshot.busy || self.pending {
                        ui.spinner();
                    }
                    ui.label(
                        RichText::new(if !self.initialized {
                            "Loading encrypted catalog..."
                        } else if self.snapshot.message.is_empty() {
                            "Ready. No server is selected automatically."
                        } else {
                            &self.snapshot.message
                        })
                        .color(if self.snapshot.error {
                            if self.snapshot.state.settings.dark {
                                Color32::from_rgb(255, 180, 171)
                            } else {
                                Color32::from_rgb(160, 28, 20)
                            }
                        } else {
                            self.muted()
                        }),
                    );
                });
            });
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(ctx.style().visuals.panel_fill)
                    .inner_margin(28),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| match self.page {
                        Page::Dashboard => self.dashboard(ui),
                        Page::Servers => self.servers(ui),
                        Page::Subscriptions => self.subscriptions(ui),
                        Page::Settings => self.settings(ui),
                    });
            });
        self.import_dialog(ctx);
    }
}
fn age(timestamp: u64) -> String {
    let seconds = nokiri::model::now().saturating_sub(timestamp);
    if seconds < 60 {
        "just now".into()
    } else if seconds < 3600 {
        format!("{} min ago", seconds / 60)
    } else if seconds < 86400 {
        format!("{} h ago", seconds / 3600)
    } else {
        format!("{} days ago", seconds / 86400)
    }
}
