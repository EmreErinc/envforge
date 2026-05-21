package com.envforge.intellij

import com.intellij.ide.projectView.PresentationData
import com.intellij.ide.projectView.ProjectViewNode
import com.intellij.ide.projectView.ProjectViewNodeDecorator
import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.project.Project
import com.intellij.ui.JBColor
import com.intellij.ui.SimpleTextAttributes
import java.awt.Color
import java.util.concurrent.ConcurrentHashMap

/**
 * Project-view badge renderer for `.env*` files. Subprocesses
 * `envforge exposure --file PATH --json` and appends a one-character
 * status marker plus colored tag to the file's tree row. Mirrors the
 * VS Code `EnvFileDecorationProvider` behavior 1:1 so cross-IDE users
 * see the same signal in either explorer.
 *
 * Caching: a per-path entry holds the last computed badge + a
 * timestamp. Repaint calls return the cached value; expired entries
 * trigger a background subprocess refresh that calls
 * `markNeedsRedraw` once data lands.
 */
class EnvForgeProjectViewDecorator : ProjectViewNodeDecorator {

    private val cache: MutableMap<String, CacheEntry> = ConcurrentHashMap()
    private val inFlight: MutableSet<String> = java.util.concurrent.ConcurrentHashMap.newKeySet()

    override fun decorate(node: ProjectViewNode<*>, data: PresentationData) {
        val file = node.virtualFile ?: return
        if (file.isDirectory) return
        if (!isEnvFile(file.name)) return

        val project = node.project ?: return
        val path = file.path

        val entry = cache[path]
        val now = System.currentTimeMillis()
        val badge = if (entry != null && now - entry.fetchedAt < CACHE_TTL_MS) {
            entry.badge
        } else {
            scheduleRefresh(project, path)
            entry?.badge // serve stale while we refresh
        } ?: return

        // Render: "  <badge>  <profile>"
        // The profile label gives the user an at-a-glance which env
        // each badge belongs to (`.env.development` → `development`,
        // `.env.local` → `local`, plain `.env` → no suffix). Color
        // matches the badge so the whole annotation reads as one unit.
        data.addText(
            "  ${badge.symbol}",
            SimpleTextAttributes(SimpleTextAttributes.STYLE_BOLD, badge.color),
        )
        val profile = envProfileFromName(file.name)
        if (profile.isNotEmpty()) {
            data.addText(
                "  $profile",
                SimpleTextAttributes(SimpleTextAttributes.STYLE_PLAIN, badge.color),
            )
        }
        data.tooltip = badge.tooltip
    }

    /// Strip `.env.` prefix and trailing `.env` suffix to pull the
    /// profile name out of a filename. Examples:
    ///   `.env.development` → `development`
    ///   `.env.local`       → `local`
    ///   `.env.production`  → `production`
    ///   `.env`             → `` (no profile)
    ///   `staging.env`      → `staging`
    private fun envProfileFromName(name: String): String {
        if (name == ".env") return ""
        if (name.startsWith(".env.")) return name.removePrefix(".env.")
        if (name.endsWith(".env") && name != ".env") return name.removeSuffix(".env")
        return ""
    }

    private fun scheduleRefresh(project: Project, path: String) {
        if (!inFlight.add(path)) return
        ApplicationManager.getApplication().executeOnPooledThread {
            try {
                val badge = computeBadge(project, path)
                cache[path] = CacheEntry(badge, System.currentTimeMillis())
                // Nudge the project view to redraw with the new badge.
                ApplicationManager.getApplication().invokeLater {
                    try {
                        com.intellij.ide.projectView.ProjectView
                            .getInstance(project)
                            .currentProjectViewPane
                            ?.updateFromRoot(true)
                    } catch (_: Exception) {
                        // Project may be closing; safe to swallow.
                    }
                }
            } finally {
                inFlight.remove(path)
            }
        }
    }

    private fun computeBadge(project: Project, path: String): Badge? {
        val binary = EnvForgeLspFactory.findEnvforgeBinary()
        val cwd = project.basePath?.let { java.io.File(it) }
        val output: String = try {
            val proc = ProcessBuilder(binary, "exposure", "--file", path)
                .directory(cwd)
                .redirectErrorStream(true)
                .start()
            val out = proc.inputStream.bufferedReader().readText()
            proc.waitFor()
            if (proc.exitValue() != 0) return null
            out
        } catch (_: Exception) {
            return null
        }

        val obj = try {
            com.google.gson.JsonParser.parseString(output).asJsonObject
        } catch (_: Exception) {
            return null
        }
        val fenceActive = obj.get("fence_active")?.asBoolean ?: false
        val entries = obj.getAsJsonArray("entries") ?: return null

        if (fenceActive) {
            return Badge(
                symbol = "🛡",
                color = GREEN,
                tooltip = "EnvForge: fence active — AI agents instructed to refuse reads.",
            )
        }
        var hasRed = false
        var hasAmber = false
        for (i in 0 until entries.size()) {
            when (entries[i].asJsonObject.get("level")?.asString) {
                "red" -> hasRed = true
                "amber" -> hasAmber = true
            }
        }
        return when {
            hasRed -> Badge(
                symbol = "!",
                color = RED,
                tooltip = "EnvForge: plaintext secrets readable by AI agents.",
            )
            hasAmber -> Badge(
                symbol = "?",
                color = AMBER,
                tooltip = "EnvForge: sensitive values present. AI-guard will redact in tool inputs.",
            )
            entries.size() > 0 -> Badge(
                symbol = "✓",
                color = GREEN,
                tooltip = "EnvForge: no plaintext secrets detected.",
            )
            else -> null
        }
    }

    private fun isEnvFile(name: String): Boolean {
        if (name == ".env.schema" || name == ".env.schema.toml") return false
        return name == ".env" ||
                name.startsWith(".env.") ||
                (name.endsWith(".env") && name != ".env.schema" && name != ".env.schema.toml") ||
                name == "env"
    }

    private data class Badge(
        val symbol: String,
        val color: Color,
        val tooltip: String,
    )

    private data class CacheEntry(
        val badge: Badge?,
        val fetchedAt: Long,
    )

    companion object {
        private const val CACHE_TTL_MS: Long = 30_000

        private val RED = JBColor(Color(0xD3, 0x2F, 0x2F), Color(0xEF, 0x53, 0x50))
        private val AMBER = JBColor(Color(0xF9, 0xA8, 0x25), Color(0xFF, 0xC1, 0x07))
        private val GREEN = JBColor(Color(0x2E, 0x7D, 0x32), Color(0x66, 0xBB, 0x6A))
    }
}
