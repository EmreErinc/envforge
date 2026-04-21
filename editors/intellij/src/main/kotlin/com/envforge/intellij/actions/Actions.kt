package com.envforge.intellij.actions

import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.ui.popup.JBPopupFactory
import com.envforge.intellij.EnvForgeRunner
import com.envforge.intellij.EnvForgeLspFactory

class ValidateAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        EnvForgeRunner.run(project, listOf("validate"), "Schema Validation")
    }
}

class ScanAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        EnvForgeRunner.run(project, listOf("scan"), "Secret Scan")
    }
}

class DoctorAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        EnvForgeRunner.run(project, listOf("doctor"), "Health Check")
    }
}

class CheckAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        EnvForgeRunner.run(project, listOf("check"), "All Checks")
    }
}

class ExportAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        val formats = listOf("dotenv", "json", "yaml", "toml", "docker", "k8s", "tfvars")

        JBPopupFactory.getInstance()
            .createPopupChooserBuilder(formats)
            .setTitle("Export Format")
            .setItemChosenCallback { format ->
                EnvForgeRunner.run(project, listOf("export", "--format", format), "Export ($format)")
            }
            .createPopup()
            .showCenteredInCurrentWindow(project)
    }
}

class SyncPushAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        EnvForgeRunner.run(project, listOf("sync", "push"), "Sync Push")
    }
}

class SyncPullAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        EnvForgeRunner.run(project, listOf("sync", "pull"), "Sync Pull")
    }
}

class SchemaGenerateAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        EnvForgeRunner.run(
            project,
            listOf("schema", "generate", "--output", ".env.schema"),
            "Schema Generate"
        )
    }
}

class ListAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        EnvForgeRunner.run(project, listOf("list"), "Variables")
    }
}

class ProfileSwitchAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        val binary = EnvForgeLspFactory.findEnvforgeBinary()

        // Get profile list
        try {
            val process = ProcessBuilder(binary, "profile", "list")
                .directory(project.basePath?.let { java.io.File(it) })
                .redirectErrorStream(true)
                .start()
            val output = process.inputStream.bufferedReader().readText()
            process.waitFor()

            val profiles = output.lines()
                .mapNotNull { Regex("""^\s+(\S+)\s+\(""").find(it)?.groupValues?.get(1) }

            if (profiles.isEmpty()) {
                EnvForgeRunner.notify(project, "No Profiles", "No profiles configured",
                    com.intellij.notification.NotificationType.INFORMATION)
                return
            }

            JBPopupFactory.getInstance()
                .createPopupChooserBuilder(profiles)
                .setTitle("Switch Profile")
                .setItemChosenCallback { name ->
                    EnvForgeRunner.run(project, listOf("profile", "switch", name), "Switch Profile")
                }
                .createPopup()
                .showCenteredInCurrentWindow(project)
        } catch (ex: Exception) {
            EnvForgeRunner.notify(project, "Error", ex.message ?: "Failed",
                com.intellij.notification.NotificationType.ERROR)
        }
    }
}

class ProfileDiffAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        val binary = EnvForgeLspFactory.findEnvforgeBinary()

        try {
            val process = ProcessBuilder(binary, "profile", "list")
                .directory(project.basePath?.let { java.io.File(it) })
                .redirectErrorStream(true)
                .start()
            val output = process.inputStream.bufferedReader().readText()
            process.waitFor()

            val profiles = output.lines()
                .mapNotNull { Regex("""^\s+(\S+)\s+\(""").find(it)?.groupValues?.get(1) }

            if (profiles.size < 2) {
                EnvForgeRunner.notify(project, "Profile Diff", "Need at least 2 profiles",
                    com.intellij.notification.NotificationType.INFORMATION)
                return
            }

            JBPopupFactory.getInstance()
                .createPopupChooserBuilder(profiles)
                .setTitle("Diff From")
                .setItemChosenCallback { from ->
                    JBPopupFactory.getInstance()
                        .createPopupChooserBuilder(profiles.filter { it != from })
                        .setTitle("Diff To")
                        .setItemChosenCallback { to ->
                            EnvForgeRunner.run(project, listOf("profile", "diff", from, to), "Profile Diff")
                        }
                        .createPopup()
                        .showCenteredInCurrentWindow(project)
                }
                .createPopup()
                .showCenteredInCurrentWindow(project)
        } catch (ex: Exception) {
            EnvForgeRunner.notify(project, "Error", ex.message ?: "Failed",
                com.intellij.notification.NotificationType.ERROR)
        }
    }
}
