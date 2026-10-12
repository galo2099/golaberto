const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const template = fs.readFileSync(path.join(__dirname, "../../app/views/championship/team.html.erb"), "utf8");
const match = template.match(/oddsHistoryPlaceholder\.bind\("plothover", function\(event, pos, item\) \{([\s\S]*?)\n      \}\);/);
assert.ok(match, "plothover tooltip handler exists");

const oddsContext = { window: {} };
vm.runInNewContext(fs.readFileSync(path.join(__dirname, "../../app/assets/javascripts/odds_format.js"), "utf8"), oddsContext);
const GolabertoOdds = oddsContext.window.GolabertoOdds;
const tooltipState = {};
const oddsHistoryTooltip = {
  html(value) { tooltipState.html = value; return this; },
  attr(name, value) { tooltipState[name] = value; return this; },
  css() { return this; },
  fadeIn() { return this; },
  hide() { return this; },
};
const oddsHistoryPlaceholder = {
  removeAttr() { return this; },
  attr(name, value) { tooltipState.placeholderTitle = value; return this; },
};
const series = {
  point_meta: [
    { recorded_on: "2026-10-11", reachability: "impossible", zone_odds: [
      { name: "Zone A", value: 0, reachability: "reachable" },
      { name: "Zone B", value: 0, reachability: "impossible" },
      { name: "Zone C", value: 0 },
    ] },
    { recorded_on: "2026-10-12", reachability: "reachable", zone_odds: [] },
    { recorded_on: "2026-10-13", zone_odds: [] },
  ],
  data: [[1, 0], [2, 0], [3, 0]],
  label: "1st",
};
const oddsHistoryPlot = { getData() { return [series]; } };
const context = {
  GolabertoOdds,
  oddsHistoryLocale: "en-US",
  oddsHistoryStatusLabels: {},
  oddsHistoryPlot,
  oddsHistoryTooltip,
  oddsHistoryPlaceholder,
  formatOddsHistoryDate(value) { return value; },
  fullDateFormatter: null,
};
const renderTooltip = vm.runInNewContext("(function(event, pos, item) {" + match[1] + "})", context);
renderTooltip.call(context, {}, { pageY: 1, pageX: 2 }, { seriesIndex: 0, dataIndex: 0 });
assert.match(tooltipState.html, /0\.0%<\/span>/, "impossible zeros show the percentage sign without a marker");
assert.match(tooltipState.html, /0\.0%<sup[^>]*>\*<\/sup>/, "reachable zone zero has an asterisk after the percentage sign");
assert.match(tooltipState.html, /0\.0%<sup[^>]*>\?<\/sup>/, "undecided zero has a question mark after the percentage sign");
assert.equal(tooltipState.title, "0.0%");
assert.equal(tooltipState.placeholderTitle, "0.0%");

renderTooltip.call(context, {}, { pageY: 1, pageX: 2 }, { seriesIndex: 0, dataIndex: 1 });
assert.match(tooltipState.html, /0\.0%<sup[^>]*>\*<\/sup>/, "reachable position zero has an asterisk after the percentage sign");
renderTooltip.call(context, {}, { pageY: 1, pageX: 2 }, { seriesIndex: 0, dataIndex: 2 });
assert.match(tooltipState.html, /0\.0%<sup[^>]*>\?<\/sup>/, "legacy metadata remains undecided");
