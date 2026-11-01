use astra_core::EngineInfo;
use astra_runtime::GameRuntime;

fn main() {
    let runtime = GameRuntime::default();

    println!("{} {}", EngineInfo::CURRENT.name, EngineInfo::CURRENT.version);
    println!("Target platform: {}", EngineInfo::CURRENT.target_platform);
    println!("Editor bootstrap is running.");
    println!("Runtime state: {:?}", runtime.state());
    println!();
    println!("This is the first workspace milestone, not yet the graphical editor.");
}
