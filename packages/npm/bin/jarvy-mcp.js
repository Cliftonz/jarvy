#!/usr/bin/env node
"use strict";

// MCP entry point: `npx jarvy-mcp` == `jarvy mcp [extra args]`.
// This is the command MCP clients (Claude Desktop, Cursor, ...) invoke.
const { run } = require("../lib/run");

run(["mcp", ...process.argv.slice(2)]);
