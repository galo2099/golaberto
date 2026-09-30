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
    return Math.sqrt(Math.min(1, Math.max(0, percentage / highest))) * 100;
  }

  function compactCount(count) {
    return "[" + String(count).padStart(2, "0") + "]";
  }

  // Work from the decimal representation so halfway values round upward even
  // when multiplying their binary floating-point value would cross the tie.
  function decimalComponents(number) {
    var match = String(number).match(/^(\d+)(?:\.(\d+))?(?:e([+-]?\d+))?$/i);
    var raw = match[1] + (match[2] || "");
    var leadingZeros = (raw.match(/^0*/) || [""])[0].length;
    return {
      digits: raw.slice(leadingZeros),
      exponent: match[1].length - leadingZeros - 1 + Number(match[3] || 0)
    };
  }

  function roundedCents(number) {
    var decimal = decimalComponents(number);
    if (!decimal.digits) return 0;
    var split = decimal.exponent + 3;
    var whole = split > 0 ? decimal.digits.slice(0, split).padEnd(split, "0") : "0";
    var nextDigit = split < 0 ? "0" : decimal.digits[split];
    return Number(whole) + (nextDigit >= "5" ? 1 : 0);
  }

  function roundedUpperGap(number) {
    // This branch only handles values in [99.995, 100), whose decimal
    // representation starts with 99. Its complement has exact decimal digits.
    var fraction = String(number).split(".")[1];
    var complement = fraction.split("");
    var carry = 1;
    for (var i = complement.length - 1; i >= 0; i--) {
      var digit = 9 - Number(complement[i]) + carry;
      complement[i] = String(digit % 10);
      carry = digit === 10 ? 1 : 0;
    }
    var gapDigits = complement.join("");
    var zeros = (gapDigits.match(/^0*/) || [""])[0].length;
    var significant = gapDigits.slice(zeros);
    var exponent = -zeros - 1;
    var digit = Number(significant[0]);
    var nextDigit = significant[1] || "0";
    // Halfway gaps round down, so halfway displayed percentages round up.
    if (nextDigit > "5" || (nextDigit === "5" && /[1-9]/.test(significant.slice(2)))) digit++;
    if (digit === 10) { digit = 1; exponent++; }
    return { digit: digit, exponent: exponent };
  }

  window.GolabertoOdds = {
    maximum: function(values) {
      return values.reduce(function(highest, value) {
        var number = validNumber(value);
        return number === null ? highest : Math.max(highest, number);
      }, 0);
    },

    backgroundColor: function(value, maximum) {
      var strength = colorStrength(value, maximum);
      if (strength <= 0) return "lightgray";
      if (strength >= 100) return "dimgray";
      var shade = Math.round(211 - 106 * strength / 100);
      return "rgb(" + shade + ", " + shade + ", " + shade + ")";
    },

    textColor: function(value, maximum) {
      return colorStrength(value, maximum) >= 65 ? "white" : "inherit";
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

    compactParts: function(value) {
      var number = validNumber(value);
      if (number === null || number < 0 || number > 100) return null;
      if (number === 0) return { integer: "0", first: "0", second: "" };
      if (number === 100) return { integer: "100", first: "0", second: "" };

      var cents = roundedCents(number);
      if (cents > 0 && cents < 10000) {
        var fraction = String(cents % 100).padStart(2, "0");
        return { integer: String(Math.floor(cents / 100)), first: fraction[0], second: fraction[1] };
      }

      if (cents === 0) {
        var decimal = decimalComponents(number);
        var digit = Number(decimal.digits[0]) + (decimal.digits[1] >= "5" ? 1 : 0);
        var exponent = decimal.exponent;
        if (digit === 10) { digit = 1; exponent++; }
        var lowerCount = -exponent - 1;
        if (lowerCount > 99) return { fallback: digit + "e" + exponent };
        return { integer: "0", first: compactCount(lowerCount), second: String(digit), count: "first" };
      }

      var gap = roundedUpperGap(number);
      var upperCount = -gap.exponent - 1;
      if (upperCount > 99) return { fallback: "100 − " + gap.digit + "e" + gap.exponent };
      return { integer: "99", first: compactCount(upperCount), second: String(10 - gap.digit), count: "first" };
    },

    compactText: function(value) {
      var parts = this.compactParts(value);
      if (parts === null) return "";
      return parts.fallback || parts.integer + "." + parts.first + parts.second;
    },

    compactHtml: function(value) {
      var parts = this.compactParts(value);
      if (parts === null) return "";
      if (parts.fallback) return '<span class="odds-compact-fallback">' + parts.fallback + '</span>';
      var fraction = function(name) {
        var text = parts[name];
        if (parts.count === name) {
          var visibleCount = String(Number(text.slice(1, 3)));
          var countClass = "odds-compact-count-digit" +
            (visibleCount.length === 2 ? " odds-compact-count-two-digits" : "");
          text = '<span class="odds-compact-count" aria-label="' + text + '">' +
            '<span class="' + countClass + '" aria-hidden="true">' + visibleCount + '</span>' +
            '<span class="odds-compact-count-dots" aria-hidden="true">..</span></span>';
        }
        return '<span class="odds-compact-slot">' + text + '</span>';
      };
      return '<span class="odds-compact-number"><span class="odds-compact-integer">' +
        parts.integer + '</span><span class="odds-compact-dot">.</span><span class="odds-compact-fraction">' +
        fraction("first") + fraction("second") + '</span></span>';
    },

    compactTitle: function(value) {
      var number = validNumber(value);
      return number === null ? "" : String(number) + "%";
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
