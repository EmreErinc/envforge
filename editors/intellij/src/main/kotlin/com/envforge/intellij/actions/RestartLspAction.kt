package com.envforge.intellij.actions

import com.envforge.intellij.EnvForgeLspFactory
import com.envforge.intellij.EnvForgeRunner
import com.intellij.notification.NotificationType
import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.project.Project

/**
 * Restart the EnvForge LSP server for the current project.
 *
 * Parity with VS Code's `envforge.restartLsp` command which calls
 * `client.stop()` then `client.start()` on the language client. We
 * mirror that by driving the same lifecycle through
 * `EnvForgeLspFactory`, which owns the server process handle.
 *
 * A confirmation dialog is shown first (same UX pattern as the VS Code
 * "Reload Window" prompt that `restartLsp` surfaces).
 */
class RestartLspAction : EnvForgeAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return

        val confirmed = com.intellij.openapi.ui.Messages.showOkCancelDialog(
            project,
            "Restart the EnvForge language server?\n\nThis will briefly clear LSP diagnostics until the server reconnects.",
            "Restart EnvForge LSP",
            "Restart",
            "Cancel",
            com.intellij.openapi.ui.Messages.getQuestionIcon(),
        )
        if (confirmed != com.intellij.openapi.ui.Messages.OK) return

        restartLsp(project)
    }

    companion object {
        /// Restarts the LSP server for `project`. May be called
        /// programmatically (e.g., after a binary update).
        fun restartLsp(project: Project) {
            try {
                EnvForgeLspFactory.restartForProject(project)
                EnvForgeRunner.notify(
                    project,
                    "EnvForge LSP",
                    "Language server restarted.",
                    NotificationType.INFORMATION,
                )
            } catch (ex: Exception) {
                EnvForgeRunner.notify(
                    project,
                    "EnvForge LSP",
                    "Restart failed: ${ex.message ?: "unknown error"}",
                    NotificationType.ERROR,
                )
            }
        }
    }
}
