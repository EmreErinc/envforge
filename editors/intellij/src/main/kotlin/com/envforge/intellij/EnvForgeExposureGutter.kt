package com.envforge.intellij

import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.editor.Editor
import com.intellij.openapi.editor.event.DocumentEvent
import com.intellij.openapi.editor.event.DocumentListener
import com.intellij.openapi.editor.event.EditorFactoryEvent
import com.intellij.openapi.editor.event.EditorFactoryListener
import com.intellij.openapi.editor.markup.GutterIconRenderer
import com.intellij.openapi.editor.markup.HighlighterLayer
import com.intellij.openapi.editor.markup.RangeHighlighter
import com.intellij.openapi.editor.markup.TextAttributes
import com.intellij.openapi.fileEditor.FileDocumentManager
import com.intellij.openapi.project.Project
import com.intellij.ui.JBColor
import java.awt.Color
import java.awt.Component
import java.awt.Graphics
import java.awt.Graphics2D
import java.awt.RenderingHints
import java.util.concurrent.ConcurrentHashMap
import javax.swing.Icon

/**
 * IDE-side renderer for the AI-exposure heatmap. Mirrors the VS Code
 * `ExposureRenderer` behavior: a colored circle in the editor gutter
 * for every env-var line in a `.env*` file, plus a hover tooltip
 * quoting the classification reason.
 *
 * Data source: `envforge exposure --file PATH --json` subprocess.
 * We avoid wiring through lsp4ij's custom-request bridge to keep the
 * dependency surface small; the CLI subcommand reuses the exact same
 * Rust classification function the LSP uses, so plugin parity holds.
 */
class EnvForgeExposureEditorListener : EditorFactoryListener {

    private val highlighters: MutableMap<Editor, List<RangeHighlighter>> =
        ConcurrentHashMap()
    private val documentListeners: MutableMap<Editor, DocumentListener> =
        ConcurrentHashMap()
    private val refreshTimers: MutableMap<Editor, java.util.concurrent.ScheduledFuture<*>> =
        ConcurrentHashMap()
    private val scheduler =
        java.util.concurrent.Executors.newScheduledThreadPool(1) { r ->
            Thread(r, "envforge-exposure-refresh").apply { isDaemon = true }
        }

    override fun editorCreated(event: EditorFactoryEvent) {
        val editor = event.editor
        val project = editor.project ?: return
        val file = FileDocumentManager.getInstance().getFile(editor.document) ?: return
        if (!isEnvFile(file.name)) return

        scheduleRefresh(editor, project)
        val listener =
            object : DocumentListener {
                override fun documentChanged(event: DocumentEvent) {
                    scheduleRefresh(editor, project)
                }
            }
        editor.document.addDocumentListener(listener)
        documentListeners[editor] = listener
    }

    override fun editorReleased(event: EditorFactoryEvent) {
        val editor = event.editor
        documentListeners.remove(editor)?.let { editor.document.removeDocumentListener(it) }
        refreshTimers.remove(editor)?.cancel(false)
        clearHighlighters(editor)
    }

    /// Debounce LSP-equivalent subprocess calls by 250 ms so a fast
    /// keystroke burst doesn't spawn one process per character.
    private fun scheduleRefresh(editor: Editor, project: Project) {
        refreshTimers.remove(editor)?.cancel(false)
        val future = scheduler.schedule({
            refresh(editor, project)
        }, 250, java.util.concurrent.TimeUnit.MILLISECONDS)
        refreshTimers[editor] = future
    }

    private fun refresh(editor: Editor, project: Project) {
        val file = FileDocumentManager.getInstance().getFile(editor.document) ?: return
        val path = file.path
        val binary = EnvForgeLspFactory.findEnvforgeBinary()

        val output: String = try {
            val proc = ProcessBuilder(binary, "exposure", "--file", path)
                .directory(project.basePath?.let { java.io.File(it) })
                .redirectErrorStream(true)
                .start()
            val out = proc.inputStream.bufferedReader().readText()
            proc.waitFor()
            if (proc.exitValue() != 0) return
            out
        } catch (_: Exception) {
            return
        }

        val entries: List<ExposureEntry> = try {
            parseExposureResponse(output)
        } catch (_: Exception) {
            return
        }

        ApplicationManager.getApplication().invokeLater {
            applyHighlighters(editor, entries)
        }
    }

    private fun applyHighlighters(editor: Editor, entries: List<ExposureEntry>) {
        clearHighlighters(editor)

        val markup = editor.markupModel
        val attrs = TextAttributes()
        val added = mutableListOf<RangeHighlighter>()
        val docLineCount = editor.document.lineCount

        for (entry in entries) {
            if (entry.line >= docLineCount) continue
            val color = colorFor(entry.level)
            val highlighter: RangeHighlighter = markup.addLineHighlighter(
                entry.line,
                HighlighterLayer.LAST,
                attrs,
            )
            val tooltipBanner = if (entry.canary) {
                "EnvForge: ${entry.level.uppercase()} · CANARY ACTIVE"
            } else {
                "EnvForge AI Exposure: ${entry.level.uppercase()}"
            }
            highlighter.gutterIconRenderer = ExposureGutterIconRenderer(
                color = color,
                tooltip = "$tooltipBanner\n${entry.reason}",
                canary = entry.canary,
            )
            highlighter.setErrorStripeMarkColor(color)
            highlighter.errorStripeTooltip = entry.reason
            added.add(highlighter)
        }
        highlighters[editor] = added
    }

    private fun clearHighlighters(editor: Editor) {
        val markup = editor.markupModel
        highlighters.remove(editor)?.forEach { markup.removeHighlighter(it) }
    }

    private fun colorFor(level: String): Color = when (level.lowercase()) {
        "red" -> JBColor(Color(0xD3, 0x2F, 0x2F), Color(0xEF, 0x53, 0x50))
        "amber" -> JBColor(Color(0xF9, 0xA8, 0x25), Color(0xFF, 0xC1, 0x07))
        "green" -> JBColor(Color(0x2E, 0x7D, 0x32), Color(0x66, 0xBB, 0x6A))
        else -> JBColor.GRAY
    }

    private fun isEnvFile(name: String): Boolean =
        name == ".env" || name.startsWith(".env.") || name.endsWith(".env")

    private fun parseExposureResponse(json: String): List<ExposureEntry> {
        val obj = com.google.gson.JsonParser.parseString(json).asJsonObject
        val arr = obj.getAsJsonArray("entries") ?: return emptyList()
        val out = mutableListOf<ExposureEntry>()
        for (i in 0 until arr.size()) {
            val e = arr[i].asJsonObject
            val line = e.get("line")?.asInt ?: continue
            val key = e.get("key")?.asString ?: ""
            val level = e.get("level")?.asString ?: continue
            val reason = e.get("reason")?.asString ?: ""
            val canary = e.get("canary")?.asBoolean ?: false
            out.add(ExposureEntry(line, key, level, reason, canary))
        }
        return out
    }
}

private data class ExposureEntry(
    val line: Int,
    val key: String,
    val level: String,
    val reason: String,
    val canary: Boolean,
)

/**
 * Renders the colored gutter glyph. We paint a filled circle directly
 * rather than ship SVG assets — keeps the plugin bundle small and
 * lets the dot pick up the active JetBrains theme via `JBColor`.
 */
private class ExposureGutterIconRenderer(
    private val color: Color,
    private val tooltip: String,
    private val canary: Boolean,
) : GutterIconRenderer() {
    override fun getIcon(): Icon =
        if (canary) ShieldIcon(color) else CircleIcon(color)
    override fun getTooltipText(): String = tooltip
    override fun equals(other: Any?): Boolean =
        other is ExposureGutterIconRenderer &&
            other.color == color &&
            other.tooltip == tooltip &&
            other.canary == canary
    override fun hashCode(): Int {
        var h = color.hashCode()
        h = 31 * h + tooltip.hashCode()
        h = 31 * h + canary.hashCode()
        return h
    }
}

private class CircleIcon(private val color: Color) : Icon {
    override fun paintIcon(c: Component?, g: Graphics, x: Int, y: Int) {
        val g2 = g.create() as Graphics2D
        try {
            g2.setRenderingHint(
                RenderingHints.KEY_ANTIALIASING,
                RenderingHints.VALUE_ANTIALIAS_ON,
            )
            g2.color = color
            g2.fillOval(x + 3, y + 3, 10, 10)
        } finally {
            g2.dispose()
        }
    }
    override fun getIconWidth(): Int = 16
    override fun getIconHeight(): Int = 16
}

/**
 * Shield-shaped gutter glyph used to mark a line where a canary
 * tripwire is registered. Visually distinct from the plain dot so the
 * "this line has a tripwire" status reads at a glance, while keeping
 * the threat-tier color encoding consistent with non-canary lines.
 */
private class ShieldIcon(private val color: Color) : Icon {
    override fun paintIcon(c: Component?, g: Graphics, x: Int, y: Int) {
        val g2 = g.create() as Graphics2D
        try {
            g2.setRenderingHint(
                RenderingHints.KEY_ANTIALIASING,
                RenderingHints.VALUE_ANTIALIAS_ON,
            )
            g2.color = color
            val path = java.awt.geom.Path2D.Float().apply {
                moveTo(x + 8f, y + 2f)
                lineTo(x + 13f, y + 4f)
                lineTo(x + 13f, y + 8f)
                quadTo(x + 13f, y + 12f, x + 8f, y + 14f)
                quadTo(x + 3f, y + 12f, x + 3f, y + 8f)
                lineTo(x + 3f, y + 4f)
                closePath()
            }
            g2.fill(path)
        } finally {
            g2.dispose()
        }
    }
    override fun getIconWidth(): Int = 16
    override fun getIconHeight(): Int = 16
}
