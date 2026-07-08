package com.jarvy.intellij.actions

import com.intellij.notification.NotificationType
import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.progress.ProgressIndicator
import com.intellij.openapi.progress.Task
import com.jarvy.intellij.JarvyCli
import com.jarvy.intellij.JarvyConfig
import com.jarvy.intellij.JarvyNotifier
import com.jarvy.intellij.validation.JarvyValidationParser
import java.io.File

/** Runs `jarvy validate --format json` and reports the issues as a balloon. */
class ValidateAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project
        val configPath = JarvyConfig.resolveConfigPath(e)
        if (configPath == null || !File(configPath).isFile) {
            JarvyNotifier.notify(
                project,
                "Jarvy",
                "No ${JarvyConfig.CONFIG_FILE_NAME} found in this project.",
                NotificationType.WARNING,
            )
            return
        }
        if (!JarvyCli.isAvailable()) {
            JarvyNotifier.notify(project, "Jarvy", NO_BINARY_MESSAGE, NotificationType.ERROR)
            return
        }

        object : Task.Backgroundable(project, "Validating ${JarvyConfig.CONFIG_FILE_NAME}", true) {
            override fun run(indicator: ProgressIndicator) {
                indicator.isIndeterminate = true
                val result = JarvyCli.run(
                    listOf("validate", "--file", configPath, "--format", "json"),
                    File(configPath).parent,
                )
                val parsed = JarvyValidationParser.parse(result.stdout)
                if (parsed == null) {
                    JarvyNotifier.notify(
                        project,
                        "Jarvy validate",
                        "Could not parse validation output.\n${result.stderr.take(500)}",
                        NotificationType.ERROR,
                    )
                    return
                }

                val type = when {
                    parsed.errorCount > 0 -> NotificationType.ERROR
                    parsed.warningCount > 0 -> NotificationType.WARNING
                    else -> NotificationType.INFORMATION
                }
                val content = if (parsed.issues.isEmpty()) {
                    "Configuration is valid."
                } else {
                    val lines = parsed.issues.take(15).joinToString("\n") { issue ->
                        val loc = issue.line?.let { "L$it: " } ?: ""
                        "[${issue.severity}] $loc${issue.message}"
                    }
                    val more = (parsed.issues.size - 15).coerceAtLeast(0)
                    val suffix = if (more > 0) "\n… and $more more" else ""
                    "${parsed.errorCount} error(s), ${parsed.warningCount} warning(s)\n$lines$suffix"
                }
                JarvyNotifier.notify(project, "Jarvy validate", content, type)
            }
        }.queue()
    }

    private companion object {
        const val NO_BINARY_MESSAGE =
            "The 'jarvy' executable was not found on your PATH. " +
                "Install Jarvy or set the JARVY_BINARY environment variable."
    }
}
