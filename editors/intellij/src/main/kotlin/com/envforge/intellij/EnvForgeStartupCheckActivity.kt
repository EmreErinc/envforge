package com.envforge.intellij

import com.intellij.ide.util.PropertiesComponent
import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.fileEditor.FileEditorManager
import com.intellij.openapi.project.Project
import com.intellij.openapi.startup.ProjectActivity

class EnvForgeStartupCheckActivity : ProjectActivity {
    override suspend fun execute(project: Project) {
        val properties = PropertiesComponent.getInstance()
        val welcomeShownKey = "com.envforge.intellij.welcomeShown"
        val welcomeShown = properties.getBoolean(welcomeShownKey, false)

        if (!welcomeShown) {
            properties.setValue(welcomeShownKey, true)
            ApplicationManager.getApplication().invokeLater {
                openWelcomeTab(project)
            }
        }

        val binaryFound = EnvForgeBinaryManager.findBinaryPath(project) != null
        if (!binaryFound) {
            ApplicationManager.getApplication().invokeLater {
                EnvForgeBinaryManager.downloadAsync(project)
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
