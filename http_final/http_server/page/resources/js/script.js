window.onload = function () {
  setInterval(function () {
    httpGetAsync("/data", updateTemp);
    plotGraph();
  }, 2000);
};

function updateTemp(newTemp) {
  document.getElementById("temp1").innerText = format_number_to_temp(newTemp);
  document.getElementById("temp2").innerText = format_number_to_temp(newTemp);
  document.body.style.backgroundColor = getBackgroundColor(newTemp);
}

function format_number_to_temp(number_as_text) {
  return (Math.round(number_as_text * 100) / 100).toFixed(1) + " °C";
}

function httpGetAsync(url, callback) {
  const xmlHttp = new XMLHttpRequest();
  xmlHttp.onreadystatechange = function () {
    if (xmlHttp.readyState == 4 && xmlHttp.status == 200)
      callback(xmlHttp.responseText);
  };
  xmlHttp.open("GET", url, true);
  xmlHttp.send(null);
}
function getBackgroundColor(temp) {
  if (temp < 25) {
    return "#6BBCD1";
  } else if (temp < 75) {
    return "#FEB938";
  } else {
    return "#FF6b42";
  }
}

function plotGraph() {
  let temperature = document.getElementById("temp1").innerText;
  let n_points = 100;
  if (localStorage.points) {
    points = JSON.parse(localStorage.points).slice(1 - n_points);
    localStorage.points = JSON.stringify([...points, temperature]);
  } else {
    points = [];
    localStorage.points = JSON.stringify([temperature]);
  }
  data_points = points.map((p) => ({ y: parseFloat(p) }));
  chart = new CanvasJS.Chart("chartContainer", {
    animationEnabled: false,
    backgroundColor: "transparent",

    data: [
      {
        type: "line",
        indexLabelFontSize: 16,
        dataPoints: data_points,
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
  chart.render();

  setTimeout(function () {
    chart.options.animationEnabled = true;
    chart.options.data[0].dataPoints.push({ y: temperature });
    chart.render();
  }, 300);
}
