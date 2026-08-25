package dev.odiador.onaji.actions

import com.intellij.ide.BrowserUtil
import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent

class ShowGraphAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        BrowserUtil.browse("http://localhost:8080")
    }
}
