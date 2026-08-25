package dev.odiador.onaji.services

import com.intellij.openapi.components.Service
import com.intellij.openapi.project.Project
import dev.odiador.onaji.daemon.OnajiDaemonClient
import dev.odiador.onaji.daemon.DaemonStatus
import dev.odiador.onaji.daemon.TaskExplanation

@Service(Service.Level.PROJECT)
class OnajiProjectService(val project: Project) {
    val client = OnajiDaemonClient()
    var isInitialized = false

    fun initialize() {
        isInitialized = true
    }

    fun getStatus(): DaemonStatus = client.getStatus()

    fun explainTask(taskId: String): TaskExplanation = client.explainTask(taskId)
}
