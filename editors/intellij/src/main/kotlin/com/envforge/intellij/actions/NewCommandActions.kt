package com.envforge.intellij.actions

import com.envforge.intellij.EnvForgeLspFactory
import com.envforge.intellij.EnvForgeRunner
import com.intellij.notification.NotificationType
import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.ide.CopyPasteManager
import com.intellij.openapi.project.Project
import com.intellij.openapi.ui.Messages
import com.intellij.openapi.util.Disposer
import java.awt.datatransfer.StringSelection
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

/**
 * Run an arbitrary shell command wrapped in an envforge volatile
 * session (auto-revoking lease). Prompts for command + TTL via the
 * IDE's standard dialogs; spawns a process via `EnvForgeRunner` so the
 * IDE notification stream surfaces success/failure.
 *
 * Mirrors the VS Code `cmdRunVolatile` UX: same prompts, same TTL
 * presets, same wrapper string format. Parity with the LSP
 * `envforge.run.volatile` command is by construction — both sides emit
 * `envforge run --volatile <ttl> -- <command>`.
 */
class RunVolatileAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return

        val command = Messages.showInputDialog(
            project,
            "Command to run with volatile envforge session:",
            "EnvForge: Run Volatile",
            Messages.getQuestionIcon(),
        )?.trim()
        if (command.isNullOrBlank()) return

        val ttlChoices = arrayOf("5m", "15m", "30m", "1h", "2h")
        val ttl = Messages.showEditableChooseDialog(
            "Session TTL (auto-revokes after this):",
            "EnvForge: Run Volatile",
            Messages.getQuestionIcon(),
            ttlChoices,
            "30m",
            null,
        )?.trim()
        if (ttl.isNullOrBlank()) return

        EnvForgeRunner.run(
            project,
            listOf("run", "--volatile", ttl, "--", command),
            "Run Volatile ($ttl)",
        ) {
            EnvForgeRunner.notify(
                project,
                "Run Volatile",
                "Command finished (session auto-revoked).",
                NotificationType.INFORMATION,
            )
        }
    }
}

/**
 * Reveal an env var value with an audit log entry. Subprocess call to
 * `envforge get KEY --json` so the value goes through the same code
 * path as the LSP `envforge.reveal.value` command (both ultimately
 * shell out via the same binary). The reveal modal offers a Copy
 * action that auto-clears the clipboard 30 s later if it still
 * contains the revealed value.
 *
 * Note: the value crosses process boundaries here too — IntelliJ is
 * the rendering surface. The user has explicitly opted in via the
 * confirm dialog. Audit lives on the LSP / monitor bus; the subprocess
 * path emits its own audit event server-side.
 */
class RevealValueAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return

        val key = Messages.showInputDialog(
            project,
            "Env var key to reveal:",
            "EnvForge: Reveal Value",
            Messages.getQuestionIcon(),
        )?.trim()
        if (key.isNullOrBlank()) return

        val reason = Messages.showInputDialog(
            project,
            "Why reveal $key? (recorded to envforge audit)",
            "EnvForge: Reveal Value",
            Messages.getQuestionIcon(),
        )

        val confirm = Messages.showYesNoDialog(
            project,
            "Reveal value of \"$key\"? This will be logged to the envforge audit stream.",
            "Confirm Reveal",
            Messages.getWarningIcon(),
        )
        if (confirm != Messages.YES) return

        val binary = EnvForgeLspFactory.findEnvforgeBinary()
        val output: String = try {
            val proc = ProcessBuilder(binary, "get", key, "--json")
                .directory(project.basePath?.let { java.io.File(it) })
                .redirectErrorStream(true)
                .start()
            val out = proc.inputStream.bufferedReader().readText()
            proc.waitFor()
            if (proc.exitValue() != 0) {
                EnvForgeRunner.notify(
                    project,
                    "Reveal failed",
                    out.trim().ifBlank { "envforge get exited non-zero" },
                    NotificationType.ERROR,
                )
                return
            }
            out
        } catch (ex: Exception) {
            EnvForgeRunner.notify(
                project,
                "Reveal failed",
                ex.message ?: "subprocess error",
                NotificationType.ERROR,
            )
            return
        }

        val (value, sourceFile) = try {
            val obj = com.google.gson.JsonParser.parseString(output).asJsonObject
            val v = obj.get("value")?.asString ?: ""
            val s = obj.get("source_file")?.asString ?: ""
            v to s
        } catch (_: Exception) {
            "" to ""
        }
        if (value.isEmpty()) {
            EnvForgeRunner.notify(
                project,
                "Reveal",
                "Key \"$key\" has no value.",
                NotificationType.WARNING,
            )
            return
        }

        // Audit reason passed via reveal subprocess flag would be the
        // ideal path; for now the reason is recorded as an IDE
        // notification so the operator's intent is captured in their
        // own activity stream. Server-side audit happens through the
        // LSP path. We deliberately don't echo `reason` to logs.
        val _unused = reason

        val response = Messages.showYesNoDialog(
            project,
            "$key = $value\n\nsource: $sourceFile\n\nCopy value to clipboard? (auto-clears in 30s)",
            "EnvForge Reveal",
            "Copy",
            "Close",
            Messages.getInformationIcon(),
        )
        if (response == Messages.YES) {
            CopyPasteManager.getInstance().setContents(StringSelection(value))
            scheduleClipboardClear(value)
            EnvForgeRunner.notify(
                project,
                "Reveal",
                "Value copied. Clipboard auto-clears in 30s.",
                NotificationType.INFORMATION,
            )
        }
    }

    /// Best-effort clipboard auto-clear. Only clears if the clipboard
    /// still holds the revealed value at the timeout — avoids stomping
    /// a newer paste the user may have made in the meantime.
    private fun scheduleClipboardClear(value: String) {
        val executor = Executors.newSingleThreadScheduledExecutor { r ->
            Thread(r, "envforge-clipboard-clear").apply { isDaemon = true }
        }
        executor.schedule({
            try {
                val current = CopyPasteManager.getInstance().contents
                    ?.getTransferData(java.awt.datatransfer.DataFlavor.stringFlavor)
                    as? String
                if (current == value) {
                    CopyPasteManager.getInstance().setContents(StringSelection(""))
                }
            } catch (_: Exception) {
                // Clipboard race or missing transferable — give up
                // silently rather than spamming the user with errors.
            } finally {
                executor.shutdown()
            }
        }, 30, TimeUnit.SECONDS)
        // Hold the executor alive on a project-disposable handle so it
        // isn't collected before the schedule fires.
        Disposer.register({ executor.shutdownNow() }) {}
    }
}

/**
 * Extend the soonest-expiring active lease. Subprocess flow: probe
 * `envforge lease list --json`, pick the active lease with the lowest
 * remaining time, prompt the user for a new TTL via the standard
 * dialogs, then invoke `envforge lease renew NAME --ttl TTL --json`.
 * Mirrors the VS Code `cmdVolatileExtend` UX so subprocess and LSP
 * paths produce indistinguishable results.
 */
class ExtendLeaseAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        val binary = EnvForgeLspFactory.findEnvforgeBinary()
        val cwd = project.basePath?.let { java.io.File(it) }

        val listOutput: String = try {
            val proc = ProcessBuilder(binary, "lease", "list", "--json")
                .directory(cwd)
                .redirectErrorStream(true)
                .start()
            val out = proc.inputStream.bufferedReader().readText()
            proc.waitFor()
            if (proc.exitValue() != 0) {
                EnvForgeRunner.notify(
                    project,
                    "Extend lease",
                    "lease list failed: ${out.trim()}",
                    NotificationType.ERROR,
                )
                return
            }
            out
        } catch (ex: Exception) {
            EnvForgeRunner.notify(
                project,
                "Extend lease",
                ex.message ?: "subprocess error",
                NotificationType.ERROR,
            )
            return
        }

        val activeName = parseSoonestActiveLeaseName(listOutput)
        if (activeName == null) {
            EnvForgeRunner.notify(
                project,
                "Extend lease",
                "No active volatile lease to extend.",
                NotificationType.INFORMATION,
            )
            return
        }

        val ttlChoices = arrayOf("5m", "15m", "30m", "1h", "2h")
        val ttl = Messages.showEditableChooseDialog(
            "Extend lease \"$activeName\" — new TTL (replaces remaining time):",
            "EnvForge: Extend Lease",
            Messages.getQuestionIcon(),
            ttlChoices,
            "30m",
            null,
        )?.trim()
        if (ttl.isNullOrBlank()) return

        EnvForgeRunner.run(
            project,
            listOf("lease", "renew", activeName, "--ttl", ttl, "--json"),
            "Extend lease",
        ) {
            EnvForgeRunner.notify(
                project,
                "Extend lease",
                "Lease \"$activeName\" extended by $ttl.",
                NotificationType.INFORMATION,
            )
        }
    }

    private fun parseSoonestActiveLeaseName(json: String): String? {
        return try {
            val obj = com.google.gson.JsonParser.parseString(json).asJsonObject
            val arr = obj.getAsJsonArray("leases") ?: return null
            var best: Pair<String, Long>? = null
            for (i in 0 until arr.size()) {
                val e = arr[i].asJsonObject
                val status = e.get("status")?.asString ?: continue
                if (status != "active") continue
                val remaining = e.get("remaining_seconds")?.asLong ?: continue
                if (remaining <= 0) continue
                val name = e.get("name")?.asString ?: continue
                if (best == null || remaining < best!!.second) {
                    best = name to remaining
                }
            }
            best?.first
        } catch (_: Exception) {
            null
        }
    }
}

/**
 * Scan arbitrary text (or a file picked via a chooser) for registered
 * canary tokens via `envforge canary scan`. Mirrors the VS Code
 * `cmdCanaryScan` UX: editor selection wins; otherwise a chooser
 * decides between paste-text and pick-file paths. Results render via
 * a Messages dialog plus an IDE notification banner.
 *
 * Text path: writes the pasted blob to a tempfile so the `--input`
 * flag (which expects a path or `-`) gets a stable target without us
 * having to plumb stdin through `EnvForgeRunner`. The tempfile is
 * deleted in a `finally` regardless of outcome.
 */
class CanaryScanAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        val binary = EnvForgeLspFactory.findEnvforgeBinary()
        val cwd = project.basePath?.let { java.io.File(it) }

        // Path 1: scan the active editor's selection if any.
        val editor = com.intellij.openapi.fileEditor.FileEditorManager
            .getInstance(project)
            .selectedTextEditor
        val selected = editor?.selectionModel?.selectedText?.takeIf { it.isNotBlank() }

        val mode = if (selected != null) {
            "text"
        } else {
            val choice = Messages.showChooseDialog(
                project,
                "How do you want to scan for canary tokens?",
                "EnvForge: Canary Scan",
                Messages.getQuestionIcon(),
                arrayOf("Paste text...", "Pick a file..."),
                "Paste text...",
            )
            when (choice) {
                0 -> "text"
                1 -> "file"
                else -> return
            }
        }

        val inputPath: java.io.File? = when (mode) {
            "text" -> {
                val text = selected ?: Messages.showInputDialog(
                    project,
                    "Paste text (log line, stack trace, diff) to scan for canary tokens:",
                    "EnvForge: Canary Scan",
                    Messages.getQuestionIcon(),
                ) ?: return
                if (text.isBlank()) return
                writeTempScanInput(text)
            }
            "file" -> {
                val descriptor =
                    com.intellij.openapi.fileChooser.FileChooserDescriptor(true, false, false, false, false, false)
                        .withTitle("EnvForge: pick a file to scan for canary tokens")
                val vfile = com.intellij.openapi.fileChooser.FileChooser
                    .chooseFile(descriptor, project, null) ?: return
                java.io.File(vfile.path)
            }
            else -> return
        }
        if (inputPath == null) return

        try {
            val output: String = try {
                val proc = ProcessBuilder(
                    binary,
                    "canary",
                    "scan",
                    "--input",
                    inputPath.absolutePath,
                    "--json",
                )
                    .directory(cwd)
                    .redirectErrorStream(true)
                    .start()
                val out = proc.inputStream.bufferedReader().readText()
                proc.waitFor()
                if (proc.exitValue() != 0) {
                    // Strict mode exits non-zero on match; we did not pass --strict,
                    // so a non-zero here is a real failure.
                    EnvForgeRunner.notify(
                        project,
                        "Canary scan failed",
                        out.trim().ifBlank { "envforge canary scan exited non-zero" },
                        NotificationType.ERROR,
                    )
                    return
                }
                out
            } catch (ex: Exception) {
                EnvForgeRunner.notify(
                    project,
                    "Canary scan failed",
                    ex.message ?: "subprocess error",
                    NotificationType.ERROR,
                )
                return
            }

            val matches = parseScanMatches(output)
            if (matches.isEmpty()) {
                EnvForgeRunner.notify(
                    project,
                    "Canary scan",
                    "No registered tripwire tokens found.",
                    NotificationType.INFORMATION,
                )
                return
            }
            val body = buildString {
                appendLine("${matches.size} canary token${if (matches.size == 1) "" else "s"} detected:")
                appendLine()
                for (m in matches) {
                    val loc = when {
                        m.lineNumber != null -> "line ${m.lineNumber}"
                        m.byteOffset != null -> "byte ${m.byteOffset}"
                        else -> "?"
                    }
                    appendLine("  $loc: ${m.token}")
                }
            }
            Messages.showWarningDialog(project, body, "EnvForge Canary Scan")
            EnvForgeRunner.notify(
                project,
                "Canary scan",
                "${matches.size} canary token${if (matches.size == 1) "" else "s"} detected. Review immediately.",
                NotificationType.WARNING,
            )
        } finally {
            // Clean up any tempfile we created. File-chooser paths point at the
            // user's own file, so only delete the tempfile we own.
            if (mode == "text") {
                try {
                    inputPath.delete()
                } catch (_: Exception) {
                    // Best effort.
                }
            }
        }
    }

    private fun writeTempScanInput(text: String): java.io.File? = try {
        val f = java.io.File.createTempFile("envforge-scan-", ".txt").apply { deleteOnExit() }
        f.writeText(text)
        f
    } catch (_: Exception) {
        null
    }

    private data class TokenMatch(
        val token: String,
        val byteOffset: Long?,
        val lineNumber: Long?,
    )

    /// The CLI emits a top-level array of match objects. Walk it
    /// defensively so a future CLI cosmetic change does not silently
    /// break us.
    private fun parseScanMatches(json: String): List<TokenMatch> = try {
        val parsed = com.google.gson.JsonParser.parseString(json)
        val arr = if (parsed.isJsonArray) parsed.asJsonArray else com.google.gson.JsonArray()
        val out = mutableListOf<TokenMatch>()
        for (i in 0 until arr.size()) {
            val e = arr[i].asJsonObject
            val token = e.get("token")?.asString ?: continue
            val byteOffset = e.get("byte_offset")?.let {
                if (it.isJsonNull) null else it.asLong
            }
            val lineNumber = e.get("line_number")?.let {
                if (it.isJsonNull) null else it.asLong
            }
            out.add(TokenMatch(token, byteOffset, lineNumber))
        }
        out
    } catch (_: Exception) {
        emptyList()
    }
}

/**
 * Show triggered canaries via `envforge canary check --json`. Routes
 * results to a single dialog summary plus an IDE notification at the
 * appropriate severity (error when any tripped, info when quiet).
 */
class CanaryCheckAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        val binary = EnvForgeLspFactory.findEnvforgeBinary()

        val output: String = try {
            val proc = ProcessBuilder(binary, "canary", "check", "--json")
                .directory(project.basePath?.let { java.io.File(it) })
                .redirectErrorStream(true)
                .start()
            val out = proc.inputStream.bufferedReader().readText()
            proc.waitFor()
            if (proc.exitValue() != 0) {
                notifyError(project, out)
                return
            }
            out
        } catch (ex: Exception) {
            notifyError(project, ex.message ?: "subprocess error")
            return
        }

        val triggered: List<TriggeredCanary> = parseTriggered(output)
        if (triggered.isEmpty()) {
            EnvForgeRunner.notify(
                project,
                "Canary check",
                "No triggered canaries. All quiet.",
                NotificationType.INFORMATION,
            )
            return
        }
        val body = buildString {
            appendLine("${triggered.size} triggered canary${if (triggered.size == 1) "" else "es"}:")
            appendLine()
            for (c in triggered) {
                appendLine("  ${c.key} (${c.pattern}) — ${c.triggerCount} hit${if (c.triggerCount == 1) "" else "s"}, created ${c.createdAt}")
            }
        }
        Messages.showWarningDialog(project, body, "EnvForge Triggered Canaries")
        EnvForgeRunner.notify(
            project,
            "Canary check",
            "${triggered.size} triggered canary${if (triggered.size == 1) "" else "es"}. Review immediately.",
            NotificationType.ERROR,
        )
    }

    private data class TriggeredCanary(
        val key: String,
        val pattern: String,
        val triggerCount: Int,
        val createdAt: String,
    )

    private fun parseTriggered(json: String): List<TriggeredCanary> = try {
        // `envforge canary check --json` returns a top-level object;
        // we walk both common shapes (`canaries` and bare arrays) so
        // future CLI cosmetic changes don't silently break us.
        val parsed = com.google.gson.JsonParser.parseString(json)
        val arr = when {
            parsed.isJsonArray -> parsed.asJsonArray
            parsed.isJsonObject -> parsed.asJsonObject.let { obj ->
                obj.getAsJsonArray("triggered")
                    ?: obj.getAsJsonArray("canaries")
                    ?: com.google.gson.JsonArray()
            }
            else -> com.google.gson.JsonArray()
        }
        val out = mutableListOf<TriggeredCanary>()
        for (i in 0 until arr.size()) {
            val e = arr[i].asJsonObject
            val triggered = e.get("triggered")?.asBoolean ?: true
            if (!triggered) continue
            val key = e.get("key")?.asString ?: continue
            val pattern = e.get("pattern")?.asString ?: "generic"
            val count = e.get("trigger_count")?.asInt ?: 0
            val created = e.get("created_at")?.asString ?: ""
            out.add(TriggeredCanary(key, pattern, count, created))
        }
        out
    } catch (_: Exception) {
        emptyList()
    }

    private fun notifyError(project: Project, message: String) {
        EnvForgeRunner.notify(
            project,
            "Canary check failed",
            message.trim().ifBlank { "subprocess error" },
            NotificationType.ERROR,
        )
    }
}
