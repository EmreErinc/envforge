package com.envforge.intellij

import com.intellij.ide.BrowserUtil
import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.options.ShowSettingsUtil
import com.intellij.openapi.project.Project
import com.intellij.ui.jcef.JBCefApp
import com.intellij.ui.jcef.JBCefBrowser
import org.cef.browser.CefBrowser
import org.cef.browser.CefFrame
import org.cef.handler.CefRequestHandlerAdapter
import org.cef.network.CefRequest
import java.awt.BorderLayout
import java.awt.Font
import java.awt.Toolkit
import java.awt.datatransfer.StringSelection
import java.net.URLDecoder
import java.nio.charset.StandardCharsets
import javax.swing.*

class EnvForgeWelcomePanel(private val project: Project) : JPanel(BorderLayout()) {

    init {
        val browserComponent = if (isJbcefSupported()) {
            try {
                createJbcefBrowserComponent()
            } catch (_: Throwable) {
                createFallbackSwingPanel()
            }
        } else {
            createFallbackSwingPanel()
        }
        add(browserComponent, BorderLayout.CENTER)
    }

    private fun isJbcefSupported(): Boolean {
        return try {
            JBCefApp.isSupported()
        } catch (_: Throwable) {
            false
        }
    }

    private fun createJbcefBrowserComponent(): JComponent {
        val browser = JBCefBrowser()
        browser.jbCefClient.addRequestHandler(object : CefRequestHandlerAdapter() {
            override fun onBeforeBrowse(
                browser: CefBrowser?,
                frame: CefFrame?,
                request: CefRequest?,
                user_gesture: Boolean,
                is_redirect: Boolean
            ): Boolean {
                val url = request?.url ?: return false
                if (url.startsWith("envforge:")) {
                    handleEnvForgeUrl(url, project)
                    return true // Cancel default browser navigation for custom scheme
                }
                return false
            }
        }, browser.cefBrowser)

        browser.loadHTML(getWelcomeHtml())
        return browser.component
    }

    companion object {
        fun handleEnvForgeUrl(url: String, project: Project) {
            ApplicationManager.getApplication().invokeLater {
                when {
                    url.startsWith("envforge:downloadCli") -> {
                        EnvForgeBinaryManager.downloadAsync(project)
                    }
                    url.startsWith("envforge:installCli") -> {
                        val installCmd = "cargo install env-forge-tui"
                        copyToClipboard(installCmd)
                        EnvForgeRunner.notify(
                            project,
                            "EnvForge Installer",
                            "Copied '$installCmd' to clipboard! Run it in your terminal to install.",
                            com.intellij.notification.NotificationType.INFORMATION
                        )
                    }
                    url.startsWith("envforge:copyCommand") -> {
                        val query = url.substringAfter("?", "")
                        val text = query.split("&")
                            .firstOrNull { it.startsWith("text=") }
                            ?.substringAfter("text=")
                            ?.let { URLDecoder.decode(it, StandardCharsets.UTF_8.name()) }
                            ?: "cargo install env-forge-tui"

                        copyToClipboard(text)
                        EnvForgeRunner.notify(
                            project,
                            "EnvForge",
                            "Copied '$text' to clipboard!",
                            com.intellij.notification.NotificationType.INFORMATION
                        )
                    }
                    url.startsWith("envforge:openUrl") -> {
                        val targetUrl = url.substringAfter("url=", "https://github.com/emreerinc/envforge")
                        BrowserUtil.browse(URLDecoder.decode(targetUrl, StandardCharsets.UTF_8.name()))
                    }
                    url.startsWith("envforge:openSettings") -> {
                        ShowSettingsUtil.getInstance().showSettingsDialog(project, "EnvForge")
                    }
                }
            }
        }

        private fun copyToClipboard(text: String) {
            val selection = StringSelection(text)
            Toolkit.getDefaultToolkit().systemClipboard.setContents(selection, selection)
        }

        private fun getWelcomeHtml(): String {
            val isMac = System.getProperty("os.name").lowercase().contains("mac")
            val getStartedLabel = if (isMac) "Get started via Homebrew or Cargo:" else "Get started via Cargo:"
            val subtextInstall = if (isMac) "Or copy terminal install command (brew / cargo)" else "Or copy terminal install command (cargo)"
            val brewBoxHtml = if (isMac) """
            <div class="code-box" style="margin-bottom: 8px;">
                <span class="code-text">brew install emreerinc/tap/envforge</span>
                <a href="envforge:copyCommand?text=brew%20install%20emreerinc%2Ftap%2Fenvforge" class="icon-btn" title="Copy to clipboard">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                        <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
                    </svg>
                </a>
            </div>
            """ else ""

            return """<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Welcome to EnvForge</title>
    <style>
        :root {
            --bg-color: #06151B;
            --card-bg: #0A2229;
            --card-border: #143A44;
            --code-bg: #0D2D37;
            --text-primary: #E2E8F0;
            --text-secondary: #94A3B8;
            --accent-teal: #0D9488;
            --accent-teal-hover: #115E59;
            --accent-light: #2DD4BF;
            --accent-orange: #F97316;
        }

        body {
            background-color: var(--bg-color);
            color: var(--text-primary);
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif;
            margin: 0;
            padding: 0;
            display: flex;
            justify-content: center;
            min-height: 100vh;
            box-sizing: border-box;
        }

        .container {
            max-width: 540px;
            width: 100%;
            padding: 40px 24px;
            display: flex;
            flex-direction: column;
            gap: 20px;
        }

        .header-top {
            font-size: 11px;
            font-weight: 700;
            letter-spacing: 1.5px;
            color: var(--text-secondary);
            text-transform: uppercase;
            margin-bottom: 8px;
        }

        .logo-title {
            font-size: 32px;
            font-weight: 900;
            letter-spacing: 2px;
            color: var(--accent-orange);
            font-family: 'Courier New', Courier, monospace;
            text-shadow: 0 0 10px rgba(249, 115, 22, 0.3);
            margin: 0 0 16px 0;
        }

        .welcome-heading {
            font-size: 22px;
            font-weight: 700;
            color: #FFFFFF;
            margin: 0 0 8px 0;
        }

        .description {
            font-size: 14px;
            line-height: 1.6;
            color: var(--text-secondary);
            margin: 0 0 16px 0;
        }

        .sub-label {
            font-size: 13px;
            color: var(--text-secondary);
            margin-bottom: 8px;
        }

        .code-box {
            background-color: var(--code-bg);
            border: 1px solid var(--card-border);
            border-radius: 6px;
            padding: 10px 14px;
            display: flex;
            align-items: center;
            justify-content: space-between;
            font-family: 'Fira Code', 'Consolas', 'Courier New', monospace;
            font-size: 13px;
            color: var(--accent-light);
        }

        .code-text {
            user-select: all;
        }

        .icon-btn {
            background: transparent;
            border: none;
            color: var(--text-secondary);
            cursor: pointer;
            padding: 4px;
            border-radius: 4px;
            display: flex;
            align-items: center;
            justify-content: center;
            transition: color 0.15s, background-color 0.15s;
        }

        .icon-btn:hover {
            color: #FFFFFF;
            background-color: rgba(255, 255, 255, 0.1);
        }

        .btn-primary {
            background-color: var(--accent-teal);
            color: #FFFFFF;
            border: none;
            border-radius: 6px;
            padding: 12px 16px;
            font-size: 14px;
            font-weight: 600;
            cursor: pointer;
            width: 100%;
            transition: background-color 0.2s ease;
            box-shadow: 0 2px 8px rgba(13, 148, 136, 0.25);
            text-align: center;
            box-sizing: border-box;
            text-decoration: none;
            display: block;
        }

        .btn-primary:hover {
            background-color: var(--accent-teal-hover);
        }

        .card {
            background-color: var(--card-bg);
            border: 1px solid var(--card-border);
            border-radius: 10px;
            padding: 20px;
            display: flex;
            flex-direction: column;
            gap: 10px;
        }

        .card-title {
            font-size: 15px;
            font-weight: 700;
            color: #FFFFFF;
            margin: 0;
        }

        .card-desc {
            font-size: 13px;
            line-height: 1.5;
            color: var(--text-secondary);
            margin: 0;
        }

        .link-text {
            color: var(--accent-light);
            text-decoration: none;
            font-size: 13px;
            font-weight: 500;
            cursor: pointer;
            display: inline-block;
            margin-top: 4px;
        }

        .link-text:hover {
            text-decoration: underline;
        }

        .footer {
            text-align: center;
            margin-top: 12px;
            padding-top: 16px;
            border-top: 1px solid rgba(255, 255, 255, 0.06);
        }

        .footer-link {
            color: var(--accent-light);
            text-decoration: none;
            font-size: 13px;
        }

        .footer-link:hover {
            text-decoration: underline;
        }

        .options-row {
            display: flex;
            justify-content: center;
            gap: 16px;
            margin-top: 8px;
            font-size: 12px;
        }
    </style>
</head>
<body>
    <div class="container">
        <div>
            <div class="header-top">ENVFORGE: WELCOME</div>
            <div class="logo-title">envforge</div>
            <h1 class="welcome-heading">Welcome to EnvForge</h1>
            <p class="description">
                Environment manager with profiles (dev / staging / prod), .env.schema validation, and a TUI in the terminal. This plugin adds a tool window and LSP; fence/guard need the CLI.
            </p>
        </div>

        <div>
            <div class="sub-label">$getStartedLabel</div>
            $brewBoxHtml
            <div class="code-box">
                <span class="code-text">cargo install env-forge-tui</span>
                <a href="envforge:copyCommand?text=cargo%20install%20env-forge-tui" class="icon-btn" title="Copy to clipboard">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                        <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
                    </svg>
                </a>
            </div>
        </div>

        <a href="envforge:downloadCli" class="btn-primary">Auto-Download EnvForge CLI Binary</a>
        <div style="text-align: center; margin-top: 4px;">
            <a href="envforge:installCli" class="link-text">$subtextInstall</a>
        </div>

        <div class="card">
            <h2 class="card-title">Standalone (without CLI)</h2>
            <p class="card-desc">You can use EnvForge natively inside your IDE without downloading any external CLI binary:</p>
            <ul style="margin: 8px 0 8px 18px; padding: 0; font-size: 13px; color: var(--text-secondary); line-height: 1.6;">
                <li><b>Native Syntax Highlighting:</b> Built-in colorizer for <code style="color: var(--accent-light);">.env</code> and <code style="color: var(--accent-light);">.env.schema</code> files.</li>
                <li><b>File Branding & Decorators:</b> EnvForge file icons, project view badges, and sidebar tool windows.</li>
                <li><b>Schema Templates:</b> Fast creation and manual editing of environment schema definitions.</li>
            </ul>
        </div>

        <div class="card">
            <h2 class="card-title">With the EnvForge CLI</h2>
            <p class="card-desc">Downloading the binary or installing via <code style="color: var(--accent-light);">brew install emreerinc/tap/envforge</code> / <code style="color: var(--accent-light);">cargo install env-forge-tui</code> unlocks:</p>
            <ul style="margin: 8px 0 8px 18px; padding: 0; font-size: 13px; color: var(--text-secondary); line-height: 1.6;">
                <li><b>Real-Time LSP Diagnostics:</b> Inline error checking, type validation, and enum enforcement.</li>
                <li><b>Fence &amp; Guard:</b> Fence writes ignore/rules for configured AI tools (not a sandbox). Guard scans .env on save.</li>
                <li><b>Hover Cards &amp; Auto-Completion:</b> Instant schema metadata, default values, and key completion.</li>
                <li><b>Multi-Profile Switching:</b> Double-click to switch between <code style="color: var(--accent-light);">dev</code>, <code style="color: var(--accent-light);">staging</code>, and <code style="color: var(--accent-light);">prod</code> profiles.</li>
                <li><b>Terminal TUI &amp; Exports:</b> Full-screen TUI (<code style="color: var(--accent-light);">envforge</code>) + export to Docker, K8s, Terraform.</li>
            </ul>
        </div>

        <div class="card">
            <h2 class="card-title">Did you know?</h2>
            <p class="card-desc" id="tipDesc">
                Loading tip...
            </p>
            <div class="code-box">
                <span class="code-text" id="tipCode">envforge doctor</span>
                <a id="copyTipLink" href="envforge:copyCommand?text=envforge%20doctor" class="icon-btn" title="Copy tip command">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                        <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
                    </svg>
                </a>
            </div>
            <a href="envforge:openUrl?url=https%3A%2F%2Fgithub.com%2Femreerinc%2Fenvforge%23readme" class="link-text">Open CLI Commands Reference</a>
        </div>

        <div class="footer">
            <div class="options-row">
                <a href="envforge:openSettings" class="link-text">Configure Executable Path</a>
            </div>
            <div style="margin-top: 12px;">
                <a href="envforge:openUrl?url=https%3A%2F%2Fgithub.com%2Femreerinc%2Fenvforge" class="footer-link">Learn more at github.com/emreerinc/envforge</a>
            </div>
        </div>
    </div>

    <script>
        const tips = [
            {
                desc: "You can run health checks to verify your environment setup and secret providers:",
                code: "envforge doctor"
            },
            {
                desc: "Fence writes ignore/rules for configured AI tools so they are less likely to ingest .env files. Not a sandbox:",
                code: "envforge fence"
            },
            {
                desc: "You can run commands with volatile secret access (secrets kept in memory only, never written to disk):",
                code: "envforge run --volatile -- npm start"
            },
            {
                desc: "Scan MCP config files for hardcoded credentials:",
                code: "envforge mcp status"
            },
            {
                desc: "Scan git commit history to audit and detect AI-assisted secret leaks across your repository:",
                code: "envforge audit --ai-leaks"
            },
            {
                desc: "Create honeypot canary credentials in your environment files to detect secret exfiltration:",
                code: "envforge canary create DB_CANARY_KEY"
            },
            {
                desc: "Auto-generate a type-safe .env.schema from your existing environment variables:",
                code: "envforge schema generate"
            },
            {
                desc: "Switch between development, staging, and production environment profiles instantly:",
                code: "envforge profile switch production"
            },
            {
                desc: "Redact secrets automatically in subprocess logs during command execution:",
                code: "envforge run --redact -- ./deploy.sh"
            }
        ];

        function loadRandomTip() {
            const selected = tips[Math.floor(Math.random() * tips.length)];
            document.getElementById('tipDesc').textContent = selected.desc;
            document.getElementById('tipCode').textContent = selected.code;
            document.getElementById('copyTipLink').href = 'envforge:copyCommand?text=' + encodeURIComponent(selected.code);
        }

        loadRandomTip();
    </script>
</body>
</html>"""
        }
    }

    private fun createFallbackSwingPanel(): JPanel {
        val panel = JPanel()
        panel.layout = BoxLayout(panel, BoxLayout.Y_AXIS)
        panel.border = BorderFactory.createEmptyBorder(30, 30, 30, 30)

        val title = JLabel("ENVFORGE: WELCOME").apply {
            font = font.deriveFont(Font.BOLD, 18f)
        }
        val desc = JLabel("<html>Profiles, .env.schema, and a TUI in the terminal.<br>This plugin adds a tool window and LSP. CLI: <b>brew install emreerinc/tap/envforge</b> or <b>cargo install env-forge-tui</b></html>").apply {
            font = font.deriveFont(Font.PLAIN, 13f)
        }

        val downloadBtn = JButton("Auto-Download CLI Binary").apply {
            addActionListener {
                EnvForgeBinaryManager.downloadAsync(project)
            }
        }

        val copyBtn = JButton("Copy Terminal Command").apply {
            addActionListener {
                copyToClipboard("cargo install env-forge-tui")
                EnvForgeRunner.notify(project, "EnvForge", "Copied 'cargo install env-forge-tui' to clipboard!", com.intellij.notification.NotificationType.INFORMATION)
            }
        }

        panel.add(title)
        panel.add(Box.createVerticalStrut(15))
        panel.add(desc)
        panel.add(Box.createVerticalStrut(20))
        panel.add(downloadBtn)
        panel.add(Box.createVerticalStrut(10))
        panel.add(copyBtn)

        return panel
    }
}
