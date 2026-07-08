import { execFile } from "child_process";
import * as path from "path";
import * as vscode from "vscode";

/** Documentation shown when the jarvy binary cannot be found. */
export const INSTALL_DOCS_URL = "https://github.com/Cliftonz/jarvy#installation";

/** Result of running a jarvy subprocess. */
export interface JarvyExecResult {
  readonly stdout: string;
  readonly stderr: string;
  /** Process exit code (`null` if the process was killed by a signal). */
  readonly code: number | null;
}

/** Raised when the configured jarvy executable is not on PATH. */
export class JarvyNotFoundError extends Error {
  constructor(public readonly executable: string) {
    super(`jarvy executable not found: ${executable}`);
    this.name = "JarvyNotFoundError";
  }
}

/** Read the user-configured path to the jarvy executable. */
export function getExecutablePath(): string {
  const configured = vscode.workspace
    .getConfiguration("jarvy")
    .get<string>("executablePath");
  const trimmed = configured?.trim();
  return trimmed && trimmed.length > 0 ? trimmed : "jarvy";
}

/**
 * Run `jarvy <args>` and resolve with captured output. A non-zero exit code
 * does NOT reject — jarvy uses exit codes to signal validation state, so the
 * caller inspects `code`/`stdout`. Only spawn failures reject: an ENOENT
 * (binary missing) becomes a {@link JarvyNotFoundError}.
 */
export function runJarvy(
  args: readonly string[],
  cwd: string | undefined,
): Promise<JarvyExecResult> {
  const executable = getExecutablePath();
  return new Promise<JarvyExecResult>((resolve, reject) => {
    execFile(
      executable,
      [...args],
      {
        cwd,
        // jarvy JSON payloads are small; 16 MiB is ample headroom.
        maxBuffer: 16 * 1024 * 1024,
        windowsHide: true,
      },
      (error, stdout, stderr) => {
        if (error) {
          const code = (error as NodeJS.ErrnoException).code;
          if (code === "ENOENT") {
            reject(new JarvyNotFoundError(executable));
            return;
          }
          // Non-zero exit surfaces here as an Error carrying `.code` (a
          // number). Treat it as a normal completion — jarvy signals via
          // exit codes, not spawn failure.
          const exitCode =
            typeof (error as { code?: unknown }).code === "number"
              ? ((error as { code: number }).code)
              : null;
          if (exitCode !== null) {
            resolve({ stdout, stderr, code: exitCode });
            return;
          }
          reject(error);
          return;
        }
        resolve({ stdout, stderr, code: 0 });
      },
    );
  });
}

/**
 * Extract the first top-level JSON object from mixed stdout. jarvy prints only
 * the JSON on the validate path, but logs or a stray banner could precede it;
 * scanning for the first balanced `{...}` keeps parsing robust.
 */
export function extractJsonObject(stdout: string): unknown | undefined {
  const start = stdout.indexOf("{");
  if (start < 0) {
    return undefined;
  }
  // Fast path: the whole (trimmed) payload is JSON.
  const trimmed = stdout.trim();
  try {
    return JSON.parse(trimmed);
  } catch {
    // Fall through to balanced-brace scan below.
  }
  let depth = 0;
  let inString = false;
  let escaped = false;
  for (let i = start; i < stdout.length; i++) {
    const ch = stdout[i];
    if (inString) {
      if (escaped) {
        escaped = false;
      } else if (ch === "\\") {
        escaped = true;
      } else if (ch === '"') {
        inString = false;
      }
      continue;
    }
    if (ch === '"') {
      inString = true;
    } else if (ch === "{") {
      depth++;
    } else if (ch === "}") {
      depth--;
      if (depth === 0) {
        const candidate = stdout.slice(start, i + 1);
        try {
          return JSON.parse(candidate);
        } catch {
          return undefined;
        }
      }
    }
  }
  return undefined;
}

/** Show a warning with a link to install docs when jarvy is missing. */
export async function warnBinaryMissing(
  executable: string,
): Promise<void> {
  const openDocs = "Install Instructions";
  const choice = await vscode.window.showWarningMessage(
    `Jarvy: could not find the '${executable}' executable on your PATH. ` +
      "Install it or set 'jarvy.executablePath' in your settings.",
    openDocs,
  );
  if (choice === openDocs) {
    await vscode.env.openExternal(vscode.Uri.parse(INSTALL_DOCS_URL));
  }
}

/**
 * Working directory for a jarvy invocation targeting `file`: the containing
 * workspace folder if any, else the file's own directory.
 */
export function cwdForFile(file: vscode.Uri): string {
  const folder = vscode.workspace.getWorkspaceFolder(file);
  if (folder) {
    return folder.uri.fsPath;
  }
  return path.dirname(file.fsPath);
}
