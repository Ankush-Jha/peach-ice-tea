"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const { titleCase } = require("./lib.js");

test("capitalizes every word", () => {
  assert.equal(titleCase("hello world"), "Hello World");
});

test("lowercases the rest of each word", () => {
  assert.equal(titleCase("hello WORLD"), "Hello World");
});

test("handles a single word", () => {
  assert.equal(titleCase("gemini"), "Gemini");
});

test("handles an empty string", () => {
  assert.equal(titleCase(""), "");
});
