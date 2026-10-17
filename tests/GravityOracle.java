import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import java.nio.file.Path;
import java.util.*;
import org.example.GameTask;
import org.example.Shape;
import org.example.GUI.Formation;
import org.example.GUI.Unit;
import org.example.elements.*;

/** Independent geometry oracle using the frozen Kit JAR's Java Ball.land(). */
public class GravityOracle {
    public static void main(String[] args) throws Exception {
        ObjectMapper json = new ObjectMapper();
        JsonNode request = json.readTree(Path.of(args[0]).toFile());
        List<Object> output = new ArrayList<>();
        for (JsonNode input : request.path("formations")) {
            List<Unit> units = Formation.decodeUnits(input.path("code").asText());
            GameTask game = new GameTask();
            game.bases = new Base[]{new Base(game,240,532,0),new Base(game,1680,532,1)};
            Unit core = units.get(0);
            game.cores[0] = new Core(game,core.x-138,core.y-342,0);
            game.cores[0].step();
            List<Shape> placed = new ArrayList<>();
            placed.add(game.cores[0]);
            for (Unit u : units.subList(1, units.size())) {
                game.unit_make(game,u.x+66,u.y+152,u.r,u.id,0);
                placed.add(game.elements.items[game.elements.size-1]);
            }
            // Full fixed frame count, no convergence logic shared with the new tool.
            for (int frame=0;frame<request.path("maxFrames").asInt(240);frame++) {
                for (Shape shape : placed) {
                    if (shape instanceof Ball ball && ball.jump_flg!=2) ball.land();
                }
            }
            List<Object> positions = new ArrayList<>();
            for (int slot=0;slot<placed.size();slot++) {
                Shape shape=placed.get(slot);
                positions.add(Map.of("slot",slot,"x",slot==0?core.x:shape.x-66,
                    "y",slot==0?core.y:shape.y-152));
            }
            output.add(Map.of("code",input.path("code").asText(),"units",positions));
        }
        json.writeValue(Path.of(args[1]).toFile(),output);
    }
}
