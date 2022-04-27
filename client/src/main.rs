fn main() {
    pretty_env_logger::init();
    let app = client::RustCord::default();
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(Box::new(app), native_options);
}
