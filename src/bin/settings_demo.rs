use counterpoint::settings::*;

fn main() {
    // Load settings from the default location
    let project_settings = parse_settings(None).unwrap();
    println!("Loaded settings: {:?}", project_settings);

    // Attempt to load from an invalid path (expected to fail)
    let is_err = parse_settings(Some("")).is_err();
    println!("Error on invalid path: {:?}", is_err);

    // Attempt to load from a custom path
    // $ cargo run --bin settings_demo -- --settings=settings/dev.toml
    let cli = Cli::parse();
    let project_settings = parse_settings(cli.settings.as_deref()).unwrap();
    println!("Loaded settings: {:?}", project_settings);
}