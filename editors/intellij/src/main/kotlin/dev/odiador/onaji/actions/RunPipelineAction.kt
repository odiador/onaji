package dev.odiador.onaji.actions

import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.ui.Messages

class RunPipelineAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        Messages.showInfoMessage(project, "Onaji pipeline execution dispatched to Daemon.", "Onaji Semantic Automation")
    }
}
