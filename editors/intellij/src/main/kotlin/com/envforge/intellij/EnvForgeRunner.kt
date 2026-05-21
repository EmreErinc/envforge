package com.envforge.intellij

import com.intellij.notification.NotificationGroupManager
import com.intellij.notification.NotificationType
import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.project.Project

/**
 * Runs envforge CLI subcommands silently on a background thread and
 * surfaces the result via the IDE's standard notification group.
 *
 * We deliberately do NOT use `OSProcessHandler` / `GeneralCommandLine`
 * here — those paths pipe their command-line text through IntelliJ's
 * process consoles (`/Users/…/envforge …`), which clutters the
 * user-visible UI with implementation detail. A plain `ProcessBuilder`
 * keeps the subprocess invisible; only the result content ever
 * reaches the user.
 */
object EnvForgeRunner {

    fun run(
        project: Project,
        args: List<String>,
        title: String,
        onSuccess: ((String) -> Unit)? = null,
    ) {
        ApplicationManager.getApplication().executeOnPooledThread {
            val binary = EnvForgeLspFactory.findEnvforgeBinary()
            val cwd = project.basePath?.let { java.io.File(it) }

            val (exitCode, output) = try {
                val proc = ProcessBuilder(listOf(binary) + args)
                    .directory(cwd)
                    .redirectErrorStream(true)
                    .start()
                val text = proc.inputStream.bufferedReader().readText()
                proc.waitFor()
                proc.exitValue() to text
            } catch (e: Exception) {
                -1 to (e.message ?: "subprocess error")
            }

            val trimmed = output.trim()
            ApplicationManager.getApplication().invokeLater {
                if (exitCode == 0) {
                    if (onSuccess != null) {
                        onSuccess(trimmed)
                    } else {
                        notify(project, title, trimmed, NotificationType.INFORMATION)
                    }
                } else {
                    notify(project, "$title failed", trimmed, NotificationType.ERROR)
                }
            }
        }
    }

    fun notify(project: Project, title: String, content: String, type: NotificationType) {
        NotificationGroupManager.getInstance()
            .getNotificationGroup("EnvForge")
            .createNotification(title, content.take(500), type)
            .notify(project)
    }
}
