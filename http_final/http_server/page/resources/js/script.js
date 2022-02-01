// Variables
var temperature = 0;
var points = [];
var chart;

// Set data-fetching loop
// will update the temperature and graph on a fixed interval
window.onload = function () {
  setInterval(function () {
    httpGetAsync("/data", updateTemp);
    plotGraph();
  }, 2000);
};

// Function to make an HTTP GET request
function httpGetAsync(url, callback) {
  const xmlHttp = new XMLHttpRequest();
  xmlHttp.onreadystatechange = function () {
    if (xmlHttp.readyState == 4 && xmlHttp.status == 200)
      callback(xmlHttp.responseText);
  };
  xmlHttp.open("GET", url, true);
  xmlHttp.send(null);
}

function updateTemp(newTemp) {
  temperature = newTemp;
  document.getElementById("temperature").innerText =
    format_number_to_temp(temperature);
  document.querySelector("#main").style.backgroundColor =
    getOverlappingColor(temperature);
}

function getOverlappingColor(temp) {
  const opacity = parseInt(255 - (temp * 255) / 100).toString(16);
  return `#8fffff${opacity}`;
}

function format_number_to_temp(number_as_text) {
  return (Math.round(number_as_text * 100) / 100).toFixed(1) + " °C";
}

function plotGraph() {
  const n_points = 100;
  const point = { y: parseFloat(temperature) };

  chart =
    chart ??
    new CanvasJS.Chart("chart_container", {
      animationEnabled: false,
      backgroundColor: "transparent",

      data: [
        {
          type: "line",
          indexLabelFontSize: 16,
          dataPoints: [],
          markerSize: 8,
          lineColor: "white",
          lineThickness: 2,
        },
      ],
      axisX: {
        gridColor: "white",
        gridThickness: 0,
        tickLength: 0,
        lineThickness: 0,
        labelFormatter: function () {
          return " ";
        },
      },
      axisY: {
        minimum: 0,
        maximum: 100,
        gridColor: "gray",
        title: "Temperature (°C)",
      },
    });

  chart.options.data[0].dataPoints = [
    ...chart.options.data[0].dataPoints,
    point,
  ].slice(-n_points);
  chart.render();
}
