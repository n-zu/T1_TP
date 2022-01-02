use std::env;

use server::init;

fn get_config_path(default_path: Option<String>) -> String {
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        return String::from(&args[1]);
    }
    if let Some(path) = default_path {
        return path;
    }
    panic!("Error: Debe especificar la ruta al archivo de configuración")
}
fn main() {
    let config_path: String = get_config_path(Some("./config.txt".to_string()));
    init(&config_path);
}
