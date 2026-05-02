package com.envforge.intellij.actions

import com.envforge.intellij.EnvForgeLspFactory
import com.envforge.intellij.EnvForgeRunner
import com.envforge.intellij.SecurityPanel
import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.project.Project
import com.intellij.openapi.ui.Messages
import com.intellij.openapi.wm.ToolWindowManager

class ToggleFenceAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        EnvForgeRunner.run(project, listOf("fence"), "Toggle Fence") {
            EnvForgeRunner.notify(project, "Fence", "Fence toggled",
                com.intellij.notification.NotificationType.INFORMATION)
            refreshSecurityPanel(project)
        }
    }
}

class ToggleGuardAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        EnvForgeRunner.run(project, listOf("guard"), "Toggle Guard") {
            EnvForgeRunner.notify(project, "Guard", "Guard toggled",
                com.intellij.notification.NotificationType.INFORMATION)
            refreshSecurityPanel(project)
        }
    }
}

class RunMcpScanAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        EnvForgeRunner.run(project, listOf("mcp-scan"), "MCP Scan") {
            refreshSecurityPanel(project)
        }
    }
}

class AddCanaryAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        val key = Messages.showInputDialog(
            project,
            "Variable key for canary token:",
            "Add Canary Token",
            com.intellij.icons.AllIcons.General.Information,
        ) ?: return

        if (key.isBlank()) {
            EnvForgeRunner.notify(project, "Add Canary", "Key cannot be empty",
                com.intellij.notification.NotificationType.WARNING)
            return
        }

        EnvForgeRunner.run(project, listOf("canary", "add", key.trim()), "Add Canary") {
            EnvForgeRunner.notify(project, "Canary", "Canary token added: ${key.trim()}",
                com.intellij.notification.NotificationType.INFORMATION)
            refreshSecurityPanel(project)
        }
    }
}

class RemoveCanaryAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        val binary = EnvForgeLspFactory.findEnvforgeBinary()

        try {
            val process = ProcessBuilder(binary, "canary", "list", "--json")
                .directory(project.basePath?.let { java.io.File(it) })
                .redirectErrorStream(true)
                .start()
            val output = process.inputStream.bufferedReader().readText()
            process.waitFor()

            val tokens = mutableListOf<String>()
            try {
                val arr = com.google.gson.JsonParser.parseString(output).asJsonArray
                for (i in 0 until arr.size()) {
                    val obj = arr[i].asJsonObject
                    val tokenKey = obj.get("key")?.asString ?: obj.get("name")?.asString ?: continue
                    tokens.add(tokenKey)
                }
            } catch (_: Exception) {}

            if (tokens.isEmpty()) {
                EnvForgeRunner.notify(project, "Remove Canary", "No canary tokens found",
                    com.intellij.notification.NotificationType.INFORMATION)
                return
            }

            val selected = Messages.showEditableChooseDialog(
                "Select canary token to remove:",
                "Remove Canary Token",
                Messages.getQuestionIcon(),
                tokens.toTypedArray(),
                tokens.firstOrNull(),
                null
            ) ?: return

            val confirm = Messages.showYesNoDialog(
                project,
                "Remove canary token for \"$selected\"?",
                "Confirm Removal",
                Messages.getQuestionIcon()
            )

            if (confirm == Messages.YES) {
                EnvForgeRunner.run(project, listOf("canary", "remove", selected), "Remove Canary") {
                    EnvForgeRunner.notify(project, "Canary", "Canary token removed: $selected",
                        com.intellij.notification.NotificationType.INFORMATION)
                    refreshSecurityPanel(project)
                }
            }
        } catch (ex: Exception) {
            EnvForgeRunner.notify(project, "Error", ex.message ?: "Failed",
                com.intellij.notification.NotificationType.ERROR)
        }
    }
}

private fun refreshSecurityPanel(project: Project) {
    ToolWindowManager.getInstance(project)
        .getToolWindow("EnvForge Security")?.contentManager?.contents?.forEach { content ->
            (content.component as? SecurityPanel)?.refresh()
        }
}
