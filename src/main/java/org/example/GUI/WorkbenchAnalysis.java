package org.example.GUI;

import org.example.I18n;
import java.util.*;

/** GUI 与 AI 共用的只读工作台分析。建议是经验启发，不代表实测提升。 */
public final class WorkbenchAnalysis {
    private WorkbenchAnalysis() {}
    public record Issue(String code, int severity, List<Integer> slots, String message,
                        Map<String, Object> evidence) {}
    public static List<Issue> analyze(List<Unit> units) {
        List<Issue> out = new ArrayList<>();
        if (units == null || units.size() < 2) return out;
        Set<Integer> skipped = Set.of(29, 30, 23, 34, 38, 40, 44);
        for (int i = 0; i < units.size(); i++) {
            Unit a = units.get(i);
            if (skipped.contains(a.id)) continue;
            for (int j = i + 1; j < units.size(); j++) {
                Unit b = units.get(j);
                if (a.id != b.id) continue;
                int dx = Math.abs(a.x-b.x), dy = Math.abs(a.y-b.y), dr = Math.abs(a.r-b.r);
                if (2*(dx+dy+dr) >= 8) continue;
                int dd = a.getDelay()-b.getDelay(), cycle = Unit.infos[a.id].cd()+Unit.infos[a.id].at();
                dd = dd < 0 ? Math.min(dd+cycle,-dd) : Math.min(-dd+cycle,dd);
                if (dd > 3) continue;
                boolean exact = dx==0 && dy==0 && dr==0;
                if (!exact && Set.of(1,7,17,18).contains(a.id) && dd>1) continue;
                String key = exact ? (j==i+1 ? "analysis.overlapExact" : "analysis.overlap") : "analysis.tooClose";
                out.add(new Issue(exact ? "identical_overlap" : "near_overlap", exact?4:3, List.of(i,j),
                        I18n.t(key,I18n.unitName(a.id),"("+a.x+", "+a.y+")"),
                        Map.of("dx",dx,"dy",dy,"dr",dr,"phaseDistance",dd)));
            }
        }
        for (String s : AssaultDetector.checkQuickestX(AssaultDetector.detect(units)))
            out.add(new Issue("assault_quickest_x",3,List.of(),s,Map.of("model","GUI heuristic")));
        List<Integer> walls = new ArrayList<>();
        for (int i=0;i<units.size();i++) if (units.get(i).id==19) walls.add(i);
        if (walls.size()==1 && units.get(walls.getFirst()).x%60!=54)
            out.add(new Issue("wall_jade_phase",3,walls,I18n.t("analysis.wallJade1"),Map.of("heuristicOnly",true)));
        if (walls.size()==2 && walls.stream().anyMatch(i -> !Set.of(43,45,53,55).contains(units.get(i).x%60)))
            out.add(new Issue("wall_jade_pair_phase",3,walls,I18n.t("analysis.wallJade2"),Map.of("heuristicOnly",true)));
        int healer=-1,repair=-1;
        for (int i=0;i<units.size();i++) {
            Unit u=units.get(i);
            if (u.id==8 && u.y==349 && (u.r>=357 || u.r<=1))
                out.add(new Issue("gun_jade_ground_angle",3,List.of(i),I18n.t("analysis.gunAngle",I18n.unitName(8),u.x,u.y,u.r),Map.of()));
            if (u.id==18 && healer<0) healer=i;
            if (u.id==17 && repair<0) repair=i;
        }
        for (int i=0;i<healer;i++) if (!units.get(i).isWallLike() && units.get(i).id!=18) {
            Unit u=units.get(healer);
            out.add(new Issue("healer_order",3,List.of(healer),I18n.t("analysis.healerFront",I18n.unitName(18),u.x,u.y,u.r),Map.of("earlierSlot",i))); break;
        }
        for (int i=0;i<repair;i++) if (units.get(i).isWall()) {
            Unit u=units.get(repair);
            out.add(new Issue("repair_order",3,List.of(repair),I18n.t("analysis.repairFront",I18n.unitName(17),u.x,u.y,u.r),Map.of("earlierSlot",i))); break;
        }
        return out;
    }
}
