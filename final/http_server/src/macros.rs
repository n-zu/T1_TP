#[macro_export]
/// Creates a String of the HTTP code of the webpage with the given refresh rate, n-points and message
macro_rules! html {
    ($x:expr, $y:expr, $z:expr) => {
        format!("
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
        ", $x, $y, $z)
    }
}

#[macro_export]
/// Creates a String of a HTTP response with the given message
macro_rules! http_response {
    ($x:expr) => {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
            $x.len(),
            $x
        )
    };
}
