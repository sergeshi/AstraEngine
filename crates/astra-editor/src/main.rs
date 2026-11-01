use astra_core::{EngineConfig, ENGINE_NAME, ENGINE_VERSION};

fn main() {
    let config = EngineConfig::default();

    println!("{ENGINE_NAME} {ENGINE_VERSION}");
    println!("Standalone editor bootstrap");
    println!("Application: {}", config.application_name);
    println!("Target frame rate: {} FPS", config.target_fps);
    println!("Editor UI and GPU viewport are scheduled for Milestone 1 and 3.");
}
