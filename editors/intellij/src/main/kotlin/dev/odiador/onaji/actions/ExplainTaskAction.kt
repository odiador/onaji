package dev.odiador.onaji.actions

import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.ui.Messages
import dev.odiador.onaji.services.OnajiProjectService

class ExplainTaskAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        val taskId = Messages.showInputDialog(project, "Enter Task ID to explain:", "Onaji Explain", Messages.getQuestionIcon()) ?: return
        val service = project.getService(OnajiProjectService::class.java)
        val explanation = service.explainTask(taskId)

        val report = """
            Task: ${explanation.taskId}
            Root Cause: ${explanation.rootCause}
            Trigger: ${explanation.triggerType}
            Cache Lookup: ${explanation.cacheLookup}
            Duration: ${explanation.durationMs} ms
        """.trimIndent()

        Messages.showInfoMessage(project, report, "Onaji Causal Explanation")
    }
}
