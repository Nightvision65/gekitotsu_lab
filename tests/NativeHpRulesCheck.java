import org.example.rust.RustBattle;
import java.util.Arrays;

/** 通过真实JNI遥测核验无治疗墙体的耗尽HP；在独立工作目录运行。 */
public class NativeHpRulesCheck {
    private static final String ALPHABET = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

    private static String block(int type, int x, int y) {
        int value = x * 1000 + y;
        char[] result = new char[6];
        result[0] = ALPHABET.charAt(type);
        for (int i = 5; i > 0; i--) {
            result[i] = ALPHABET.charAt(value % 61);
            value /= 61;
        }
        return new String(result);
    }

    public static void main(String[] args) {
        int blueHp = args.length == 0 ? 100 : Integer.parseInt(args[0]);
        int elevatorHp = args.length == 0 ? 75 : Integer.parseInt(args[1]);
        if (!RustBattle.available()) throw new AssertionError(RustBattle.unavailableReason());
        if (args.length == 0 && !RustBattle.engineVersion().equals("gkt-core (catalog-hp-v1)")) {
            throw new AssertionError("加载了错误的JNI构建：" + RustBattle.engineVersion());
        }
        String enemy = block(0, 328, 334);
        for (int i = 0; i < 12; i++) enemy += block(3, 364 - i * 8, 349);
        for (int i = 0; i < 6; i++) enemy += block(1, 340 - i * 10, 315);
        for (int type : new int[]{25, 28, 29, 30, 60}) {
            String fort = block(0, 52, 334) + block(type, 320, 349);
            // 上方铁壁约束电梯行程，使其在核心结束对局之前被完整破坏。
            if (type == 60) fort += block(28, 320, 285);
            double[] payload = RustBattle.runBatchTelemetry(fort, new String[]{enemy},
                    new int[]{2}, 65536, 1, 120, null);
            int games = (int) payload[0], bins = (int) payload[1], slots = (int) payload[3];
            if (games != 1 || slots != (type == 60 ? 3 : 2)) throw new AssertionError("遥测头不符");
            int at = 7 + games * 10 + 3 * bins * 7;
            double damage = Arrays.stream(payload, at, at + slots).sum();
            double deaths = Arrays.stream(payload, at + slots, at + 2 * slots).sum();
            int expected = switch (type) {
                case 25 -> 35;
                case 28 -> 150;
                case 29 -> 15;
                case 30 -> blueHp;
                case 60 -> elevatorHp + 150; // 电梯及其上方铁壁。
                default -> throw new AssertionError();
            };
            if (damage != expected || deaths != (type == 60 ? 2 : 1)) {
                throw new AssertionError("类型" + type + "：实际受创=" + damage + "，阵亡=" + deaths
                        + "，预期受创=" + expected);
            }
            System.out.println("JNI类型" + type + "：耗尽HP=" + (type == 60 ? damage - 150 : damage));
        }
        System.out.println("JNI HP规则通过：" + RustBattle.engineVersion());
    }
}
