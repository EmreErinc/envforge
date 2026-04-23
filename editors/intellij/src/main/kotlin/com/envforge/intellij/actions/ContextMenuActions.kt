package com.envforge.intellij.actions

import com.intellij.openapi.actionSystem.*
import com.intellij.openapi.fileEditor.FileEditorManager
import com.intellij.openapi.vfs.LocalFileSystem

/**
 * Checks whether a filename matches EnvForge's .env file patterns.
 */
fun isEnvFile(fileName: String): Boolean {
    return fileName.startsWith(".env") || fileName.endsWith(".env") || fileName == ".env.schema"
}

/**
 * ActionGroup shown in ProjectView and Editor context menus for .env files.
 * Registered in plugin.xml with add-to-group for ProjectViewPopupMenu and EditorPopupMenu.
 */
class EnvForgeFileActionGroup : ActionGroup() {

    init {
        isPopup = true
    }

    override fun getChildren(e: AnActionEvent?): Array<AnAction> {
        return arrayOf(
            ValidateAction(),
            ScanAction(),
            ExportAction(),
            SchemaGenerateAction(),
            CheckAction(),
        )
    }

    override fun update(e: AnActionEvent) {
        val file = e.getData(CommonDataKeys.VIRTUAL_FILE)
        val visible = file != null && isEnvFile(file.name)
        e.presentation.isEnabledAndVisible = visible
    }

    override fun getActionUpdateThread(): ActionUpdateThread = ActionUpdateThread.BGT
}
