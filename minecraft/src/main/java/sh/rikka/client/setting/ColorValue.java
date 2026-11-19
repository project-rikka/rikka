package sh.rikka.client.setting;

import com.mojang.serialization.Codec;
import com.mojang.serialization.codecs.RecordCodecBuilder;
import net.minecraft.util.ExtraCodecs;

public record ColorValue(int argb, boolean chroma) {
	public static final Codec<ColorValue> CODEC = RecordCodecBuilder.create(instance -> instance.group(
		ExtraCodecs.STRING_ARGB_COLOR.fieldOf("argb").forGetter(ColorValue::argb),
		Codec.BOOL.optionalFieldOf("chroma", false).forGetter(ColorValue::chroma)
	).apply(instance, ColorValue::new));

	public ColorValue withArgb(int argb) {
		return new ColorValue(argb, this.chroma);
	}

	public ColorValue withChroma(boolean chroma) {
		return new ColorValue(this.argb, chroma);
	}
}
