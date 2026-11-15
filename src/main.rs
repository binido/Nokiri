mod ui;
use nokiri::platform::{self, SystemSecrets, Vault};

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("nokiri=info")
        .with_target(false)
        .init();
    let dir = platform::data_dir().map_err(|e| eframe::Error::AppCreation(e.into()))?;
    let _lock = platform::instance_lock(&dir).map_err(|e| eframe::Error::AppCreation(e.into()))?;
    let icon = image::load_from_memory(include_bytes!("../assets/icons/nokiri.png"))
        .unwrap()
        .into_rgba8();
    let (width, height) = icon.dimensions();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Nokiri")
            .with_inner_size([980.0, 780.0])
            .with_min_inner_size([720.0, 560.0])
            .with_app_id("io.github.binido.nokiri")
            .with_icon(eframe::egui::IconData {
                rgba: icon.into_raw(),
                width,
                height,
            }),
        ..Default::default()
    };
    eframe::run_native(
        "Nokiri",
        options,
        Box::new(move |cc| {
            Ok(Box::new(ui::Nokiri::new(
                cc,
                Vault::new(dir.join("catalog.enc"), Box::new(SystemSecrets)),
            )))
        }),
    )
}
