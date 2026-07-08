package com.jarvy.intellij.validation

import com.intellij.lang.annotation.AnnotationHolder
import com.intellij.lang.annotation.ExternalAnnotator
import com.intellij.lang.annotation.HighlightSeverity
import com.intellij.openapi.editor.Document
import com.intellij.openapi.editor.Editor
import com.intellij.openapi.fileEditor.FileDocumentManager
import com.intellij.openapi.util.TextRange
import com.intellij.psi.PsiFile
import com.jarvy.intellij.JarvyCli
import java.io.File
import java.nio.file.Files

/**
 * Runs `jarvy validate --format json` against the in-editor content of a
 * `jarvy.toml` and surfaces the reported issues as editor annotations.
 *
 * ExternalAnnotator has three phases:
 *  - [collectInformation] (EDT, read access): snapshot the buffer text.
 *  - [doAnnotate] (background thread): shell out to `jarvy` — the slow part.
 *  - [apply] (EDT, read access): translate the parsed result into annotations.
 *
 * The live buffer is written to a temp file so unsaved edits are validated
 * too; the reported `line` numbers therefore line up with the editor.
 */
class JarvyExternalAnnotator :
    ExternalAnnotator<JarvyExternalAnnotator.CollectedInfo, JarvyValidationResult>() {

    data class CollectedInfo(val fileName: String, val text: String, val originalPath: String?)

    override fun collectInformation(file: PsiFile, editor: Editor, hasErrors: Boolean): CollectedInfo? {
        val virtualFile = file.virtualFile ?: return null
        if (!isJarvyConfig(virtualFile.name)) return null
        return CollectedInfo(virtualFile.name, editor.document.text, virtualFile.path)
    }

    override fun doAnnotate(collectedInfo: CollectedInfo?): JarvyValidationResult? {
        val info = collectedInfo ?: return null
        if (!JarvyCli.isAvailable()) return null

        val tempFile = Files.createTempFile("jarvy-validate-", ".toml")
        return try {
            Files.write(tempFile, info.text.toByteArray(Charsets.UTF_8))
            val workDir = info.originalPath?.let { File(it).parent }
            val result = JarvyCli.run(
                listOf("validate", "--file", tempFile.toString(), "--format", "json"),
                workDir,
            )
            if (result.stdout.isBlank()) null else JarvyValidationParser.parse(result.stdout)
        } catch (_: Exception) {
            null
        } finally {
            runCatching { Files.deleteIfExists(tempFile) }
        }
    }

    override fun apply(file: PsiFile, annotationResult: JarvyValidationResult?, holder: AnnotationHolder) {
        val result = annotationResult ?: return
        val virtualFile = file.virtualFile ?: return
        val document: Document = FileDocumentManager.getInstance().getDocument(virtualFile) ?: return

        for (issue in result.issues) {
            val severity = when (issue.severity.lowercase()) {
                "error" -> HighlightSeverity.ERROR
                "warning" -> HighlightSeverity.WARNING
                else -> HighlightSeverity.WEAK_WARNING
            }
            val message = buildString {
                append("Jarvy: ")
                append(issue.message)
                issue.suggestion?.let { append(" (suggestion: ").append(it).append(')') }
            }

            val line = issue.line
            if (line != null && line in 1..document.lineCount) {
                val start = document.getLineStartOffset(line - 1)
                val end = document.getLineEndOffset(line - 1)
                holder.newAnnotation(severity, message)
                    .range(TextRange(start, end))
                    .create()
            } else {
                // No usable line: surface as a file-level banner annotation.
                holder.newAnnotation(severity, message)
                    .fileLevel()
                    .create()
            }
        }
    }

    private fun isJarvyConfig(name: String): Boolean =
        name.equals("jarvy.toml", ignoreCase = true)
}
