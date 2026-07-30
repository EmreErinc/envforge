package com.envforge.intellij

import com.intellij.openapi.actionSystem.*
import com.intellij.openapi.fileEditor.FileEditorManager
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.LocalFileSystem
import com.intellij.ui.PopupHandler
import com.intellij.ui.components.JBScrollPane
import com.intellij.ui.treeStructure.Tree
import java.awt.BorderLayout
import java.awt.event.MouseAdapter
import java.awt.event.MouseEvent
import javax.swing.*
import javax.swing.tree.DefaultMutableTreeNode
import javax.swing.tree.DefaultTreeModel

class ProfilesPanel(private val project: Project) : JPanel(BorderLayout()) {
    private val model = DefaultTreeModel(DefaultMutableTreeNode("Profiles"))
    private val tree = Tree(model)
    private var currentProfiles = listOf<ProfileData>()

    init {
        val toolbar = createToolbar()
        add(toolbar, BorderLayout.NORTH)
        add(JBScrollPane(tree), BorderLayout.CENTER)

        tree.cellRenderer = ProfileCellRenderer()

        tree.addMouseListener(object : MouseAdapter() {
            override fun mouseClicked(e: MouseEvent) {
                if (e.clickCount == 2) {
                    val node = tree.lastSelectedPathComponent as? DefaultMutableTreeNode ?: return
                    val data = node.userObject as? ProfileData ?: return
                    if (!data.active) {
                        EnvForgeRunner.run(project, listOf("profile", "switch", data.name), "Switch Profile") {
                            SwingUtilities.invokeLater { refresh() }
                        }
                    }
                }
            }
        })

        installContextMenu()
        refresh()
    }

    fun refresh() {
        val binary = try { EnvForgeLspFactory.findEnvforgeBinary(project) } catch (_: Exception) { "" }
        if (binary.isEmpty()) {
            val root = DefaultMutableTreeNode("Profiles")
            root.add(DefaultMutableTreeNode("EnvForge CLI is disabled or not found — Run 'cargo install env-forge-tui'"))
            SwingUtilities.invokeLater {
                model.setRoot(root)
                model.reload()
            }
            return
        }
        Thread {
            try {
                val process = ProcessBuilder(binary, "profile", "list")
                    .directory(project.basePath?.let { java.io.File(it) })
                    .redirectErrorStream(true)
                    .start()
                val output = process.inputStream.bufferedReader().readText()
                process.waitFor()

                val profiles = mutableListOf<ProfileData>()
                for (line in output.lines()) {
                    val match = Regex("""^\s+(\S+)\s+\(([^)]+)\)(.*)""").find(line) ?: continue
                    val name = match.groupValues[1]
                    val file = match.groupValues[2]
                    val active = match.groupValues[3].contains("active")
                    profiles.add(ProfileData(name, file, active))
                }
                currentProfiles = profiles

                val root = DefaultMutableTreeNode("Profiles")
                for (p in profiles) {
                    root.add(DefaultMutableTreeNode(p))
                }

                SwingUtilities.invokeLater {
                    model.setRoot(root)
                    model.reload()
                }
            } catch (_: Exception) {}
        }.start()
    }

    private fun installContextMenu() {
        val group = DefaultActionGroup().apply {
            add(object : AnAction(
                "Switch to Profile",
                "Activate this profile",
                com.intellij.icons.AllIcons.Actions.Checked,
            ) {
                override fun actionPerformed(e: AnActionEvent) {
                    val node = tree.lastSelectedPathComponent as? DefaultMutableTreeNode ?: return
                    val data = node.userObject as? ProfileData ?: return
                    if (!data.active) {
                        EnvForgeRunner.run(project, listOf("profile", "switch", data.name), "Switch Profile") {
                            SwingUtilities.invokeLater { refresh() }
                        }
                    }
                }
                override fun update(e: AnActionEvent) {
                    val node = tree.lastSelectedPathComponent as? DefaultMutableTreeNode
                    val data = node?.userObject as? ProfileData
                    e.presentation.isEnabledAndVisible = data != null && !data.active
                }
                override fun getActionUpdateThread() = ActionUpdateThread.BGT
            })

            add(object : AnAction(
                "Open Profile File",
                "Open the profile's .env file in the editor",
                com.intellij.icons.AllIcons.FileTypes.Text,
            ) {
                override fun actionPerformed(e: AnActionEvent) {
                    val node = tree.lastSelectedPathComponent as? DefaultMutableTreeNode ?: return
                    val data = node.userObject as? ProfileData ?: return
                    openProfileFile(data.file)
                }
                override fun update(e: AnActionEvent) {
                    val node = tree.lastSelectedPathComponent as? DefaultMutableTreeNode
                    e.presentation.isEnabledAndVisible = node?.userObject is ProfileData
                }
                override fun getActionUpdateThread() = ActionUpdateThread.BGT
            })

            addSeparator()

            add(object : AnAction(
                "Diff vs Active Profile",
                "Show differences against the active profile",
                com.intellij.icons.AllIcons.Actions.Diff,
            ) {
                override fun actionPerformed(e: AnActionEvent) {
                    val node = tree.lastSelectedPathComponent as? DefaultMutableTreeNode ?: return
                    val data = node.userObject as? ProfileData ?: return
                    EnvForgeRunner.run(project, listOf("profile", "diff", data.name), "Profile Diff")
                }
                override fun update(e: AnActionEvent) {
                    val node = tree.lastSelectedPathComponent as? DefaultMutableTreeNode
                    e.presentation.isEnabledAndVisible = node?.userObject is ProfileData
                }
                override fun getActionUpdateThread() = ActionUpdateThread.BGT
            })

            addSeparator()

            add(object : AnAction(
                "Delete Profile",
                "Remove this profile",
                com.intellij.icons.AllIcons.Actions.GC,
            ) {
                override fun actionPerformed(e: AnActionEvent) {
                    val node = tree.lastSelectedPathComponent as? DefaultMutableTreeNode ?: return
                    val data = node.userObject as? ProfileData ?: return
                    val confirm = com.intellij.openapi.ui.Messages.showYesNoDialog(
                        project,
                        "Delete profile \"${data.name}\"?",
                        "Confirm Deletion",
                        com.intellij.openapi.ui.Messages.getWarningIcon(),
                    )
                    if (confirm == com.intellij.openapi.ui.Messages.YES) {
                        EnvForgeRunner.run(project, listOf("profile", "delete", data.name), "Delete Profile") {
                            SwingUtilities.invokeLater { refresh() }
                        }
                    }
                }
                override fun update(e: AnActionEvent) {
                    val node = tree.lastSelectedPathComponent as? DefaultMutableTreeNode
                    val data = node?.userObject as? ProfileData
                    e.presentation.isEnabledAndVisible = data != null && !data.active
                }
                override fun getActionUpdateThread() = ActionUpdateThread.BGT
            })
        }
        PopupHandler.installPopupMenu(tree, group, "EnvForgeProfilesPopup")
    }

    /// Open the profile's backing .env file in the IDE editor.
    /// `filePath` comes from the `profile list` output (the second column).
    /// Resolves relative paths against the project base directory.
    private fun openProfileFile(filePath: String) {
        val file = java.io.File(filePath).let { f ->
            if (f.isAbsolute) f
            else java.io.File(project.basePath ?: return, filePath)
        }
        val vFile = LocalFileSystem.getInstance().refreshAndFindFileByIoFile(file) ?: run {
            EnvForgeRunner.notify(
                project,
                "Open Profile File",
                "File not found: ${file.absolutePath}",
                com.intellij.notification.NotificationType.WARNING,
            )
            return
        }
        SwingUtilities.invokeLater {
            FileEditorManager.getInstance(project).openFile(vFile, true)
        }
    }

    private fun createToolbar(): JComponent {
        val group = DefaultActionGroup().apply {
            add(object : com.envforge.intellij.actions.EnvForgeAction() {
                init {
                    templatePresentation.text = "Refresh"
                    templatePresentation.description = "Refresh profiles"
                    templatePresentation.icon = com.intellij.icons.AllIcons.Actions.Refresh
                }
                override fun actionPerformed(e: AnActionEvent) = refresh()
            })
            add(object : com.envforge.intellij.actions.EnvForgeAction() {
                init {
                    templatePresentation.text = "Add Profile..."
                    templatePresentation.description = "Create a new profile"
                    templatePresentation.icon = com.intellij.icons.AllIcons.General.Add
                }
                override fun actionPerformed(e: AnActionEvent) {
                    val name = com.intellij.openapi.ui.Messages.showInputDialog(project, "Enter profile name:", "Add Profile", null)
                    if (!name.isNullOrBlank()) {
                        EnvForgeRunner.run(project, listOf("profile", "create", name), "Add Profile") {
                            SwingUtilities.invokeLater { refresh() }
                        }
                    }
                }
            })
        }
        val actionToolbar = ActionManager.getInstance().createActionToolbar("EnvForgeProfilesToolbar", group, true)
        actionToolbar.targetComponent = this
        return actionToolbar.component
    }
}
