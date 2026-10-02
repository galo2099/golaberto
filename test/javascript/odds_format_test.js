const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const context = { window: {} };
const source = fs.readFileSync(path.join(__dirname, "../../app/assets/javascripts/odds_format.js"), "utf8");
vm.runInNewContext(source, context);
const odds = context.window.GolabertoOdds;

assert.equal(odds.maximum([null, 0, 12.5, 25]), 25);
assert.equal(odds.maximum([null, 0]), 0);
assert.equal(odds.backgroundColor(null, 25), "lightgray");
assert.equal(odds.backgroundColor(0, 25), "lightgray");
assert.equal(odds.backgroundColor(25, 0), "lightgray");
assert.equal(odds.backgroundColor(12.5, 25), "rgb(136, 136, 136)");
assert.equal(odds.backgroundColor(10, 57), "rgb(167, 167, 167)");
assert.equal(odds.backgroundColor(1e-10, 25), "rgb(211, 211, 211)");
assert.equal(odds.backgroundColor(25, 25), "dimgray");
assert.equal(odds.textColor(10, 25), "inherit");
assert.equal(odds.textColor(11, 25), "white");

const teamOdds = [[0.5, 0.1], [0.25, 0.4]];
const firstZoneOdds = teamOdds.map(function(row) { return odds.zoneValue(row, [1]); });
const secondZoneOdds = teamOdds.map(function(row) { return odds.zoneValue(row, [2]); });
assert.equal(odds.backgroundColor(firstZoneOdds[0], odds.maximum(firstZoneOdds)), "dimgray");
assert.equal(odds.backgroundColor(firstZoneOdds[1], odds.maximum(firstZoneOdds)), "rgb(136, 136, 136)");
assert.equal(odds.backgroundColor(secondZoneOdds[1], odds.maximum(secondZoneOdds)), "dimgray");
assert.equal(odds.backgroundColor(secondZoneOdds[0], odds.maximum(secondZoneOdds)), "rgb(158, 158, 158)");

assert.equal(odds.format(0.0001, "en-US"), "0.[03]1%");
assert.equal(odds.html(0.0001, "en-US"), odds.compactHtml(0.0001) + "%");
assert.equal(odds.format(0.00004321, "en-US"), "0.[04]4%");
assert.equal(odds.format(0.009999, "en-US"), "0.01%");
assert.equal(odds.format(1e-15, "en-US"), "0.[14]1%");
assert.equal(odds.title(0.0001, "en-US"), "1e-4%");
assert.equal(odds.title(0.00004321, "en-US"), "4.321e-5%");
assert.equal(odds.title(0.009999, "en-US"), "9.999e-3%");
assert.equal(odds.format(99.9999, "en-US"), "99.[03]9%");
assert.equal(odds.format(99.99996, "en-US"), "99.[04]6%");
assert.equal(odds.zoneValue([77.75802002807019, 17.064525344641567, 5.177454627288232], [1, 2, 3]), 100);
assert.equal(odds.zoneValue([99.9999999999, 0, 0], [1, 2], ["reachable", "impossible", "impossible"]), 100);
assert.equal(odds.zoneValue([99.9999999, 0, 0.0000001], [1, 2]), 99.9999999);
assert.equal(odds.zoneValue([99.9999999999, null], [1]), 99.9999999999);
assert.equal(odds.zoneValue([99.9999999999, null, 0], [1, 2]), 99.9999999999);
assert.equal(odds.title(99.9999, "en-US"), "99.9999%");
assert.equal(odds.title(99.95, "en-US"), "99.95%");
assert.equal(odds.title(0.012345, "en-US"), "0.01235%");
assert.equal(odds.title(12.3456, "en-US"), "12.35%");
assert.notEqual(odds.title(99.99999999999999, "en-US"), "100%");
assert.equal(odds.format(0, "en-US"), "0.00%");
assert.equal(odds.format(0.01, "en-US"), "0.01%");
assert.equal(odds.format(99.99, "en-US"), "99.99%");
assert.equal(odds.format(100, "en-US"), "100.00%");
assert.equal(odds.format(null, "en-US"), "");
assert.equal(odds.format(0.5, "pt-BR"), "0,50%");
assert.equal(odds.format(0.0001, "pt-BR"), "0.[03]1%");
assert.equal(odds.format(99.9999, "pt-BR"), "99.[03]9%");
assert.equal(odds.title(0.0001, "pt-BR"), "1e-4%");
assert.equal(odds.title(0.00004321, "pt-BR"), "4,321e-5%");
assert.equal(odds.title(12.3456, "pt-BR"), "12,35%");
assert.equal(odds.fraction(null), "");
assert.equal(odds.fraction(0), ".0000");
assert.equal(odds.fraction(98), ".9800");
assert.equal(odds.fraction(50), ".5000");
assert.equal(odds.fraction(99.75), ".9975");
assert.equal(odds.fraction(99.99), ".9999");
assert.equal(odds.fraction(99.99997), "1-3e-7");
assert.equal(odds.fraction(0.01), ".0001");
assert.equal(odds.fraction(0.00004321), "4e-7");
assert.equal(odds.fraction(100), "1.000");

const compactExamples = [
  [99.98, "99.98"],
  [99.9999999321, "99.[07]3"],
  [99.9999999789, "99.[07]8"],
  [99.9999994, "99.[06]4"],
  [99.999999905, "99.[07]1"],
  [99.9999999049, "99.[06]9"],
  [99.995, "99.[02]5"],
  [42.35453, "42.35"],
  [0.0000004, "0.[06]4"],
  [0.0000000000876, "0.[10]9"],
  [12, "12.00"],
  [100, "100.0"],
  [0, "0.0"],
  [1.005, "1.01"],
  [0.005, "0.01"],
  [0.00000045, "0.[06]5"],
  [0.00000095, "0.[05]1"],
  [99.99945, "99.[03]5"],
  [99.999449999, "99.[03]4"],
  [99.99999999999999, "99.[13]9"],
  [1e-100, "0.[99]1"],
  [1e-101, "1e-101"]
];
compactExamples.forEach(function(example) {
  assert.equal(odds.compactText(example[0]), example[1], String(example[0]));
});
assert.equal(odds.compactText(null), "");
assert.match(odds.compactHtml(0.0000004), /odds-compact-count/);
assert.match(odds.compactHtml(0.0000004), /\[06\]/);
function visibleCountDigits(value) {
  return [...odds.compactHtml(value).matchAll(/class="odds-compact-count-digit(?: odds-compact-count-two-digits)?" aria-hidden="true">(\d+)<\/span>/g)]
    .map(function(match) { return match[1]; });
}
assert.deepEqual(visibleCountDigits(0.0000004), ["6"]);
assert.deepEqual(visibleCountDigits(0.0000000000876), ["10"]);
assert.match(odds.compactHtml(0.0000004), /class="odds-compact-count-dots" aria-hidden="true">\.\.<\/span>/);
assert.equal(odds.compactTitle(0.0000004), "4e-7%");

assert.equal(odds.title(0.01, "en-US"), "0.01%");
assert.equal(odds.title(100, "en-US"), "100%");
assert.equal(odds.compactTitle(0.0000123456, "en-US"), "1.235e-5%");
assert.equal(odds.compactTitle(12.3456, "en-US"), "12.35%");

assert.equal(odds.title(99.999912345, "en-US"), "99.99991235%");
assert.equal(odds.compactTitle(99.99997891234, "en-US"), "99.99997891%");
assert.equal(odds.title(99.999912345, "pt-BR"), "99,99991235%");

// A numbered rank uses the same endpoint normalization as a zone.
assert.equal(odds.zoneValue([99.99999999999062, 0, 0], [1], ["reachable", "impossible", "impossible"]), 100);
assert.equal(odds.compactText(odds.zoneValue([99.99999999999062, 0, 0], [1], ["reachable", "impossible", "impossible"])), "100.0");
assert.equal(odds.zoneValue([99.99999999999062, 9.38e-12, 0], [1]), 99.99999999999062);

assert.equal(odds.html(99.999, "en-US"), odds.compactHtml(99.999) + "%");
assert.ok(odds.html(99.999, "en-US").includes('class="odds-compact-count-dots"'));

assert.equal(odds.reachability([0, 0], ["impossible", "impossible"], [1, 2]), "impossible");
assert.equal(odds.reachability([0, 0], ["reachable", "undecided"], [1, 2]), "reachable");
assert.equal(odds.reachability([0, 0], ["impossible", "undecided"], [1, 2]), "undecided");
assert.equal(odds.reachability([0, 0], [], [1]), "undecided");
assert.equal(odds.reachability([1, 0], [], [1]), "reachable");
assert.equal(odds.positionHtml(0, "impossible", "en-US", false), '<span class="odds-zero" aria-label="0.0">0.0</span>');
assert.match(odds.positionHtml(0, "reachable", "en-US", true), /<sup[^>]*>\*<\/sup>/);
assert.match(odds.positionHtml(0, "undecided", "en-US", true), /<sup[^>]*>\?<\/sup>/);
assert.equal(odds.positionHtml(0.0000004, "reachable", "en-US", true), odds.compactHtml(0.0000004));
assert.equal(odds.positionTitle(0, "reachable", "pt-BR", {reachable: "Possível"}), "0.0 — Possível");
assert.equal(odds.zoneValue([99.99999999999062, 0, 0], [1]), 99.99999999999062);
assert.equal(odds.zoneValue([99.99999999999062, 0, 0], [1], ["reachable", "reachable", "impossible"]), 99.99999999999062);

assert.equal(odds.zoneValue([], [1]), null);
assert.equal(odds.zoneValue([0], []), null);

assert.equal(odds.positionTitle(0, "impossible", "en-US"), "0.0");
assert.equal(odds.positionTitle(0, "undecided", "en-US"), "0.0 — Reachability not resolved.");
assert.match(odds.positionHtml(0, "reachable", "en-US", true), /class="odds-compact-dot">\.<\/span>/);
["impossible", "reachable", "undecided"].forEach(function(status) {
  var markup = odds.positionHtml(0, status, "en-US", true);
  assert.match(markup, /class="odds-zero odds-compact-number"/);
  assert.match(markup, /class="odds-compact-integer">0<\/span>/);
  assert.match(markup, /class="odds-compact-fraction"/);
  assert.equal(markup.replace(/<[^>]*>/g, ""), "0.0" + (status === "impossible" ? "" : status === "reachable" ? "*" : "?"));
});
assert.match(odds.positionHtml(0, "undecided", "en-US", false), />0\.0<sup/);
