var temperature = new Dygraph(
    document.getElementById("fig-temperature"),
    "temperature.csv", {
        rollPeriod: 6,
        dateWindow: [Date.now() - 2 * 30 * 24 * 60 * 60 * 1000, Date.now()],
        showRoller: true,
        showRangeSelector: true,
        labelsUTC: true,
        height: 300,
        axes: {
          y: {
            axisLabelFormatter: function(y) {
              return y + '&deg;C';
            }
          }
        }
    }
);
