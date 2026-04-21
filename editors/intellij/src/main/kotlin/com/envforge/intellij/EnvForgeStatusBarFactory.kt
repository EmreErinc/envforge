package com.envforge.intellij

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

class EnvForgeStatusWidget(private val project: Project) : StatusBarWidget, StatusBarWidget.TextPresentation {
    private var statusBar: StatusBar? = null
    private var text: String = "envforge"

    override fun ID(): String = "EnvForgeStatus"

    override fun install(statusBar: StatusBar) {
        this.statusBar = statusBar
        refresh()
    }

    override fun getPresentation(): StatusBarWidget.WidgetPresentation = this
    override fun getText(): String = text
    override fun getTooltipText(): String = "EnvForge — environment variable manager"
    override fun getAlignment(): Float = 0f

    override fun dispose() {}

    private fun refresh() {
        try {
            val binary = EnvForgeLspFactory.findEnvforgeBinary()
            val process = ProcessBuilder(binary, "list", "--json")
                .directory(project.basePath?.let { java.io.File(it) })
                .redirectErrorStream(true)
                .start()

            val output = process.inputStream.bufferedReader().readText()
            process.waitFor()

            if (process.exitValue() == 0) {
                // Count entries from JSON array
                val count = output.trim().let {
                    if (it.startsWith("[")) {
                        it.count { c -> c == '{' }
                    } else 0
                }
                text = "$count vars"
            }
        } catch (_: Exception) {
            text = "envforge"
        }
        statusBar?.updateWidget(ID())
    }
}
