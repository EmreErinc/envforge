package com.envforge.intellij

import com.intellij.execution.configurations.GeneralCommandLine
import com.intellij.execution.process.OSProcessHandler
import com.intellij.execution.process.ProcessAdapter
import com.intellij.execution.process.ProcessEvent
import com.intellij.notification.NotificationGroupManager
import com.intellij.notification.NotificationType
import com.intellij.openapi.project.Project
import com.intellij.openapi.util.Key

object EnvForgeRunner {

    fun run(
        project: Project,
        args: List<String>,
        title: String,
        onSuccess: ((String) -> Unit)? = null,
    ) {
        val binary = EnvForgeLspFactory.findEnvforgeBinary()
        val cmd = GeneralCommandLine(binary).apply {
            addParameters(args)
            workDirectory = project.basePath?.let { java.io.File(it) }
        }

        val handler = OSProcessHandler(cmd)
        val output = StringBuilder()

        handler.addProcessListener(object : ProcessAdapter() {
            override fun onTextAvailable(event: ProcessEvent, outputType: Key<*>) {
                output.append(event.text)
            }

            override fun processTerminated(event: ProcessEvent) {
                val text = output.toString().trim()
                if (event.exitCode == 0) {
                    onSuccess?.invoke(text)
                        ?: notify(project, title, text, NotificationType.INFORMATION)
                } else {
                    notify(project, "$title failed", text, NotificationType.ERROR)
                }
            }
        })

        handler.startNotify()
    }

    fun notify(project: Project, title: String, content: String, type: NotificationType) {
        NotificationGroupManager.getInstance()
            .getNotificationGroup("EnvForge")
            .createNotification(title, content.take(500), type)
            .notify(project)
    }
}
