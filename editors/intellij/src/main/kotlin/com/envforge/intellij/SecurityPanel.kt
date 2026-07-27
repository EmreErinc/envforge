package com.envforge.intellij

import com.google.gson.JsonParser
import com.intellij.openapi.actionSystem.*
import com.intellij.openapi.project.Project
import com.intellij.openapi.wm.ToolWindow
import com.intellij.openapi.wm.ToolWindowFactory
import com.intellij.openapi.project.DumbAware
import com.intellij.ui.components.JBScrollPane
import com.intellij.ui.content.ContentFactory
import com.intellij.ui.treeStructure.Tree
import java.awt.BorderLayout
import java.awt.event.MouseAdapter
import java.awt.event.MouseEvent
import javax.swing.*
import javax.swing.tree.DefaultMutableTreeNode
import javax.swing.tree.DefaultTreeModel
import java.awt.Component

// Class is used by EnvForgeToolWindowFactory
class SecurityPanel(private val project: Project) : JPanel(BorderLayout()) {
    private val model = DefaultTreeModel(DefaultMutableTreeNode("Security"))
    private val tree = Tree(model)

    init {
        val toolbar = createToolbar()
        add(toolbar, BorderLayout.NORTH)
        add(JBScrollPane(tree), BorderLayout.CENTER)

        // Use ColoredTreeCellRenderer to match Variables tab exactly
        tree.cellRenderer = SecurityCellRenderer()

        tree.addMouseListener(object : MouseAdapter() {
            override fun mouseClicked(e: MouseEvent) {
                if (e.clickCount == 2) {
                    val node = tree.lastSelectedPathComponent as? DefaultMutableTreeNode ?: return
                    val data = node.userObject as? SecurityItem ?: return
                    if (data.actionId != null) {
                        val action = ActionManager.getInstance().getAction(data.actionId)
                        if (action != null) {
                            action.actionPerformed(
                                AnActionEvent.createFromAnAction(action, null, "SecurityPanel", 
                                    ActionManager.getInstance().createActionToolbar("SecurityPanel", DefaultActionGroup(action), true).context_as_data_context())
                            )
                        }
                    }
                }
            }
        })

        installContextMenu()
        refresh()
    }

    private fun ActionToolbar.context_as_data_context(): DataContext = DataContext.EMPTY_CONTEXT

    fun refresh() {
        val binary = try { EnvForgeLspFactory.findEnvforgeBinary() } catch (_: Exception) { "" }
        if (binary.isEmpty()) {
            val root = DefaultMutableTreeNode("Security")
            root.add(DefaultMutableTreeNode("EnvForge CLI is disabled or not found — Run 'cargo install env-forge-tui'"))
            SwingUtilities.invokeLater {
                model.setRoot(root)
                model.reload()
            }
            return
        }
        Thread {
            try {
                val workDir = project.basePath?.let { java.io.File(it) }

                val fenceJson = runCli(binary, listOf("fence", "--status", "--json"), workDir)
                val guardJson = runCli(binary, listOf("ai-hook", "status", "--json"), workDir)
                val mcpJson = runCli(binary, listOf("mcp", "status", "--json"), workDir)
                val canaryJson = runCli(binary, listOf("canary", "list", "--json"), workDir)

                val root = DefaultMutableTreeNode("Security")

                root.add(buildFenceNode(fenceJson))
                root.add(buildGuardNode(guardJson))
                root.add(buildLifecycleNode())
                root.add(buildAnalyticsNode())
                root.add(buildMcpNode(mcpJson))
                root.add(buildCanaryNode(canaryJson))

                SwingUtilities.invokeLater {
                    model.setRoot(root)
                    model.reload()
                    for (i in 0 until tree.rowCount) tree.expandRow(i)
                }
            } catch (_: Exception) {}
        }.start()
    }

    private fun runCli(binary: String, args: List<String>, workDir: java.io.File?): String? {
        return try {
            val process = ProcessBuilder(binary).apply {
                command().addAll(args)
                if (workDir != null) directory(workDir)
                redirectErrorStream(true)
            }.start()
            val output = process.inputStream.bufferedReader().readText()
            process.waitFor()
            if (process.exitValue() == 0 && output.trim().isNotEmpty()) output else null
        } catch (_: Exception) { null }
    }

    private fun buildFenceNode(json: String?): DefaultMutableTreeNode {
        val node = DefaultMutableTreeNode(SecurityItem("Fence", "Fence", null, null))
        if (json == null) {
            node.add(DefaultMutableTreeNode(SecurityItem("Status", "Unable to load", null, null)))
            return node
        }
        try {
            val obj = JsonParser.parseString(json).asJsonObject
            val active = obj.get("all_fenced")?.asBoolean ?: false
            node.userObject = SecurityItem("Fence", "Fence", if (active) "Active" else "Inactive", "EnvForge.ToggleFence")
            node.add(DefaultMutableTreeNode(SecurityItem("All Fenced", if (active) "Yes" else "No", null, null)))
            val files = obj.getAsJsonArray("files")
            if (files != null) {
                for (i in 0 until minOf(files.size(), 20)) {
                    val f = files[i].asJsonObject
                    val path = f.get("path").asString
                    val fenced = f.get("fenced").asBoolean
                    node.add(DefaultMutableTreeNode(SecurityItem(path, if (fenced) "Fenced" else "Exposed", null, null)))
                }
            }
        } catch (_: Exception) {
            node.add(DefaultMutableTreeNode(SecurityItem("Status", "Parse error", null, null)))
        }
        return node
    }

    private fun buildLifecycleNode(): DefaultMutableTreeNode {
        val node = DefaultMutableTreeNode(SecurityItem("Lifecycle", "Lifecycle", "Governance", null))
        node.add(DefaultMutableTreeNode(SecurityItem("Run Lifecycle Check", "Evaluate rules", null, "EnvForge.RunLifecycleCheck")))
        node.add(DefaultMutableTreeNode(SecurityItem("Manage Rules", "Lifecycle rule list", null, "EnvForge.ManageLifecycleRules")))
        node.add(DefaultMutableTreeNode(SecurityItem("Audit Trail", "View sync & access history", null, "EnvForge.ViewAuditTrail")))
        return node
    }

    private fun buildAnalyticsNode(): DefaultMutableTreeNode {
        val node = DefaultMutableTreeNode(SecurityItem("Analytics", "Analytics", "Usage", null))
        node.add(DefaultMutableTreeNode(SecurityItem("Show Unused Secrets", "Dormant for 90 days", null, "EnvForge.ShowUnusedSecrets")))
        node.add(DefaultMutableTreeNode(SecurityItem("Usage Summary", "Event & secret counts", null, "EnvForge.ShowUsageSummary")))
        node.add(DefaultMutableTreeNode(SecurityItem("Monitor Stream", "Real-time access events", null, "EnvForge.MonitorStream")))
        return node
    }

    private fun buildGuardNode(json: String?): DefaultMutableTreeNode {
        val node = DefaultMutableTreeNode(SecurityItem("Guard", "Guard", null, null))
        if (json == null) {
            node.add(DefaultMutableTreeNode(SecurityItem("Status", "Unable to load", null, null)))
            return node
        }
        try {
            val obj = JsonParser.parseString(json).asJsonObject
            val enabled = obj.get("enabled")?.asBoolean ?: false
            node.userObject = SecurityItem("Guard", "Guard", if (enabled) "Enabled" else "Disabled", "EnvForge.ToggleGuard")
            node.add(DefaultMutableTreeNode(SecurityItem("Hooks Installed", if (enabled) "Yes" else "No", null, null)))
            val tools = obj.getAsJsonArray("tools")
            if (tools != null) {
                for (i in 0 until tools.size()) {
                    val t = tools[i].asJsonObject
                    val name = t.get("name").asString
                    val installed = t.get("installed").asBoolean
                    node.add(DefaultMutableTreeNode(SecurityItem(name, if (installed) "Active" else "Missing", null, null)))
                }
            }
        } catch (_: Exception) {
            node.add(DefaultMutableTreeNode(SecurityItem("Status", "Parse error", null, null)))
        }
        return node
    }

    private fun buildMcpNode(json: String?): DefaultMutableTreeNode {
        val node = DefaultMutableTreeNode(SecurityItem("MCP Security", "MCP Scan", null, null))
        if (json == null) {
            node.add(DefaultMutableTreeNode(SecurityItem("Status", "Not yet scanned", null, null)))
            return node
        }
        try {
            val obj = JsonParser.parseString(json).asJsonObject
            val findings = obj.get("total_findings")?.asInt ?: 0
            val files = obj.get("vulnerable_files")?.asInt ?: 0
            node.userObject = SecurityItem("MCP Security", "MCP Scan", if (findings > 0) "$findings issues" else "Clean", "EnvForge.RunMcpScan")
            node.add(DefaultMutableTreeNode(SecurityItem("Vulnerable Files", files.toString(), null, null)))
            node.add(DefaultMutableTreeNode(SecurityItem("Total Secrets Found", findings.toString(), null, null)))
            
            val details = obj.getAsJsonArray("findings")
            if (details != null) {
                for (i in 0 until minOf(details.size(), 10)) {
                    val f = details[i].asJsonObject
                    val key = f.get("key").asString
                    val file = f.get("file").asString.split("/").last()
                    node.add(DefaultMutableTreeNode(SecurityItem("Issue", "$key in $file", null, null)))
                }
            }
        } catch (_: Exception) {
            node.add(DefaultMutableTreeNode(SecurityItem("Status", "Parse error", null, null)))
        }
        return node
    }

    private fun buildCanaryNode(json: String?): DefaultMutableTreeNode {
        val node = DefaultMutableTreeNode(SecurityItem("Canary Tokens", "Canary Tokens", null, null))
        if (json == null) {
            node.add(DefaultMutableTreeNode(SecurityItem("Status", "Unable to load", null, null)))
            return node
        }
        try {
            val arr = JsonParser.parseString(json).asJsonArray
            node.userObject = SecurityItem("Canary Tokens", "Canary Tokens", "${arr.size()} tokens", "EnvForge.AddCanary")
            if (arr.size() == 0) {
                node.add(DefaultMutableTreeNode(SecurityItem("No canary tokens", "Add one to detect exfiltration", null, null)))
            }
            for (i in 0 until minOf(arr.size(), 50)) {
                val obj = arr[i].asJsonObject
                val key = obj.get("key").asString
                val triggered = obj.get("triggered").asBoolean
                node.add(DefaultMutableTreeNode(SecurityItem(
                    key, if (triggered) "TRIGGERED" else "Clean", null,
                    if (!triggered) "EnvForge.RemoveCanary" else null
                )))
            }
        } catch (_: Exception) {
            node.add(DefaultMutableTreeNode(SecurityItem("Status", "Parse error", null, null)))
        }
        return node
    }

    private fun createToolbar(): JComponent {
        val group = DefaultActionGroup().apply {
            add(object : AnAction("Refresh", "Refresh security status", com.intellij.icons.AllIcons.Actions.Refresh) {
                override fun actionPerformed(e: AnActionEvent) = refresh()
            })
        }
        val actionToolbar = ActionManager.getInstance().createActionToolbar("EnvForgeSecurityToolbar", group, true)
        actionToolbar.targetComponent = this
        return actionToolbar.component
    }

    private fun installContextMenu() {
        val group = DefaultActionGroup().apply {
            add(object : AnAction("Toggle Status", "Active or disable security feature", null) {
                override fun actionPerformed(e: AnActionEvent) {
                    val node = tree.lastSelectedPathComponent as? DefaultMutableTreeNode ?: return
                    val data = node.userObject as? SecurityItem ?: return
                    if (data.actionId != null) {
                        val action = ActionManager.getInstance().getAction(data.actionId)
                        action?.actionPerformed(e)
                        refresh()
                    }
                }
                override fun update(e: AnActionEvent) {
                    val node = tree.lastSelectedPathComponent as? DefaultMutableTreeNode
                    val data = node?.userObject as? SecurityItem
                    e.presentation.isEnabledAndVisible = data?.actionId != null
                }
            })
        }
        com.intellij.ui.PopupHandler.installPopupMenu(tree, group, "EnvForgeSecurityPopup")
    }
}

data class SecurityItem(val label: String, val category: String, val description: String?, val actionId: String?) {
    override fun toString(): String {
        return if (description != null) "$label ($description)" else label
    }
}

class SecurityCellRenderer : com.intellij.ui.ColoredTreeCellRenderer() {
    override fun customizeCellRenderer(
        tree: JTree, value: Any?, sel: Boolean, expanded: Boolean,
        leaf: Boolean, row: Int, hasFocus: Boolean
    ) {
        val node = value as? DefaultMutableTreeNode ?: return
        val item = node.userObject as? SecurityItem ?: return

        append(item.label, com.intellij.ui.SimpleTextAttributes.REGULAR_BOLD_ATTRIBUTES)
        if (item.description != null) {
            append(" — ", com.intellij.ui.SimpleTextAttributes.REGULAR_ATTRIBUTES)
            append(item.description, com.intellij.ui.SimpleTextAttributes.GRAY_ATTRIBUTES)
        }

        icon = when {
            item.label == "TRIGGERED" -> com.intellij.icons.AllIcons.General.Error
            item.label == "Issue" -> com.intellij.icons.AllIcons.General.Warning
            item.category == "Fence" && item.description == "Inactive" -> com.intellij.icons.AllIcons.General.Warning
            item.category == "Guard" && item.description == "Disabled" -> com.intellij.icons.AllIcons.General.Warning
            item.category == "Canary Tokens" && item.description != null && item.description.endsWith("tokens") -> com.intellij.icons.AllIcons.Nodes.SecurityRole
            expanded -> com.intellij.icons.AllIcons.Nodes.Folder
            else -> com.intellij.icons.AllIcons.Nodes.Variable
        }

        toolTipText = if (item.actionId != null) "Double-click to execute" else null
    }
}
