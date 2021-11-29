mod client;
mod interface;
mod observer;
mod setup;

use gtk::{
    prelude::{ApplicationExt, ApplicationExtManual},
    Application,
};

fn main() {
    let app = Application::builder()
        .application_id("ar.uba.fi.rostovfc.mqtt")
        .build();

    app.connect_startup(|_app| {
        setup::load_css();
    });

    app.connect_activate(move |app| {
        setup::build_ui(app);
    });

    app.run();
}
