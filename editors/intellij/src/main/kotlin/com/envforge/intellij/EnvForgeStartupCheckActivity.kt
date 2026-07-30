package com.envforge.intellij

import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.fileEditor.FileEditorManager
import com.intellij.openapi.project.Project
import com.intellij.openapi.startup.ProjectActivity

class EnvForgeStartupCheckActivity : ProjectActivity {
    override suspend fun execute(project: Project) {
        val managedExists = EnvForgeBinaryManager.managedBinaryFile.exists()
        val binaryFound = EnvForgeBinaryManager.findBinaryPath(project) != null

        if (!managedExists || !binaryFound) {
            ApplicationManager.getApplication().invokeLater {
                openWelcomeTab(project)
                if (!binaryFound) {
                    EnvForgeBinaryManager.downloadAsync(project)
                }
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
