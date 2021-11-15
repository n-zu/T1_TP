mod client;
mod client_packets;
mod interface;
mod observer;
use crate::interface::Controller;
use gtk::prelude::CssProviderExt;
use gtk::{gdk, Builder, StyleContext};
use gtk::{
    prelude::{ApplicationExt, ApplicationExtManual, BuilderExtManual, GtkWindowExt, WidgetExt},
    Application, Window,
};

fn main() {
    let app = Application::builder()
        .application_id("ar.uba.fi.rostovfc.mqtt")
        .build();

    app.connect_activate(move |app| {
        // Load the compiled resource bundle
        // let resources_bytes = include_bytes!("resources/resources.xml");
        // let resource_data = Bytes::from(&resources_bytes[..]);
        // let res = gio::Resource::from_data(&resource_data).unwrap();
        // gio::resources_register(&res);

        // Load the CSS
        let glade_css = include_str!("resources/mqtt.glade");
        let provider = gtk::CssProvider::new();
        let _ = provider.load_from_path(glade_css);
        StyleContext::add_provider_for_screen(
            &gdk::Screen::default().expect("Error initializing gtk css provider."),
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

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
