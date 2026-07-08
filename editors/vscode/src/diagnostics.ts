import * as vscode from "vscode";
import {
  cwdForFile,
  JarvyNotFoundError,
  runJarvy,
  extractJsonObject,
  warnBinaryMissing,
} from "./jarvyCli";
import {
  JarvySeverity,
  JarvyValidationIssue,
  parseValidationResult,
} from "./types";

/** Diagnostic source label shown in the Problems panel. */
export const DIAGNOSTIC_SOURCE = "jarvy";

/**
 * Diagnostic `code` tag set on "unknown tool" errors so the code-action
 * provider can offer a quick fix without re-parsing the message.
 */
export const UNKNOWN_TOOL_CODE = "jarvy.unknown-tool";

/** High-level config status, surfaced on the status bar. */
export type ConfigStatus = "valid" | "invalid" | "no-config" | "unknown";

/** Outcome of a validation run. */
export interface ValidationSummary {
  readonly status: ConfigStatus;
  readonly errorCount: number;
  readonly warningCount: number;
}

const UNKNOWN_TOOL_PREFIX = "Unknown tool:";

function toVscodeSeverity(severity: JarvySeverity): vscode.DiagnosticSeverity {
  switch (severity) {
    case "error":
      return vscode.DiagnosticSeverity.Error;
    case "warning":
      return vscode.DiagnosticSeverity.Warning;
    case "info":
      return vscode.DiagnosticSeverity.Information;
  }
}

/**
 * Build a range for an issue. jarvy reports an optional 1-based `line` and no
 * column, so we highlight the whole line when available, otherwise the first
 * line of the document (file-level diagnostic).
 */
function rangeForIssue(
  document: vscode.TextDocument,
  issue: JarvyValidationIssue,
): vscode.Range {
  if (issue.line !== undefined && issue.line >= 1) {
    const lineIdx = Math.min(issue.line - 1, Math.max(document.lineCount - 1, 0));
    return document.lineAt(lineIdx).range;
  }
  const firstLineEnd = document.lineCount > 0 ? document.lineAt(0).range.end : new vscode.Position(0, 0);
  return new vscode.Range(new vscode.Position(0, 0), firstLineEnd);
}

function buildDiagnostic(
  document: vscode.TextDocument,
  issue: JarvyValidationIssue,
): vscode.Diagnostic {
  const diagnostic = new vscode.Diagnostic(
    rangeForIssue(document, issue),
    issue.suggestion ? `${issue.message}\n${issue.suggestion}` : issue.message,
    toVscodeSeverity(issue.severity),
  );
  diagnostic.source = DIAGNOSTIC_SOURCE;
  if (issue.message.startsWith(UNKNOWN_TOOL_PREFIX)) {
    diagnostic.code = UNKNOWN_TOOL_CODE;
  }
  return diagnostic;
}

/**
 * Run `jarvy validate` against `document`, publish diagnostics into
 * `collection`, and return a summary for the status bar.
 *
 * Never throws: a missing binary shows a one-time warning and returns an
 * `unknown` status; a malformed payload falls back to a file-level error.
 */
export async function validateDocument(
  document: vscode.TextDocument,
  collection: vscode.DiagnosticCollection,
  strict: boolean,
): Promise<ValidationSummary> {
  const uri = document.uri;
  const args = ["validate", "--file", uri.fsPath, "--format", "json"];
  if (strict) {
    args.push("--strict");
  }

  let result;
  try {
    result = await runJarvy(args, cwdForFile(uri));
  } catch (err) {
    if (err instanceof JarvyNotFoundError) {
      collection.delete(uri);
      await warnBinaryMissing(err.executable);
      return { status: "unknown", errorCount: 0, warningCount: 0 };
    }
    // Unexpected spawn failure — surface as a single file-level diagnostic.
    const message = err instanceof Error ? err.message : String(err);
    collection.set(uri, [fileLevelError(document, `Failed to run jarvy validate: ${message}`)]);
    return { status: "unknown", errorCount: 1, warningCount: 0 };
  }

  const parsed = parseValidationResult(extractJsonObject(result.stdout));
  if (!parsed) {
    collection.set(uri, [
      fileLevelError(
        document,
        "jarvy validate returned output that could not be parsed as JSON.",
      ),
    ]);
    return { status: "unknown", errorCount: 1, warningCount: 0 };
  }

  const diagnostics = parsed.issues.map((issue) => buildDiagnostic(document, issue));
  collection.set(uri, diagnostics);

  const errorCount = diagnostics.filter(
    (d) => d.severity === vscode.DiagnosticSeverity.Error,
  ).length;
  const warningCount = diagnostics.filter(
    (d) => d.severity === vscode.DiagnosticSeverity.Warning,
  ).length;

  return {
    status: errorCount > 0 ? "invalid" : "valid",
    errorCount,
    warningCount,
  };
}

function fileLevelError(
  document: vscode.TextDocument,
  message: string,
): vscode.Diagnostic {
  const end = document.lineCount > 0 ? document.lineAt(0).range.end : new vscode.Position(0, 0);
  const diagnostic = new vscode.Diagnostic(
    new vscode.Range(new vscode.Position(0, 0), end),
    message,
    vscode.DiagnosticSeverity.Error,
  );
  diagnostic.source = DIAGNOSTIC_SOURCE;
  return diagnostic;
}
