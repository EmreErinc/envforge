package com.envforge.intellij

import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.fileEditor.FileEditorManager
import com.intellij.openapi.project.Project
import com.intellij.openapi.startup.ProjectActivity

class EnvForgeStartupCheckActivity : ProjectActivity {
    override suspend fun execute(project: Project) {
        val binaryFound = try {
            EnvForgeLspFactory.findEnvforgeBinary()
            true
        } catch (_: Throwable) {
            false
        }

        if (!binaryFound) {
            ApplicationManager.getApplication().invokeLater {
                openWelcomeTab(project)
            }
        }
    }

    companion object {
        fun openWelcomeTab(project: Project) {
            val fileManager = FileEditorManager.getInstance(project)
            val existing = fileManager.openFiles.firstOrNull { it is EnvForgeWelcomeVirtualFile }
            if (existing != null) {
                fileManager.openFile(existing, true)
            } else {
                val vFile = EnvForgeWelcomeVirtualFile()
                fileManager.openFile(vFile, true)
            }
        }
    }
}
