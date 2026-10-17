import org.example.GameTask;
import org.example.elements.Base;
import org.example.elements.wall.Elevator;
import org.example.elements.wall.Turbo;

/** catalog-hp-v1目录HP规则；覆盖Java回退构造和加速贡献。 */
public class UnitHpRulesCheck {
    public static void main(String[] args) {
        GameTask game = new GameTask();
        game.bases = new Base[]{new Base(game,240,532,0),new Base(game,1680,532,1)};
        Turbo blue = new Turbo(game,150,300,0,30);
        Elevator elevator = new Elevator(game,200,300,0,60);
        if (blue.hp != 100 || blue.max_hp != 100) throw new AssertionError("蓝加速器HP应为100");
        if (elevator.hp != 75 || elevator.max_hp != 75) throw new AssertionError("电梯壁HP应为75");
        if (game.bases[0].axl != 3) throw new AssertionError("蓝加速贡献应保持2");
        blue.kill();
        if (game.bases[0].axl != 1) throw new AssertionError("加速器死亡应扣回贡献");
        System.out.println("Java HP修正规则通过：蓝加速器100，电梯壁75，加速贡献不变");
    }
}
