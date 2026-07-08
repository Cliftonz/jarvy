package com.jarvy.intellij

import com.intellij.execution.configurations.GeneralCommandLine
import com.intellij.execution.configurations.PathEnvironmentVariableUtil
import com.intellij.execution.process.ProcessOutput
import com.intellij.execution.util.ExecUtil
import com.intellij.openapi.diagnostic.logger
import java.nio.charset.StandardCharsets

/**
 * Thin wrapper around the `jarvy` executable.
 *
 * Locates the binary on the user's `PATH` (or via the `JARVY_BINARY`
 * environment override) and runs it with [GeneralCommandLine], capturing
 * stdout / stderr / exit code. All calls are blocking and MUST be invoked
 * off the EDT (e.g. from a `Task.Backgroundable` or an ExternalAnnotator's
 * `doAnnotate`).
 */
object JarvyCli {
    private val LOG = logger<JarvyCli>()

    const val EXECUTABLE_NAME = "jarvy"

    /** Result of a single `jarvy` invocation. */
    data class Result(
        val exitCode: Int,
        val stdout: String,
        val stderr: String,
        val timedOut: Boolean,
    )

    /**
     * Resolve the absolute path to the `jarvy` binary, or `null` if it can't
     * be found. Honors the `JARVY_BINARY` environment override first, then
     * performs a PATH lookup (the IDE's cross-platform `which`/`where`).
     */
    fun locate(): String? {
        System.getenv("JARVY_BINARY")?.takeIf { it.isNotBlank() }?.let { return it }
        val onPath = PathEnvironmentVariableUtil.findInPath(EXECUTABLE_NAME)
        return onPath?.absolutePath
    }

    fun isAvailable(): Boolean = locate() != null

    /**
     * Run `jarvy <args>` in [workDir] (or the process default when null).
     *
     * @param timeoutMs hard cap; on expiry the process is killed and
     *   [Result.timedOut] is `true`.
     */
    fun run(args: List<String>, workDir: String?, timeoutMs: Int = 120_000): Result {
        val exe = locate() ?: return Result(
            exitCode = -1,
            stdout = "",
            stderr = "The '$EXECUTABLE_NAME' executable was not found on PATH.",
            timedOut = false,
        )

        val commandLine = GeneralCommandLine(exe)
            .withParameters(args)
            .withCharset(StandardCharsets.UTF_8)
        if (workDir != null) {
            commandLine.withWorkDirectory(workDir)
        }

        return try {
            val output: ProcessOutput = ExecUtil.execAndGetOutput(commandLine, timeoutMs)
            Result(
                exitCode = output.exitCode,
                stdout = output.stdout,
                stderr = output.stderr,
                timedOut = output.isTimeout,
            )
        } catch (e: Exception) {
            LOG.warn("Failed to run jarvy ${args.joinToString(" ")}", e)
            Result(exitCode = -1, stdout = "", stderr = e.message.orEmpty(), timedOut = false)
        }
    }
}
