package com.jarvy.intellij.actions

import com.intellij.notification.NotificationType
import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.progress.ProgressIndicator
import com.intellij.openapi.progress.Task
import com.jarvy.intellij.JarvyCli
import com.jarvy.intellij.JarvyConfig
import com.jarvy.intellij.JarvyNotifier
import java.io.File

/**
 * Runs `jarvy setup --file <jarvy.toml>` in the background.
 *
 * Jarvy detects the absence of a TTY and runs non-interactively, so it won't
 * block on prompts. The (potentially long) install is capped at 30 minutes.
 */
class SetupAction : AnAction() {
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

        object : Task.Backgroundable(project, "Running jarvy setup", true) {
            override fun run(indicator: ProgressIndicator) {
                indicator.isIndeterminate = true
                val result = JarvyCli.run(
                    listOf("setup", "--file", configPath),
                    File(configPath).parent,
                    timeoutMs = 30 * 60 * 1000,
                )
                val type = if (result.exitCode == 0) {
                    NotificationType.INFORMATION
                } else {
                    NotificationType.ERROR
                }
                val combined = (result.stdout + "\n" + result.stderr).trim()
                val tail = combined.takeLast(1500).ifBlank { "Completed." }
                JarvyNotifier.notify(project, "Jarvy setup (exit ${result.exitCode})", tail, type)
            }
        }.queue()
    }

    private companion object {
        const val NO_BINARY_MESSAGE =
            "The 'jarvy' executable was not found on your PATH. " +
                "Install Jarvy or set the JARVY_BINARY environment variable."
    }
}
