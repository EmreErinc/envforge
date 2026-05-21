package com.envforge.intellij

import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.project.Project
import com.intellij.openapi.wm.StatusBar
import com.intellij.openapi.wm.StatusBarWidget
import com.intellij.openapi.wm.StatusBarWidgetFactory

class EnvForgeStatusBarFactory : StatusBarWidgetFactory {
    override fun getId(): String = "EnvForgeStatus"
    override fun getDisplayName(): String = "EnvForge"
    override fun isAvailable(project: Project): Boolean = true

    override fun createWidget(project: Project): StatusBarWidget {
        return EnvForgeStatusWidget(project)
    }
}

class EnvForgeStatusWidget(private val project: Project) :
    StatusBarWidget, StatusBarWidget.TextPresentation {
    private var statusBar: StatusBar? = null
    private var text: String = "envforge"
    private var tooltip: String = "EnvForge — environment variable manager"

    override fun ID(): String = "EnvForgeStatus"

    override fun install(statusBar: StatusBar) {
        this.statusBar = statusBar
        scheduleRefresh()
    }

    override fun getPresentation(): StatusBarWidget.WidgetPresentation = this
    override fun getText(): String = text
    override fun getTooltipText(): String = tooltip
    override fun getAlignment(): Float = 0f

    override fun dispose() {}

    private fun scheduleRefresh() {
        ApplicationManager.getApplication().executeOnPooledThread {
            refresh()
        }
    }

    private fun refresh() {
        val varCount = runCli(listOf("list", "--json"))?.let { out ->
            if (out.trim().startsWith("[")) out.count { it == '{' } else 0
        } ?: 0

        val fenceState = runCli(listOf("fence", "--status", "--json"))?.let { out ->
            parseFenceStatus(out)
        }

        val volatileLease = runCli(listOf("lease", "list", "--json"))?.let { out ->
            parseSoonestActiveLease(out)
        }

        val pieces = mutableListOf("$varCount vars")
        val tooltipLines = mutableListOf<String>()
        when (fenceState) {
            true -> {
                pieces += "AI BLOCKED"
                tooltipLines += "Fence: ACTIVE."
            }
            false -> {
                pieces += "AI ALLOWED"
                tooltipLines += "Fence: inactive. Run Tools > EnvForge > Toggle Fence."
            }
            null -> {}
        }
        if (volatileLease != null) {
            pieces += "volatile: ${formatDuration(volatileLease.remainingSeconds)}"
            val keyDesc = volatileLease.keyCount
                ?.let { "$it key${if (it == 1) "" else "s"}" }
                ?: "all keys"
            tooltipLines += "Lease \"${volatileLease.name}\" — $keyDesc, ${formatDuration(volatileLease.remainingSeconds)} remaining."
        }

        tooltip = if (tooltipLines.isEmpty()) {
            "EnvForge — environment variable manager"
        } else {
            tooltipLines.joinToString(" ")
        }
        text = pieces.joinToString(" · ")
        ApplicationManager.getApplication().invokeLater {
            statusBar?.updateWidget(ID())
        }
    }

    private data class VolatileLease(
        val name: String,
        val remainingSeconds: Long,
        val keyCount: Int?,
    )

    /// Pull the soonest-expiring active lease out of `envforge lease
    /// list --json`. Mirrors the LSP `envforge.volatile.status`
    /// dispatcher so subprocess and LSP consumers see consistent state.
    private fun parseSoonestActiveLease(json: String): VolatileLease? {
        return try {
            val obj = com.google.gson.JsonParser.parseString(json).asJsonObject
            val arr = obj.getAsJsonArray("leases") ?: return null
            var best: VolatileLease? = null
            for (i in 0 until arr.size()) {
                val e = arr[i].asJsonObject
                val status = e.get("status")?.asString ?: continue
                if (status != "active") continue
                val remaining = e.get("remaining_seconds")?.asLong ?: continue
                if (remaining <= 0) continue
                val name = e.get("name")?.asString ?: continue
                val keyCount = e.get("key_count")?.let {
                    if (it.isJsonNull) null else it.asInt
                }
                if (best == null || remaining < best!!.remainingSeconds) {
                    best = VolatileLease(name, remaining, keyCount)
                }
            }
            best
        } catch (_: Exception) {
            null
        }
    }

    private fun formatDuration(totalSeconds: Long): String {
        val s = totalSeconds.coerceAtLeast(0)
        val h = s / 3600
        val m = (s % 3600) / 60
        val sec = s % 60
        return when {
            h > 0 -> "${h}h ${m}m"
            m > 0 -> "${m}m ${sec}s"
            else -> "${sec}s"
        }
    }

    private fun runCli(args: List<String>): String? = try {
        val binary = EnvForgeLspFactory.findEnvforgeBinary()
        val process = ProcessBuilder(listOf(binary) + args)
            .directory(project.basePath?.let { java.io.File(it) })
            .redirectErrorStream(true)
            .start()
        val out = process.inputStream.bufferedReader().readText()
        process.waitFor()
        if (process.exitValue() == 0) out else null
    } catch (_: Exception) {
        null
    }

    /// Parse `fence --status --json` output and return whether every
    /// fence file is in place. `null` means the call failed or output
    /// was unparseable; the caller treats that as unknown and hides
    /// the indicator rather than guessing.
    private fun parseFenceStatus(json: String): Boolean? = try {
        val obj = com.google.gson.JsonParser.parseString(json).asJsonObject
        obj.get("all_fenced")?.asBoolean
    } catch (_: Exception) {
        null
    }
}
