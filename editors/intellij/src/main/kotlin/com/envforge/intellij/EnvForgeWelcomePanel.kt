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
        if (JBCefApp.isSupported()) {
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
            add(browser.component, BorderLayout.CENTER)
        } else {
            add(createFallbackSwingPanel(), BorderLayout.CENTER)
        }
    }

    companion object {
        fun handleEnvForgeUrl(url: String, project: Project) {
            ApplicationManager.getApplication().invokeLater {
                when {
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
                AI-native environment security and secret management system. Protect secrets from AI coding agents, enforce schema validation, and manage multi-profile environment variables.
            </p>
        </div>

        <div>
            <div class="sub-label">Get started by running:</div>
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

        <a href="envforge:installCli" class="btn-primary">Install EnvForge CLI</a>

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
                desc: "EnvForge can prevent AI coding agents (Cursor, Copilot, Claude Code, Windsurf) from reading raw credentials in your workspace:",
                code: "envforge fence"
            },
            {
                desc: "You can run commands with volatile secret access (secrets kept in memory only, never written to disk):",
                code: "envforge run --volatile -- npm start"
            },
            {
                desc: "Scan AI tool configuration files (Cursor, Claude, Windsurf) for hardcoded plaintext credentials:",
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
        val desc = JLabel("<html>AI-native environment security and secret management system.<br>Requires the EnvForge CLI. Run: <b>cargo install env-forge-tui</b></html>").apply {
            font = font.deriveFont(Font.PLAIN, 13f)
        }

        val copyBtn = JButton("Copy Install Command").apply {
            addActionListener {
                copyToClipboard("cargo install env-forge-tui")
                EnvForgeRunner.notify(project, "EnvForge", "Copied 'cargo install env-forge-tui' to clipboard!", com.intellij.notification.NotificationType.INFORMATION)
            }
        }

        panel.add(title)
        panel.add(Box.createVerticalStrut(15))
        panel.add(desc)
        panel.add(Box.createVerticalStrut(20))
        panel.add(copyBtn)

        return panel
    }
}
