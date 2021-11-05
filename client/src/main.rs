mod client;
mod client_error;
mod connack;
mod connect;
mod controller;
mod disconnect;
mod publish;
mod subscribe;
use crate::controller::Controller;
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

        // Don't forget to make all widgets visible.
        win.show_all();

        Controller::new(builder);
    });

    app.run();
}

/*
fn do_something(client :&mut Client) {
    // create subscribe packet
    let subscribe_packet = Subscribe::new(
        vec![
            Topic::new("topic", QoSLevel::QoSLevel0).unwrap(),
            Topic::new("topic/sub", QoSLevel::QoSLevel1).unwrap(),
        ],
        1,
    );

    // create publish packet
    let publish_packet = Publish::new(
        false,
        QoSLevel::QoSLevel1,
        false,
        "topic",
        "message",
        Some(2),
    )
    .unwrap();

    client.subscribe(subscribe_packet).unwrap();

    client.publish(publish_packet).unwrap();
}
 */
