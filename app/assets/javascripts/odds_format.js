(function(window) {
  function validNumber(value) {
    if (value === null || value === undefined || value === "") return null;
    var number = Number(value);
    return isFinite(number) ? number : null;
  }

  window.GolabertoOdds = {
    format: function(value, locale) {
      var number = validNumber(value);
      if (number === null) return "";
      var options = {
        minimumFractionDigits: 2,
        maximumFractionDigits: 2
      };
      if (number > 0 && number < 0.01) return "<" + (0.01).toLocaleString(locale, options) + "%";
      if (number > 99.99 && number < 100) return ">" + (99.99).toLocaleString(locale, options) + "%";
      return number.toLocaleString(locale, options) + "%";
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
