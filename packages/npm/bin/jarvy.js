#!/usr/bin/env node
"use strict";

// Full jarvy CLI passthrough: `npx --package=jarvy-mcp jarvy setup`, etc.
const { run } = require("../lib/run");

run(process.argv.slice(2));
