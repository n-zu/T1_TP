mod client;
mod client_packets;
mod interface;
mod observer;
use crate::interface::Controller;

use gtk::prelude::CssProviderExt;
use gtk::Builder;
use gtk::{
    gdk,
    prelude::{ApplicationExt, ApplicationExtManual, BuilderExtManual, GtkWindowExt, WidgetExt},
    Application, Window,
};

fn main() {
    let app = Application::builder()
        .application_id("ar.uba.fi.rostovfc.mqtt")
        .build();

    app.connect_startup(|_app| {
        // Load CSS
        let provider = gtk::CssProvider::new();
        let style = include_bytes!("resources/mqtt.css");
        provider.load_from_data(style).expect("Failed to load CSS");
        gtk::StyleContext::add_provider_for_screen(
            &gdk::Screen::default().expect("Error initializing gtk css provider."),
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    });

    app.connect_activate(move |app| {
        // Load the window UI
        let glade_src = include_str!("resources/mqtt.glade");
        let builder = Builder::from_string(glade_src);

        // We get the main window and set details
        let win: Window = builder.object("main_window").unwrap();
        win.set_application(Some(app));
        win.set_default_width(320);
        win.set_default_height(200);
        win.set_title("MQTT Client");

        // We show the window.
        win.show_all();

        // We create the controller.
        Controller::new(builder);
    });

    app.run();
}
