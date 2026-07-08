// Types mirroring the JSON emitted by `jarvy validate --format json`.
//
// Source of truth: src/commands/validate.rs in the jarvy repo. The
// `ValidationResult` struct serializes (serde, pretty-printed, no envelope)
// as the shape below. `severity` is `#[serde(rename_all = "lowercase")]`, so
// it is one of "error" | "warning" | "info". `line` (1-based) and
// `suggestion` are `Option`s emitted with `skip_serializing_if = "Option::is_none"`,
// so they are absent — not null — when unset. There is NO column information.

/** Severity of a single validation issue, as emitted by jarvy. */
export type JarvySeverity = "error" | "warning" | "info";

/** A single validation issue from `jarvy validate --format json`. */
export interface JarvyValidationIssue {
  readonly severity: JarvySeverity;
  readonly message: string;
  /** 1-based line number. Absent when jarvy could not attribute a line. */
  readonly line?: number;
  /** Optional remediation hint. */
  readonly suggestion?: string;
}

/** Top-level result of `jarvy validate --format json`. */
export interface JarvyValidationResult {
  readonly path: string;
  readonly valid: boolean;
  readonly error_count: number;
  readonly warning_count: number;
  readonly issues: readonly JarvyValidationIssue[];
}

/**
 * Narrow an unknown parsed value into a `JarvyValidationResult`.
 * Returns `undefined` when the shape does not match, so callers can fall
 * back to a file-level diagnostic instead of trusting a malformed payload.
 */
export function parseValidationResult(
  value: unknown,
): JarvyValidationResult | undefined {
  if (typeof value !== "object" || value === null) {
    return undefined;
  }
  const obj = value as Record<string, unknown>;
  if (typeof obj.valid !== "boolean" || !Array.isArray(obj.issues)) {
    return undefined;
  }
  const issues: JarvyValidationIssue[] = [];
  for (const raw of obj.issues) {
    if (typeof raw !== "object" || raw === null) {
      continue;
    }
    const issue = raw as Record<string, unknown>;
    const severity = issue.severity;
    if (
      severity !== "error" &&
      severity !== "warning" &&
      severity !== "info"
    ) {
      continue;
    }
    const message =
      typeof issue.message === "string" ? issue.message : "Validation issue";
    const line =
      typeof issue.line === "number" && Number.isFinite(issue.line)
        ? issue.line
        : undefined;
    const suggestion =
      typeof issue.suggestion === "string" ? issue.suggestion : undefined;
    issues.push({ severity, message, line, suggestion });
  }
  return {
    path: typeof obj.path === "string" ? obj.path : "",
    valid: obj.valid,
    error_count:
      typeof obj.error_count === "number" ? obj.error_count : 0,
    warning_count:
      typeof obj.warning_count === "number" ? obj.warning_count : 0,
    issues,
  };
}
