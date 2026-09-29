(function(window) {
  function validNumber(value) {
    if (value === null || value === undefined || value === "") return null;
    var number = Number(value);
    return isFinite(number) ? number : null;
  }

  window.GolabertoOdds = {
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

    format: function(value, locale, compact) {
      var number = validNumber(value);
      if (number === null) return "";
      var options = {
        minimumFractionDigits: 2,
        maximumFractionDigits: 2
      };
      if (number > 0 && number < 0.01) {
        var scientific = number.toExponential(0).split("e");
        return scientific[0] + "e" + Number(scientific[1]) + (compact ? "" : "%");
      }
      if (number > 99.99 && number < 100) {
        return ">" + (99.99).toLocaleString(locale, options) + (compact ? "" : "%");
      }
      return number.toLocaleString(locale, options) + (compact ? "" : "%");
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
