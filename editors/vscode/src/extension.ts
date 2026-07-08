import * as path from "path";
import * as vscode from "vscode";
import {
  UNKNOWN_TOOL_CODE,
  validateDocument,
  ValidationSummary,
} from "./diagnostics";
import {
  cwdForFile,
  getExecutablePath,
  JarvyNotFoundError,
  runJarvy,
  warnBinaryMissing,
} from "./jarvyCli";

/** True when `document` is a jarvy.toml config we should validate. */
function isJarvyConfig(document: vscode.TextDocument): boolean {
  if (document.languageId === "jarvy-toml") {
    return true;
  }
  return path.basename(document.uri.fsPath) === "jarvy.toml";
}

/** Manages per-document debounce timers for on-change validation. */
class Debouncer {
  private readonly timers = new Map<string, NodeJS.Timeout>();

  schedule(key: string, delayMs: number, fn: () => void): void {
    this.cancel(key);
    this.timers.set(
      key,
      setTimeout(() => {
        this.timers.delete(key);
        fn();
      }, delayMs),
    );
  }

  cancel(key: string): void {
    const existing = this.timers.get(key);
    if (existing) {
      clearTimeout(existing);
      this.timers.delete(key);
    }
  }

  dispose(): void {
    for (const timer of this.timers.values()) {
      clearTimeout(timer);
    }
    this.timers.clear();
  }
}

/** Quick-fix provider offering "Run jarvy setup" on unknown-tool diagnostics. */
class JarvyCodeActionProvider implements vscode.CodeActionProvider {
  static readonly providedCodeActionKinds = [vscode.CodeActionKind.QuickFix];

  provideCodeActions(
    _document: vscode.TextDocument,
    _range: vscode.Range | vscode.Selection,
    context: vscode.CodeActionContext,
  ): vscode.CodeAction[] {
    const actions: vscode.CodeAction[] = [];
    for (const diagnostic of context.diagnostics) {
      if (diagnostic.code !== UNKNOWN_TOOL_CODE) {
        continue;
      }
      const action = new vscode.CodeAction(
        "Run jarvy setup to provision configured tools",
        vscode.CodeActionKind.QuickFix,
      );
      action.command = {
        command: "jarvy.setup",
        title: "Run jarvy setup",
      };
      action.diagnostics = [diagnostic];
      actions.push(action);
      // One action is enough even if several unknown-tool diagnostics overlap.
      break;
    }
    return actions;
  }
}

let diagnostics: vscode.DiagnosticCollection;
let statusBar: vscode.StatusBarItem;
const debouncer = new Debouncer();

function strictEnabled(): boolean {
  return vscode.workspace
    .getConfiguration("jarvy")
    .get<boolean>("validate.strict", true);
}

function debounceMs(): number {
  const value = vscode.workspace
    .getConfiguration("jarvy")
    .get<number>("validate.debounceMs", 500);
  return Number.isFinite(value) && value >= 0 ? value : 500;
}

function renderStatus(summary: ValidationSummary | undefined): void {
  if (!summary) {
    statusBar.hide();
    return;
  }
  const { status, errorCount, warningCount } = summary;
  switch (status) {
    case "valid":
      statusBar.text = "$(check) Jarvy";
      statusBar.tooltip = warningCount > 0
        ? `jarvy.toml is valid (${warningCount} warning(s))`
        : "jarvy.toml is valid";
      statusBar.backgroundColor = undefined;
      break;
    case "invalid":
      statusBar.text = `$(error) Jarvy: ${errorCount}`;
      statusBar.tooltip = `jarvy.toml has ${errorCount} error(s), ${warningCount} warning(s)`;
      statusBar.backgroundColor = new vscode.ThemeColor(
        "statusBarItem.errorBackground",
      );
      break;
    case "no-config":
      statusBar.text = "$(circle-slash) Jarvy: no config";
      statusBar.tooltip = "No jarvy.toml found in this workspace";
      statusBar.backgroundColor = undefined;
      break;
    case "unknown":
    default:
      statusBar.text = "$(question) Jarvy";
      statusBar.tooltip = "jarvy status unknown (is the jarvy CLI installed?)";
      statusBar.backgroundColor = undefined;
      break;
  }
  statusBar.command = "jarvy.validate";
  statusBar.show();
}

async function validateAndReport(document: vscode.TextDocument): Promise<void> {
  if (!isJarvyConfig(document)) {
    return;
  }
  const summary = await validateDocument(document, diagnostics, strictEnabled());
  renderStatus(summary);
}

/** Find the active jarvy.toml: the active editor's, else the first in workspace. */
async function findActiveConfig(): Promise<vscode.TextDocument | undefined> {
  const active = vscode.window.activeTextEditor?.document;
  if (active && isJarvyConfig(active)) {
    return active;
  }
  const opened = vscode.workspace.textDocuments.find(isJarvyConfig);
  if (opened) {
    return opened;
  }
  const found = await vscode.workspace.findFiles("**/jarvy.toml", "**/node_modules/**", 1);
  if (found.length === 0) {
    return undefined;
  }
  return vscode.workspace.openTextDocument(found[0]);
}

async function commandValidate(): Promise<void> {
  const document = await findActiveConfig();
  if (!document) {
    renderStatus({ status: "no-config", errorCount: 0, warningCount: 0 });
    void vscode.window.showInformationMessage("Jarvy: no jarvy.toml found in this workspace.");
    return;
  }
  await validateAndReport(document);
}

async function commandSetup(): Promise<void> {
  const document = await findActiveConfig();
  const executable = getExecutablePath();
  const terminal = vscode.window.createTerminal({ name: "jarvy setup" });
  terminal.show();
  if (document) {
    terminal.sendText(`${executable} setup --file ${quoteArg(document.uri.fsPath)}`);
  } else {
    terminal.sendText(`${executable} setup`);
  }
}

async function commandDoctor(output: vscode.OutputChannel): Promise<void> {
  const document = await findActiveConfig();
  const cwd = document ? cwdForFile(document.uri) : workspaceCwd();
  output.clear();
  output.show(true);
  output.appendLine("Running jarvy doctor...");
  try {
    const result = await runJarvy(["doctor", "--format", "json"], cwd);
    if (result.stdout.trim().length > 0) {
      output.appendLine(result.stdout.trimEnd());
    }
    if (result.stderr.trim().length > 0) {
      output.appendLine("--- stderr ---");
      output.appendLine(result.stderr.trimEnd());
    }
    output.appendLine(`\njarvy doctor exited with code ${result.code ?? "unknown"}.`);
  } catch (err) {
    if (err instanceof JarvyNotFoundError) {
      await warnBinaryMissing(err.executable);
      output.appendLine("jarvy executable not found.");
      return;
    }
    output.appendLine(`Failed to run jarvy doctor: ${err instanceof Error ? err.message : String(err)}`);
  }
}

function workspaceCwd(): string | undefined {
  return vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
}

/** Minimal shell-safe quoting for a file path passed to the integrated terminal. */
function quoteArg(value: string): string {
  if (/^[A-Za-z0-9_./\\:-]+$/.test(value)) {
    return value;
  }
  return `"${value.replace(/(["\\$`])/g, "\\$1")}"`;
}

export function activate(context: vscode.ExtensionContext): void {
  diagnostics = vscode.languages.createDiagnosticCollection("jarvy");
  statusBar = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
  const doctorOutput = vscode.window.createOutputChannel("Jarvy Doctor");

  context.subscriptions.push(
    diagnostics,
    statusBar,
    doctorOutput,
    debouncer,
    vscode.commands.registerCommand("jarvy.validate", () => void commandValidate()),
    vscode.commands.registerCommand("jarvy.setup", () => void commandSetup()),
    vscode.commands.registerCommand("jarvy.doctor", () => void commandDoctor(doctorOutput)),
    vscode.languages.registerCodeActionsProvider(
      [{ language: "jarvy-toml" }, { pattern: "**/jarvy.toml" }],
      new JarvyCodeActionProvider(),
      { providedCodeActionKinds: JarvyCodeActionProvider.providedCodeActionKinds },
    ),
    vscode.workspace.onDidSaveTextDocument((document) => {
      const onSave = vscode.workspace
        .getConfiguration("jarvy")
        .get<boolean>("validate.onSave", true);
      if (onSave && isJarvyConfig(document)) {
        void validateAndReport(document);
      }
    }),
    vscode.workspace.onDidChangeTextDocument((event) => {
      const onChange = vscode.workspace
        .getConfiguration("jarvy")
        .get<boolean>("validate.onChange", true);
      const document = event.document;
      if (!onChange || !isJarvyConfig(document)) {
        return;
      }
      debouncer.schedule(document.uri.toString(), debounceMs(), () => {
        void validateAndReport(document);
      });
    }),
    vscode.workspace.onDidCloseTextDocument((document) => {
      if (isJarvyConfig(document)) {
        debouncer.cancel(document.uri.toString());
      }
    }),
    vscode.window.onDidChangeActiveTextEditor((editor) => {
      if (editor && isJarvyConfig(editor.document)) {
        void validateAndReport(editor.document);
      }
    }),
  );

  // Validate any jarvy.toml already open at activation, or discover one.
  void commandValidate();
}

export function deactivate(): void {
  debouncer.dispose();
}
