mod client;
mod connack;
mod connect;
mod publish;
mod subscribe;

use crate::{client::Client, connect::{ConnectBuilder, QoSLevel}, publish::Publish, subscribe::{Subscribe, Topic}};
// extern crate gtk;

fn main() -> Result<(), String> {
    let mut client = Client::new("127.0.0.1:1883").map_err(|err| -> String { err.to_string() })?;

    client.connect(ConnectBuilder::new("rust", 15, false)?.build()?)?;

    println!("Conexion exitosa");

    // let aplication = gtk::Application::new(Some("mqtt_client"), Default::default());

    // application.connect_activate(|app| {
    //     // let window = gtk::ApplicationWindow::new(app);
    //     // window.set_title("MQTT Client");
    //     // window.set_default_size(400, 300);
    //     // window.show_all();
    //     build_ui(app);
    // });

    // aplication.run();
    do_something(&mut client);
    Ok(())
}

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

// fn build_ui(app: &gtk::Aplication) {
//     let glade_src = include_str!("mqtt.glade");
//     let builder = gtk::Builder::new_from_string(glade_src);

//     let window: gtk::Window = builder.get_object("main_window").unwrap();
//     window.set_aplication(Some(app));
//     //
//     //
//     window.show_all();
// }

#[cfg(test)]
mod tests {
    use crate::{
        connect::QoSLevel,
        publish::Publish,
        subscribe::{Subscribe, Topic},
    };

    use super::*;

    /**/
    #[test]
    fn test() {
        let mut client = Client::new("127.0.0.1:1883")
            .map_err(|err| -> String { err.to_string() })
            .unwrap();

        client
            .connect(
                ConnectBuilder::new("rust", 15, false)
                    .unwrap()
                    .build()
                    .unwrap(),
            )
            .unwrap();

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
    /**/
}
