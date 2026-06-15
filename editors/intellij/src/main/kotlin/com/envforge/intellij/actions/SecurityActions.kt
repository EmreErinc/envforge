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

        // Probe current state via `envforge fence --status --json`, then
        // flip direction. Falls back to enable on probe failure so the
        // action always does something useful.
        val basePath = project.basePath?.let { java.io.File(it) }
        val binary = EnvForgeLspFactory.findEnvforgeBinary()
        val currentlyFenced = try {
            val proc = ProcessBuilder(binary, "fence", "--status", "--json")
                .directory(basePath)
                .redirectErrorStream(true)
                .start()
            val out = proc.inputStream.bufferedReader().readText()
            proc.waitFor()
            if (proc.exitValue() == 0) {
                val obj = com.google.gson.JsonParser.parseString(out).asJsonObject
                obj.get("all_fenced")?.asBoolean ?: false
            } else {
                false
            }
        } catch (_: Exception) {
            false
        }

        val args = if (currentlyFenced) listOf("fence", "--disable") else listOf("fence")
        val title = if (currentlyFenced) "Disable Fence" else "Enable Fence"
        val message = if (currentlyFenced) "Fence disabled" else "Fence enabled"

        EnvForgeRunner.run(project, args, title) {
            EnvForgeRunner.notify(project, "Fence", message,
                com.intellij.notification.NotificationType.INFORMATION)
            refreshSecurityPanel(project)
        }
    }
}

class ToggleGuardAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return

        // Probes current state via `envforge ai-hook status --json`.
        val basePath = project.basePath?.let { java.io.File(it) }
        val binary = EnvForgeLspFactory.findEnvforgeBinary()
        val guardStatus = try {
            val proc = ProcessBuilder(binary, "ai-hook", "status", "--json")
                .directory(basePath)
                .redirectErrorStream(true)
                .start()
            val out = proc.inputStream.bufferedReader().readText()
            proc.waitFor()
            if (proc.exitValue() == 0) {
                com.google.gson.JsonParser.parseString(out).asJsonObject
            } else {
                null
            }
        } catch (_: Exception) {
            null
        }

        if (guardStatus == null) {
            EnvForgeRunner.notify(project, "Guard", "Unable to determine guard status",
                com.intellij.notification.NotificationType.ERROR)
            return
        }

        val enabled = guardStatus.get("enabled")?.asBoolean ?: false
        val toolsArr = guardStatus.getAsJsonArray("tools") ?: com.google.gson.JsonArray()
        val installedTools = mutableListOf<String>()
        for (i in 0 until toolsArr.size()) {
            val t = toolsArr[i].asJsonObject
            if (t.get("installed")?.asBoolean == true) {
                installedTools.add(t.get("name").asString)
            }
        }

        if (enabled) {
            val choices = (listOf("All") + installedTools).toTypedArray()
            val selected = Messages.showEditableChooseDialog(
                "Select AI tool to remove hooks from:",
                "EnvForge: Toggle Guard",
                Messages.getQuestionIcon(),
                choices,
                "All",
                null
            ) ?: return

            val toolsToRemove = if (selected == "All") {
                listOf("claude-code", "cursor", "copilot")
            } else {
                listOf(selected.lowercase().replace(" ", "-"))
            }

            for (t in toolsToRemove) {
                EnvForgeRunner.run(project, listOf("ai-hook", "remove", t), "Remove Guard Hook ($t)")
            }
        } else {
            val choices = arrayOf("Claude Code", "Cursor", "Both")
            val selected = Messages.showEditableChooseDialog(
                "Select AI tool to install hooks for:",
                "EnvForge: Toggle Guard",
                Messages.getQuestionIcon(),
                choices,
                "Both",
                null
            ) ?: return

            val toolsToInstall = when (selected) {
                "Both" -> listOf("claude-code", "cursor")
                "Claude Code" -> listOf("claude-code")
                "Cursor" -> listOf("cursor")
                else -> listOf(selected.lowercase().replace(" ", "-"))
            }

            for (t in toolsToInstall) {
                EnvForgeRunner.run(project, listOf("ai-hook", "install", t), "Install Guard Hook ($t)")
            }
        }
        refreshSecurityPanel(project)
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
                EnvForgeRunner.run(project, listOf("canary", "delete", selected), "Remove Canary") {
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

class RunLifecycleCheckAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        EnvForgeRunner.run(project, listOf("lifecycle", "check"), "Lifecycle Check")
    }
}

class ManageLifecycleRulesAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        EnvForgeRunner.run(project, listOf("lifecycle", "rule", "list"), "Lifecycle Rules")
    }
}

class ViewAuditTrailAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        EnvForgeRunner.run(project, listOf("audit", "-n", "100"), "Audit Trail")
    }
}

class ShowUnusedSecretsAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        EnvForgeRunner.run(project, listOf("analytics", "unused"), "Unused Secrets")
    }
}

class ShowUsageSummaryAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        EnvForgeRunner.run(project, listOf("analytics", "summary"), "Usage Summary")
    }
}

class MonitorStreamAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        val binary = EnvForgeLspFactory.findEnvforgeBinary()
        
        // Use the IDE's terminal to run the stream
        val terminalManager = com.intellij.terminal.JBTerminalWidget.getTerminalWidgets(project).firstOrNull()
        // Actually, better to use the ToolWindowManager to find/create a terminal
        val terminalView = com.intellij.openapi.wm.ToolWindowManager.getInstance(project).getToolWindow("Terminal")
        terminalView?.show {
            EnvForgeRunner.run(project, listOf("monitor", "stream"), "Monitor Stream")
        }
    }
}

private fun refreshSecurityPanel(project: Project) {
    ToolWindowManager.getInstance(project)
        .getToolWindow("EnvForge Security")?.contentManager?.contents?.forEach { content ->
            (content.component as? SecurityPanel)?.refresh()
        }
}
