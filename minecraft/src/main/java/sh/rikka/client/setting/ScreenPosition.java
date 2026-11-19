package sh.rikka.client.setting;

import com.mojang.serialization.Codec;
import com.mojang.serialization.codecs.RecordCodecBuilder;

public record ScreenPosition(Anchor anchor, double x, double y) {
	public static final Codec<ScreenPosition> CODEC = RecordCodecBuilder.create(instance -> instance.group(
		Anchor.CODEC.fieldOf("anchor").forGetter(ScreenPosition::anchor),
		Numeric.FINITE_DOUBLE.fieldOf("x").forGetter(ScreenPosition::x),
		Numeric.FINITE_DOUBLE.fieldOf("y").forGetter(ScreenPosition::y)
	).apply(instance, ScreenPosition::new));

	public ScreenPosition {
		Numeric.requireFinite(x);
		Numeric.requireFinite(y);
	}

	public float left(float screenWidth, float width) {
		return place(this.anchor.horizontal(), this.x, screenWidth, width);
	}

	public float top(float screenHeight, float height) {
		return place(this.anchor.vertical(), this.y, screenHeight, height);
	}

	private static float place(float factor, double offset, float screen, float size) {
		float room = Math.max(0.0F, screen - size);
		return Math.clamp((float) (factor * room + offset), 0.0F, room);
	}
}
