package dev.odiador.onaji.toolwindow

import com.intellij.ui.components.JBLabel
import com.intellij.ui.components.JBPanel
import java.awt.BorderLayout
import java.awt.Font
import java.awt.GridLayout
import javax.swing.BorderFactory
import javax.swing.JButton
import javax.swing.JPanel

class PipelinePanel : JBPanel<PipelinePanel>(BorderLayout()) {
    init {
        border = BorderFactory.createEmptyBorder(12, 12, 12, 12)

        val header = JBLabel("ONΛJI Active Pipeline").apply {
            font = font.deriveFont(Font.BOLD, 15f)
        }
        add(header, BorderLayout.NORTH)

        val content = JPanel(GridLayout(4, 1, 6, 6)).apply {
            add(JBLabel("✓ validate_openapi (42 ms)"))
            add(JBLabel("⚡ generate_kotlin (cached)"))
            add(JBLabel("● compile_android (running 8.2s)"))
        }
        add(content, BorderLayout.CENTER)

        val actions = JPanel().apply {
            add(JButton("Run Pipeline"))
            add(JButton("View DAG"))
        }
        add(actions, BorderLayout.SOUTH)
    }
}
