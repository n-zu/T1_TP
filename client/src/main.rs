mod client;
mod client_packets;
mod interface;
mod observer;
use crate::interface::Controller;
use gtk::Builder;
use gtk::{
    prelude::{ApplicationExt, ApplicationExtManual, BuilderExtManual, GtkWindowExt, WidgetExt},
    Application, Window,
};

fn main() {
    let app = Application::builder()
        .application_id("ar.uba.fi.rostovfc.mqtt")
        .build();

    let glade_src = include_str!("mqtt.glade");
    app.connect_activate(move |app| {
        // We create the main window.
        let builder = Builder::from_string(glade_src);
        let win: Window = builder.object("main_window").unwrap();
        win.set_application(Some(app));
        win.set_default_width(320);
        win.set_default_height(200);
        win.set_title("MQTT Client");

        win.show_all();

        Controller::new(builder);
    });

    app.run();
}
