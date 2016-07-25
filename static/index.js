var options = {
    rollPeriod: 6,
    dateWindow: [Date.now() - 2 * 30 * 24 * 60 * 60 * 1000, Date.now()],
    showRoller: true,
    showRangeSelector: true,
    labelsUTC: true,
    height: 300
};

var socOptions = options;
socOptions.axes = {
    y: {
        axisLabelFormatter: function(y) {
            return y + '%';
        }
    }
};
var soc = new Dygraph(
    document.getElementById("fig-soc"),
    "soc.csv", options);

var temperatureOptions = options;
temperatureOptions.rollPeriod = 24;
temperatureOptions.axes = {
    y: {
        axisLabelFormatter: function(y) {
            return y + '&deg;C';
        }
    }
};
var temperature = new Dygraph(
    document.getElementById("fig-temperature"),
    "temperature.csv", options);

var sync = Dygraph.synchronize(soc, temperature, { range: false });
