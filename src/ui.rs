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
    import_pending: bool,
    import_error: Option<String>,
    removal: Option<(Action, String)>,
    subscription_mode: bool,
    input: String,
    name: String,
    renames: BTreeMap<String, String>,
    draft: Settings,
    show_input: bool,
    allow_unverified_tls: bool,
    initialized: bool,
    pending: bool,
    logo: egui::TextureHandle,
}
impl Nokiri {
    pub fn new(cc: &eframe::CreationContext<'_>, vault: Vault) -> Self {
        Self::with_context(cc.egui_ctx.clone(), vault)
    }
    fn with_context(ctx: egui::Context, vault: Vault) -> Self {
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
        ctx.set_fonts(fonts);
        let rgba = image::load_from_memory(include_bytes!("../assets/icons/nokiri.png"))
            .unwrap()
            .into_rgba8();
        let logo = ctx.load_texture(
            "Nokiri mark",
            egui::ColorImage::from_rgba_unmultiplied(
                [rgba.width() as usize, rgba.height() as usize],
                rgba.as_raw(),
            ),
            Default::default(),
        );
        Self {
            worker: Handle::spawn(vault, ctx),
            snapshot: Snapshot::default(),
            page: Page::Dashboard,
            search: String::new(),
            import_open: false,
            import_pending: false,
            import_error: None,
            removal: None,
            subscription_mode: true,
            input: String::new(),
            name: String::new(),
            renames: BTreeMap::new(),
            draft: Settings::default(),
            show_input: false,
            allow_unverified_tls: false,
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
        if self.draft.dark {
            Color32::from_rgb(255, 180, 91)
        } else {
            Color32::from_rgb(133, 72, 0)
        }
    }
    fn surface(&self) -> Color32 {
        if self.draft.dark {
            Color32::from_rgb(38, 35, 31)
        } else {
            Color32::from_rgb(245, 238, 230)
        }
    }
    fn muted(&self) -> Color32 {
        if self.draft.dark {
            Color32::from_rgb(193, 183, 170)
        } else {
            Color32::from_rgb(104, 91, 75)
        }
    }
    fn card(&self) -> egui::Frame {
        egui::Frame::new()
            .fill(self.surface())
            .corner_radius(20)
            .inner_margin(18)
    }
    fn available(&self) -> bool {
        self.initialized && !self.snapshot.busy && !self.pending && !self.snapshot.locked
    }
    fn style(&self, ctx: &egui::Context) {
        let dark = self.draft.dark;
        let mut style = (*ctx.style()).clone();
        style.visuals = if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        style.visuals.override_text_color = Some(if dark {
            Color32::from_rgb(240, 225, 208)
        } else {
            Color32::from_rgb(39, 27, 15)
        });
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
        style.wrap_mode = Some(egui::TextWrapMode::Wrap);
        style.spacing.item_spacing = Vec2::new(12.0, 8.0);
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
        ui.add_space(12.0);
    }
    fn primary(&self, ui: &mut egui::Ui, text: &str) -> egui::Response {
        ui.scope(|ui| {
            let accent = self.accent();
            let visuals = ui.visuals_mut();
            visuals.widgets.inactive.weak_bg_fill = accent;
            visuals.widgets.hovered.weak_bg_fill = if self.draft.dark {
                Color32::from_rgb(255, 199, 133)
            } else {
                Color32::from_rgb(112, 59, 0)
            };
            visuals.widgets.active.weak_bg_fill = if self.draft.dark {
                Color32::from_rgb(235, 154, 61)
            } else {
                Color32::from_rgb(92, 48, 0)
            };
            for widget in [
                &mut visuals.widgets.inactive,
                &mut visuals.widgets.hovered,
                &mut visuals.widgets.active,
            ] {
                widget.bg_stroke = egui::Stroke::NONE;
            }
            ui.add(
                egui::Button::new(RichText::new(text).color(if self.draft.dark {
                    Color32::from_rgb(67, 35, 0)
                } else {
                    Color32::WHITE
                }))
                .corner_radius(24),
            )
        })
        .inner
    }
    fn navigation(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("navigation")
            .exact_width(216.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(self.surface()).inner_margin(18))
            .show(ctx, |ui| {
                ui.add_space(14.0);
                ui.horizontal_wrapped(|ui| {
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
                    let response = ui.add_sized(
                        [ui.available_width(), 48.0],
                        egui::Button::selectable(
                            active,
                            (
                                RichText::new(symbol.to_string())
                                    .font(FontId::new(21.0, FontFamily::Name("Symbols".into())))
                                    .color(if active { self.accent() } else { self.muted() }),
                                RichText::new(label),
                                egui::Atom::grow(),
                            ),
                        )
                        .corner_radius(24),
                    );
                    response.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::WidgetType::SelectableLabel,
                            ui.is_enabled(),
                            active,
                            label,
                        )
                    });
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
            ui.horizontal_wrapped(|ui| {
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
                .size(28.0),
            );
            ui.label(
                RichText::new(
                    selected.map_or("Import a subscription or proxy URL to get started.", |s| {
                        self.snapshot.state.source_name(&s.source)
                    }),
                )
                .color(self.muted()),
            );
            ui.add_space(14.0);
            ui.horizontal_wrapped(|ui| {
                ui.add_enabled_ui(
                    self.available()
                        && (self.snapshot.running
                            || self.snapshot.state.selected_server().is_some()),
                    |ui| {
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
                    },
                );
                if ui.button("Choose server").clicked() {
                    self.page = Page::Servers;
                }
            });
        });
        ui.add_space(8.0);
        self.card().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new("Local proxy").size(19.0));
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(format!("127.0.0.1:{}", self.snapshot.state.settings.port))
                        .size(23.0),
                );
                if ui.button("Copy URL").clicked() {
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
            ui.horizontal_wrapped(|ui| {
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
        ui.horizontal_wrapped(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("Search servers or sources")
                    .desired_width((ui.available_width() - 200.0).clamp(140.0, 260.0)),
            );
            if ui.button("Import").clicked() {
                self.open_import(false);
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
                "Import a subscription or paste a proxy URL.",
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

                    ui.label(RichText::new(&server.proxy.name).size(18.0).strong());
                    ui.label(
                        RichText::new(format!(
                            "{source}  /  {}  /  {}",
                            server.proxy.description(),
                            &server.id[..6]
                        ))
                        .small()
                        .color(self.muted()),
                    );
                    if server.proxy.unverified_tls() {
                        ui.colored_label(
                            self.error_color(),
                            "Server TLS verification is disabled for this profile",
                        );
                    }
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
                    ui.horizontal_wrapped(|ui| {
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
                            if ui.button("Test").clicked() {
                                self.send(Action::Test(vec![server.id.clone()]));
                            }
                            if server.source == "manual" && ui.button("Remove").clicked() {
                                self.removal = Some((
                                    Action::RemoveServer(server.id.clone()),
                                    server.proxy.name.clone(),
                                ));
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
        ui.horizontal_wrapped(|ui| {
            if self.primary(ui, "Add subscription").clicked() {
                self.open_import(true);
            }
            if ui.button("Import proxy / YAML").clicked() {
                self.open_import(false);
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
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Update").clicked() {
                            self.send(Action::Update(sub.id.clone()));
                        }
                        if ui.button("Rename").clicked() {
                            self.renames.insert(sub.id.clone(), sub.name.clone());
                        }
                        if ui.button("Remove").clicked() {
                            self.removal = Some((
                                Action::RemoveSubscription(sub.id.clone()),
                                sub.name.clone(),
                            ));
                        }
                    });
                    if let Some(name) = self.renames.get_mut(&sub.id) {
                        let mut save = false;
                        ui.horizontal_wrapped(|ui| {
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
            ui.horizontal_wrapped(|ui| {
                let label = ui.label("Mixed proxy port");
                ui.add(egui::DragValue::new(&mut self.draft.port).range(1..=65535))
                    .labelled_by(label.id);
            });
            ui.label(
                RichText::new("Bound to 127.0.0.1. Changing the port restarts a running proxy.")
                    .small()
                    .color(self.muted()),
            );
            let mut dark = self.draft.dark;
            let mut start = self.draft.start_on_launch;
            ui.checkbox(&mut dark, "Dark theme");
            ui.checkbox(&mut start, "Start the selected proxy when Nokiri opens");
            ui.add_enabled_ui(self.available(), |ui| {
                if self.primary(ui, "Save settings").clicked() {
                    self.send(Action::Settings(Settings {
                        port: self.draft.port,
                        dark,
                        start_on_launch: start,
                    }));
                }
            });
            self.draft.dark = dark;
            self.draft.start_on_launch = start;
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
    fn open_import(&mut self, subscription: bool) {
        self.subscription_mode = subscription;
        self.import_open = true;
        self.import_error = None;
    }
    fn close_import(&mut self) {
        if self.import_error.is_some() {
            self.snapshot.error = false;
            self.snapshot.message.clear();
        }
        self.import_open = false;
        self.input.clear();
        self.name.clear();
        self.show_input = false;
        self.allow_unverified_tls = false;
        self.import_error = None;
    }
    fn import_dialog(&mut self, ctx: &egui::Context) {
        if !self.import_open {
            return;
        }
        let mut cancel = false;
        let response = egui::Modal::new(egui::Id::new("import_servers"))
            .frame(self.card())
            .show(ctx, |ui| {
                ui.set_width(470.0_f32.min(ctx.content_rect().width() - 72.0));
                ui.heading(if self.subscription_mode { "Add subscription" } else { "Import servers" });
                egui::ScrollArea::vertical().max_height((ctx.content_rect().height()-220.0).max(160.0)).show(ui, |ui| {
                    ui.add_enabled_ui(!self.import_pending, |ui| {
                        if self.subscription_mode {
                            let label=ui.label("Subscription name");
                            ui.add(egui::TextEdit::singleline(&mut self.name).hint_text("Personal servers").desired_width(f32::INFINITY)).labelled_by(label.id);
                            let label=ui.label("HTTPS subscription URL");
                            ui.add(egui::TextEdit::singleline(&mut self.input).password(!self.show_input).hint_text("https://provider.example/sub/...").desired_width(f32::INFINITY)).labelled_by(label.id);
                            ui.label(RichText::new("Fetched directly from the provider. Redirects are refused.").small().color(self.muted()));
                        } else {
                            let label=ui.label("Paste proxy URLs, Base64 subscriptions, or Clash YAML");
                            egui::ScrollArea::vertical().id_salt("import_text").max_height(180.0).show(ui,|ui| {
                                ui.add(egui::TextEdit::multiline(&mut self.input).password(!self.show_input).desired_rows(6).desired_width(f32::INFINITY)).labelled_by(label.id);
                            });
                            ui.label(RichText::new("VLESS, VMess, Trojan, Shadowsocks, Hysteria 1/2, HTTP and SOCKS5. Unsupported extensions are reported explicitly.").small().color(self.muted()));
                        }
                        ui.checkbox(&mut self.show_input,"Show contents");
                        ui.checkbox(&mut self.allow_unverified_tls,"Allow profiles without verified server TLS");
                        if self.allow_unverified_tls { ui.colored_label(self.error_color(),"Unverified TLS cannot establish the proxy server's identity. Use only when you trust the source; a certificate pin is safer."); }
                    });
                    ui.label(RichText::new("Your catalog is encrypted with a key in the system credential store.").small().color(self.muted()));
                    if let Some(error)=&self.import_error { ui.colored_label(self.error_color(), error); }
                });
                ui.horizontal_wrapped(|ui| {
                    ui.add_enabled_ui(self.available() && !self.input.trim().is_empty() && (!self.subscription_mode || !self.name.trim().is_empty()),|ui| {
                        if self.primary(ui,"Import").clicked() {
                            let action=if self.subscription_mode { Action::AddSubscription(self.name.trim().into(),self.input.trim().into(),self.allow_unverified_tls) } else { Action::Import(self.input.trim().into(),self.allow_unverified_tls) };
                            self.send(action);
                            self.import_pending=self.pending;
                        }
                    });
                    if self.import_pending { ui.spinner(); ui.label("Importing..."); }
                    else if ui.button("Cancel").clicked() { cancel=true; }
                });
            });
        if !self.import_pending && (cancel || response.should_close()) {
            self.close_import();
        }
    }
    fn error_color(&self) -> Color32 {
        if self.draft.dark {
            Color32::from_rgb(255, 180, 171)
        } else {
            Color32::from_rgb(160, 28, 20)
        }
    }
    fn removal_dialog(&mut self, ctx: &egui::Context) {
        let Some((_, name)) = &self.removal else {
            return;
        };
        let name = name.clone();
        let mut remove = false;
        let mut cancel = false;
        let response=egui::Modal::new(egui::Id::new("confirm_removal")).frame(self.card()).show(ctx,|ui| {
            ui.set_width(400.0_f32.min(ctx.content_rect().width()-72.0));
            ui.heading("Remove from Nokiri?");
            ui.label(&name);
            ui.label("This removes the saved entry and its servers. Removing the selected server stops the proxy. You will need the original URL to import it again.");
            ui.horizontal_wrapped(|ui| {
                remove=self.primary(ui,"Remove").clicked();
                cancel=ui.button("Cancel").clicked();
            });
        });
        if remove {
            if let Some((action, _)) = self.removal.take() {
                self.send(action);
            }
        } else if cancel || response.should_close() {
            self.removal = None;
        }
    }
}
impl Nokiri {
    fn receive_snapshot(&mut self, snapshot: Snapshot) {
        if !self.initialized || self.draft == self.snapshot.state.settings {
            self.draft = snapshot.state.settings.clone();
        }
        self.initialized = true;
        if !snapshot.busy {
            self.pending = false;
        }
        if self.import_pending && !snapshot.busy {
            self.import_pending = false;
            self.import_error = snapshot.error.then(|| snapshot.message.clone());
            if !snapshot.error {
                self.close_import();
            }
        }
        self.snapshot = snapshot;
    }
    fn render(&mut self, ctx: &egui::Context) {
        while let Ok(snapshot) = self.worker.rx.try_recv() {
            self.receive_snapshot(snapshot);
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
                            self.error_color()
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
        self.removal_dialog(ctx);
    }
}
impl eframe::App for Nokiri {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.render(ctx);
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

#[cfg(test)]
mod tests {
    use super::*;
    use nokiri::platform::SecretStore;
    struct NoSecrets;
    impl SecretStore for NoSecrets {
        fn key(&self, _: bool) -> anyhow::Result<Vec<u8>> {
            anyhow::bail!("UI rendering must not read credentials")
        }
    }
    pub(super) fn app() -> (tempfile::TempDir, egui::Context, Nokiri) {
        let dir = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let mut app = Nokiri::with_context(
            ctx.clone(),
            Vault::new(dir.path().join("catalog.enc"), Box::new(NoSecrets)),
        );
        let snapshot = app
            .worker
            .rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        app.receive_snapshot(snapshot);
        (dir, ctx, app)
    }
    pub(super) fn frame(
        app: &mut Nokiri,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(980.0, 780.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| app.render(ctx),
        )
    }
    #[test]
    fn sidebar_background_covers_its_reserved_width() {
        let (_dir, ctx, mut app) = app();
        let output = frame(&mut app, &ctx, vec![]);
        let sidebar = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Rect(rect)
                    if rect.rect.min == egui::Pos2::ZERO && rect.fill == app.surface() =>
                {
                    Some((rect.rect, shape.clip_rect))
                }
                _ => None,
            })
            .expect("sidebar surface");
        assert!(
            sidebar.0.right() <= sidebar.1.right() + 1.0,
            "Sidebar reserves {} px but paints only {} px",
            sidebar.0.right(),
            sidebar.1.right()
        );
    }
    #[test]
    fn navigation_icon_is_part_of_the_click_target() {
        let (_dir, ctx, mut app) = app();
        let output = frame(&mut app, &ctx, vec![]);
        let label = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text.ends_with("Subscriptions") => {
                    Some(egui::Rect::from_min_size(text.pos, text.galley.size()))
                }
                _ => None,
            })
            .expect("subscription navigation label");
        let pos = egui::pos2(40.0, label.center().y);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(pos)]);
        for pressed in [true, false] {
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
        }
        assert!(
            app.page == Page::Subscriptions,
            "Clicking the icon must select its page"
        );
    }
}

#[cfg(test)]
mod layout_tests {
    use super::tests::{app, frame};
    use super::*;
    #[test]
    fn all_pages_keep_surfaces_inside_the_viewport_with_long_names() {
        let (_dir, ctx, mut app) = app();
        frame(&mut app, &ctx, vec![]);
        app.initialized = true;
        let mut proxy = nokiri::subscription::parse_vless(
            "vless://00000000-0000-4000-8000-000000000001@127.0.0.1:12345#Fixture",
        )
        .unwrap();
        proxy.name = "LongServerName".repeat(18);
        app.snapshot.state.replace_source("fixture", vec![proxy]);
        app.snapshot.state.selected = Some(app.snapshot.state.servers[0].id.clone());
        app.snapshot
            .state
            .subscriptions
            .push(nokiri::model::Subscription {
                id: "fixture".into(),
                name: "LongSubscriptionName".repeat(6),
                url: "https://example.invalid/fixture".into(),
                updated: None,
                status: "Not updated".into(),
                allow_unverified_tls: false,
            });
        for dark in [true, false] {
            app.draft.dark = dark;
            for size in [Vec2::new(720.0, 560.0), Vec2::new(980.0, 780.0)] {
                for page in [
                    Page::Dashboard,
                    Page::Servers,
                    Page::Subscriptions,
                    Page::Settings,
                ] {
                    app.page = page;
                    let output = ctx.run(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                            ..Default::default()
                        },
                        |ctx| app.render(ctx),
                    );
                    for shape in output.shapes {
                        if let egui::Shape::Text(text) = &shape.shape
                            && (text.galley.job.text.contains("LongServerName")
                                || text.galley.job.text.contains("LongSubscriptionName"))
                        {
                            assert!(
                                text.pos.x + text.galley.size().x <= shape.clip_rect.right() + 1.0,
                                "Long names must wrap inside their card"
                            );
                        }
                        if let egui::Shape::Rect(rect) = shape.shape
                            && (rect.fill == app.surface()
                                || rect.fill == ctx.style().visuals.selection.bg_fill)
                        {
                            assert!(
                                rect.rect.right() <= shape.clip_rect.right() + 1.0,
                                "Surface overflows its painted area: {:?} / {:?}",
                                rect.rect,
                                shape.clip_rect
                            );
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod form_tests {
    use super::tests::{app, frame};
    use super::*;
    #[test]
    fn settings_draft_survives_status_updates_without_mutating_saved_settings() {
        let (_dir, _, mut app) = app();
        let saved = app.snapshot.state.settings.clone();
        app.draft.port = 8888;
        app.draft.dark = !saved.dark;
        let update = Snapshot {
            message: "Connection checked".into(),
            ..app.snapshot.clone()
        };
        app.receive_snapshot(update);
        assert!(app.snapshot.state.settings == saved);
        assert_eq!(app.draft.port, 8888);
        assert_ne!(app.draft.dark, saved.dark);
    }
    #[test]
    fn failed_import_keeps_the_dialog_and_input_for_correction() {
        let (_dir, ctx, mut app) = app();
        app.open_import(false);
        app.input = "invalid input".into();
        app.import_pending = true;
        app.receive_snapshot(Snapshot {
            error: true,
            message: "Expected a VLESS URL".into(),
            ..Default::default()
        });
        assert!(app.import_open);
        assert_eq!(app.input, "invalid input");
        assert_eq!(app.import_error.as_deref(), Some("Expected a VLESS URL"));
        let output = frame(&mut app, &ctx, vec![]);
        assert!(output.shapes.iter().any(|shape|matches!(&shape.shape,egui::Shape::Text(text) if text.galley.job.text=="Expected a VLESS URL")));
    }
}

#[cfg(test)]
mod removal_tests {
    use super::tests::{app, frame};
    use super::*;
    #[test]
    fn canceling_removal_does_not_send_a_delete_action() {
        let (_dir, ctx, mut app) = app();
        app.removal = Some((
            Action::RemoveSubscription("fixture".into()),
            "Fixture".into(),
        ));
        frame(&mut app, &ctx, vec![]);
        let output = frame(&mut app, &ctx, vec![]);
        assert!(!app.pending);
        let pos = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == "Cancel" => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
            .expect("removal cancellation button");
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(pos)]);
        for pressed in [true, false] {
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
        }
        assert!(app.removal.is_none());
        assert!(!app.pending, "Cancel must never queue a deletion");
    }
}
