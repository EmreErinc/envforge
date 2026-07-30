package com.envforge.intellij

import com.envforge.intellij.actions.isEnvFile
import com.intellij.codeInspection.LocalInspectionTool
import com.intellij.codeInspection.ProblemHighlightType
import com.intellij.codeInspection.ProblemsHolder
import com.intellij.openapi.util.TextRange
import com.intellij.psi.PsiElementVisitor
import com.intellij.psi.PsiFile

class FenceInactiveInspection : LocalInspectionTool() {
    override fun buildVisitor(holder: ProblemsHolder, isOnTheFly: Boolean): PsiElementVisitor {
        return object : com.intellij.psi.PsiElementVisitor() {
            override fun visitFile(file: PsiFile) {
                if (isEnvFile(file.name) && !isFenceActive(file)) {
                    val bundle = java.util.ResourceBundle.getBundle("messages.EnvForgeBundle")
                    val message = bundle.getString("fence.inactive.message")
                    val fix = FenceActivateFix()

                    holder.registerProblem(
                        file,
                        message,
                        ProblemHighlightType.WEAK_WARNING,
                        TextRange(0, minOf(file.textLength, 1)),
                        fix,
                    )
                }
            }
        }
    }

    private fun isFenceActive(file: PsiFile): Boolean {
        val project = file.project
        val workDir = project.basePath?.let { java.io.File(it) }

        return try {
            val binary = EnvForgeLspFactory.findEnvforgeBinary(project)
            val process = ProcessBuilder(binary, "fence", "--status", "--json")
                .directory(workDir)
                .start()
            val output = process.inputStream.bufferedReader().readText()
            process.waitFor()
            if (process.exitValue() != 0) return true
            val obj = com.google.gson.JsonParser.parseString(output.trim()).asJsonObject
            obj.get("all_fenced")?.asBoolean ?: obj.get("active")?.asBoolean ?: false
        } catch (_: Exception) {
            true
        }
    }
}
