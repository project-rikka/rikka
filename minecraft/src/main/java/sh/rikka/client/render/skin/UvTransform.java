package sh.rikka.client.render.skin;

import net.minecraft.client.renderer.texture.UvMapping;
import org.jspecify.annotations.Nullable;

public record UvTransform(float uScale, float vScale, float uOffset, float vOffset) {
	private static final UvTransform IDENTITY = new UvTransform(1.0F, 1.0F, 0.0F, 0.0F);

	public static UvTransform identity() {
		return IDENTITY;
	}

	public static UvTransform of(@Nullable UvMapping mapping) {
		if (mapping == null) {
			return IDENTITY;
		}
		float uOffset = mapping.getU(0.0F);
		float vOffset = mapping.getV(0.0F);
		return new UvTransform(mapping.getU(1.0F) - uOffset, mapping.getV(1.0F) - vOffset, uOffset, vOffset);
	}
}
