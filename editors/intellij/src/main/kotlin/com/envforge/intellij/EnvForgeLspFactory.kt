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
    }
}
