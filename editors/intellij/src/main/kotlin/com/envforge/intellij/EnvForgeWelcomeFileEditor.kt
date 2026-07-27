package com.envforge.intellij

import com.intellij.openapi.fileEditor.FileEditor
import com.intellij.openapi.fileEditor.FileEditorLocation
import com.intellij.openapi.fileEditor.FileEditorState
import com.intellij.openapi.fileEditor.FileEditorProvider
import com.intellij.openapi.project.DumbAware
import com.intellij.openapi.project.Project
import com.intellij.openapi.util.UserDataHolderBase
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.testFramework.LightVirtualFile
import java.beans.PropertyChangeListener
import javax.swing.JComponent

class EnvForgeWelcomeVirtualFile : LightVirtualFile("EnvForge Welcome", EnvFileType, "")

class EnvForgeWelcomeFileEditorProvider : FileEditorProvider, DumbAware {
    override fun getEditorTypeId(): String = "EnvForgeWelcomeEditor"

    override fun accept(project: Project, file: VirtualFile): Boolean {
        return file is EnvForgeWelcomeVirtualFile
    }

    override fun createEditor(project: Project, file: VirtualFile): FileEditor {
        return EnvForgeWelcomeFileEditor(project, file)
    }

    override fun getPolicy(): com.intellij.openapi.fileEditor.FileEditorPolicy {
        return com.intellij.openapi.fileEditor.FileEditorPolicy.HIDE_DEFAULT_EDITOR
    }
}

class EnvForgeWelcomeFileEditor(
    private val project: Project,
    private val virtualFile: VirtualFile
) : UserDataHolderBase(), FileEditor, DumbAware {

    private val panel = EnvForgeWelcomePanel(project)

    override fun getComponent(): JComponent = panel
    override fun getPreferredFocusedComponent(): JComponent? = panel
    override fun getName(): String = "EnvForge Welcome"
    override fun setState(state: FileEditorState) {}
    override fun isModified(): Boolean = false
    override fun isValid(): Boolean = true
    override fun addPropertyChangeListener(listener: PropertyChangeListener) {}
    override fun removePropertyChangeListener(listener: PropertyChangeListener) {}
    override fun getCurrentLocation(): FileEditorLocation? = null
    override fun getFile(): VirtualFile = virtualFile
    override fun dispose() {}
}
