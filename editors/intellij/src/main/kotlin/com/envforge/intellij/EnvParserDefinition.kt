package com.envforge.intellij

import com.intellij.lang.ASTNode
import com.intellij.lang.ParserDefinition
import com.intellij.lang.PsiParser
import com.intellij.lexer.Lexer
import com.intellij.openapi.project.Project
import com.intellij.psi.FileViewProvider
import com.intellij.psi.PsiElement
import com.intellij.psi.PsiFile
import com.intellij.psi.TokenType
import com.intellij.psi.tree.IFileElementType
import com.intellij.psi.tree.TokenSet

class EnvParserDefinition : ParserDefinition {
    companion object {
        val FILE = IFileElementType(EnvLanguage)
    }

    override fun createLexer(project: Project?): Lexer = EnvLexer()

    override fun createParser(project: Project?): PsiParser = PsiParser { root, builder ->
        val rootMarker = builder.mark()
        while (!builder.eof()) {
            builder.advanceLexer()
        }
        rootMarker.done(root)
        builder.treeBuilt
    }

    override fun getFileNodeType(): IFileElementType = FILE
    override fun getCommentTokens(): TokenSet = TokenSet.EMPTY
    override fun getStringLiteralElements(): TokenSet = TokenSet.EMPTY

    override fun createElement(node: ASTNode): PsiElement =
        com.intellij.extapi.psi.ASTWrapperPsiElement(node)

    override fun createFile(viewProvider: FileViewProvider): PsiFile = EnvPsiFile(viewProvider)

    override fun getWhitespaceTokens(): TokenSet = TokenSet.create(TokenType.WHITE_SPACE)
}

class EnvPsiFile(viewProvider: FileViewProvider) :
    com.intellij.extapi.psi.PsiFileBase(viewProvider, EnvLanguage) {
    override fun getFileType() = EnvFileType
    override fun toString() = "EnvForge .env File"
}
