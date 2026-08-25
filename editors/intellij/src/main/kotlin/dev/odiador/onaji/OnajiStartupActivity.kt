package dev.odiador.onaji

import com.intellij.openapi.project.Project
import com.intellij.openapi.startup.ProjectActivity
import dev.odiador.onaji.services.OnajiProjectService
import java.io.File

class OnajiStartupActivity : ProjectActivity {
    override suspend fun execute(project: Project) {
        val basePath = project.basePath ?: return
        val manifestFile = File(basePath, "onaji.yaml")

        if (manifestFile.exists()) {
            val service = project.getService(OnajiProjectService::class.java)
            service.initialize()
        }
    }
}
