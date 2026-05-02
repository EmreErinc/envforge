package com.envforge.intellij

import com.intellij.codeInspection.LocalQuickFix
import com.intellij.codeInspection.ProblemDescriptor
import com.intellij.openapi.project.Project
import com.intellij.openapi.util.NlsContexts
import org.jetbrains.annotations.NotNull

class FenceActivateFix : LocalQuickFix {
    override fun getName(): String =
        java.util.ResourceBundle.getBundle("messages.EnvForgeBundle")
            .getString("fence.inactive.fix")

    override fun getFamilyName(): String = "EnvForge"

    override fun applyFix(project: Project, descriptor: ProblemDescriptor) {
        EnvForgeRunner.run(project, listOf("fence"), "Activate Fence") {
            EnvForgeRunner.notify(project, "Fence", "Fence activated",
                com.intellij.notification.NotificationType.INFORMATION)
        }
    }
}
