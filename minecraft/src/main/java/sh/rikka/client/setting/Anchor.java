package sh.rikka.client.setting;

import com.mojang.serialization.Codec;

public enum Anchor {
	TOP_LEFT(0.0F, 0.0F),
	TOP_CENTER(0.5F, 0.0F),
	TOP_RIGHT(1.0F, 0.0F),
	CENTER_LEFT(0.0F, 0.5F),
	CENTER(0.5F, 0.5F),
	CENTER_RIGHT(1.0F, 0.5F),
	BOTTOM_LEFT(0.0F, 1.0F),
	BOTTOM_CENTER(0.5F, 1.0F),
	BOTTOM_RIGHT(1.0F, 1.0F);

	public static final Codec<Anchor> CODEC = EnumCodecs.of(Anchor.class);

	private final float horizontal;
	private final float vertical;

	Anchor(float horizontal, float vertical) {
		this.horizontal = horizontal;
		this.vertical = vertical;
	}

	public float horizontal() {
		return this.horizontal;
	}

	public float vertical() {
		return this.vertical;
	}
}
