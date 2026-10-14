//! The supplied Airlock vector logo, rasterized once and embedded in the binary.

pub fn logo() -> anyhow::Result<eframe::egui::IconData> {
    let image = image::load_from_memory(include_bytes!("../assets/logo.png"))?.into_rgba8();
    Ok(eframe::egui::IconData {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    })
}
