use std::env;

use server::init;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.get(1).is_none() {
        println!("Error: Debe especificar la ruta al archivo de configuración");
        return;
    }
    init(&args[1]);
}
