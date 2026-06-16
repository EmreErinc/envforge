package com.envforge.intellij

import com.intellij.execution.configurations.GeneralCommandLine
import com.intellij.openapi.project.Project
import com.redhat.devtools.lsp4ij.LanguageServerFactory
import com.redhat.devtools.lsp4ij.server.StreamConnectionProvider
import com.redhat.devtools.lsp4ij.server.OSProcessStreamConnectionProvider

class EnvForgeLspFactory : LanguageServerFactory {

    override fun createConnectionProvider(project: Project): StreamConnectionProvider {
        val binary = findEnvforgeBinary()
        val cmd = GeneralCommandLine(binary, "lsp")
        cmd.workDirectory = project.basePath?.let { java.io.File(it) }
        return OSProcessStreamConnectionProvider(cmd)
    }

    companion object {
        fun findEnvforgeBinary(): String {
            val candidates = listOf(
                System.getenv("ENVFORGE_PATH"),
                "${System.getProperty("user.home")}/.cargo/bin/envforge",
                "/usr/local/bin/envforge",
                "/opt/homebrew/bin/envforge",
            )

            for (path in candidates) {
                if (path != null && java.io.File(path).canExecute()) {
                    return path
                }
            }

            return "envforge"
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
