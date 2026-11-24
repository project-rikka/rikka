package sh.rikka.client.setting;

import com.mojang.serialization.Codec;
import com.mojang.serialization.DataResult;
import net.minecraft.util.ExtraCodecs;

public record DoubleRange(double low, double high) {
	public static final Codec<DoubleRange> CODEC = ExtraCodecs.intervalCodec(
		Numeric.FINITE_DOUBLE,
		"low",
		"high",
		DoubleRange::checked,
		DoubleRange::low,
		DoubleRange::high
	);

	public DoubleRange {
		Numeric.requireFinite(low);
		Numeric.requireFinite(high);
		if (low > high) {
			throw new IllegalArgumentException("low " + low + " is above high " + high);
		}
	}

	private static DataResult<DoubleRange> checked(double low, double high) {
		return low <= high ? DataResult.success(new DoubleRange(low, high)) : DataResult.error(() -> "low " + low + " is above high " + high);
	}

	public boolean contains(double value) {
		return value >= this.low && value <= this.high;
	}
}
