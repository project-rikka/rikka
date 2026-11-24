package sh.rikka.client.setting;

import com.mojang.serialization.Codec;
import com.mojang.serialization.DataResult;
import java.util.HashMap;
import java.util.Locale;
import java.util.Map;

final class EnumCodecs {
	private EnumCodecs() {
	}

	static String name(Enum<?> constant) {
		return constant.name().toLowerCase(Locale.ROOT);
	}

	static <E extends Enum<E>> Codec<E> of(Class<E> type) {
		Map<String, E> constants = new HashMap<>();
		for (E constant : type.getEnumConstants()) {
			constants.put(name(constant), constant);
		}
		return Codec.STRING.comapFlatMap(text -> {
			E constant = constants.get(text);
			return constant != null ? DataResult.success(constant) : DataResult.error(() -> "unknown " + type.getSimpleName() + ": " + text);
		}, EnumCodecs::name);
	}
}
