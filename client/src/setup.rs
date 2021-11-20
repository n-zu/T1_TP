use crate::interface::Controller;
//use gtk::{gdk_pixbuf::PixbufLoader, prelude::PixbufLoaderExt};
use gtk::prelude::{BuilderExtManual, CssProviderExt, GtkWindowExt, WidgetExt};
use gtk::{gdk, Application, Builder, Window};

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

pub fn load_main_window(app: &Application) {
    // Load the window UI
    let glade_src = include_str!("resources/mqtt.xml");
    let builder = Builder::from_string(glade_src);

    // We get the main window and set details
    let win: Window = builder.object("main_window").unwrap();
    win.set_application(Some(app));
    win.set_default_width(320);
    win.set_default_height(200);

    /* No logré hacer andar el ícono
    let loader = PixbufLoader::with_type("png").unwrap();
    loader.write(include_bytes!("resources/logo.png")).unwrap();
    loader.close().unwrap();
    win.set_icon(Some(&loader.pixbuf().unwrap()));
    */

    win.set_icon_name(Some("gtk-network"));
    // We show the window.
    win.show_all();

    // We create the controller.
    Controller::new(builder);
}
