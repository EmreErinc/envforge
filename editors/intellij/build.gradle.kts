import org.jetbrains.intellij.platform.gradle.TestFrameworkType

plugins {
    id("org.jetbrains.intellij.platform") version "2.2.1"
    kotlin("jvm") version "1.9.25"
}

group = "com.envforge"
version = "0.3.0"

repositories {
    mavenCentral()
    intellijPlatform {
        defaultRepositories()
    }
}

dependencies {
    implementation("com.google.code.gson:gson:2.11.0")
    intellijPlatform {
        intellijIdeaCommunity("2024.2")
        plugin("com.redhat.devtools.lsp4ij:0.19.3")
        bundledPlugin("com.intellij.java")
        bundledPlugin("org.toml.lang")
        testFramework(TestFrameworkType.Platform)
    }
    testImplementation(kotlin("test"))
}

intellijPlatform {
    pluginVerification {
        ides {
            recommended()
        }
    }
}

tasks {
    patchPluginXml {
        sinceBuild.set("242")
        untilBuild.set("262.*")
    }
    publishPlugin {
        // CI sets JETBRAINS_MARKETPLACE_TOKEN. Local: -PintellijPublishToken=...
        token.set(
            providers.environmentVariable("JETBRAINS_MARKETPLACE_TOKEN")
                .orElse(providers.gradleProperty("intellijPublishToken")),
        )
    }
}

kotlin {
    jvmToolchain(21)
}
