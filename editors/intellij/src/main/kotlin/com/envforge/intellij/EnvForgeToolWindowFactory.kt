package com.envforge.intellij

import com.intellij.openapi.actionSystem.*
import com.intellij.openapi.fileEditor.FileEditorManager
import com.intellij.openapi.project.DumbAware
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.LocalFileSystem
import com.intellij.openapi.wm.ToolWindow
import com.intellij.openapi.wm.ToolWindowFactory
import com.intellij.ui.components.JBScrollPane
import com.intellij.ui.content.ContentFactory
import com.intellij.ui.PopupHandler
import com.intellij.ui.treeStructure.Tree
import java.awt.BorderLayout
import java.awt.datatransfer.StringSelection
import java.awt.Toolkit
import java.awt.event.MouseAdapter
import java.awt.event.MouseEvent
import javax.swing.*
import javax.swing.tree.DefaultMutableTreeNode
import javax.swing.tree.DefaultTreeModel
import javax.swing.tree.DefaultTreeCellRenderer
import java.awt.Component

class EnvForgeToolWindowFactory : ToolWindowFactory, DumbAware {
    override fun createToolWindowContent(project: Project, toolWindow: ToolWindow) {
        val panel = EnvForgeToolWindowPanel(project)
        val content = ContentFactory.getInstance().createContent(panel, "", false)
        toolWindow.contentManager.addContent(content)
    }
}

class EnvForgeToolWindowPanel(private val project: Project) : JPanel(BorderLayout()) {
    private val profileModel = DefaultTreeModel(DefaultMutableTreeNode("Profiles"))
    private val profileTree = Tree(profileModel)
    private val varModel = DefaultTreeModel(DefaultMutableTreeNode("Variables"))
    private val varTree = Tree(varModel)
    private var grouped = true

    init {
        // Toolbar
        val toolbar = createToolbar()
        add(toolbar, BorderLayout.NORTH)

        // Split: profiles on top, variables below
        val splitPane = JSplitPane(JSplitPane.VERTICAL_SPLIT).apply {
            topComponent = JBScrollPane(profileTree).apply {
                border = BorderFactory.createTitledBorder("Profiles")
            }
            bottomComponent = JBScrollPane(varTree).apply {
                border = BorderFactory.createTitledBorder("Variables")
            }
            dividerLocation = 120
            resizeWeight = 0.2
        }
        add(splitPane, BorderLayout.CENTER)

        // Profile click → switch
        profileTree.addMouseListener(object : MouseAdapter() {
            override fun mouseClicked(e: MouseEvent) {
                if (e.clickCount == 2) {
                    val node = profileTree.lastSelectedPathComponent as? DefaultMutableTreeNode ?: return
                    val info = node.userObject as? ProfileData ?: return
                    if (!info.active) {
                        EnvForgeRunner.run(project, listOf("profile", "switch", info.name), "Switch Profile") {
                            refresh()
                        }
                    }
                }
            }
        })

        // Variable click → copy key
        varTree.addMouseListener(object : MouseAdapter() {
            override fun mouseClicked(e: MouseEvent) {
                if (e.clickCount == 2) {
                    val node = varTree.lastSelectedPathComponent as? DefaultMutableTreeNode ?: return
                    val data = node.userObject as? VarData ?: return
                    copyToClipboard(data.key)
                    EnvForgeRunner.notify(project, "Copied", "Key: ${data.key}",
                        com.intellij.notification.NotificationType.INFORMATION)
                }
            }
        })

        // Context menu for variables (Action-based, replaces JPopupMenu)
        installVarContextMenu()

        // Context menu for profiles
        installProfileContextMenu()

        // Custom renderer
        varTree.cellRenderer = EnvVarCellRenderer()
        profileTree.cellRenderer = ProfileCellRenderer()

        refresh()
    }

    private fun createToolbar(): JComponent {
        val group = DefaultActionGroup().apply {
            add(object : AnAction("Refresh", "Refresh variables and profiles", com.intellij.icons.AllIcons.Actions.Refresh) {
                override fun actionPerformed(e: AnActionEvent) = refresh()
            })
            add(object : AnAction("Toggle Grouping", "Toggle variable grouping", com.intellij.icons.AllIcons.Actions.GroupBy) {
                override fun actionPerformed(e: AnActionEvent) {
                    grouped = !grouped
                    loadVariables()
                }
            })
        }
        val actionToolbar = ActionManager.getInstance().createActionToolbar("EnvForgeToolbar", group, true)
        actionToolbar.targetComponent = this
        return actionToolbar.component
    }

    private fun installVarContextMenu() {
        val group = DefaultActionGroup().apply {
            add(object : AnAction("Copy Key Name", "Copy variable key to clipboard", com.intellij.icons.AllIcons.Actions.Copy) {
                override fun actionPerformed(e: AnActionEvent) {
                    val node = varTree.lastSelectedPathComponent as? DefaultMutableTreeNode ?: return
                    val data = node.userObject as? VarData ?: return
                    copyToClipboard(data.key)
                }
                override fun update(e: AnActionEvent) {
                    val node = varTree.lastSelectedPathComponent as? DefaultMutableTreeNode
                    e.presentation.isEnabledAndVisible = node?.userObject is VarData
                }
                override fun getActionUpdateThread() = ActionUpdateThread.BGT
            })
            addSeparator()
            add(object : AnAction("Copy Value", "Copy variable value to clipboard", com.intellij.icons.AllIcons.Actions.EditSource) {
                override fun actionPerformed(e: AnActionEvent) {
                    val node = varTree.lastSelectedPathComponent as? DefaultMutableTreeNode ?: return
                    val data = node.userObject as? VarData ?: return
                    copyToClipboard(data.value)
                }
                override fun update(e: AnActionEvent) {
                    val node = varTree.lastSelectedPathComponent as? DefaultMutableTreeNode
                    e.presentation.isEnabledAndVisible = node?.userObject is VarData
                }
                override fun getActionUpdateThread() = ActionUpdateThread.BGT
            })
            addSeparator()
            add(object : AnAction("Copy KEY=VALUE", "Copy key=value pair to clipboard", com.intellij.icons.AllIcons.Nodes.Variable) {
                override fun actionPerformed(e: AnActionEvent) {
                    val node = varTree.lastSelectedPathComponent as? DefaultMutableTreeNode ?: return
                    val data = node.userObject as? VarData ?: return
                    copyToClipboard("${data.key}=${data.value}")
                }
                override fun update(e: AnActionEvent) {
                    val node = varTree.lastSelectedPathComponent as? DefaultMutableTreeNode
                    e.presentation.isEnabledAndVisible = node?.userObject is VarData
                }
                override fun getActionUpdateThread() = ActionUpdateThread.BGT
            })
        }
        PopupHandler.installPopupMenu(varTree, group, "EnvForgeVarPopup")
    }

    private fun installProfileContextMenu() {
        val group = DefaultActionGroup().apply {
            add(object : AnAction("Switch to Profile", "Activate this profile", null) {
                override fun actionPerformed(e: AnActionEvent) {
                    val node = profileTree.lastSelectedPathComponent as? DefaultMutableTreeNode ?: return
                    val info = node.userObject as? ProfileData ?: return
                    if (!info.active) {
                        EnvForgeRunner.run(project, listOf("profile", "switch", info.name), "Switch Profile") {
                            refresh()
                        }
                    }
                }
                override fun update(e: AnActionEvent) {
                    val node = profileTree.lastSelectedPathComponent as? DefaultMutableTreeNode
                    val data = node?.userObject as? ProfileData
                    e.presentation.isEnabledAndVisible = data != null && !data.active
                }
                override fun getActionUpdateThread() = ActionUpdateThread.EDT
            })
            add(object : AnAction("Diff Against Active", "Compare this profile with the active profile", null) {
                override fun actionPerformed(e: AnActionEvent) {
                    val node = profileTree.lastSelectedPathComponent as? DefaultMutableTreeNode ?: return
                    val info = node.userObject as? ProfileData ?: return
                    // Find the active profile name
                    val root = profileModel.root as? DefaultMutableTreeNode ?: return
                    val activeProfile = (0 until root.childCount)
                        .mapNotNull { (root.getChildAt(it) as? DefaultMutableTreeNode)?.userObject as? ProfileData }
                        .firstOrNull { it.active }
                    if (activeProfile != null && activeProfile.name != info.name) {
                        EnvForgeRunner.run(project, listOf("profile", "diff", activeProfile.name, info.name), "Profile Diff")
                    }
                }
                override fun update(e: AnActionEvent) {
                    val node = profileTree.lastSelectedPathComponent as? DefaultMutableTreeNode
                    val data = node?.userObject as? ProfileData
                    e.presentation.isEnabledAndVisible = data != null && !data.active
                }
                override fun getActionUpdateThread() = ActionUpdateThread.EDT
            })
            add(object : AnAction("Open Profile File", "Open the profile's .env file in editor", null) {
                override fun actionPerformed(e: AnActionEvent) {
                    val node = profileTree.lastSelectedPathComponent as? DefaultMutableTreeNode ?: return
                    val info = node.userObject as? ProfileData ?: return
                    val basePath = project.basePath ?: return
                    val vFile = LocalFileSystem.getInstance().findFileByPath("$basePath/${info.file}")
                    if (vFile != null) {
                        FileEditorManager.getInstance(project).openFile(vFile, true)
                    }
                }
                override fun update(e: AnActionEvent) {
                    val node = profileTree.lastSelectedPathComponent as? DefaultMutableTreeNode
                    e.presentation.isEnabledAndVisible = node?.userObject is ProfileData
                }
                override fun getActionUpdateThread() = ActionUpdateThread.EDT
            })
        }
        PopupHandler.installPopupMenu(profileTree, group, "EnvForgeProfilePopup")
    }

    fun refresh() {
        loadProfiles()
        loadVariables()
    }

    private fun loadProfiles() {
        val binary = EnvForgeLspFactory.findEnvforgeBinary()
        Thread {
            try {
                val process = ProcessBuilder(binary, "profile", "list")
                    .directory(project.basePath?.let { java.io.File(it) })
                    .redirectErrorStream(true)
                    .start()
                val output = process.inputStream.bufferedReader().readText()
                process.waitFor()

                val root = DefaultMutableTreeNode("Profiles")
                for (line in output.lines()) {
                    val match = Regex("""^\s+(\S+)\s+\(([^)]+)\)(.*)""").find(line) ?: continue
                    val name = match.groupValues[1]
                    val file = match.groupValues[2]
                    val active = match.groupValues[3].contains("active")
                    root.add(DefaultMutableTreeNode(ProfileData(name, file, active)))
                }

                SwingUtilities.invokeLater {
                    profileModel.setRoot(root)
                    profileModel.reload()
                    expandAll(profileTree)
                }
            } catch (_: Exception) {}
        }.start()
    }

    private fun loadVariables() {
        val binary = EnvForgeLspFactory.findEnvforgeBinary()
        Thread {
            try {
                val process = ProcessBuilder(binary, "list", "--json")
                    .directory(project.basePath?.let { java.io.File(it) })
                    .redirectErrorStream(true)
                    .start()
                val output = process.inputStream.bufferedReader().readText()
                process.waitFor()

                val vars = parseVars(output)
                val root = DefaultMutableTreeNode("Variables")

                if (grouped) {
                    val groups = groupByPrefix(vars)
                    for ((groupName, groupVars) in groups) {
                        val groupNode = DefaultMutableTreeNode("$groupName (${groupVars.size})")
                        for (v in groupVars) {
                            groupNode.add(DefaultMutableTreeNode(v))
                        }
                        root.add(groupNode)
                    }
                } else {
                    for (v in vars) {
                        root.add(DefaultMutableTreeNode(v))
                    }
                }

                SwingUtilities.invokeLater {
                    varModel.setRoot(root)
                    varModel.reload()
                    if (!grouped) expandAll(varTree)
                }
            } catch (_: Exception) {}
        }.start()
    }

    private fun parseVars(json: String): List<VarData> {
        return try {
            val array = com.google.gson.JsonParser.parseString(json).asJsonArray
            array.map { obj ->
                val o = obj.asJsonObject
                VarData(
                    key = o.get("key")?.asString ?: "",
                    value = o.get("value")?.asString ?: "",
                    sourceFile = o.get("source_file")?.asString ?: "",
                )
            }
        } catch (_: Exception) {
            emptyList()
        }
    }

    private fun groupByPrefix(vars: List<VarData>): List<Pair<String, List<VarData>>> {
        val prefixMap = mutableMapOf<String, MutableList<VarData>>()
        val ungrouped = mutableListOf<VarData>()

        for (v in vars) {
            val parts = v.key.split("_")
            if (parts.size >= 2) {
                val prefix = parts[0] + "_*"
                prefixMap.getOrPut(prefix) { mutableListOf() }.add(v)
            } else {
                ungrouped.add(v)
            }
        }

        val groups = mutableListOf<Pair<String, List<VarData>>>()
        for ((prefix, entries) in prefixMap.toSortedMap()) {
            if (entries.size >= 2) {
                groups.add(prefix to entries)
            } else {
                ungrouped.addAll(entries)
            }
        }
        if (ungrouped.isNotEmpty()) {
            groups.add("Other" to ungrouped)
        }
        return groups
    }

    private fun expandAll(tree: Tree) {
        for (i in 0 until tree.rowCount) {
            tree.expandRow(i)
        }
    }

    private fun copyToClipboard(text: String) {
        Toolkit.getDefaultToolkit().systemClipboard.setContents(StringSelection(text), null)
    }
}

data class ProfileData(val name: String, val file: String, val active: Boolean) {
    override fun toString() = if (active) "$name (active)" else name
}

data class VarData(val key: String, val value: String, val sourceFile: String) {
    override fun toString() = key
}

private val SENSITIVE_PATTERNS = listOf("SECRET", "PASSWORD", "TOKEN", "KEY", "PRIVATE", "CREDENTIAL", "AUTH")

class EnvVarCellRenderer : com.intellij.ui.ColoredTreeCellRenderer() {
    override fun customizeCellRenderer(
        tree: JTree, value: Any?, sel: Boolean, expanded: Boolean,
        leaf: Boolean, row: Int, hasFocus: Boolean
    ) {
        val node = value as? DefaultMutableTreeNode ?: return
        val data = node.userObject

        if (data is VarData) {
            val sensitive = SENSITIVE_PATTERNS.any { data.key.uppercase().contains(it) }
            val displayValue = if (sensitive && data.value.length > 4) {
                data.value.take(3) + "***"
            } else if (data.value.length > 40) {
                data.value.take(35) + "..."
            } else {
                data.value
            }

            append(data.key, com.intellij.ui.SimpleTextAttributes.REGULAR_BOLD_ATTRIBUTES)
            append(" = ", com.intellij.ui.SimpleTextAttributes.REGULAR_ATTRIBUTES)
            append(displayValue, com.intellij.ui.SimpleTextAttributes.REGULAR_ATTRIBUTES)
            icon = if (sensitive) com.intellij.icons.AllIcons.Nodes.SecurityRole else com.intellij.icons.AllIcons.Nodes.Variable
        } else {
            append(value?.toString() ?: "", com.intellij.ui.SimpleTextAttributes.REGULAR_ATTRIBUTES)
            icon = com.intellij.icons.AllIcons.Nodes.Folder
        }
    }
}

class ProfileCellRenderer : com.intellij.ui.ColoredTreeCellRenderer() {
    override fun customizeCellRenderer(
        tree: JTree, value: Any?, sel: Boolean, expanded: Boolean,
        leaf: Boolean, row: Int, hasFocus: Boolean
    ) {
        val node = value as? DefaultMutableTreeNode ?: return
        val data = node.userObject

        if (data is ProfileData) {
            append(data.name, com.intellij.ui.SimpleTextAttributes.REGULAR_BOLD_ATTRIBUTES)
            if (data.active) {
                append("  active", com.intellij.ui.SimpleTextAttributes.REGULAR_ATTRIBUTES)
            }
            icon = if (data.active) com.intellij.icons.AllIcons.Actions.Checked
                   else com.intellij.icons.AllIcons.Nodes.EmptyNode
            toolTipText = if (data.active) "Currently active" else "Double-click to switch"
        } else {
            append(value?.toString() ?: "", com.intellij.ui.SimpleTextAttributes.REGULAR_ATTRIBUTES)
        }
    }
}
