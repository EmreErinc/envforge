package com.envforge.intellij.actions

import com.envforge.intellij.EnvForgeStartupCheckActivity
import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent

class ShowWelcomeAction : AnAction(
    "Welcome & Installer",
    "Show EnvForge Welcome & CLI Installer page",
    com.intellij.icons.AllIcons.Actions.Help
) {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        EnvForgeStartupCheckActivity.openWelcomeTab(project)
    }
}
