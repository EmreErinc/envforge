package com.envforge.intellij

import com.intellij.openapi.fileTypes.LanguageFileType
import com.intellij.openapi.util.IconLoader
import javax.swing.Icon

object EnvFileType : LanguageFileType(EnvLanguage) {
    override fun getName(): String = "EnvForge .env"
    override fun getDescription(): String = "EnvForge environment file"
    override fun getDefaultExtension(): String = "env"
    override fun getIcon(): Icon? = ENV_ICON

    private val ENV_ICON: Icon? = runCatching {
        IconLoader.getIcon("/icons/envforge.svg", EnvFileType::class.java)
    }.getOrNull()
}
