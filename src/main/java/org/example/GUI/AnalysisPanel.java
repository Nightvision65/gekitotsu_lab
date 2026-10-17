package org.example.GUI;

import javax.swing.*;
import javax.swing.text.BadLocationException;
import javax.swing.text.SimpleAttributeSet;
import javax.swing.text.StyleConstants;
import javax.swing.text.StyledDocument;
import java.awt.*;
import java.util.List;

public class AnalysisPanel extends JPanel {
    private final JTextPane textPane;

    private List<Unit> units;

    private static final Color LEVEL_2_COLOR = new Color(80, 170, 240);
    private static final Color LEVEL_3_COLOR = new Color(210, 170, 0);
    private static final Color LEVEL_4_COLOR = new Color(220, 60, 60);

    public AnalysisPanel() {
        super(new BorderLayout());
        setBorder(BorderFactory.createTitledBorder(org.example.I18n.t("analysis.title")));

        textPane = new JTextPane();
        textPane.setEditable(false);
        textPane.setFont(org.example.I18n.font(Font.PLAIN, 14));
        textPane.setMargin(new Insets(0, 0, 0, 0));

        JScrollPane scrollPane = new JScrollPane(textPane);
        scrollPane.setBorder(null);
        scrollPane.setVerticalScrollBarPolicy(JScrollPane.VERTICAL_SCROLLBAR_ALWAYS);
        scrollPane.setHorizontalScrollBarPolicy(JScrollPane.HORIZONTAL_SCROLLBAR_NEVER);
        add(scrollPane, BorderLayout.CENTER);
    }

    /** 设置待分析的单位列表，同时自动检测突击组并重新分析。 */
    public void setUnits(List<Unit> u) {
        units = u;
        refresh();
    }

    public void clear() {
        textPane.setText("");
    }

    // ============ 工作流调度 ============

    private void refresh() {
        textPane.setText("");
        if (units == null || units.size() < 2) return;

        StyledDocument doc = textPane.getStyledDocument();
        SimpleAttributeSet base = mkStyle(null);
        SimpleAttributeSet level2 = mkStyle(LEVEL_2_COLOR);
        SimpleAttributeSet level3 = mkStyle(LEVEL_3_COLOR);
        SimpleAttributeSet level4 = mkStyle(LEVEL_4_COLOR);

        for (WorkbenchAnalysis.Issue issue : WorkbenchAnalysis.analyze(units)) {
            SimpleAttributeSet style = switch (issue.severity()) {
                case 2 -> level2; case 3 -> level3; case 4 -> level4; default -> base;
            };
            appendLine(doc, issue.message(), style);
        }
    }

    // ============ 工具方法 ============

    private void appendLine(StyledDocument doc, String text, SimpleAttributeSet style) {
        try {
            if (doc.getLength() > 0) {
                doc.insertString(doc.getLength(), "\n", style);
            }
            doc.insertString(doc.getLength(), text, style);
        } catch (BadLocationException ignored) {}
    }

    private SimpleAttributeSet mkStyle(Color fg) {
        SimpleAttributeSet s = new SimpleAttributeSet();
        StyleConstants.setFontFamily(s, org.example.I18n.fontFamily());
        StyleConstants.setFontSize(s, 14);
        StyleConstants.setForeground(s, fg != null ? fg : textPane.getForeground());
        return s;
    }
}
