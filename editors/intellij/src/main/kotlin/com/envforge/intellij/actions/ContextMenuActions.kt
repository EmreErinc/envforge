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
        // IntelliJ 2024+ requires every menu item carry non-empty
        // text. The action classes here inherit `AnAction()` with no
        // text set in their constructors — plugin.xml supplies text
        // for the Tools-menu copies but these context-menu instances
        // are constructed in code and bypass that path. Set the text
        // on each instance explicitly to avoid `Empty menu item text`
        // crashes on right-click.
        fun labeled(action: AnAction, text: String, desc: String): AnAction {
            action.templatePresentation.text = text
            action.templatePresentation.description = desc
            return action
        }
        return arrayOf(
            labeled(
                ValidateAction(),
                "Validate Against Schema",
                "Run envforge schema validation on this file",
            ),
            labeled(
                ScanAction(),
                "Scan for Secret Leaks",
                "Scan this file for leaked secrets",
            ),
            labeled(
                ExportAction(),
                "Export Variables...",
                "Export env vars to a different format",
            ),
            labeled(
                SchemaGenerateAction(),
                "Generate Schema",
                "Generate .env.schema from this file",
            ),
            labeled(
                CheckAction(),
                "Run All Checks",
                "Run doctor + validate + scan + age + drift",
            ),
        )
    }

    override fun update(e: AnActionEvent) {
        val file = e.getData(CommonDataKeys.VIRTUAL_FILE)
        val isEnv = file != null && isEnvFile(file.name)
        val hasCli = try {
            com.envforge.intellij.EnvForgeLspFactory.findEnvforgeBinary(e.project).isNotEmpty()
        } catch (_: Throwable) {
            false
        }
        e.presentation.isVisible = isEnv
        e.presentation.isEnabled = isEnv && hasCli
        if (isEnv && !hasCli) {
            e.presentation.text = "EnvForge (CLI Not Installed)"
        }
    }

    override fun getActionUpdateThread(): ActionUpdateThread = ActionUpdateThread.BGT
}
