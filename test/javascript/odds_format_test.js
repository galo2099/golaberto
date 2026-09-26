const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const context = { window: {} };
const source = fs.readFileSync(path.join(__dirname, "../../app/assets/javascripts/odds_format.js"), "utf8");
vm.runInNewContext(source, context);
const odds = context.window.GolabertoOdds;

assert.equal(odds.format(0.0001, "en-US"), "<0.01%");
assert.equal(odds.html(0.0001, "en-US"), "&lt;0.01%");
assert.equal(odds.title(0.0001, "en-US"), "0.0001%");
assert.equal(odds.format(99.9999, "en-US"), ">99.99%");
assert.equal(odds.title(99.9999, "en-US"), "99.9999%");
assert.notEqual(odds.title(99.99999999999999, "en-US"), "100%");
assert.equal(odds.format(0, "en-US"), "0.00%");
assert.equal(odds.format(0.01, "en-US"), "0.01%");
assert.equal(odds.format(99.99, "en-US"), "99.99%");
assert.equal(odds.format(100, "en-US"), "100.00%");
assert.equal(odds.format(null, "en-US"), "");
assert.equal(odds.format(0.5, "pt-BR"), "0,50%");
assert.equal(odds.format(0.0001, "pt-BR"), "<0,01%");
assert.equal(odds.format(99.9999, "pt-BR"), ">99,99%");
assert.equal(odds.title(0.0001, "pt-BR"), "0,0001%");
