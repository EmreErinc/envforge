package com.envforge.intellij

import com.intellij.execution.configurations.GeneralCommandLine
import com.intellij.openapi.project.Project
import com.redhat.devtools.lsp4ij.LanguageServerFactory
import com.redhat.devtools.lsp4ij.server.StreamConnectionProvider
import com.redhat.devtools.lsp4ij.server.OSProcessStreamConnectionProvider

class EnvForgeLspFactory : LanguageServerFactory {

    override fun createConnectionProvider(project: Project): StreamConnectionProvider {
        // H5: never spawn the envforge binary against an untrusted project —
        // it would run against project content and can write fence files.
        if (!isProjectTrusted(project)) {
            throw IllegalStateException(
                "EnvForge: project is not trusted; the language server will not start. " +
                    "Trust this project to enable EnvForge."
            )
        }
        val binary = findEnvforgeBinary(project)
        val cmd = GeneralCommandLine(binary, "lsp")
        cmd.workDirectory = project.basePath?.let { java.io.File(it) }
        return OSProcessStreamConnectionProvider(cmd)
    }

    companion object {
        /// Whether `project` is trusted (IntelliJ "Trusted Projects").
        ///
        /// Resolved via reflection so this compiles across platform versions
        /// where the API moved package (`com.intellij.ide.trustedProjects`
        /// in newer builds, `com.intellij.ide.impl` in older). If neither is
        /// present we default to trusted to avoid breaking the plugin on an
        /// unexpected platform — the API is present on the supported 242+ range.
        fun isProjectTrusted(project: Project): Boolean {
            val classNames = listOf(
                "com.intellij.ide.trustedProjects.TrustedProjects",
                "com.intellij.ide.impl.TrustedProjects",
            )
            for (name in classNames) {
                try {
                    val cls = Class.forName(name)
                    val method = cls.getMethod("isProjectTrusted", Project::class.java)
                    return method.invoke(null, project) as? Boolean ?: continue
                } catch (_: Throwable) {
                    // Try the next known location.
                }
            }
            return true
        }

        fun findEnvforgeBinary(project: Project? = null): String {
            return EnvForgeBinaryManager.findBinaryPath(project) ?: throw IllegalStateException(
                "envforge binary not found. Build locally with 'cargo build --release', set ENVFORGE_PATH, or install via cargo."
            )
        }

        /// Restart the language server for `project` via the lsp4ij
        /// LanguageServiceManager. Mirrors what VS Code does when
        /// `envforge.restartLsp` calls `client.stop(); client.start()`.
        fun restartForProject(project: Project) {
            try {
                // lsp4ij exposes LanguageServiceManager.getInstance(project)
                // which can stop/start all servers for a project.
                val managerClass = Class.forName(
                    "com.redhat.devtools.lsp4ij.LanguageServiceManager"
                )
                val getInstance = managerClass.getMethod("getInstance", Project::class.java)
                val manager = getInstance.invoke(null, project)
                val stopAll = managerClass.getMethod("stopAllServers")
                stopAll.invoke(manager)
                // Brief pause so the OS reclaims the port / pipe.
                Thread.sleep(300)
                val startAll = managerClass.getMethod("startServersIfNeeded", Project::class.java)
                startAll.invoke(manager, project)
            } catch (_: ClassNotFoundException) {
                // lsp4ij not present (unlikely given plugin deps). Fall
                // back to a no-op — the user can restart the IDE instead.
                throw RuntimeException(
                    "lsp4ij LanguageServiceManager not found. " +
                    "Please restart the IDE to reload the LSP server."
                )
            }
        }
    }
}
