package sh.rikka.client.setting;

import com.mojang.serialization.Codec;
import com.mojang.serialization.DataResult;
import net.minecraft.util.ExtraCodecs;

public record IntRange(int low, int high) {
	public static final Codec<IntRange> CODEC = ExtraCodecs.intervalCodec(Codec.INT, "low", "high", IntRange::checked, IntRange::low, IntRange::high);

	public IntRange {
		if (low > high) {
			throw new IllegalArgumentException("low " + low + " is above high " + high);
		}
	}

	private static DataResult<IntRange> checked(int low, int high) {
		return low <= high ? DataResult.success(new IntRange(low, high)) : DataResult.error(() -> "low " + low + " is above high " + high);
	}

	public boolean contains(int value) {
		return value >= this.low && value <= this.high;
	}
}
