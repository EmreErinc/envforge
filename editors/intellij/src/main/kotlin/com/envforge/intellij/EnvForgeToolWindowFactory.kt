package com.envforge.intellij

import com.intellij.openapi.actionSystem.*
import com.intellij.openapi.fileEditor.FileEditorManager
import com.intellij.openapi.project.DumbAware
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.LocalFileSystem
import com.intellij.openapi.wm.ToolWindow
import com.intellij.openapi.wm.ToolWindowFactory
import com.intellij.ui.components.JBScrollPane
import com.intellij.ui.components.JBTabbedPane
import com.intellij.ui.content.ContentFactory
import com.intellij.ui.PopupHandler
import com.intellij.ui.SearchTextField
import com.intellij.ui.treeStructure.Tree
import com.intellij.ui.JBColor
import com.intellij.openapi.ui.Messages
import com.intellij.execution.RunManager
import com.intellij.notification.NotificationType
import java.awt.BorderLayout
import java.awt.datatransfer.StringSelection
import java.awt.Toolkit
import java.awt.event.MouseAdapter
import java.awt.event.MouseEvent
import javax.swing.*
import javax.swing.tree.DefaultMutableTreeNode
import javax.swing.tree.DefaultTreeModel
import java.awt.Component

class EnvForgeToolWindowFactory : ToolWindowFactory, DumbAware {
    override fun createToolWindowContent(project: Project, toolWindow: ToolWindow) {
        val panel = EnvForgeMainPanel(project)
        val content = ContentFactory.getInstance().createContent(panel, "", false)
        toolWindow.contentManager.addContent(content)
    }
}

class EnvForgeMainPanel(private val project: Project) : JPanel(BorderLayout()) {
    init {
        val tabbedPane = JBTabbedPane()
        tabbedPane.addTab("Variables", EnvForgeToolWindowPanel(project))
        tabbedPane.addTab("Profiles", ProfilesPanel(project))
        tabbedPane.addTab("Security", SecurityPanel(project))
        add(tabbedPane, BorderLayout.CENTER)
    }
}

class EnvForgeToolWindowPanel(private val project: Project) : JPanel(BorderLayout()) {
    private val varModel = DefaultTreeModel(DefaultMutableTreeNode("Variables"))
    private val varTree = Tree(varModel)
    private var grouped = true
    private val searchField = SearchTextField()

    init {
        // Top Panel: Search + Toolbar
        val topPanel = JPanel(BorderLayout())
        topPanel.add(searchField, BorderLayout.CENTER)
        val toolbar = createToolbar()
        topPanel.add(toolbar, BorderLayout.EAST)
        add(topPanel, BorderLayout.NORTH)

        // Variables tree
        add(JBScrollPane(varTree).apply {
            border = BorderFactory.createTitledBorder("Variables")
        }, BorderLayout.CENTER)

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

        installVarContextMenu()

        varTree.cellRenderer = EnvVarCellRenderer()

        refresh()
    }

    private fun createToolbar(): JComponent {
        val group = DefaultActionGroup().apply {
            add(createGearMenu())
            addSeparator()
            add(object : AnAction("Load into Run Configs", "Inject env vars into IDE run configurations", com.intellij.icons.AllIcons.Actions.Execute) {
                override fun actionPerformed(e: AnActionEvent) {
                    val confirmed = Messages.showYesNoDialog(
                        project,
                        "This will overwrite environment variables in all run configurations. Continue?",
                        "Load into Run Configurations",
                        Messages.getQuestionIcon()
                    )
                    if (confirmed == Messages.YES) {
                        loadAndInjectVariables()
                    }
                }
            })
            addSeparator()
            add(object : AnAction("Toggle Grouping", "Toggle variable grouping", com.intellij.icons.AllIcons.Actions.GroupBy) {
                override fun actionPerformed(e: AnActionEvent) {
                    grouped = !grouped
                    if (searchField.text.isEmpty()) {
                        loadVariables()
                    } else {
                        searchVariables(searchField.text)
                    }
                }
            })
        }
        val actionToolbar = ActionManager.getInstance().createActionToolbar("EnvForgeToolbar", group, true)
        actionToolbar.targetComponent = this
        return actionToolbar.component
    }

    private fun createGearMenu(): ActionGroup {
        val gearGroup = DefaultActionGroup("Actions", true).apply {
            templatePresentation.icon = com.intellij.icons.AllIcons.General.Settings
        }

        gearGroup.add(object : AnAction("Refresh All", "Refresh everything", com.intellij.icons.AllIcons.Actions.Refresh) {
            override fun actionPerformed(e: AnActionEvent) = refresh()
        })

        gearGroup.addSeparator()

        gearGroup.add(object : AnAction("Add Variable...", "Set a new variable", com.intellij.icons.AllIcons.General.Add) {
            override fun actionPerformed(e: AnActionEvent) {
                val input = Messages.showInputDialog(project, "Enter key=value:", "Add Variable", null)
                if (!input.isNullOrBlank() && input.contains("=")) {
                    EnvForgeRunner.run(project, listOf("set", input.trim()), "Add Variable") {
                        SwingUtilities.invokeLater { refresh() }
                    }
                }
            }
        })

        return gearGroup
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
            add(object : AnAction("Copy key=value", "Copy full assignment to clipboard", com.intellij.icons.AllIcons.Actions.Copy) {
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

            addSeparator()

            add(object : AnAction("Edit Value...", "Update variable value", com.intellij.icons.AllIcons.Actions.Edit) {
                override fun actionPerformed(e: AnActionEvent) {
                    val node = varTree.lastSelectedPathComponent as? DefaultMutableTreeNode ?: return
                    val data = node.userObject as? VarData ?: return

                    val newValue = Messages.showInputDialog(
                        project,
                        "New value for ${data.key}:",
                        "Edit Value",
                        null,
                        data.value,
                        null
                    )

                    if (newValue != null && newValue != data.value) {
                        EnvForgeRunner.run(project, listOf("set", "${data.key}=$newValue"), "Edit Value") {
                            SwingUtilities.invokeLater { refresh() }
                        }
                    }
                }
                override fun update(e: AnActionEvent) {
                    val node = varTree.lastSelectedPathComponent as? DefaultMutableTreeNode
                    e.presentation.isEnabledAndVisible = node?.userObject is VarData
                }
                override fun getActionUpdateThread() = ActionUpdateThread.BGT
            })

            add(object : AnAction("Rename Variable...", "Change variable name", com.intellij.icons.AllIcons.Actions.RefactoringBulb) {
                override fun actionPerformed(e: AnActionEvent) {
                    val node = varTree.lastSelectedPathComponent as? DefaultMutableTreeNode ?: return
                    val data = node.userObject as? VarData ?: return

                    val newKey = Messages.showInputDialog(
                        project,
                        "New name for ${data.key}:",
                        "Rename Variable",
                        null,
                        data.key,
                        null
                    )

                    if (!newKey.isNullOrBlank() && newKey != data.key) {
                        EnvForgeRunner.run(project, listOf("move", data.key, newKey), "Rename Variable") {
                            SwingUtilities.invokeLater { refresh() }
                        }
                    }
                }
                override fun update(e: AnActionEvent) {
                    val node = varTree.lastSelectedPathComponent as? DefaultMutableTreeNode
                    e.presentation.isEnabledAndVisible = node?.userObject is VarData
                }
                override fun getActionUpdateThread() = ActionUpdateThread.BGT
            })

            addSeparator()

            add(object : AnAction("Delete Variable", "Remove variable from config", com.intellij.icons.AllIcons.Actions.GC) {
                override fun actionPerformed(e: AnActionEvent) {
                    val node = varTree.lastSelectedPathComponent as? DefaultMutableTreeNode ?: return
                    val data = node.userObject as? VarData ?: return

                    val confirm = Messages.showYesNoDialog(
                        project,
                        "Are you sure you want to delete '${data.key}'?",
                        "Delete Variable",
                        Messages.getQuestionIcon()
                    )

                    if (confirm == Messages.YES) {
                        EnvForgeRunner.run(project, listOf("delete", data.key), "Delete Variable") {
                            SwingUtilities.invokeLater { refresh() }
                        }
                    }
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

    fun refresh() {
        if (searchField.text.isEmpty()) {
            loadVariables()
        } else {
            searchVariables(searchField.text)
        }
    }

    private fun loadVariables() {
        val binary = EnvForgeLspFactory.findEnvforgeBinary()
        Thread {
            try {
                val process = ProcessBuilder(binary, "list", "--json", "--reveal")
                    .directory(project.basePath?.let { java.io.File(it) })
                    .redirectErrorStream(true)
                    .start()
                val output = process.inputStream.bufferedReader().readText()
                process.waitFor()

                val vars = parseVars(output)
                updateVarTree(vars)
            } catch (_: Exception) {}
        }.start()
    }

    private fun searchVariables(query: String) {
        val binary = EnvForgeLspFactory.findEnvforgeBinary()
        Thread {
            try {
                val process = ProcessBuilder(binary, "search", query, "--json", "--reveal")
                    .directory(project.basePath?.let { java.io.File(it) })
                    .redirectErrorStream(true)
                    .start()
                val output = process.inputStream.bufferedReader().readText()
                process.waitFor()

                val vars = parseVars(output)
                updateVarTree(vars)
            } catch (_: Exception) {}
        }.start()
    }

    private fun updateVarTree(vars: List<VarData>) {
        val root = DefaultMutableTreeNode("Variables")
        if (grouped && searchField.text.isEmpty()) {
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
            if (!grouped || searchField.text.isNotEmpty()) expandAll(varTree)
        }
    }

    private fun loadAndInjectVariables() {
        val binary = EnvForgeLspFactory.findEnvforgeBinary()
        Thread {
            try {
                // Get all environment variables from envforge
                val process = ProcessBuilder(binary, "list", "--json", "--reveal")
                    .directory(project.basePath?.let { java.io.File(it) })
                    .redirectErrorStream(true)
                    .start()
                val output = process.inputStream.bufferedReader().readText()
                process.waitFor()

                val vars = parseVars(output)
                if (vars.isNotEmpty()) {
                    // Inject into all run configurations
                    SwingUtilities.invokeLater {
                        injectToRunConfigurations(vars)
                        EnvForgeRunner.notify(
                            project,
                            "EnvForge",
                            "✅ Loaded ${vars.size} environment variables into run configurations",
                            NotificationType.INFORMATION
                        )
                    }
                }
            } catch (e: Exception) {
                SwingUtilities.invokeLater {
                    EnvForgeRunner.notify(
                        project,
                        "EnvForge Error",
                        "Failed to inject environment variables: ${e.message}",
                        NotificationType.ERROR
                    )
                }
            }
        }.start()
    }

    private fun injectToRunConfigurations(vars: List<VarData>) {
        try {
            val runManager = RunManager.getInstance(project)
            val envMap = mutableMapOf<String, String>()

            // Convert VarData list to environment variable map
            for (v in vars) {
                envMap[v.key] = v.value
            }

            // Inject into all run configurations
            val allConfigs = runManager.allSettings
            for (config in allConfigs) {
                val runConfig = config.configuration
                val getEnvs = runConfig.javaClass.methods.firstOrNull { it.name == "getEnvs" && it.parameterCount == 0 }
                    ?: continue
                val setEnvs = runConfig.javaClass.methods.firstOrNull { it.name == "setEnvs" && it.parameterCount == 1 }
                    ?: continue

                @Suppress("UNCHECKED_CAST")
                val currentEnv = ((getEnvs.invoke(runConfig) as? Map<String, String>) ?: emptyMap()).toMutableMap()
                currentEnv.putAll(envMap)
                setEnvs.invoke(runConfig, currentEnv)

                val setPassParentEnvs = runConfig.javaClass.methods.firstOrNull {
                    it.name == "setPassParentEnvs" && it.parameterCount == 1
                }
                setPassParentEnvs?.invoke(runConfig, true)
            }
        } catch (e: Exception) {
            // Silently fail - not all run configs support env injection
        }
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
