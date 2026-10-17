package org.example.ai;

import com.fasterxml.jackson.databind.*;
import org.example.*;
import org.example.GUI.*;
import org.example.rust.RustBattle;
import java.nio.file.*;
import java.security.MessageDigest;
import java.util.*;

/** 非交互、无 GUI 启动、副作用隔离由调用器负责的结构化接口。 */
public final class AiApi {
    private static final ObjectMapper JSON = new ObjectMapper();
    private AiApi() {}
    public static Map<String,Object> obj(Object... fields) {
        Map<String,Object> out=new LinkedHashMap<>();
        for(int i=0;i<fields.length;i+=2) out.put((String)fields[i],fields[i+1]);
        return out;
    }
    static void require(boolean ok,String message) {if(!ok) throw new IllegalArgumentException(message);}
    public static Formation decode(String code) {
        require(code.length()>=6 && code.length()%6==0 && code.length()<=3072,"只接受6的倍数纯阵码，最多512槽位");
        require("012".indexOf(code.charAt(0))>=0,"核心必须在slot0");
        for(int slot=0;slot<code.length()/6;slot++) {
            String chunk=code.substring(slot*6,slot*6+6);
            int type=Main.pskey.indexOf(chunk.charAt(0));
            require(slot==0 || type>=1 && type<=60,"非核心单位ID必须1..60");
            for(int j=1;j<6;j++) require(Main.pskey.indexOf(chunk.charAt(j))>=0 && Main.pskey.indexOf(chunk.charAt(j))<61,"坐标必须base61，不允许Z");
            int[] raw=Main.to_xyr(chunk.substring(1));
            int x=raw[0]-(slot==0?52:16), y=raw[1]-(slot==0?58:20);
            require(x>=0 && x<=(slot==0?276:348) && y>=0 && y<=(slot==0?276:349) && raw[2]<360,"slot="+slot+"坐标或角度越界");
        }
        return Formation.decode(code);
    }
    static List<Fort> forts(JsonNode node) {
        require(node!=null && node.isArray() && !node.isEmpty(),"阵集必须是非空数组");
        List<Fort> out=new ArrayList<>();
        for(JsonNode n:node) {String code=n.path("code").asText(); decode(code);
            String name=n.path("name").asText("");require(!name.matches("(?s).*[&/#\\r\\n].*"),"名称不能含分隔符");out.add(new Fort(name,code));}
        return out;
    }
    static String text(List<Fort> list) {return String.join("/",list.stream().map(f->f.name()+"&"+f.code()).toList());}
    static String sha(byte[] data) throws Exception {return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(data));}
    static void rust() {if(!RustBattle.available()) throw new IllegalStateException("Rust不可用："+RustBattle.unavailableReason());}
    public static Map<String,Object> score(List<Result> results,int side) {
        int w=0,l=0,d=0,u=0,e=0;
        for(Result r:results) switch(r.status) {case 0->d++;case -1->u++;case 1,2->{if(r.status==side+1)w++;else l++;}default->e++;}
        return obj("W",w,"L",l,"D",d,"U",u,"E",e,"N",results.size(),"D_scored",d+u,
                "scorePercent",e==0 && !results.isEmpty()?100.0*(w+0.5*(d+u))/results.size():null,
                "scoringPolicy","timeout-as-draw-v1","complete",e==0);
    }
    static List<Map<String,Object>> rows(List<Fort>a,List<Fort>b,int frames,int threads) {
        rust(); List<Result> rs=RustBattle.runBatch(a.stream().map(Fort::code).toArray(String[]::new),b.stream().map(Fort::code).toArray(String[]::new),frames,threads,null);
        require(rs.size()==a.size()*b.size(),"对战矩阵缺局");
        List<Map<String,Object>> rows=new ArrayList<>();
        for(int k=0;k<rs.size();k++) {Result r=rs.get(k);rows.add(obj("p1Index",k/b.size(),"p2Index",k%b.size(),"status",r.status,"winnerHp",r.winnerHp,"frames",r.framePassed));}
        return rows;
    }
    static List<Result> results(List<Map<String,Object>> rows) {
        return rows.stream().map(x->new Result((int)x.get("status"),(int)x.get("winnerHp"),(int)x.get("frames"),0)).toList();
    }
    static Map<String,Object> formation(Fort fort) {
        Formation f=decode(fort.code());List<Unit> stable=FormationStabilizer.stabilize(f.units);
        int cost=0;List<Object> units=new ArrayList<>();
        for(int slot=0;slot<f.units.size();slot++) {Unit u=f.units.get(slot),s=stable.get(slot);Unit.Info info=Unit.infos[u.id];cost+=info.cost();
            units.add(obj("slot",slot,"id",u.id,"name",I18n.unitName(u.id),"x",u.x,"y",u.y,"r",u.r,"cost",info.cost(),
                "catalogHp",info.hp(),"workbenchObjectHp",u.hp,"initialDelayGui",u.getDelay(),"cd",info.cd(),"at",info.at(),"shoot",info.shoot(),
                "core",u.isCore(),"wall",u.isWall(),"stabilizedX",s.x,"stabilizedY",s.y));}
        List<Object> groups=new ArrayList<>();
        for(AssaultGroup g:AssaultDetector.detect(f.units)) {List<Object> members=new ArrayList<>();
            for(Unit u:g.unitsBefore) members.add(obj("slot",f.units.indexOf(u),"beforeWall",true,"quickestX",u.getQuickestXList(g.wallX,true)));
            for(Unit u:g.unitsAfter) members.add(obj("slot",f.units.indexOf(u),"beforeWall",false,"quickestX",u.getQuickestXList(g.wallX,false)));
            groups.add(obj("kind",g.isFar?"far":"near","wallSlot",g.wallIndex,"wallX",g.wallX,"wallY",g.wallY,"members",members,
                    "note",g.isFar?"GUI初始几何检测":"GUI合并所有near组；代表墙不能视为每个成员的唯一实际触发墙"));}
        return obj("name",fort.name(),"code",fort.code(),"coordinateSpace","kit-workbench","cost",cost,"overDefaultBudget",cost>1500,
                "accelLevel",f.getAccelLevel(),"units",units,"assaultGroups",groups,"issues",WorkbenchAnalysis.analyze(f.units),
                "limits",List.of("工作台建议及稳定化是辅助模型，不能代替胜率证据","catalogHp不保证等于Rust构造HP；battle.units.initialHp为实际战斗值"));
    }
    static Map<String,Object> contribution(List<Fort> a,List<Fort>b,int frames,int threads,JsonNode req,long maxJobs) {
        int remove=req.path("deleteCount").asInt(1),n=decode(a.getFirst().code()).units.size()-1;
        require(remove==1 || remove==2,"贡献分析支持单删或双删");
        long count=remove==1?n:(long)n*(n-1)/2;
        require((count+1)*b.size()<=maxJobs,"贡献分析超过maxJobs预算");
        List<Map<String,Object>> baseRows=rows(a,b,frames,threads); Map<String,Object> base=score(results(baseRows),0);
        List<Unit> units=decode(a.getFirst().code()).units;List<Object> combos=new ArrayList<>();
        for(int i=0;i<n;i++) for(int j=(remove==1?i:i+1);j<(remove==1?i+1:n);j++) {
            int[] deleted=remove==1?new int[]{i}:new int[]{i,j};
            // 直接保留原六字符块，避免GUI的BOSS HP后缀/回编码裁剪污染反事实。
            String original=a.getFirst().code();StringBuilder kept=new StringBuilder(original.substring(0,6));
            for(int k=0;k<n;k++) {final int index=k;if(Arrays.stream(deleted).noneMatch(d->d==index))kept.append(original,k*6+6,k*6+12);}
            String code=kept.toString();
            Map<String,Object> scored=score(results(rows(List.of(new Fort("removed",code)),b,frames,threads)),0);
            int removedCost=Unit.infos[units.get(i+1).id].cost();
            if(remove==2)removedCost+=Unit.infos[units.get(j+1).id].cost();
            Object delta=base.get("scorePercent")==null || scored.get("scorePercent")==null ? null : (double)scored.get("scorePercent")-(double)base.get("scorePercent");
            combos.add(obj("slots",Arrays.stream(deleted).map(k->k+1).toArray(),"removedCost",removedCost,"code",code,"score",scored,"deltaPoints",delta));
        }
        return obj("baseline",base,"baselineResults",baseRows,"combos",combos,"testedCombos",combos.size(),
                "interpretation","deltaPoints=删后得分-原阵得分；墙/加速器的删除可能改变整个阵。不是普遍单位强度。","complete",combos.size()==count && base.get("scorePercent")!=null && combos.stream().allMatch(x -> ((Map<?,?>)x).get("deltaPoints")!=null));
    }
    static Map<String,Object> matchup(List<Fort>a,List<Fort>b,int frames,int threads,JsonNode req) throws Exception {
        MatchupAnalyzer.Report gui=MatchupAnalyzer.analyze(text(a),text(b),threads,req.path("strength").asBoolean(false),null,()->false);
        List<Result> rs=new ArrayList<>();for(int i=0;i<gui.statuses().length;i++) rs.add(new Result(gui.statuses()[i],gui.winnerHp()[i],gui.frames()[i],0));
        List<Object> sensitivity=new ArrayList<>(),accel=new ArrayList<>();
        List<Formation> decoded=b.stream().map(f->decode(f.code())).toList();
        for(int type=1;type<=60;type++) {List<Result> with=new ArrayList<>(),without=new ArrayList<>();
            final int t=type; for(int i=0;i<rs.size();i++) (decoded.get(i).units.stream().skip(1).anyMatch(x->x.id==t)?with:without).add(rs.get(i));
            if(!with.isEmpty()) sensitivity.add(obj("typeId",type,"with",score(with,0),"without",score(without,0)));
        }
        TreeSet<Integer> levels=new TreeSet<>();decoded.forEach(f->levels.add(f.getAccelLevel()));
        for(int level:levels) {List<Result> group=new ArrayList<>();for(int i=0;i<rs.size();i++)if(decoded.get(i).getAccelLevel()==level)group.add(rs.get(i));
            accel.add(obj("level",level,"score",score(group,0)));}
        return obj("score",score(rs,0),"sensitivity",sensitivity,"acceleration",accel,"guiReport",gui,
                "guiScoringPolicy","legacy-excludes-U; Keener和原GUI统计仅作辅助，不能作为新主评分", "interpretation","对手带某兵种的相关性不是该兵种造成失败的因果证据");
    }
    static Map<String,Object> battlefield(List<Fort>a,List<Fort>b,int frames,int threads,JsonNode req) throws Exception {
        int bin=req.path("binFrames").asInt(120); require(bin>=1 && bin<=65536,"binFrames必须1..65536");
        BattlefieldAnalyzer.Report r=BattlefieldAnalyzer.analyze(text(a),text(b),BattlefieldAnalyzer.Filter.valueOf(req.path("filter").asText("ALL")),threads,bin,null,()->false);
        List<Object> slots=new ArrayList<>(),timeline=new ArrayList<>(),games=new ArrayList<>();
        for(var m:r.metas()) slots.add(obj("slot",m.core()?0:m.index()+1,"reportIndex",m.index(),"typeId",m.type(),"x",m.x(),"y",m.y(),"r",m.r(),
                "damageTaken",r.slotDamage()[m.index()],"deathCount",r.slotDeaths()[m.index()]));
        for(int group=0;group<3;group++)for(int k=0;k<r.bins();k++) {double count=r.timelineCount(group,k);if(count==0)continue;
            timeline.add(obj("group",group,"frame",k*bin,"sampleCount",count,"p1CoreHpMean",r.timelineValue(group,k,0)/count,
                    "p2CoreHpMean",r.timelineValue(group,k,1)/count,"p1AliveMean",r.timelineValue(group,k,2)/count,"p2AliveMean",r.timelineValue(group,k,3)/count,
                    "p1ActiveAttackMean",r.timelineValue(group,k,4)/count,"p2ActiveAttackMean",r.timelineValue(group,k,5)/count));}
        for(int k=0;k<r.games();k++)games.add(obj("index",k,"firstCollisionFrame",r.scalar(k,0),"firstP1CoreDamageFrame",r.scalar(k,1),
                "firstP2CoreDamageFrame",r.scalar(k,2),"collisionCount",r.scalar(k,3),"p1Damage",r.scalar(k,4),"p2Damage",r.scalar(k,5),
                "p1CoreDamage",r.scalar(k,6),"p2CoreDamage",r.scalar(k,7),"p1Deaths",r.scalar(k,8),"p2Deaths",r.scalar(k,9)));
        return obj("games",r.games(),"filter",r.filter(),"slots",slots,"timeline",timeline,"gameScalars",games,"rawGuiReport",r,
                "limits",List.of("后期样本是尚未结束的长局，不是全体对局","slotDamage是受创，不是攻击者输出；activeAttack是存量，不是发射或命中次数"));
    }
    public static Map<String,Object> execute(JsonNode req) throws Exception {
        I18n.setLang("zh"); int frames=req.path("maxFrames").asInt(65536),threads=req.path("threads").asInt(4);
        long maxJobs=req.path("maxJobs").asLong(12800);
        require(frames>=0 && frames<=1_000_000 && threads>=1 && threads<=256 && maxJobs>=1 && maxJobs<=1_000_000,"帧数、线程或预算越界");
        Main.MAX_FRAME_LIMIT=frames;Main.MAX_THREADS=threads;
        String action=req.path("action").asText();Map<String,Object> out=obj("schemaVersion","gkt-ai-analysis-v1","action",action,"kitVersion",Main.VERSION,"labVersion",Main.LAB_VERSION,
                "maxFrames",frames,"threads",threads,"seedPolicy","engine-default","scoringPolicy","timeout-as-draw-v1");
        switch(action) {
            case "capabilities" -> out.put("capabilities",obj("actions",List.of("catalog","formation","trajectory","battle","contribution","matchup","battlefield"),
                    "sparseBattle","tools/ai/ai.py battle，独立可重建Rust observer","sideConvention","0=1P,1=2P","coordinates",List.of("kit-workbench","battle-world")));
            case "probe" -> {out.put("rustAvailable",RustBattle.available());out.put("rustReason",RustBattle.unavailableReason());out.put("engineVersion",RustBattle.engineVersion());}
            case "catalog" -> {List<Object> catalog=new ArrayList<>();JsonNode details=JSON.readTree(AiApi.class.getResourceAsStream("/unit_details_zh.json")).path("units");
                for(int id=0;id<Unit.infos.length;id++) {Unit.Info u=Unit.infos[id];JsonNode detail=null;for(JsonNode d:details)if(d.path("id").asInt()==id)detail=d;
                    catalog.add(obj("id",id,"name",I18n.unitName(id),"cost",u.cost(),"tech",u.tech(),"catalogHp",u.hp(),"cd",u.cd(),"at",u.at(),"shoot",u.shoot(),"dmgRaw",u.dmg(),"core",Unit.isCore(id),"wall",Unit.isWall(id),"detail",detail));}
                out.put("units",catalog);out.put("limits",List.of("dmgRaw含编码含义；不能直接横比DPS","catalogHp不是实际Rust构造HP"));}
            case "trajectory" -> {Main.formulaTable=new FormulaTable();out.put("prediction",TrajectoryModel.predict(Main.formulaTable,req.path("near").asBoolean(false),req.path("ballFirst").asBoolean(false),
                    req.path("x").asInt(),req.path("y").asInt(),req.path("wallX").asInt(),req.path("speed1").asInt(),req.path("speed2").asInt(),req.path("times").asInt(1)));}
            default -> {List<Fort>a=forts(req.get("p1"));out.put("p1",a);
                if(action.equals("formation")) out.put("formations",a.stream().map(AiApi::formation).toList());
                else {List<Fort>b=forts(req.get("p2"));out.put("p2",b);require((long)a.size()*b.size()<=maxJobs,"对战矩阵超过maxJobs");rust();
                    if(action.equals("battle")) {require((long)a.size()*b.size()*(req.path("bothSides").asBoolean()?2:1)<=maxJobs,"换边后超过maxJobs");
                        List<Map<String,Object>> forward=rows(a,b,frames,threads);out.put("forward",forward);List<Object> scores=new ArrayList<>();
                        for(int i=0;i<a.size();i++) scores.add(obj("candidateIndex",i,"score",score(results(forward.subList(i*b.size(),(i+1)*b.size())),0)));out.put("scores",scores);
                        if(req.path("bothSides").asBoolean())out.put("reverse",rows(b,a,frames,threads));}
                    else {require(a.size()==1,"分析只接受一个候选");
                        if(action.equals("matchup") && req.path("strength").asBoolean(false))require(MatchupAnalyzer.strengthBattleEstimate(b.size())+b.size()<=maxJobs,"Keener超过maxJobs");
                        if(action.equals("battlefield")) require((long)b.size()*2<=maxJobs,"战场分析两阶段超过maxJobs");
                        out.put("report",switch(action) {case "contribution"->contribution(a,b,frames,threads,req,maxJobs);case "matchup"->matchup(a,b,frames,threads,req);case "battlefield"->battlefield(a,b,frames,threads,req);default->throw new IllegalArgumentException("未知action："+action);});}
                }
            }
        }
        try(var in=AiApi.class.getResourceAsStream("/native/gkt_jni.dll")) {if(in!=null)out.put("bundledDllSha256",sha(in.readAllBytes()));}
        return out;
    }
    public static void main(String[] args) {
        try {
            require(args.length==2,"用法：--ai request.json output.json");Path out=Path.of(args[1]);require(!Files.exists(out),"拒绝覆盖已有输出");
            Map<String,Object> result=execute(JSON.readTree(Path.of(args[0]).toFile()));
            try(var stream=Files.newOutputStream(out,StandardOpenOption.CREATE_NEW)) {JSON.writeValue(stream,result);}
        } catch(Exception e) {System.err.println("AI接口失败："+e.getMessage());System.exit(2);}
    }
}
