package com.jarvy.intellij.validation

import com.google.gson.Gson
import com.google.gson.annotations.SerializedName

/**
 * Kotlin mirror of the JSON emitted by `jarvy validate --format json`.
 *
 * Schema source of truth: `src/commands/validate.rs` — `ValidationResult` /
 * `ValidationIssue` in the Jarvy Rust CLI (serialized directly, no envelope):
 *
 * ```json
 * {
 *   "path": "./jarvy.toml",
 *   "valid": true,
 *   "error_count": 0,
 *   "warning_count": 0,
 *   "issues": [
 *     { "severity": "error|warning|info", "message": "...",
 *       "line": 5, "suggestion": "..." }
 *   ]
 * }
 * ```
 * `line` and `suggestion` are omitted by the CLI when absent
 * (`skip_serializing_if = "Option::is_none"`).
 */
data class JarvyValidationResult(
    @SerializedName("path") val path: String = "",
    @SerializedName("valid") val valid: Boolean = false,
    @SerializedName("error_count") val errorCount: Int = 0,
    @SerializedName("warning_count") val warningCount: Int = 0,
    @SerializedName("issues") val issues: List<JarvyIssue> = emptyList(),
)

data class JarvyIssue(
    /** "error" | "warning" | "info" (serde `rename_all = "lowercase"`). */
    @SerializedName("severity") val severity: String = "info",
    @SerializedName("message") val message: String = "",
    /** 1-based line number, or null when the issue is not line-specific. */
    @SerializedName("line") val line: Int? = null,
    @SerializedName("suggestion") val suggestion: String? = null,
)

/** Parses `jarvy validate --format json` output; returns null on failure. */
object JarvyValidationParser {
    private val gson = Gson()

    fun parse(json: String): JarvyValidationResult? = try {
        gson.fromJson(json, JarvyValidationResult::class.java)
    } catch (_: Exception) {
        null
    }
}
