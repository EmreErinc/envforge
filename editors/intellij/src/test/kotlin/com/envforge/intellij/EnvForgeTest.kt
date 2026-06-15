package com.envforge.intellij

import com.intellij.testFramework.fixtures.BasePlatformTestCase
import org.junit.Test

class EnvForgeTest : BasePlatformTestCase() {

    @Test
    fun testEnvFileType() {
        val file = myFixture.configureByText(".env", "KEY=VALUE")
        assertEquals(EnvFileType, file.fileType)
    }

    @Test
    fun testEnvLanguage() {
        val file = myFixture.configureByText(".env", "KEY=VALUE")
        assertEquals(EnvLanguage, file.language)
    }
}
