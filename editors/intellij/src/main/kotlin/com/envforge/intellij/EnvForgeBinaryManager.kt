package com.envforge.intellij

import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.diagnostic.Logger
import com.intellij.openapi.progress.ProgressIndicator
import com.intellij.openapi.progress.ProgressManager
import com.intellij.openapi.progress.Task
import com.intellij.openapi.project.Project
import com.intellij.util.io.HttpRequests
import java.io.File
import java.io.FileOutputStream
import java.util.zip.GZIPInputStream

object EnvForgeBinaryManager {
    private val LOG = Logger.getInstance(EnvForgeBinaryManager::class.java)

    const val DEFAULT_VERSION = "v0.2.1"
    
    private val isWindows: Boolean
        get() = System.getProperty("os.name").lowercase().contains("win")

    private val binaryName: String
        get() = if (isWindows) "envforge.exe" else "envforge"

    val managedBinaryDir: File
        get() {
            val userHome = System.getProperty("user.home")
            val dir = File(userHome, ".envforge/bin")
            if (!dir.exists()) {
                dir.mkdirs()
            }
            return dir
        }

    val managedBinaryFile: File
        get() = File(managedBinaryDir, binaryName)

    /**
     * Finds the envforge binary path using resolution order:
     * 1. ENVFORGE_PATH environment variable
     * 2. Active project's target/release or target/debug binaries (for dev/local builds)
     * 3. Managed plugin directory (~/.envforge/bin/envforge)
     * 4. System standard paths (~/.cargo/bin, /usr/local/bin, /opt/homebrew/bin)
     */
    fun findBinaryPath(project: Project? = null): String? {
        val projectCandidates = project?.basePath?.let { base ->
            listOf(
                "$base/target/release/$binaryName",
                "$base/target/debug/$binaryName",
            )
        } ?: emptyList()

        val candidates = projectCandidates + listOf(
            System.getenv("ENVFORGE_PATH"),
            managedBinaryFile.absolutePath,
            "${System.getProperty("user.home")}/.cargo/bin/$binaryName",
            "/usr/local/bin/$binaryName",
            "/opt/homebrew/bin/$binaryName",
        )

        for (path in candidates) {
            if (path != null) {
                val file = File(path)
                if (file.exists() && file.canExecute()) {
                    return file.absolutePath
                }
            }
        }
        return null
    }

    /**
     * If a local Rust build exists in the workspace (target/release or target/debug),
     * copy it to the managed binary location (~/.envforge/bin/envforge).
     */
    fun copyLocalBuildIfAvailable(project: Project? = null): Boolean {
        val base = project?.basePath ?: return false
        val releaseCandidate = File(base, "target/release/$binaryName")
        val debugCandidate = File(base, "target/debug/$binaryName")
        
        val source = when {
            releaseCandidate.exists() && releaseCandidate.canExecute() -> releaseCandidate
            debugCandidate.exists() && debugCandidate.canExecute() -> debugCandidate
            else -> null
        }

        if (source != null) {
            try {
                source.copyTo(managedBinaryFile, overwrite = true)
                managedBinaryFile.setExecutable(true, false)
                managedBinaryFile.setReadable(true, false)
                LOG.info("Copied local build binary from ${source.absolutePath} to ${managedBinaryFile.absolutePath}")
                return true
            } catch (e: Exception) {
                LOG.warn("Failed to copy local build binary from ${source.absolutePath}", e)
            }
        }
        return false
    }

    /**
     * Determines the OS & Architecture target triple for GitHub releases.
     */
    fun getTargetTriple(): String? {
        val os = System.getProperty("os.name").lowercase()
        val arch = System.getProperty("os.arch").lowercase()

        val archStr = when {
            arch.contains("aarch64") || arch.contains("arm64") -> "aarch64"
            arch.contains("x86_64") || arch.contains("amd64") -> "x86_64"
            else -> return null
        }

        val osStr = when {
            os.contains("mac") || os.contains("darwin") -> "apple-darwin"
            os.contains("win") -> "pc-windows-msvc"
            os.contains("nux") || os.contains("nix") -> "unknown-linux-gnu"
            else -> return null
        }

        return "$archStr-$osStr"
    }

    /**
     * Downloads and installs the EnvForge CLI binary synchronously.
     */
    fun downloadAndInstall(indicator: ProgressIndicator? = null): Boolean {
        val targetTriple = getTargetTriple() ?: run {
            LOG.warn("Unsupported OS/architecture for auto-download: ${System.getProperty("os.name")} ${System.getProperty("os.arch")}")
            return false
        }

        val ext = if (isWindows) "zip" else "tar.gz"
        val downloadUrl = "https://github.com/emreerinc/envforge/releases/download/$DEFAULT_VERSION/envforge-$DEFAULT_VERSION-$targetTriple.$ext"

        val tempFile = File(managedBinaryDir, "envforge-download-temp.$ext")
        val targetFile = managedBinaryFile

        try {
            indicator?.text = "Downloading EnvForge CLI ($DEFAULT_VERSION)..."
            indicator?.isIndeterminate = true

            LOG.info("Downloading EnvForge binary from $downloadUrl to ${tempFile.absolutePath}")

            HttpRequests.request(downloadUrl)
                .connectTimeout(15000)
                .readTimeout(30000)
                .saveToFile(tempFile, indicator)

            indicator?.text = "Extracting EnvForge CLI..."

            if (ext == "tar.gz") {
                extractTarGzFile(tempFile, targetFile)
            } else {
                tempFile.copyTo(targetFile, overwrite = true)
            }

            tempFile.delete()

            if (targetFile.exists()) {
                targetFile.setExecutable(true, false)
                targetFile.setReadable(true, false)
                LOG.info("Successfully installed EnvForge binary at ${targetFile.absolutePath}")
                return true
            }
        } catch (e: Exception) {
            LOG.warn("Failed to download EnvForge binary from $downloadUrl", e)
            if (tempFile.exists()) tempFile.delete()
        }
        return false
    }

    private fun extractTarGzFile(tarGzFile: File, outputFile: File) {
        GZIPInputStream(tarGzFile.inputStream()).use { gzip ->
            val bytes = gzip.readBytes()
            outputFile.writeBytes(bytes)
        }
    }

    /**
     * Triggers asynchronous resolution or download of binary with IntelliJ Progress Indicator.
     */
    fun downloadAsync(project: Project, onComplete: ((Boolean) -> Unit)? = null) {
        ProgressManager.getInstance().run(object : Task.Backgroundable(project, "EnvForge: Resolving CLI Binary", true) {
            var success = false

            override fun run(indicator: ProgressIndicator) {
                // First check if local workspace binary exists
                if (copyLocalBuildIfAvailable(project)) {
                    success = true
                    return
                }

                // Otherwise attempt online download
                success = downloadAndInstall(indicator)
            }

            override fun onSuccess() {
                if (success) {
                    EnvForgeRunner.notify(
                        project,
                        "EnvForge CLI Ready",
                        "EnvForge CLI binary ready at ${managedBinaryFile.absolutePath}",
                        com.intellij.notification.NotificationType.INFORMATION
                    )
                    EnvForgeLspFactory.restartForProject(project)
                } else {
                    EnvForgeRunner.notify(
                        project,
                        "EnvForge CLI Binary Not Found",
                        "Could not download CLI binary (Release $DEFAULT_VERSION not found on GitHub). Build locally with 'cargo build --release' or set ENVFORGE_PATH.",
                        com.intellij.notification.NotificationType.WARNING
                    )
                }
                onComplete?.invoke(success)
            }

            override fun onThrowable(error: Throwable) {
                EnvForgeRunner.notify(
                    project,
                    "EnvForge Resolution Error",
                    "Error resolving EnvForge CLI: ${error.message}",
                    com.intellij.notification.NotificationType.ERROR
                )
                onComplete?.invoke(false)
            }
        })
    }
}
