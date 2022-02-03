// Variables
// making them global so they can be accessed from the browser console
let temperature = undefined;
let n_points_big = 100;
let n_points_small = 30;
let chart;
let mainLoop;

const TIMEOUT = 2000;

// Set data-fetching loop
// will update the temperature and graph on a fixed interval
window.onload = function () {
  mainLoop = setInterval(function () {
    httpGetAsync("/data", updateTemp);
    plotGraph();
  }, TIMEOUT);
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
      formatNumberToTemp(temperature);
  document.body.style.backgroundColor = getBackGroundColor(temperature);
}

// function that takes a number from 0 to 100 and returns a color
function getBackGroundColor(temp) {
  const n = parseInt(200 - 2 * temp);
  return `hsl(${n},70%,60%)`;
}

// Function that receives a number as a String, rounds it to 1 decimal
// and appends a suffix "°C" to the final result
function formatNumberToTemp(number_as_text) {
  return (Math.round(number_as_text * 100) / 100).toFixed(1) + " °C";
}

function plotGraph() {
  const point = {y: parseFloat(temperature)};
  const isBig = window.innerWidth > 1000;

  chart =
      chart ??
      new CanvasJS.Chart("chart_container", {
      animationEnabled: true,
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
      },
    });

  chart.options.data[0].dataPoints = [
    ...chart.options.data[0].dataPoints,
    point,
  ]
    .slice(-(isBig ? n_points_big : n_points_small))
    .map(({ y }, i) => ({ x: i, y: y }));

  chart.options.axisY.title = isBig ? "Temperature (°C)" : undefined;

  chart.render();
}
