import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.example.*;
import org.example.GUI.Formation;
import org.example.GUI.Unit;
import org.example.rust.RustBattle;
import java.nio.file.*;
import java.util.*;

/** 只调用发行包公开接口；不修改引擎。由 kit_lab.py 校验输入和限制执行时间。 */
public class KitLab {
    static final ObjectMapper JSON = new ObjectMapper();

    static Map<String, Object> obj(Object... fields) {
        Map<String, Object> out = new LinkedHashMap<>();
        for (int i = 0; i < fields.length; i += 2) out.put((String) fields[i], fields[i + 1]);
        return out;
    }

    static List<Fort> forts(JsonNode node) {
        List<Fort> out = new ArrayList<>();
        for (JsonNode f : node) out.add(new Fort(f.get("name").asText(), f.get("code").asText()));
        return out;
    }

    static String text(List<Fort> forts) {
        return String.join("/\n", forts.stream().map(f -> f.name() + "&" + f.code()).toList());
    }

    static Map<String, Object> audit(Fort f) {
        Formation formation = Formation.decode(f.name() + "&" + f.code());
        List<Object> units = new ArrayList<>();
        int cost = 0;
        List<String> warnings = new ArrayList<>();
        for (int slot = 0; slot < formation.units.size(); slot++) {
            Unit u = formation.units.get(slot);
            Unit.Info info = Unit.infos[u.id];
            cost += info.cost();
            if (u.x < 0 || u.x > (u.isCore() ? 276 : 348)
                    || u.y < 0 || u.y > (u.isCore() ? 276 : 349)) warnings.add("坐标越界，slot=" + slot);
            if (u.r < 0 || u.r >= 360) warnings.add("角度未规范化，slot=" + slot);
            units.add(obj("slot", slot, "nonCoreIndex", slot - 1, "id", u.id,
                    "name", info.name(), "x", u.x, "y", u.y, "r", u.r,
                    "hp", u.hp, "cost", info.cost(), "tech", info.tech(),
                    "isCore", u.isCore(), "isWall", u.isWall()));
        }
        String encoded = formation.encode();
        String roundtrip = encoded.substring(encoded.indexOf('&') + 1);
        String roundtripCode = roundtrip.split("#", 2)[0];
        if (roundtrip.contains("#")) warnings.add("工作台回编码附加HP；保留原纯码，检查核心解码后的HP初始化");
        return obj("name", f.name(), "code", f.code(), "cost", cost,
                "accelLevel", formation.getAccelLevel(), "roundtripEqual", roundtrip.equals(f.code()),
                "roundtripCodeEqual", roundtripCode.equals(f.code()), "encodedHasHpSuffix", roundtrip.contains("#"),
                "warnings", warnings, "units", units);
    }

    static List<Object> battles(List<Fort> a, List<Fort> b, String engine, int frames, int threads) {
        List<Result> results = new ArrayList<>();
        if (engine.equals("rust")) {
            if (!RustBattle.available()) throw new IllegalStateException(RustBattle.unavailableReason());
            results = RustBattle.runBatch(a.stream().map(Fort::code).toArray(String[]::new),
                    b.stream().map(Fort::code).toArray(String[]::new), frames, threads, null);
        } else {
            for (Fort p1 : a) for (Fort p2 : b)
                results.add(new GameTask().run_single(Main.compileFort(p1), Main.compileFort(p2)));
        }
        List<Object> out = new ArrayList<>();
        for (int i = 0; i < a.size(); i++) for (int j = 0; j < b.size(); j++) {
            Result r = results.get(i * b.size() + j);
            out.add(obj("p1Index", i, "p2Index", j, "p1", a.get(i).name(), "p2", b.get(j).name(),
                    "status", r.status, "winnerHp", r.winnerHp, "frames", r.framePassed));
        }
        return out;
    }

    public static void main(String[] args) throws Exception {
        I18n.setLang("zh");
        JsonNode req = JSON.readTree(Path.of(args[0]).toFile());
        int frames = req.path("maxFrames").asInt(65536);
        int threads = req.path("threads").asInt(4);
        Main.MAX_FRAME_LIMIT = frames;
        Main.MAX_THREADS = threads;
        String action = req.get("action").asText();
        String engine = req.path("engine").asText("rust");
        Map<String, Object> out = obj("action", action, "kitVersion", Main.VERSION,
                "javaVersion", System.getProperty("java.version"), "maxFrames", frames,
                "threadsRequested", threads, "seedPolicy", "engine-default");
        if (action.equals("probe")) {
            Utils rng = new Utils();
            out.put("javaInitialSeed", rng.universalSeed);
            out.put("alphabet", Main.pskey);
            out.put("alphabetLength", Main.pskey.length());
            out.put("newlineOnlyCount", Setting.parseFortsRaw("a&000000\nb&000000").size());
            out.put("slashSeparatedCount", Setting.parseFortsRaw("a&000000/\nb&000000").size());
            out.put("hpSuffixStripped", Setting.parseFortsRaw("a&000000#0000").get(0).code());
            out.put("rustAvailable", RustBattle.available());
            out.put("rustReason", RustBattle.unavailableReason());
            out.put("rustVersion", RustBattle.engineVersion());
        } else if (action.equals("catalog")) {
            List<Object> rows = new ArrayList<>();
            for (int id = 0; id <= 62; id++) {
                Unit.Info u = Unit.infos[id];
                rows.add(obj("id", id, "codeChar", String.valueOf(Main.pskey.charAt(id % 61 + id / 61)),
                        "name", u.name(), "cost", u.cost(), "tech", u.tech(), "hp", u.hp(),
                        "cd", u.cd(), "at", u.at(), "shoot", u.shoot(), "dmgRaw", u.dmg(),
                        "isCore", Unit.isCore(id), "isWall", Unit.isWall(id)));
            }
            out.put("units", rows);
        } else {
            List<Fort> p1 = forts(req.get("p1"));
            if (action.equals("audit")) {
                out.put("formations", p1.stream().map(KitLab::audit).toList());
            } else {
                List<Fort> p2 = forts(req.get("p2"));
                out.put("engine", engine);
                out.put("engineVersion", engine.equals("rust") ? RustBattle.engineVersion() : "kit-java-" + Main.VERSION);
                out.put("p1", p1);
                out.put("p2", p2);
                if (action.equals("battle")) {
                    out.put("forward", battles(p1, p2, engine, frames, threads));
                    if (req.path("bothSides").asBoolean()) out.put("reverse", battles(p2, p1, engine, frames, threads));
                } else {
                    if (!engine.equals("rust") || !RustBattle.available())
                        throw new IllegalStateException("分析命令要求 Rust 可用，防止隐式更换引擎");
                    if (p1.size() != 1) throw new IllegalArgumentException("分析命令只接受一个原阵");
                    Object report = switch (action) {
                        case "contribution" -> ContributionAnalyzer.analyze(text(p1), text(p2),
                            new ContributionAnalyzer.Params(req.path("deleteCount").asInt(1), 100, threads,
                                ContributionAnalyzer.SortKey.DELTA, ContributionAnalyzer.SortOrder.DESC), null, () -> false);
                        case "matchup" -> MatchupAnalyzer.analyze(text(p1), text(p2), threads,
                            req.path("strength").asBoolean(false), null, () -> false);
                        case "battlefield" -> BattlefieldAnalyzer.analyze(text(p1), text(p2),
                            BattlefieldAnalyzer.Filter.valueOf(req.path("filter").asText("ALL")), threads,
                            req.path("binFrames").asInt(120), null, () -> false);
                        default -> throw new IllegalArgumentException("未知 action: " + action);
                    };
                    out.put("report", report);
                }
            }
        }
        JSON.writerWithDefaultPrettyPrinter().writeValue(Path.of(args[1]).toFile(), out);
    }
}
