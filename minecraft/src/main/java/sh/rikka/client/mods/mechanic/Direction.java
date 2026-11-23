package sh.rikka.client.mods.mechanic;

import java.util.function.Function;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Options;
import org.jspecify.annotations.Nullable;

enum Direction {
	FORWARD(options -> options.keyUp),
	BACKWARD(options -> options.keyDown),
	LEFT(options -> options.keyLeft),
	RIGHT(options -> options.keyRight);

	private static final Direction[] ALL = values();

	private final Function<Options, KeyMapping> key;

	Direction(Function<Options, KeyMapping> key) {
		this.key = key;
	}

	static @Nullable Direction of(Options options, KeyMapping key) {
		for (Direction direction : ALL) {
			if (direction.key(options) == key) {
				return direction;
			}
		}
		return null;
	}

	KeyMapping key(Options options) {
		return this.key.apply(options);
	}

	Direction opposite() {
		return switch (this) {
			case FORWARD -> BACKWARD;
			case BACKWARD -> FORWARD;
			case LEFT -> RIGHT;
			case RIGHT -> LEFT;
		};
	}
}
