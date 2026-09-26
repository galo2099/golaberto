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
        return number.toExponential().replace(".", decimal)
          .replace(/e([+-])0+(\d+)/, "e$1$2").replace("e+", "e") + "%";
      }
      return number.toLocaleString(locale, {
        maximumSignificantDigits: 17,
        useGrouping: false
      }) + "%";
    }
  };
})(window);
