package com.envforge.intellij

import com.intellij.openapi.actionSystem.*
import com.intellij.openapi.project.Project
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

        refresh()
    }

    fun refresh() {
        Thread {
            try {
                val binary = EnvForgeLspFactory.findEnvforgeBinary()
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

    private fun createToolbar(): JComponent {
        val group = DefaultActionGroup().apply {
            add(object : AnAction("Refresh", "Refresh profiles", com.intellij.icons.AllIcons.Actions.Refresh) {
                override fun actionPerformed(e: AnActionEvent) = refresh()
            })
            add(object : AnAction("Add Profile...", "Create a new profile", com.intellij.icons.General.Add) {
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
