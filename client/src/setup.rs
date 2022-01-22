use gtk::gdk_pixbuf::PixbufLoader;
use gtk::prelude::{BuilderExtManual, CssProviderExt, GtkWindowExt, PixbufLoaderExt, WidgetExt};
use gtk::{gdk, Application, Builder, Window};

use crate::interface::Controller;

/// Loads CSS into the application
pub fn load_css() {
    let provider = gtk::CssProvider::new();
    let style = include_bytes!("resources/mqtt.css");
    provider.load_from_data(style).expect("Failed to load CSS");
    gtk::StyleContext::add_provider_for_screen(
        &gdk::Screen::default().expect("Error initializing gtk css provider."),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

/// Builds the UI
pub fn build_ui(app: &Application) {
    // Load the window UI
    let glade_src = include_str!("resources/mqtt.xml");
    let builder = Builder::from_string(glade_src);

    // We get the main window and set details
    let win: Window = builder.object("main_window").unwrap();
    win.set_application(Some(app));
    win.set_default_width(640);
    win.set_default_height(480);

    let loader = PixbufLoader::with_type("ico").unwrap();
    loader.write(include_bytes!("resources/logo.ico")).unwrap();
    loader.close().unwrap();
    win.set_icon(Some(&loader.pixbuf().unwrap()));

    // We show the window.
    win.show_all();

    // We create the controller.
    Controller::new(builder);
}
