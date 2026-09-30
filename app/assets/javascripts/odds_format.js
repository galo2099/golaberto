(function(window) {
  function validNumber(value) {
    if (value === null || value === undefined || value === "") return null;
    var number = Number(value);
    return isFinite(number) ? number : null;
  }

  function colorStrength(value, maximum) {
    var percentage = validNumber(value);
    var highest = validNumber(maximum);
    if (percentage === null || highest === null || highest <= 0) return 0;
    return Math.min(100, Math.max(0, percentage / highest * 100));
  }

  function zoneRgb(color) {
    var value = String(color || "").trim();
    var match = /^#([0-9a-f]{3})$/i.exec(value);
    if (match) return match[1].split("").map(function(digit) { return parseInt(digit + digit, 16); });
    match = /^#([0-9a-f]{6})$/i.exec(value);
    if (match) return match[1].match(/../g).map(function(channel) { return parseInt(channel, 16); });
    match = /^rgb\(\s*(\d{1,3})\s*,\s*(\d{1,3})\s*,\s*(\d{1,3})\s*\)$/i.exec(value);
    if (match) {
      var channels = match.slice(1).map(Number);
      return channels.every(function(channel) { return channel <= 255; }) ? channels : null;
    }
    var named = { black: [0, 0, 0], white: [255, 255, 255], lightgray: [211, 211, 211], lightgrey: [211, 211, 211] };
    if (named[value.toLowerCase()]) return named[value.toLowerCase()];

    // The zone editor can also store CSS color names. Resolve those with the browser.
    if (!window.document || !window.document.body || !window.getComputedStyle) return null;
    var probe = window.document.createElement("span");
    probe.style.color = value;
    if (!probe.style.color) return null;
    window.document.body.appendChild(probe);
    var computed = window.getComputedStyle(probe).color;
    window.document.body.removeChild(probe);
    match = /^rgba?\(\s*(\d{1,3})\s*,\s*(\d{1,3})\s*,\s*(\d{1,3})/.exec(computed);
    return match ? match.slice(1).map(Number) : null;
  }

  window.GolabertoOdds = {
    maximum: function(values) {
      return values.reduce(function(highest, value) {
        var number = validNumber(value);
        return number === null ? highest : Math.max(highest, number);
      }, 0);
    },

    backgroundColor: function(value, zoneColor, maximum) {
      var color = zoneColor || "lightgray";
      var strength = colorStrength(value, maximum);
      if (strength <= 0) return "lightgray";
      if (strength >= 100) return color;
      var rgb = zoneRgb(color);
      if (!rgb) return "lightgray";
      var weight = strength / 100;
      return "rgb(" + rgb.map(function(channel) {
        return Math.round(211 * (1 - weight) + channel * weight);
      }).join(", ") + ")";
    },

    textColor: function(value, zoneColor, maximum) {
      return zoneColor === "black" && colorStrength(value, maximum) >= 45 ? "white" : "inherit";
    },

    zoneValue: function(odds, positions) {
      var value = positions.reduce(function(sum, pos) {
        var probability = odds[Number(pos) - 1];
        return sum + (probability == null ? 0 : Number(probability));
      }, 0);
      var uniquePositions = positions.filter(function(pos, index) {
        return positions.indexOf(pos) === index;
      });
      var noOutsideOdds = odds.every(function(probability, index) {
        return positions.indexOf(index + 1) !== -1 || validNumber(probability) === 0;
      });
      var allOddsKnown = odds.every(function(probability) { return validNumber(probability) !== null; });
      if (uniquePositions.length === positions.length && noOutsideOdds && allOddsKnown &&
          Math.abs(value - 100) <= 0.0001) return 100;
      return value;
    },

    format: function(value, locale) {
      var number = validNumber(value);
      if (number === null) return "";
      var options = {
        minimumFractionDigits: 2,
        maximumFractionDigits: 2
      };
      if (number > 0 && number < 0.01) {
        var scientific = number.toExponential(0).split("e");
        return scientific[0] + "e" + Number(scientific[1]) + "%";
      }
      if (number > 99.99 && number < 100) {
        return ">" + (99.99).toLocaleString(locale, options) + "%";
      }
      return number.toLocaleString(locale, options) + "%";
    },

    // Match the server-rendered standings cells, including the decimal dot.
    fraction: function(value) {
      var number = validNumber(value);
      if (number === null) return "";
      if (number === 0) return ".0000";
      if (number === 100) return "1.000";

      var probability = number / 100;
      if (probability > 0 && probability < 0.0001) {
        var small = probability.toExponential(0).split("e");
        return small[0] + "e" + Number(small[1]);
      }
      if (probability > 0.9999 && probability < 1) {
        var complement = ((100 - number) / 100).toExponential(0).split("e");
        return "1-" + complement[0] + "e" + Number(complement[1]);
      }

      return probability.toFixed(4).replace(/^0\./, ".");
    },

    html: function(value, locale) {
      return this.format(value, locale)
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;");
    },

    title: function(value, locale) {
      var number = validNumber(value);
      if (number === null) return "";
      if (number > 0 && number < 0.01) {
        var decimal = new Intl.NumberFormat(locale).formatToParts(1.1).filter(function(part) {
          return part.type === "decimal";
        })[0].value;
        var scientific = number.toExponential(3).split("e");
        var mantissa = scientific[0].replace(/0+$/, "").replace(/\.$/, "").replace(".", decimal);
        return mantissa + "e" + Number(scientific[1]) + "%";
      }
      if (number >= 0.01 && number <= 99.99) {
        var formatter = new Intl.NumberFormat(locale, {
          maximumSignificantDigits: 4,
          useGrouping: false
        });
        var rounded = formatter.format(number);
        return (rounded === formatter.format(100) ? formatter.format(99.99) : rounded) + "%";
      }
      return number.toLocaleString(locale, {
        maximumSignificantDigits: 17,
        useGrouping: false
      }) + "%";
    }
  };
})(window);
