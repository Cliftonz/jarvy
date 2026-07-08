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

/** Runs `jarvy doctor` and reports a summary balloon. */
class DoctorAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project
        if (!JarvyCli.isAvailable()) {
            JarvyNotifier.notify(project, "Jarvy", NO_BINARY_MESSAGE, NotificationType.ERROR)
            return
        }
        val base = project?.basePath

        object : Task.Backgroundable(project, "Running jarvy doctor", true) {
            override fun run(indicator: ProgressIndicator) {
                indicator.isIndeterminate = true
                val args = mutableListOf("doctor")
                val config = base?.let { File(it, JarvyConfig.CONFIG_FILE_NAME) }
                if (config != null && config.isFile) {
                    args += listOf("--file", config.absolutePath)
                }
                val result = JarvyCli.run(args, base)
                val type = if (result.exitCode == 0) {
                    NotificationType.INFORMATION
                } else {
                    NotificationType.WARNING
                }
                val combined = (result.stdout + "\n" + result.stderr).trim()
                val tail = combined.takeLast(1500).ifBlank { "No output." }
                JarvyNotifier.notify(project, "Jarvy doctor (exit ${result.exitCode})", tail, type)
            }
        }.queue()
    }

    private companion object {
        const val NO_BINARY_MESSAGE =
            "The 'jarvy' executable was not found on your PATH. " +
                "Install Jarvy or set the JARVY_BINARY environment variable."
    }
}
