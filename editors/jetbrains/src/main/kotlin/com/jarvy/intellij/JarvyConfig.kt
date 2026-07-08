package com.jarvy.intellij

import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.actionSystem.CommonDataKeys
import java.io.File

/** Locates the project's `jarvy.toml`. */
object JarvyConfig {
    const val CONFIG_FILE_NAME = "jarvy.toml"

    /**
     * Best-effort resolution of the `jarvy.toml` to act on:
     *  1. the file the action was invoked on, if it is a `jarvy.toml`; else
     *  2. `<project base>/jarvy.toml`.
     *
     * The returned path is NOT guaranteed to exist — callers decide whether a
     * missing file is an error.
     */
    fun resolveConfigPath(e: AnActionEvent): String? {
        val selected = e.getData(CommonDataKeys.VIRTUAL_FILE)
        if (selected != null && selected.name.equals(CONFIG_FILE_NAME, ignoreCase = true)) {
            return selected.path
        }
        val base = e.project?.basePath ?: return null
        return File(base, CONFIG_FILE_NAME).absolutePath
    }
}
