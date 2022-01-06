use config::config::Config;
use std::{
    io::prelude::*,
    net::{TcpListener, TcpStream},
    sync::{mpsc::Receiver, Arc, Mutex},
};

pub struct Server {
    config: Config,
    connections: Arc<Mutex<Vec<TcpStream>>>,
}

impl Server {
    pub fn new(config: &Config) -> Self {
        Server {
            config: config.clone(),
            connections: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn run(self: Arc<Self>, reciever: Receiver<String>) {
        let server = self.clone();

        let _connection_listener = std::thread::spawn(move || {
            server.handle_connections();
        });

        let _message_listener = std::thread::spawn(move || {
            self.handle_messages(reciever);
        });
    }

    fn handle_connections(self: Arc<Self>) {
        let listener =
            TcpListener::bind(&format!("{}:{}", self.config.server, self.config.port)).unwrap();
        println!(
            "_________\n\nListening on {}:{}\n_________",
            self.config.server, self.config.port
        );
        for stream in listener.incoming() {
            let stream = stream.unwrap();
            self.connections.lock().unwrap().push(stream);
        }
    }

    fn handle_messages(self: Arc<Self>, reciever: Receiver<String>) {
        loop {
            let message = reciever.recv().unwrap();
            for stream in self.connections.lock().unwrap().iter() {
                let _stream = stream.try_clone().unwrap();
                Self::post_message(_stream, &message);
                stream.shutdown(std::net::Shutdown::Both).unwrap();
            }
            self.connections.lock().unwrap().clear();
        }
    }

    fn post_message(mut stream: TcpStream, message: &str) {
        let mut buffer = [0; 1024];

        stream.read(&mut buffer).unwrap();
        println!(
            "\x1b[0;33m----------------\n\n{}\n----------------\x1b[0m",
            String::from_utf8_lossy(&buffer)
        );

        let html = format!(
            "
            <!DOCTYPE html>
            <html lang=\"en\">
            <head>
                <title>monitor</title>
                <script>
                setTimeout(function() {{
                    window.location.reload(1);
                }}, {});
                var points;
                var points2;
                var chart;
                window.onload = function () {{

                    let temp = document.getElementById('temp').innerText;
                    let npoints = {};

                    if(localStorage.points){{
                        points = JSON.parse(localStorage.points).slice(1-npoints);
                        localStorage.points = JSON.stringify([...points, temp]);
                    }} else {{
                        points = [];
                        localStorage.points = JSON.stringify([temp]);
                    }}

                    console.log(points);
                    points2 = points.map( p => ({{y:parseFloat(p)}}) );

                    chart = new CanvasJS.Chart(\"chartContainer\", {{
                        animationEnabled: false,
                        theme: \"light2\",
                        title:{{
                            text: \"TIME SLICE\"
                        }},
                        data: [{{        
                            type: \"line\",
                            indexLabelFontSize: 16,
                            dataPoints: points2,
                        }}],
                        axisX:{{gridThickness: 0,tickLength: 0,lineThickness: 0,labelFormatter: function(){{return \" \";}}}},
                        axisY:{{minimum: 0,maximum: 100}},
                    }});

                    console.log(points2);

                    chart.render();
                    
                    setTimeout(function() {{
                        chart.options.animationEnabled=true;
                        chart.options.data[0].dataPoints.push({{ y: temp}});
                        chart.render();
                    }}, 300);

                }}
            </script>
            </head>
            <body>
                <div style=\"display: flex;align-items: center;justify-content: center;min-height: 100vh;\">
                    <h1 id=\"temp\">{}</h1>
                </div>
                <div id=\"chartContainer\" style=\"height: 300px; width: 100%;\"></div>
            <script src=\"https://canvasjs.com/assets/script/canvasjs.min.js\"></script>
            </body>
            </html>
            ",
            200, // refresh rate
            10, // n-points
            message
        );

        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
            html.len(),
            html
        );

        stream.write(response.as_bytes()).unwrap();
        stream.flush().unwrap();
    }
}
