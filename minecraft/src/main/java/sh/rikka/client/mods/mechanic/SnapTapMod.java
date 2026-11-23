package sh.rikka.client.mods.mechanic;

import java.util.Arrays;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.client.Options;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.mod.Category;
import sh.rikka.client.mod.Mod;

public final class SnapTapMod extends Mod {
	private static volatile @Nullable SnapTapMod active;

	private final long[] pressedAt = new long[Direction.values().length];
	private long sequence;

	public SnapTapMod() {
		super("snap_tap", Category.MECHANIC, "keyboard_keys");
	}

	static @Nullable SnapTapMod active() {
		return active;
	}

	@Override
	protected void onEnable() {
		Arrays.fill(this.pressedAt, 0L);
		this.sequence = 0L;
		active = this;
	}

	@Override
	protected void onDisable() {
		active = null;
	}

	boolean resolve(KeyMapping key, boolean down) {
		Options options = Minecraft.getInstance().options;
		Direction direction = Direction.of(options, key);
		if (!down || direction == null) {
			return down;
		}
		Direction opposite = direction.opposite();
		return !opposite.key(options).isDown() || this.pressedAt[direction.ordinal()] > this.pressedAt[opposite.ordinal()];
	}

	void pressed(KeyMapping key) {
		Direction direction = Direction.of(Minecraft.getInstance().options, key);
		if (direction != null) {
			this.pressedAt[direction.ordinal()] = ++this.sequence;
		}
	}
}
