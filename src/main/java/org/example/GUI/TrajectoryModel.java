package org.example.GUI;

import java.util.*;

/** 原 TraceCanvas 弹道公式的共享数据模型；不是 Rust 对战观测。 */
public final class TrajectoryModel {
    private TrajectoryModel() {}
    public record Point(int frame, double worldX, double worldY, double opponentRelativeX,
                        double opponentRelativeY) {}
    public record Prediction(int triggerFrame, List<Point> points, String coordinateSpace,
                             String evidenceKind) {}
    public static Prediction predict(FormulaTable table, boolean near, boolean ballFirst,
                                      int x, int y, int wallX, int sp1, int sp2, int times) {
        if (table==null || !table.isLoaded()) throw new IllegalStateException("轨迹公式未加载");
        if (sp1<0 || sp1>15 || sp2<0 || sp2>15 || times<1 || times>100)
            throw new IllegalArgumentException("轨迹等级必须0..15，次数1..100");
        int t = near ? table.findNearTriggerTime(sp1,sp2,times)+(ballFirst?1:0)
                : 884-wallX+900*(times-1)+(ballFirst?1:0);
        if (t<=0) return new Prediction(t,List.of(),"battle-world + opponent-base-relative","GUI formula prediction");
        int trigger=t;
        double[] pos=table.evaluate(sp1,sp2,t);
        double px=pos[FormulaTable.DIM_P1_X]-174+x, py=pos[FormulaTable.DIM_P1_Y]-380+y;
        double xs=near?4:15,ys=near?-8:-4;
        List<Point> points=new ArrayList<>();
        while (py<566 && points.size()<4096) {
            t++; ys+=0.32; px+=xs; py+=ys;
            px=(int)(20*px)*0.05; py=(int)(20*py)*0.05;
            pos=table.evaluate(sp1,sp2,t);
            points.add(new Point(t,px,py,px-pos[FormulaTable.DIM_P2_X],py-pos[FormulaTable.DIM_P2_Y]));
        }
        return new Prediction(trigger,points,"battle-world + opponent-base-relative","GUI formula prediction");
    }
}
